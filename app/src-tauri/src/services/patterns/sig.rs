//! Signature synthesis, ported from tools/pattern-tracker/ost_patterns/sig.py.
//!
//! A signature is the byte prefix at the function entry. It must be unique in
//! the scan space and no longer than `SIG_CAP`; the published length is the
//! shortest unique prefix, floored at `SIG_FLOOR` so a hotfix that shifts a few
//! instructions cannot re-ambiguate it.

use super::pe::Pe;

pub const SIG_CAP: usize = 80;
pub const SIG_MIN: usize = 8;
pub const SIG_FLOOR: usize = 24;

pub fn hex_bytes(raw: &[u8]) -> String {
    let mut out = String::with_capacity(raw.len() * 3);
    for (i, b) in raw.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        out.push_str(&format!("{b:02X}"));
    }
    out
}

/// Overlapping occurrence count, matching `bytes.count`-style scanning.
pub fn occurrence_count(space: &[u8], needle: &[u8]) -> usize {
    if needle.is_empty() || needle.len() > space.len() {
        return 0;
    }
    let mut count = 0usize;
    let mut start = 0usize;
    while start + needle.len() <= space.len() {
        match find_from(space, needle, start) {
            Some(idx) => {
                count += 1;
                start = idx + 1;
            }
            None => break,
        }
    }
    count
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

pub fn shortest_unique_prefix(space: &[u8], needle: &[u8]) -> Result<Vec<u8>, String> {
    let from = SIG_MIN.min(needle.len());
    for size in from..=needle.len() {
        let prefix = &needle[..size];
        if occurrence_count(space, prefix) == 1 {
            return Ok(prefix.to_vec());
        }
    }
    Err(format!(
        "no unique prefix within {} bytes (first bytes {})",
        needle.len(),
        hex_bytes(&needle[..SIG_MIN.min(needle.len())])
    ))
}

pub fn scan_space(pe: &Pe) -> Result<Vec<u8>, String> {
    let sec = pe.section(".text").map_err(|e| e.to_string())?;
    let start = sec.raw_ptr as usize;
    let end = (start + sec.raw_size as usize).min(pe.blob.len());
    if start >= end {
        return Err("no scan sections found".to_string());
    }
    Ok(pe.blob[start..end].to_vec())
}

/// Published signature for the function at `rva`.
pub fn signature_at(pe: &Pe, rva: u32, cap: usize, space: &[u8]) -> Result<String, String> {
    let raw = pe.read(rva, cap).map_err(|e| e.to_string())?.to_vec();
    let mut best = shortest_unique_prefix(space, &raw)?;
    if best.len() < SIG_FLOOR {
        let trimmed = raw[..SIG_FLOOR.min(cap).min(raw.len())].to_vec();
        if occurrence_count(space, &trimmed) != 1 {
            return Err(format!("floor length {SIG_FLOOR} is not unique at 0x{rva:X}"));
        }
        best = trimmed;
    }
    Ok(hex_bytes(&best))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_roundtrip_shape() {
        assert_eq!(hex_bytes(&[0x48, 0x8b, 0x0f]), "48 8B 0F");
    }

    #[test]
    fn overlapping_occurrences_are_counted() {
        assert_eq!(occurrence_count(b"aaaa", b"aa"), 3);
        assert_eq!(occurrence_count(b"abcabc", b"abc"), 2);
    }

    #[test]
    fn shortest_unique_prefix_stops_at_first_unique_run() {
        let space = b"xxABCDEFGHyy";
        // 8 bytes is SIG_MIN, so the whole 8-byte window is returned.
        assert_eq!(shortest_unique_prefix(space, b"ABCDEFGH").unwrap(), b"ABCDEFGH");
    }
}