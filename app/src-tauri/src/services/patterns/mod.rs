//! In-app channel generation: Rust port of tools/pattern-tracker.
//!
//! Steam ships a new `steamui.dll` / `steamclient64.dll` with most client
//! updates, and every channel file is named after that DLL's SHA-256. Without
//! generation inside the app, each update forced a manual `python -m
//! ost_patterns.cli` run on a machine that has the repository and Python.
//!
//! Scope: the two `pattern` channels, derived from the binaries alone. The
//! `ipc` channel additionally needs the upstream `IPCMessages.steamd`, so it is
//! not generated here.

pub mod ipc;
pub mod pe;
pub mod resolve;
pub mod rtti;
pub mod sig;
pub mod tomlio;

use std::path::Path;

use sha2::{Digest, Sha256};

/// Rule sets are embedded at compile time: the app ships a single EXE, so it
/// cannot read them from `tools/` at runtime.
const RULES_STEAMUI: &str = include_str!("../../../../../tools/pattern-tracker/rules/steamui.toml");
const RULES_STEAMCLIENT: &str =
    include_str!("../../../../../tools/pattern-tracker/rules/steamclient.toml");

#[derive(Debug, Clone)]
pub struct GeneratedPattern {
    pub channel: String,
    pub component: String,
    pub sha256: String,
    pub text: String,
    pub hooks: usize,
}

/// DLL filename for a channel component.
pub fn dll_file_for(component: &str) -> Option<&'static str> {
    match component {
        "steamui" => Some("steamui.dll"),
        "steamclient" => Some("steamclient64.dll"),
        _ => None,
    }
}

fn rules_for(component: &str) -> Option<&'static str> {
    match component {
        "steamui" => Some(RULES_STEAMUI),
        "steamclient" => Some(RULES_STEAMCLIENT),
        _ => None,
    }
}

pub fn sha256_file(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

/// Resolve every hook for `component` against `dll` and render the channel TOML.
pub fn generate(dll: &Path, component: &str) -> Result<GeneratedPattern, String> {
    let rules_text = rules_for(component)
        .ok_or_else(|| format!("未知组件: {component}（支持 steamui / steamclient）"))?;
    let pe = pe::Pe::load(dll).map_err(|e| e.to_string())?;
    let (channel, hooks) = resolve::load_rules(rules_text)?;
    let mut resolver = resolve::Resolver::new(&pe, hooks)?;
    let mut entries = resolver.resolve_all()?;
    let hooks = entries.len();
    let text = tomlio::dump_pattern(&mut entries);
    let sha256 = sha256_file(dll)?;
    Ok(GeneratedPattern {
        channel,
        component: component.to_string(),
        sha256,
        text,
        hooks,
    })
}

/// Resolve the `ipc` channel: wire constants from `dll`, ids/arity from `idl`.
pub fn generate_ipc(dll: &Path, idl: &Path) -> Result<GeneratedPattern, String> {
    let idl_text =
        std::fs::read_to_string(idl).map_err(|e| format!("{}: {e}", idl.display()))?;
    let pe = pe::Pe::load(dll).map_err(|e| e.to_string())?;
    let (interfaces, failures) = ipc::build(&pe, &idl_text);
    if interfaces.is_empty() {
        return Err(format!(
            "没有从 IDL 里解析出可用的 IPC 方法（{} 处失败）",
            failures.len()
        ));
    }
    let hooks = ipc::method_count(&interfaces);
    let text = ipc::dump(&interfaces);
    let sha256 = sha256_file(dll)?;
    Ok(GeneratedPattern {
        channel: "ipc".to_string(),
        component: "steamclient".to_string(),
        sha256,
        text,
        hooks,
    })
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_rules_parse() {
        let (_c, ui) = resolve::load_rules(RULES_STEAMUI).unwrap();
        let (_c, client) = resolve::load_rules(RULES_STEAMCLIENT).unwrap();
        assert_eq!(ui.len(), 10, "steamui rule count changed");
        assert_eq!(client.len(), 25, "steamclient rule count changed");
    }

    #[test]
    fn unknown_component_is_rejected() {
        assert!(generate(Path::new("nope.dll"), "ipc").is_err());
    }

    /// Byte-level parity with the Python pipeline: compare everything except the
    /// provenance comment line. Skipped unless OST_TEST_DLL + OST_GOLDEN are set.
    #[test]
    fn matches_python_pipeline_when_golden_given() {
        let (Ok(dll), Ok(golden)) = (
            std::env::var("OST_TEST_DLL"),
            std::env::var("OST_GOLDEN"),
        ) else {
            return;
        };
        let component = std::env::var("OST_TEST_COMPONENT").unwrap_or_else(|_| "steamui".into());
        let got = generate(Path::new(&dll), &component).expect("generate");
        let want = std::fs::read_to_string(&golden).expect("golden");
        let strip = |text: &str| -> String {
            text.lines()
                .filter(|line| !line.starts_with('#'))
                .collect::<Vec<_>>()
                .join("\n")
        };
        assert_eq!(strip(&got.text), strip(&want), "channel TOML differs from python");
    }
    /// End-to-end check against a real install; skipped unless OST_TEST_DLL is
    /// set (CI has no Steam).
    #[test]
    fn resolves_real_steamui_when_available() {
        let Ok(path) = std::env::var("OST_TEST_DLL") else {
            return;
        };
        let got = generate(Path::new(&path), "steamui").expect("generate");
        assert_eq!(got.channel, "steamui");
        assert_eq!(got.hooks, 10);
        assert!(got.text.contains("name = \"AddProtobufAsBinary\""));
        assert!(got.text.ends_with('\n'));
    }
}