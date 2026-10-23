//! Zero-seed rule engine, ported from tools/pattern-tracker/ost_patterns/resolve.py.
//!
//! Rules never guess: every rule must produce exactly one candidate function,
//! otherwise the hook is reported unresolved and the run fails. A silently wrong
//! RVA would disable a hook at best and patch the wrong function at worst.

use std::collections::{HashMap, HashSet};

use serde::Deserialize;

use super::pe::Pe;
use super::rtti::Rtti;
use super::sig;
use super::tomlio::PatternEntry;

#[derive(Debug, Clone)]
pub struct HookRule {
    pub name: String,
    pub kind: String,
    pub strings: Vec<String>,
    pub of: Option<String>,
    pub of_anchor: Vec<String>,
    pub depth: usize,
    pub span: Option<usize>,
    pub leaf_only: bool,
    pub min_callers: Option<usize>,
    pub class_name: Option<String>,
    pub slot: Option<usize>,
    pub note: String,
}

#[derive(Debug, Clone)]
pub struct Resolved {
    pub name: String,
    pub rva: u32,
    pub via: String,
}

#[derive(Deserialize)]
struct RulesDoc {
    channel: String,
    #[serde(default)]
    hook: Vec<HookDoc>,
}

fn one() -> usize {
    1
}

#[derive(Deserialize)]
struct HookDoc {
    name: String,
    kind: String,
    #[serde(default)]
    strings: Vec<String>,
    of: Option<String>,
    #[serde(default)]
    of_anchor: Vec<String>,
    #[serde(default = "one")]
    depth: usize,
    span: Option<usize>,
    #[serde(default)]
    leaf_only: bool,
    min_callers: Option<usize>,
    class_name: Option<String>,
    slot: Option<usize>,
    #[serde(default)]
    note: String,
}

/// Parse a rules TOML into `(channel, rules)`.
pub fn load_rules(text: &str) -> Result<(String, Vec<HookRule>), String> {
    let doc: RulesDoc = toml::from_str(text).map_err(|e| format!("rules: {e}"))?;
    if doc.hook.is_empty() {
        return Err("rules: no [[hook]] entries".to_string());
    }
    let hooks = doc
        .hook
        .into_iter()
        .map(|h| HookRule {
            name: h.name,
            kind: h.kind,
            strings: h.strings,
            of: h.of,
            of_anchor: h.of_anchor,
            depth: h.depth.max(1),
            span: h.span,
            leaf_only: h.leaf_only,
            min_callers: h.min_callers,
            class_name: h.class_name,
            slot: h.slot,
            note: h.note,
        })
        .collect();
    Ok((doc.channel, hooks))
}

pub struct Resolver<'a> {
    pe: &'a Pe,
    hooks: HashMap<String, HookRule>,
    order: Vec<String>,
    resolved: HashMap<String, Resolved>,
    space: Vec<u8>,
    rtti: Option<Rtti<'a>>,
}

