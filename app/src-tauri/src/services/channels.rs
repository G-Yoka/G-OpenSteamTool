//! Build inspection and data-channel management.
//!
//! The manager used to only know about DLLs. Since the 2026-10 Steam updates the
//! install state is defined by three things the DLL consumes at start-up:
//!
//! * `pattern/<component>/<sha256>.toml` — function locations (RVA + signature)
//! * `ipc/steamclient/<sha256>.toml`     — interface/method wire constants
//! * the layout offsets baked into `OpenSteamTool.dll` (`SteamLayouts.gen.h`)
//!
//! This module reports that state and installs/removes the two data files. It is
//! deliberately read-mostly: nothing here patches Steam or starts a process.

use super::patterns;
use crate::manager::{self, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

const COMPONENTS: [&str; 2] = ["steamui", "steamclient"];
const CHANNELS: [&str; 2] = ["pattern", "ipc"];

#[derive(Debug, Clone, Serialize)]
pub struct ChannelFile {
    pub channel: String,
    pub component: String,
    pub path: String,
    pub present: bool,
    pub bytes: u64,
    pub entries: u32,
    pub entry_label: String,
    pub expected_name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DllProvenance {
    pub present: bool,
    pub path: String,
    pub sha256: String,
    pub size: u64,
    /// `pubData/cubData` parsed out of the build marker, e.g. `Some(("0x10","0x18"))`.
    pub layout_signature: Option<String>,
    pub has_suppress_support: bool,
    pub has_ipc_channel: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct TomlSnapshot {
    pub log_level: String,
    pub manifest_url: String,
    pub remote_url_template: String,
    pub suppress_missing_popup: bool,
    pub legacy_pattern_mirror: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CrashDump {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub modified_unix: u64,
    pub exception_code: Option<u32>,
    pub faulting_module: Option<String>,
    pub faulting_offset: Option<u64>,
    pub fault_address: Option<u64>,
    pub verdict: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct BuildInspection {
    pub steamui_sha256: Option<String>,
    pub steamclient_sha256: Option<String>,
    pub steam_version: Option<String>,
    pub dll: DllProvenance,
    pub channels: Vec<ChannelFile>,
    pub toml: TomlSnapshot,
    pub crash_dumps: Vec<CrashDump>,
}

fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).map_err(|e| format!("读取 {} 失败: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn component_sha(steam_dir: &Path, component: &str) -> Option<String> {
    let file = match component {
        "steamui" => "steamui.dll",
        "steamclient" => "steamclient64.dll",
        _ => return None,
    };
    let path = steam_dir.join(file);
    if !path.is_file() {
        return None;
    }
    sha256_file(&path).ok()
}

fn count_entries(channel: &str, text: &str) -> (u32, String) {
    if channel == "ipc" {
        let methods = text.matches("funcHash").count() as u32;
        let interfaces = text
            .lines()
            .filter(|line| {
                let t = line.trim();
                t.starts_with('[') && t.ends_with(']') && !t.contains('.')
            })
            .count() as u32;
        (methods, format!("{interfaces} 接口 / {methods} 方法"))
    } else {
        let hooks = text.matches("name =").count() as u32;
        (hooks, format!("{hooks} 钩子"))
    }
}

fn inspect_channel(steam_dir: &Path, channel: &str, component: &str) -> ChannelFile {
    let expected_name = component_sha(steam_dir, component).unwrap_or_default();
    let path = steam_dir
        .join("opensteamtool")
        .join(channel)
        .join(component)
        .join(format!("{expected_name}.toml"));
    let mut file = ChannelFile {
        channel: channel.to_string(),
        component: component.to_string(),
        path: manager::display_path(&path),
        present: false,
        bytes: 0,
        entries: 0,
        entry_label: "缺失".into(),
        expected_name: expected_name.clone(),
    };
    if expected_name.is_empty() || !path.is_file() {
        return file;
    }
    if let Ok(meta) = fs::metadata(&path) {
        file.present = true;
        file.bytes = meta.len();
    }
    if let Ok(text) = fs::read_to_string(&path) {
        let (count, label) = count_entries(channel, &text);
        file.entries = count;
        file.entry_label = label;
    }
    file
}

fn inspect_dll(steam_dir: &Path) -> DllProvenance {
    let path = steam_dir.join("OpenSteamTool.dll");
    let mut dll = DllProvenance {
        present: false,
        path: manager::display_path(&path),
        sha256: String::new(),
        size: 0,
        layout_signature: None,
        has_suppress_support: false,
        has_ipc_channel: false,
    };
    let Ok(bytes) = fs::read(&path) else {
        return dll;
    };
    dll.present = true;
    dll.size = bytes.len() as u64;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    dll.sha256 = format!("{:x}", hasher.finalize());

    // Build marker emitted by SteamLayouts.gen.h (see src/Steam/...).
    let marker = b"OSTLAYOUT:";
    if let Some(pos) = bytes.windows(marker.len()).position(|w| w == marker) {
        let tail = &bytes[pos + marker.len()..];
        let end = tail
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(tail.len().min(32));
        if let Ok(text) = std::str::from_utf8(&tail[..end]) {
            dll.layout_signature = Some(text.trim().to_string());
        }
    }
    dll.has_suppress_support = bytes
        .windows(b"suppress_missing_popup".len())
        .any(|w| w == b"suppress_missing_popup");
    dll.has_ipc_channel = bytes.windows(b"{channel}".len()).any(|w| w == b"{channel}");
    dll
}

fn steam_version(steam_dir: &Path) -> Option<String> {
    let log = steam_dir.join("logs").join("console_log.txt");
    let text = fs::read_to_string(log).ok()?;
    text.lines()
        .rev()
        .find_map(|line| line.split("Client version:").nth(1).map(|v| v.trim().to_string()))
}

fn inspect_toml(steam_dir: &Path) -> TomlSnapshot {
    let path = steam_dir.join("opensteamtool.toml");
    let mut snapshot = TomlSnapshot {
        log_level: "info".into(),
        manifest_url: "wudrm".into(),
        remote_url_template: String::new(),
        suppress_missing_popup: false,
        legacy_pattern_mirror: String::new(),
    };
    let Ok(text) = fs::read_to_string(&path) else {
        return snapshot;
    };
    let Ok(value) = text.parse::<toml::Value>() else {
        return snapshot;
    };
    if let Some(level) = value
        .get("log")
        .and_then(|v| v.get("level"))
        .and_then(|v| v.as_str())
    {
        snapshot.log_level = level.to_string();
    }
    if let Some(url) = value
        .get("manifest")
        .and_then(|v| v.get("url"))
        .and_then(|v| v.as_str())
    {
        snapshot.manifest_url = url.to_string();
    }
    if let Some(template) = value
        .get("remote")
        .and_then(|v| v.get("url_template"))
        .and_then(|v| v.as_str())
    {
        snapshot.remote_url_template = template.to_string();
    }
    snapshot.suppress_missing_popup = value
        .get("ui")
        .and_then(|v| v.get("suppress_missing_popup"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if let Some(mirror) = value
        .get("pattern")
        .and_then(|v| v.get("mirror"))
        .and_then(|v| v.as_str())
    {
        snapshot.legacy_pattern_mirror = mirror.to_string();
    }
    snapshot
}

// ── minidump ────────────────────────────────────────────────────────────────

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    bytes.get(offset..offset + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn read_u64(bytes: &[u8], offset: usize) -> Option<u64> {
    bytes.get(offset..offset + 8).map(|s| {
        u64::from_le_bytes([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]])
    })
}

fn utf16_name(bytes: &[u8], offset: usize) -> Option<String> {
    let length = read_u32(bytes, offset)? as usize;
    let raw = bytes.get(offset + 4..offset + 4 + length)?;
    let units: Vec<u16> = raw
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    Some(String::from_utf16_lossy(&units))
}

struct DumpFacts {
    exception_code: Option<u32>,
    fault_address: Option<u64>,
    module: Option<String>,
    offset: Option<u64>,
}

fn parse_minidump(bytes: &[u8]) -> Option<DumpFacts> {
    if bytes.get(0..4)? != b"MDMP" {
        return None;
    }
    let stream_count = read_u32(bytes, 8)? as usize;
    let dir_rva = read_u32(bytes, 12)? as usize;

    let mut modules: Vec<(u64, u32, String)> = Vec::new();
    let mut exception_rva: Option<usize> = None;

    for index in 0..stream_count {
        let entry = dir_rva + index * 12;
        let stream_type = read_u32(bytes, entry)?;
        let rva = read_u32(bytes, entry + 8)? as usize;
        match stream_type {
            4 => {
                let count = read_u32(bytes, rva)? as usize;
                for i in 0..count.min(512) {
                    let base_off = rva + 4 + i * 108;
                    let base = read_u64(bytes, base_off)?;
                    let size = read_u32(bytes, base_off + 8)?;
                    let name_rva = read_u32(bytes, base_off + 20)? as usize;
                    if let Some(name) = utf16_name(bytes, name_rva) {
                        modules.push((base, size, name));
                    }
                }
            }
            6 => exception_rva = Some(rva),
            _ => {}
        }
    }

    let rva = exception_rva?;
    let exception_code = read_u32(bytes, rva + 8);
    let instruction = read_u64(bytes, rva + 24);
    // ExceptionInformation[0] = access type (0 read / 1 write), [1] = address.
    let fault_address = read_u64(bytes, rva + 48);

    let mut module = None;
    let mut offset = None;
    if let Some(address) = instruction {
        for (base, size, name) in &modules {
            if address >= *base && address < base.saturating_add(*size as u64) {
                module = Some(name.clone());
                offset = Some(address - base);
                break;
            }
        }
    }

    Some(DumpFacts {
        exception_code,
        fault_address,
        module,
        offset,
    })
}

fn verdict_for(module: Option<&str>) -> String {
    match module {
        Some(name) if name.contains("OpenSteamTool") => {
            "OpenSteamTool 内部异常：优先核对 layout 偏移与钩子返回的指针".into()
        }
        Some(name) if name.contains("steamclient64") => {
            "steamclient 内部异常：多为钩子把错误的指针/长度交给了 Steam".into()
        }
        Some(name) if name.contains("SteamUI") => {
            "SteamUI 内部异常：检查 pattern 中 steamui 钩子是否命中".into()
        }
        Some(_) => "其他模块异常：与数据通道基本无关".into(),
        None => "无法定位故障模块".into(),
    }
}

fn scan_dumps(steam_dir: &Path, limit: usize) -> Vec<CrashDump> {
    let dir = steam_dir.join("dumps");
    let Ok(read_dir) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut dumps: Vec<CrashDump> = Vec::new();
    for entry in read_dir.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("dmp") {
            continue;
        }
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
        let meta = entry.metadata().ok();
        let facts = fs::read(&path).ok().and_then(|bytes| parse_minidump(&bytes));
        let (module, offset, code, fault) = match facts {
            Some(f) => (f.module, f.offset, f.exception_code, f.fault_address),
            None => (None, None, None, None),
        };
        dumps.push(CrashDump {
            name,
            path: manager::display_path(&path),
            size: meta.as_ref().map(|m| m.len()).unwrap_or(0),
            modified_unix: meta
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0),
            exception_code: code,
            faulting_module: module.clone(),
            faulting_offset: offset,
            fault_address: fault,
            verdict: verdict_for(module.as_deref()),
        });
    }
    dumps.sort_by(|a, b| b.modified_unix.cmp(&a.modified_unix));
    dumps.truncate(limit);
    dumps
}

// ── commands ────────────────────────────────────────────────────────────────

pub fn inspect(steam_dir: String) -> Result<BuildInspection> {
    let dir = manager::validate_steam_dir(&steam_dir)?;
    let mut channels = Vec::new();
    for channel in CHANNELS {
        for component in COMPONENTS {
            // ipc only exists for steamclient today.
            if channel == "ipc" && component != "steamclient" {
                continue;
            }
            channels.push(inspect_channel(&dir, channel, component));
        }
    }
    Ok(BuildInspection {
        steamui_sha256: component_sha(&dir, "steamui"),
        steamclient_sha256: component_sha(&dir, "steamclient"),
        steam_version: steam_version(&dir),
        dll: inspect_dll(&dir),
        channels,
        toml: inspect_toml(&dir),
        crash_dumps: scan_dumps(&dir, 8),
    })
}

fn channel_dir(steam_dir: &Path, channel: &str, component: &str) -> Result<PathBuf> {
    if !CHANNELS.contains(&channel) {
        return Err(format!("未知通道: {channel}"));
    }
    if !COMPONENTS.contains(&component) {
        return Err(format!("未知组件: {component}"));
    }
    if channel == "ipc" && component != "steamclient" {
        return Err("ipc 通道仅用于 steamclient".into());
    }
    Ok(steam_dir.join("opensteamtool").join(channel).join(component))
}

pub fn install_file(
    steam_dir: String,
    channel: String,
    component: String,
    source: String,
) -> Result<ChannelFile> {
    let dir = manager::validate_steam_dir(&steam_dir)?;
    let target_dir = channel_dir(&dir, &channel, &component)?;
    let sha = component_sha(&dir, &component)
        .ok_or_else(|| format!("找不到 {component} 的 DLL，无法确定目标文件名"))?;
    let source_path = PathBuf::from(&source);
    if !source_path.is_file() {
        return Err(format!("源文件不存在: {source}"));
    }
    fs::create_dir_all(&target_dir).map_err(|e| format!("创建目录失败: {e}"))?;
    let target = target_dir.join(format!("{sha}.toml"));
    fs::copy(&source_path, &target).map_err(|e| format!("复制失败: {e}"))?;
    Ok(inspect_channel(&dir, &channel, &component))
}

/// Generate the `pattern` channel file for the current Steam build, in-process.
///
/// `ipc` is deliberately excluded: its wire constants need the upstream
/// `IPCMessages.steamd`, which is not shipped with this app.
pub fn generate_channel(
    steam_dir: String,
    channel: String,
    component: String,
) -> Result<ChannelFile> {
    let dir = manager::validate_steam_dir(&steam_dir)?;
    if channel != "pattern" {
        return Err(
            "ipc 通道需要上游 IPCMessages.steamd，应用内目前只能生成 pattern".to_string(),
        );
    }
    let dll_name = patterns::dll_file_for(&component)
        .ok_or_else(|| format!("未知组件: {component}"))?;
    let dll = dir.join(dll_name);
    if !dll.is_file() {
        return Err(format!("找不到 {dll_name}，请先确认 Steam 目录"));
    }
    let generated = patterns::generate(&dll, &component)?;
    let target_dir = dir.join("opensteamtool").join(&channel).join(&component);
    std::fs::create_dir_all(&target_dir).map_err(|e| e.to_string())?;
    let target = target_dir.join(format!("{}.toml", generated.sha256));
    write_atomic(&target, generated.text.as_bytes())?;
    Ok(inspect_channel(&dir, &channel, &component))
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
    if path.exists() {
        std::fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}
pub fn remove_file(steam_dir: String, channel: String, component: String) -> Result<ChannelFile> {
    let dir = manager::validate_steam_dir(&steam_dir)?;
    let target_dir = channel_dir(&dir, &channel, &component)?;
    if let Some(sha) = component_sha(&dir, &component) {
        let target = target_dir.join(format!("{sha}.toml"));
        if target.is_file() {
            fs::remove_file(&target).map_err(|e| format!("删除失败: {e}"))?;
        }
    }
    Ok(inspect_channel(&dir, &channel, &component))
}

pub fn dumps(steam_dir: String) -> Result<Vec<CrashDump>> {
    let dir = manager::validate_steam_dir(&steam_dir)?;
    Ok(scan_dumps(&dir, 20))
}
