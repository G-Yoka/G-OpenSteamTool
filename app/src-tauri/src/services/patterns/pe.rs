//! Minimal x64 PE reader, ported from tools/pattern-tracker/ost_patterns/pe.py.
//!
//! Scope is deliberately narrow: sections + RVA translation, NUL-terminated
//! ASCII lookup, RIP-relative `lea`/`mov` xrefs, direct `call rel32` xrefs and
//! `.pdata` function boundaries. No external crate, no unsafe.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct PeError(pub String);

impl std::fmt::Display for PeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for PeError {}

type Result<T> = std::result::Result<T, PeError>;

#[derive(Debug, Clone)]
pub struct Section {
    pub name: String,
    pub va: u32,
    pub vsize: u32,
    pub raw_ptr: u32,
    pub raw_size: u32,
}

impl Section {
    fn vend(&self) -> u32 {
        self.va.saturating_add(self.vsize.max(self.raw_size))
    }

    fn contains(&self, rva: u32) -> bool {
        rva >= self.va && rva < self.vend()
    }
}

#[derive(Debug)]
pub struct Pe {
    pub path: PathBuf,
    pub blob: Vec<u8>,
    pub image_base: u64,
    pub size_of_image: u32,
    pub sections: Vec<Section>,
    /// sections sorted by va, for `section_of`
    sorted: Vec<Section>,
    lea: HashMap<u32, Vec<u32>>,
    rip: HashMap<u32, Vec<u32>>,
    thunks: HashMap<u32, u32>,
    call: HashMap<u32, Vec<u32>>,
    call_sites: Vec<(u32, u32)>,
    funcs: Vec<(u32, u32)>,
    func_starts: Vec<u32>,
}

const RIP_MODRM: [u8; 8] = [0x05, 0x0D, 0x15, 0x1D, 0x25, 0x2D, 0x35, 0x3D];