impl<'a> Resolver<'a> {
    pub fn new(pe: &'a Pe, hooks: Vec<HookRule>) -> Result<Resolver<'a>, String> {
        let order: Vec<String> = hooks.iter().map(|h| h.name.clone()).collect();
        let map: HashMap<String, HookRule> =
            hooks.into_iter().map(|h| (h.name.clone(), h)).collect();
        let space = sig::scan_space(pe)?;
        Ok(Resolver {
            pe,
            hooks: map,
            order,
            resolved: HashMap::new(),
            space,
            rtti: None,
        })
    }

    pub fn order(&self) -> &[String] {
        &self.order
    }

    pub fn resolved_count(&self) -> usize {
        self.resolved.len()
    }

    fn funcs_for_strings(&self, strings: &[String]) -> Result<HashSet<u32>, String> {
        let mut per_string: Vec<HashSet<u32>> = Vec::new();
        for text in strings {
            let rvas = self.pe.find_cstring(text);
            if rvas.is_empty() {
                return Err(format!("literal not found: {text:?}"));
            }
            let mut funcs: HashSet<u32> = HashSet::new();
            for rva in rvas {
                if let Some(sites) = self.pe.lea_xrefs().get(&rva) {
                    for site in sites {
                        if let Some((begin, _)) = self.pe.func_of(*site) {
                            funcs.insert(begin);
                        }
                    }
                }
            }
            if funcs.is_empty() {
                return Err(format!("literal has no code reference: {text:?}"));
            }
            per_string.push(funcs);
        }
        let mut iter = per_string.into_iter();
        let mut common = match iter.next() {
            Some(first) => first,
            None => return Err("no strings given".to_string()),
        };
        for next in iter {
            common = common.intersection(&next).copied().collect();
        }
        Ok(common)
    }

    fn span_of(&self, func_rva: u32) -> Option<usize> {
        self.pe
            .func_at(func_rva)
            .map(|(begin, end)| (end - begin) as usize)
    }

    fn base_and_origin(&self, hook: &HookRule) -> Result<(u32, String), String> {
        if let Some(of) = &hook.of {
            let base = self
                .resolved
                .get(of)
                .ok_or_else(|| format!("{}: needs a resolved `of` ({of})", hook.name))?;
            return Ok((base.rva, of.clone()));
        }
        if !hook.of_anchor.is_empty() {
            let cands = self.funcs_for_strings(&hook.of_anchor)?;
            if cands.len() != 1 {
                return Err(format!(
                    "{}: of_anchor matched {} functions",
                    hook.name,
                    cands.len()
                ));
            }
            let base = *cands.iter().next().unwrap();
            return Ok((base, format!("anchor {:?}", hook.of_anchor[0])));
        }
        Err(format!(
            "{}: kind={} needs `of` or `of_anchor`",
            hook.name, hook.kind
        ))
    }

    fn resolve_one(&mut self, hook: &HookRule) -> Result<Resolved, String> {
        match hook.kind.as_str() {
            "same_as" => {
                let of = hook
                    .of
                    .clone()
                    .ok_or_else(|| format!("{}: kind=same_as needs a resolved `of`", hook.name))?;
                let base = self
                    .resolved
                    .get(&of)
                    .ok_or_else(|| format!("{}: kind=same_as needs a resolved `of`", hook.name))?;
                Ok(Resolved {
                    name: hook.name.clone(),
                    rva: base.rva,
                    via: format!("same_as {of}"),
                })
            }
            "vtable_slot" => {
                let class_name = hook
                    .class_name
                    .clone()
                    .ok_or_else(|| format!("{}: vtable_slot needs class_name + slot", hook.name))?;
                let slot = hook
                    .slot
                    .ok_or_else(|| format!("{}: vtable_slot needs class_name + slot", hook.name))?;
                if self.rtti.is_none() {
                    self.rtti = Some(Rtti::new(self.pe));
                }
                let rtti = self.rtti.as_ref().unwrap();
                let tables = rtti.vtables_for_class(&class_name);
                if tables.is_empty() {
                    return Err(format!(
                        "{}: no RTTI vtable for class {class_name:?}",
                        hook.name
                    ));
                }
                let mut cands: HashSet<u32> = HashSet::new();
                for table in &tables {
                    if table.functions.len() > slot {
                        cands.insert(table.functions[slot]);
                    }
                }
                if cands.len() != 1 {
                    return Err(format!(
                        "{}: class {class_name} has {} vtables, slot {slot} gave {} candidates",
                        hook.name,
                        tables.len(),
                        cands.len()
                    ));
                }
                let rva = *cands.iter().next().unwrap();
                Ok(Resolved {
                    name: hook.name.clone(),
                    rva,
                    via: format!("vtable {class_name}[{slot}]"),
                })
            }
            "global_thunk" => {
                if hook.of.is_none() && hook.of_anchor.is_empty() {
                    return Err(format!(
                        "{}: global_thunk needs `of` or `of_anchor`",
                        hook.name
                    ));
                }
                let (base, origin) = self.base_and_origin(hook)?;
                let (begin, end) = self
                    .pe
                    .func_at(base)
                    .ok_or_else(|| format!("{}: base 0x{base:X} has no .pdata entry", hook.name))?;
                let mut globals_touched: HashSet<u32> = HashSet::new();
                for (target, sites) in self.pe.rip_refs() {
                    if sites.iter().any(|&s| s >= begin && s < end) {
                        globals_touched.insert(*target);
                    }
                }
                if globals_touched.is_empty() {
                    return Err(format!("{}: base {origin} touches no globals", hook.name));
                }
                let thunks = self.pe.getter_thunks();
                let matches: Vec<u32> = thunks
                    .iter()
                    .filter(|(_, g)| globals_touched.contains(g))
                    .map(|(site, _)| *site)
                    .collect();
                if matches.len() != 1 {
                    return Err(format!(
                        "{}: {} getter thunks read a global of {origin}",
                        hook.name,
                        matches.len()
                    ));
                }
                Ok(Resolved {
                    name: hook.name.clone(),
                    rva: matches[0],
                    via: format!("getter thunk for a global of {origin}"),
                })
            }
            "anchor" => {
                if hook.strings.is_empty() {
                    return Err(format!("{}: anchor rule needs strings", hook.name));
                }
                let cands = self.funcs_for_strings(&hook.strings)?;
                if cands.len() != 1 {
                    return Err(format!(
                        "{}: anchor matched {} functions",
                        hook.name,
                        cands.len()
                    ));
                }
                Ok(Resolved {
                    name: hook.name.clone(),
                    rva: *cands.iter().next().unwrap(),
                    via: format!("anchor {:?}", hook.strings[0]),
                })
            }
            kind @ ("callers" | "callee") => {
                let (base, origin) = self.base_and_origin(hook)?;
                let mut frontier: HashSet<u32> = HashSet::new();
                frontier.insert(base);
                for _ in 0..hook.depth.max(1) {
                    let mut step: HashSet<u32> = HashSet::new();
                    for func in &frontier {
                        let next = if kind == "callers" {
                            self.pe.callers_of(*func)
                        } else {
                            self.pe.callees_of(*func)
                        };
                        step.extend(next);
                    }
                    frontier = step;
                }
                let mut cands = frontier;
                if !hook.strings.is_empty() {
                    let allowed = self.funcs_for_strings(&hook.strings)?;
                    cands = cands.intersection(&allowed).copied().collect();
                }
                if let Some(span) = hook.span {
                    cands = cands
                        .into_iter()
                        .filter(|c| self.span_of(*c) == Some(span))
                        .collect();
                }
                if hook.leaf_only {
                    cands = cands
                        .into_iter()
                        .filter(|c| self.pe.func_at(*c).is_none())
                        .collect();
                }
                if let Some(floor) = hook.min_callers {
                    cands = cands
                        .into_iter()
                        .filter(|c| self.pe.callers_of(*c).len() >= floor)
                        .collect();
                }
                if cands.len() != 1 {
                    return Err(format!(
                        "{}: {kind} of {origin} matched {} functions",
                        hook.name,
                        cands.len()
                    ));
                }
                let mut extra = String::new();
                if let Some(span) = hook.span {
                    extra.push_str(&format!(" span={span}"));
                }
                if hook.leaf_only {
                    extra.push_str(" leaf");
                }
                Ok(Resolved {
                    name: hook.name.clone(),
                    rva: *cands.iter().next().unwrap(),
                    via: format!("{kind} of {origin}{extra}"),
                })
            }
            other => Err(format!("{}: unknown kind {other:?}", hook.name)),
        }
    }

    fn ensure(&mut self, name: &str, stack: &mut Vec<String>) -> Result<(), String> {
        if self.resolved.contains_key(name) {
            return Ok(());
        }
        if stack.iter().any(|s| s == name) {
            let mut chain = stack.clone();
            chain.push(name.to_string());
            return Err(format!("dependency cycle: {}", chain.join(" -> ")));
        }
        let hook = self
            .hooks
            .get(name)
            .cloned()
            .ok_or_else(|| format!("no rule for {name}"))?;
        if let Some(of) = hook.of.clone() {
            stack.push(name.to_string());
            self.ensure(&of, stack)?;
            stack.pop();
        }
        let resolved = self.resolve_one(&hook)?;
        self.resolved.insert(name.to_string(), resolved);
        Ok(())
    }

    pub fn resolve_all(&mut self) -> Result<Vec<PatternEntry>, String> {
        let mut entries = Vec::new();
        for name in self.order.clone() {
            let mut stack = Vec::new();
            self.ensure(&name, &mut stack)?;
            let rva = self.resolved.get(&name).unwrap().rva;
            let sig = sig::signature_at(self.pe, rva, sig::SIG_CAP, &self.space)?;
            entries.push(PatternEntry { name, rva, sig });
        }
        Ok(entries)
    }

    pub fn via_of(&self, name: &str) -> Option<&str> {
        self.resolved.get(name).map(|r| r.via.as_str())
    }
}