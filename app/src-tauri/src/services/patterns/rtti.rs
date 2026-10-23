//! MSVC RTTI -> vtable -> slot, ported from tools/pattern-tracker/ost_patterns/rtti.py.
//!
//! Used by `vtable_slot` rules: virtuals that are only reachable through the
//! vtable (`CSteamUIAppController::RunFrame`, `CCMInterface::RecvPkt`).

use std::collections::{HashMap, HashSet};

use super::pe::Pe;

const COL_SIZE: usize = 24;

#[derive(Debug, Clone)]
pub struct VTable {
    pub rva: u32,
    pub functions: Vec<u32>,
}

pub struct Rtti<'a> {
    pe: &'a Pe,
    by_name: HashMap<String, u32>,
    cols: Vec<u32>,
    vtables: Vec<VTable>,
}

impl<'a> Rtti<'a> {
    pub fn new(pe: &'a Pe) -> Rtti<'a> {
        let by_name = collect_type_descriptors(pe);
        let cols = collect_cols(pe);
        let vtables = collect_vtables(pe, &cols);
        Rtti {
            pe,
            by_name,
            cols,
            vtables,
        }
    }

    pub fn vtables(&self) -> &[VTable] {
        &self.vtables
    }

    pub fn col_count(&self) -> usize {
        self.cols.len()
    }

    /// TypeDescriptor RVAs whose decorated name matches `name`.
    pub fn find_class(&self, name: &str) -> Vec<u32> {
        let mut hits: Vec<u32> = Vec::new();
        for (decorated, td) in &self.by_name {
            if decorated.len() < 4 {
                continue;
            }
            let bare = decorated[4..].trim_end_matches('@');
            let suffix = format!("{name}@");
            if bare == name || bare.starts_with(&suffix) || bare == format!("{name}@@") {
                hits.push(*td);
            }
        }
        hits.sort_unstable();
        hits
    }

    pub fn vtables_for_class(&self, class_name: &str) -> Vec<VTable> {
        let descriptors: HashSet<u32> = self.find_class(class_name).into_iter().collect();
        if descriptors.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        for table in &self.vtables {
            let head_rva = match table.rva.checked_sub(8) {
                Some(v) => v,
                None => continue,
            };
            let raw = match self.pe.read(head_rva, 8) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let mut buf = [0u8; 8];
            buf.copy_from_slice(raw);
            let col = u64::from_le_bytes(buf).wrapping_sub(self.pe.image_base) as u32;
            let td = match self.pe.u32_at(col.wrapping_add(12)) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if descriptors.contains(&td) {
                out.push(table.clone());
            }
        }
        out
    }
}

fn collect_type_descriptors(pe: &Pe) -> HashMap<String, u32> {
    let mut table: HashMap<String, u32> = HashMap::new();
    let mut start = 0usize;
    while start + 4 <= pe.blob.len() {
        let idx = match find_from(&pe.blob, b".?AV", start) {
            Some(v) => v,
            None => break,
        };
        start = idx + 1;
        let end = match pe.blob[idx..].iter().position(|&b| b == 0) {
            Some(p) => idx + p,
            None => break,
        };
        let name = String::from_utf8_lossy(&pe.blob[idx..end]).into_owned();
        if let Some(rva) = pe.rva_of_offset(idx) {
            if rva >= 16 {
                table.entry(name).or_insert(rva - 16);
            }
        }
    }
    table
}

fn collect_cols(pe: &Pe) -> Vec<u32> {
    let mut cols = Vec::new();
    for section_name in [".rdata", ".data"] {
        let sec = match pe.section(section_name) {
            Ok(s) => s.clone(),
            Err(_) => continue,
        };
        let start = sec.raw_ptr as usize;
        let end = (start + sec.raw_size as usize).min(pe.blob.len());
        if end <= start + COL_SIZE {
            continue;
        }
        let raw = &pe.blob[start..end];
        if raw.len() <= COL_SIZE {
            continue;
        }
        let mut off = 0usize;
        while off < raw.len() - COL_SIZE {
            let sig = u32::from_le_bytes([raw[off], raw[off + 1], raw[off + 2], raw[off + 3]]);
            if sig != 0 && sig != 1 {
                off += 4;
                continue;
            }
            let p_type = u32::from_le_bytes([
                raw[off + 12],
                raw[off + 13],
                raw[off + 14],
                raw[off + 15],
            ]);
            if pe.section_of(p_type).is_none() {
                off += 4;
                continue;
            }
            let p_self = u32::from_le_bytes([
                raw[off + 20],
                raw[off + 21],
                raw[off + 22],
                raw[off + 23],
            ]);
            let want = sec.va.wrapping_add(off as u32);
            if p_self != want {
                off += 4;
                continue;
            }
            match pe.cstring(p_type.wrapping_add(16), 64) {
                Some(text) if text.starts_with(".?A") => cols.push(want),
                _ => {}
            }
            off += 4;
        }
    }
    cols
}

fn collect_vtables(pe: &Pe, cols: &[u32]) -> Vec<VTable> {
    let col_vas: HashSet<u64> = cols
        .iter()
        .map(|&rva| pe.image_base.wrapping_add(rva as u64))
        .collect();
    let text = match pe.text() {
        Ok(t) => t.clone(),
        Err(_) => return Vec::new(),
    };
    let text_lo = text.va;
    let text_hi = text.va.saturating_add(text.vsize);

    let mut found = Vec::new();
    for section_name in [".rdata", ".data"] {
        let sec = match pe.section(section_name) {
            Ok(s) => s.clone(),
            Err(_) => continue,
        };
        let start = sec.raw_ptr as usize;
        let end = (start + sec.raw_size as usize).min(pe.blob.len());
        if end <= start + 16 {
            continue;
        }
        let raw = &pe.blob[start..end];
        let mut off = 0usize;
        while off + 16 <= raw.len() {
            let mut head = [0u8; 8];
            head.copy_from_slice(&raw[off..off + 8]);
            if !col_vas.contains(&u64::from_le_bytes(head)) {
                off += 8;
                continue;
            }
            let vtable_rva = sec.va.wrapping_add(off as u32).wrapping_add(8);
            let mut funcs: Vec<u32> = Vec::new();
            let mut probe = off + 8;
            while probe + 8 <= raw.len() {
                let mut buf = [0u8; 8];
                buf.copy_from_slice(&raw[probe..probe + 8]);
                let value = u64::from_le_bytes(buf);
                let func_rva = if value > pe.image_base {
                    value.wrapping_sub(pe.image_base)
                } else {
                    value
                } as u32;
                if func_rva < text_lo || func_rva >= text_hi {
                    break;
                }
                funcs.push(func_rva);
                probe += 8;
            }
            if !funcs.is_empty() {
                found.push(VTable {
                    rva: vtable_rva,
                    functions: funcs,
                });
            }
            off += 8;
        }
    }
    found
}

fn find_from(hay: &[u8], needle: &[u8], start: usize) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    let last = hay.len() - needle.len();
    let mut i = start;
    while i <= last {
        if hay[i] == needle[0] && &hay[i..i + needle.len()] == needle {
            return Some(i);
        }
        i += 1;
    }
    None
}