fn rd_u16(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

fn rd_u32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

fn rd_u64(b: &[u8], at: usize) -> u64 {
    let mut v = [0u8; 8];
    v.copy_from_slice(&b[at..at + 8]);
    u64::from_le_bytes(v)
}

fn rd_i32(b: &[u8], at: usize) -> i32 {
    i32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

impl Pe {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Pe> {
        let path = path.as_ref().to_path_buf();
        let blob = std::fs::read(&path).map_err(|e| PeError(format!("{}: {e}", path.display())))?;
        if blob.len() < 0x40 || &blob[0..2] != b"MZ" {
            return Err(PeError(format!("{}: not an MZ image", path.display())));
        }
        let e_lfanew = rd_u32(&blob, 0x3C) as usize;
        if e_lfanew + 24 > blob.len() || &blob[e_lfanew..e_lfanew + 4] != b"PE\0\0" {
            return Err(PeError(format!("{}: missing PE signature", path.display())));
        }
        let coff = e_lfanew + 4;
        let nsec = rd_u16(&blob, coff + 2) as usize;
        let opt_size = rd_u16(&blob, coff + 16) as usize;
        let opt = coff + 20;
        let magic = rd_u16(&blob, opt);
        if magic != 0x20B {
            return Err(PeError(format!(
                "{}: not PE32+ (magic {magic:#x})",
                path.display()
            )));
        }
        let image_base = rd_u64(&blob, opt + 24);
        let size_of_image = rd_u32(&blob, opt + 56);

        let mut sections = Vec::with_capacity(nsec);
        let sec_off = opt + opt_size;
        for i in 0..nsec {
            let base = sec_off + i * 40;
            if base + 40 > blob.len() {
                return Err(PeError(format!("{}: truncated section table", path.display())));
            }
            let raw_name = &blob[base..base + 8];
            let end = raw_name.iter().position(|&b| b == 0).unwrap_or(8);
            let name = String::from_utf8_lossy(&raw_name[..end]).into_owned();
            sections.push(Section {
                name,
                vsize: rd_u32(&blob, base + 8),
                va: rd_u32(&blob, base + 12),
                raw_size: rd_u32(&blob, base + 16),
                raw_ptr: rd_u32(&blob, base + 20),
            });
        }
        let mut sorted = sections.clone();
        sorted.sort_by_key(|s| s.va);

        let mut pe = Pe {
            path,
            blob,
            image_base,
            size_of_image,
            sections,
            sorted,
            lea: HashMap::new(),
            rip: HashMap::new(),
            thunks: HashMap::new(),
            call: HashMap::new(),
            call_sites: Vec::new(),
            funcs: Vec::new(),
            func_starts: Vec::new(),
        };
        pe.build_lea();
        pe.build_rip();
        pe.build_thunks();
        pe.build_calls();
        pe.build_funcs()?;
        Ok(pe)
    }

    // ------------------------------------------------------------------ basics

    pub fn section(&self, name: &str) -> Result<&Section> {
        self.sections
            .iter()
            .find(|s| s.name == name)
            .ok_or_else(|| PeError(format!("{}: no {name} section", self.path.display())))
    }

    pub fn text(&self) -> Result<&Section> {
        self.section(".text")
    }

    pub fn section_of(&self, rva: u32) -> Option<&Section> {
        let mut lo = 0usize;
        let mut hi = self.sorted.len();
        while lo < hi {
            let mid = (lo + hi) / 2;
            if self.sorted[mid].va <= rva {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo == 0 {
            return None;
        }
        let sec = &self.sorted[lo - 1];
        if sec.contains(rva) {
            Some(sec)
        } else {
            None
        }
    }

    pub fn offset(&self, rva: u32) -> Result<usize> {
        let sec = self
            .section_of(rva)
            .ok_or_else(|| PeError(format!("rva {rva:#x} outside any section")))?;
        let delta = rva - sec.va;
        if delta >= sec.raw_size {
            return Err(PeError(format!("rva {rva:#x} has no file backing")));
        }
        Ok((sec.raw_ptr + delta) as usize)
    }

    pub fn read(&self, rva: u32, size: usize) -> Result<&[u8]> {
        let off = self.offset(rva)?;
        if off + size > self.blob.len() {
            return Err(PeError(format!("short read at rva {rva:#x} (+{size})")));
        }
        Ok(&self.blob[off..off + size])
    }

    pub fn u32_at(&self, rva: u32) -> Result<u32> {
        Ok(rd_u32(self.read(rva, 4)?, 0))
    }

    pub fn rva_of_offset(&self, off: usize) -> Option<u32> {
        for sec in &self.sections {
            let start = sec.raw_ptr as usize;
            if off >= start && off < start + sec.raw_size as usize {
                return Some(sec.va + (off - start) as u32);
            }
        }
        None
    }

    // ----------------------------------------------------------------- strings

    pub fn cstring(&self, rva: u32, limit: usize) -> Option<String> {
        let sec = self.section_of(rva)?;
        if !matches!(sec.name.as_str(), ".rdata" | ".data" | ".text") {
            return None;
        }
        let off = self.offset(rva).ok()?;
        let end = self.blob[off..(off + limit).min(self.blob.len())]
            .iter()
            .position(|&b| b == 0)
            .map(|p| off + p)?;
        if end == off {
            return None;
        }
        let raw = &self.blob[off..end];
        if !raw.iter().all(|&b| (0x20..0x7F).contains(&b)) {
            return None;
        }
        Some(String::from_utf8_lossy(raw).into_owned())
    }

    pub fn find_cstring(&self, text: &str) -> Vec<u32> {
        let mut needle = text.as_bytes().to_vec();
        needle.push(0);
        let mut hits = Vec::new();
        let mut start = 0usize;
        while start + needle.len() <= self.blob.len() {
            match find_from(&self.blob, &needle, start) {
                Some(idx) => {
                    if let Some(rva) = self.rva_of_offset(idx) {
                        hits.push(rva);
                    }
                    start = idx + 1;
                }
                None => break,
            }
        }
        hits
    }

    // ------------------------------------------------------------------- xrefs

    fn build_lea(&mut self) {
        let (va, ptr, size) = match self.text() {
            Ok(s) => (s.va, s.raw_ptr as usize, s.raw_size as usize),
            Err(_) => return,
        };
        let blob = &self.blob[ptr..(ptr + size).min(self.blob.len())];
        let mut i = 0usize;
        while i + 2 < blob.len() {
            let (rex, len) = if (0x40..=0x4F).contains(&blob[i]) && blob[i + 1] == 0x8D {
                (true, 3usize)
            } else if blob[i] == 0x8D {
                (false, 2usize)
            } else {
                i += 1;
                continue;
            };
            if !RIP_MODRM.contains(&blob[i + len - 1]) {
                i += 1;
                continue;
            }
            let disp_at = i + len;
            if disp_at + 4 > blob.len() {
                break;
            }
            let disp = rd_i32(blob, disp_at);
            let insn_rva = va + i as u32;
            let target = insn_rva as i64 + if rex { 7 } else { 6 } + disp as i64;
            if target > 0 && target < self.size_of_image as i64 {
                self.lea.entry(target as u32).or_default().push(insn_rva);
            }
            i += len;
        }
        for v in self.lea.values_mut() {
            v.sort_unstable();
        }
    }

    fn build_rip(&mut self) {
        let (va, ptr, size) = match self.text() {
            Ok(s) => (s.va, s.raw_ptr as usize, s.raw_size as usize),
            Err(_) => return,
        };
        let blob = &self.blob[ptr..(ptr + size).min(self.blob.len())];
        let mut i = 0usize;
        while i + 2 < blob.len() {
            let is_plain = blob[i] == 0x8B || blob[i] == 0x8D;
            let (rex, len) = if (0x40..=0x4F).contains(&blob[i])
                && (blob[i + 1] == 0x8B || blob[i + 1] == 0x8D)
            {
                (true, 3usize)
            } else if is_plain {
                (false, 2usize)
            } else {
                i += 1;
                continue;
            };
            if !RIP_MODRM.contains(&blob[i + len - 1]) {
                i += 1;
                continue;
            }
            let disp_at = i + len;
            if disp_at + 4 > blob.len() {
                break;
            }
            let disp = rd_i32(blob, disp_at);
            let insn_rva = va + i as u32;
            let target = insn_rva as i64 + if rex { 7 } else { 6 } + disp as i64;
            if target > 0 && target < self.size_of_image as i64 {
                self.rip.entry(target as u32).or_default().push(insn_rva);
            }
            i += len;
        }
        for v in self.rip.values_mut() {
            v.sort_unstable();
        }
    }

    fn build_thunks(&mut self) {
        let (va, ptr, size) = match self.text() {
            Ok(s) => (s.va, s.raw_ptr as usize, s.raw_size as usize),
            Err(_) => return,
        };
        let blob = &self.blob[ptr..(ptr + size).min(self.blob.len())];
        let mut i = 0usize;
        while i + 8 <= blob.len() {
            if blob[i] == 0x48 && blob[i + 1] == 0x8B && blob[i + 2] == 0x05 && blob[i + 7] == 0xC3 {
                let disp = rd_i32(blob, i + 3);
                let insn_rva = va + i as u32;
                self.thunks.insert(insn_rva, (insn_rva as i64 + 7 + disp as i64) as u32);
                i += 8;
            } else {
                i += 1;
            }
        }
    }

    fn build_calls(&mut self) {
        let (va, ptr, size) = match self.text() {
            Ok(s) => (s.va, s.raw_ptr as usize, s.raw_size as usize),
            Err(_) => return,
        };
        let blob = &self.blob[ptr..(ptr + size).min(self.blob.len())];
        for i in 0..blob.len() {
            if blob[i] != 0xE8 || i + 5 > blob.len() {
                continue;
            }
            let rel = rd_i32(blob, i + 1);
            let insn_rva = va + i as u32;
            let callee = insn_rva as i64 + 5 + rel as i64;
            if callee > 0 && callee < self.size_of_image as i64 {
                self.call.entry(callee as u32).or_default().push(insn_rva);
            }
        }
        for v in self.call.values_mut() {
            v.sort_unstable();
        }
        let mut sites: Vec<(u32, u32)> = Vec::new();
        for (callee, callers) in &self.call {
            for site in callers {
                sites.push((*site, *callee));
            }
        }
        sites.sort_unstable();
        self.call_sites = sites;
    }

    pub fn lea_xrefs(&self) -> &HashMap<u32, Vec<u32>> {
        &self.lea
    }

    pub fn rip_refs(&self) -> &HashMap<u32, Vec<u32>> {
        &self.rip
    }

    pub fn getter_thunks(&self) -> &HashMap<u32, u32> {
        &self.thunks
    }

    pub fn call_xrefs(&self) -> &HashMap<u32, Vec<u32>> {
        &self.call
    }

    // --------------------------------------------------------------- functions

    fn build_funcs(&mut self) -> Result<()> {
        let pdata = match self.section(".pdata") {
            Ok(s) => s.clone(),
            Err(_) => return Ok(()),
        };
        let count = (pdata.raw_size / 12) as usize;
        let mut rows: Vec<(u32, u32)> = Vec::with_capacity(count);
        for i in 0..count {
            let at = pdata.raw_ptr as usize + i * 12;
            if at + 12 > self.blob.len() {
                break;
            }
            let begin = rd_u32(&self.blob, at);
            let end = rd_u32(&self.blob, at + 4);
            if begin == 0 || end <= begin {
                continue;
            }
            rows.push((begin, end));
        }
        rows.sort_unstable();
        self.func_starts = rows.iter().map(|r| r.0).collect();
        self.funcs = rows;
        Ok(())
    }

    pub fn func_of(&self, rva: u32) -> Option<(u32, u32)> {
        if self.funcs.is_empty() {
            return None;
        }
        let i = match self.func_starts.binary_search(&rva) {
            Ok(pos) => pos,
            Err(pos) => pos.checked_sub(1)?,
        };
        let mut j = i as isize;
        let stop = (i as isize - 4).max(0);
        while j >= stop {
            let (begin, end) = self.funcs[j as usize];
            if begin <= rva && rva < end {
                return Some((begin, end));
            }
            j -= 1;
        }
        None
    }

    pub fn func_at(&self, rva: u32) -> Option<(u32, u32)> {
        match self.func_starts.binary_search(&rva) {
            Ok(pos) => Some(self.funcs[pos]),
            Err(_) => None,
        }
    }

    pub fn strings_in_func(&self, begin: u32, end: u32, min_len: usize) -> HashMap<u32, String> {
        let mut out = HashMap::new();
        for (target, sites) in &self.lea {
            if !sites.iter().any(|&s| s >= begin && s < end) {
                continue;
            }
            if let Some(text) = self.cstring(*target, 300) {
                if text.len() >= min_len {
                    out.insert(*target, text);
                }
            }
        }
        out
    }

    pub fn funcs_referencing(&self, rva: u32) -> Vec<u32> {
        let mut out: Vec<u32> = Vec::new();
        if let Some(sites) = self.lea.get(&rva) {
            for site in sites {
                if let Some((begin, _)) = self.func_of(*site) {
                    out.push(begin);
                }
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    pub fn callers_of(&self, func_rva: u32) -> Vec<u32> {
        let mut out: Vec<u32> = Vec::new();
        if let Some(sites) = self.call.get(&func_rva) {
            for site in sites {
                if let Some((begin, _)) = self.func_of(*site) {
                    out.push(begin);
                }
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    pub fn callees_of(&self, func_rva: u32) -> Vec<u32> {
        let (begin, end) = match self.func_at(func_rva) {
            Some(v) => v,
            None => return Vec::new(),
        };
        let lo = self.call_sites.partition_point(|&(site, _)| site < begin);
        let hi = self.call_sites.partition_point(|&(site, _)| site < end);
        let mut out: Vec<u32> = self.call_sites[lo..hi].iter().map(|&(_, c)| c).collect();
        out.sort_unstable();
        out.dedup();
        out
    }
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