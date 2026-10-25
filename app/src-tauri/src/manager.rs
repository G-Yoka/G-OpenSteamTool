use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsStr,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use steam_vdf_parser::value::Obj as VdfObject;
use steam_vdf_parser::value::Value as VdfValue;
use tauri::path::BaseDirectory;
use tauri::Manager;
#[cfg(windows)]
use windows::Win32::{
    Foundation::CloseHandle,
    System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Module32FirstW, Module32NextW, Process32FirstW, Process32NextW,
        MODULEENTRY32W, PROCESSENTRY32W, TH32CS_SNAPMODULE, TH32CS_SNAPMODULE32,
        TH32CS_SNAPPROCESS,
    },
};
#[cfg(windows)]
use winreg::{enums::*, RegKey};

const DLL_NAMES: [&str; 3] = ["OpenSteamTool.dll", "dwmapi.dll", "xinput1_4.dll"];
const OPENSTEAMTOOL_DLL: &[u8] = include_bytes!("../resources/dlls/OpenSteamTool.dll");
const DWMAPI_DLL: &[u8] = include_bytes!("../resources/dlls/dwmapi.dll");
const XINPUT1_4_DLL: &[u8] = include_bytes!("../resources/dlls/xinput1_4.dll");
const INSTALL_MANIFEST: &str = ".g-opensteamtool-dlls.json";
const META_PREFIX: &str = "-- GOST-META: ";
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub type Result<T> = std::result::Result<T, String>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagerSettings {
    pub log_level: String,
    pub manifest_url: String,
    pub timeout_resolve_ms: u64,
    pub timeout_connect_ms: u64,
    pub timeout_send_ms: u64,
    pub timeout_recv_ms: u64,
    pub lua_paths: Vec<String>,
    /// `[remote] url_template` - the key OpenSteamTool actually reads. It must
    /// contain `{channel}`, `{component}` and `{sha256}`.
    #[serde(default)]
    pub remote_url_template: String,
    /// `[ui] suppress_missing_popup` - only honoured by our own DLL build.
    #[serde(default)]
    pub suppress_missing_popup: bool,
    /// Legacy `[pattern] mirror` key. Read for migration only; the DLL ignores
    /// it, so it is never written back.
    #[serde(default)]
    pub pattern_mirror: String,
    /// Write `<Steam>/config/lua/manifest.lua` with the manifest-code fallback
    /// chain. The bundled DLL only knows `opensteamtool`/`wudrm`/`steamrun`,
    /// and all three stopped working after Valve added the ownership check, so
    /// the download channel now has to be supplied through Lua.
    #[serde(default = "default_true")]
    pub manage_manifest_lua: bool,
    /// Optional self-hosted endpoint prefix (for example
    /// `http://127.0.0.1:8787/manifest/`). The gid is appended verbatim.
    #[serde(default)]
    pub manifest_lua_endpoint: String,
}

impl Default for ManagerSettings {
    fn default() -> Self {
        Self {
            log_level: "info".into(),
            manifest_url: "opensteamtool".into(),
            timeout_resolve_ms: 5000,
            timeout_connect_ms: 5000,
            timeout_send_ms: 10000,
            timeout_recv_ms: 10000,
            lua_paths: Vec::new(),
            remote_url_template: String::new(),
            suppress_missing_popup: false,
            pattern_mirror: String::new(),
            manage_manifest_lua: true,
            manifest_lua_endpoint: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppIdEntry {
    pub appid: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unlock_flag: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depot_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManifestEntry {
    pub depot_id: u32,
    pub manifest_gid: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GameConfig {
    pub appid: u32,
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub depot_key: Option<String>,
    pub access_token: Option<String>,
    pub manifest_gid: Option<String>,
    pub app_ticket_hex: Option<String>,
    pub e_ticket_hex: Option<String>,
    pub stat_steam_id: Option<String>,
    #[serde(default)]
    pub appid_entries: Vec<AppIdEntry>,
    #[serde(default)]
    pub manifest_entries: Vec<ManifestEntry>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DllState {
    Missing,
    Managed,
    Foreign,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DllLoadState {
    Loaded,
    NotLoaded,
    SteamNotRunning,
    VerifyFailed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DllStatus {
    pub name: String,
    pub state: DllState,
    pub target_path: String,
    pub resource_hash: Option<String>,
    pub target_hash: Option<String>,
    pub hash_matched: bool,
    pub loaded_by_steam: bool,
    pub load_state: DllLoadState,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScanState {
    pub steam_dir: String,
    pub steam_valid: bool,
    pub steam_running: bool,
    pub steam_version: Option<String>,
    pub config_exists: bool,
    pub lua_count: usize,
    pub dlls: Vec<DllStatus>,
    pub dll_resources_ready: bool,
    pub missing_dll_resources: Vec<String>,
    pub log_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppMetadata {
    pub appid: u32,
    pub name: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscoveredDepot {
    pub depot_id: u32,
    pub depot_key: Option<String>,
    pub manifest_gid: Option<String>,
}

/// One DLC of the discovered App: its own AppId, the depots the DLC ships, and
/// whether any of those depots already has a local `config.vdf` key.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscoveredDlc {
    pub appid: u32,
    pub name: Option<String>,
    pub depots: Vec<DiscoveredDepot>,
    pub key_found: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DepotDiscovery {
    pub appid: u32,
    pub name: String,
    pub depots: Vec<DiscoveredDepot>,
    /// DLC enumerated from `appinfo.extended.listofdlc`; DLC with a local key
    /// are sorted first so the UI can default-select the owned ones.
    pub dlc: Vec<DiscoveredDlc>,
    pub app_source: String,
    pub config_found: bool,
}

/// Which part of a game an AppId / DepotId belongs to. Used to label the rows of
/// an already-saved Lua, where the discovery result itself is long gone.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DepotOwnership {
    pub appid: u32,
    /// `"base"` or `"dlc"`.
    pub group: String,
    pub dlc_appid: Option<u32>,
    pub dlc_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LogFile {
    pub name: String,
    pub content: String,
    pub size_bytes: u64,
    pub modified_time: Option<u64>,
    pub line_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct InstallManifest {
    dlls: BTreeMap<String, String>,
}

struct SteamModuleScan {
    steam_running: bool,
    verify_failed: bool,
    modules: Vec<(String, PathBuf)>,
}

pub fn validate_steam_dir<P: AsRef<Path>>(steam_dir: P) -> Result<PathBuf> {
    let path = steam_dir.as_ref();
    if !path.is_dir() {
        return Err("Steam directory does not exist".into());
    }
    let steam_exe = path.join("steam.exe");
    if !steam_exe.is_file() {
        return Err("Selected directory must contain steam.exe".into());
    }
    fs::canonicalize(path).map_err(|err| err.to_string())
}

pub fn detect_steam_dir() -> Result<Option<PathBuf>> {
    detect_steam_dir_from_candidates(steam_dir_candidates())
}

pub fn detect_steam_dir_from_candidates<I>(candidates: I) -> Result<Option<PathBuf>>
where
    I: IntoIterator<Item = PathBuf>,
{
    for candidate in candidates {
        if let Ok(path) = validate_steam_dir(&candidate) {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

pub fn display_path<P: AsRef<Path>>(path: P) -> String {
    strip_verbatim_prefix(&path.as_ref().display().to_string())
}

pub fn strip_verbatim_prefix(path: &str) -> String {
    if let Some(rest) = path.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = path.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        path.to_string()
    }
}

pub fn steam_dir_candidates() -> Vec<PathBuf> {
    let mut candidates = steam_registry_candidates();
    if let Ok(path) = std::env::var("ProgramFiles(x86)") {
        candidates.push(PathBuf::from(path).join("Steam"));
    }
    if let Ok(path) = std::env::var("ProgramFiles") {
        candidates.push(PathBuf::from(path).join("Steam"));
    }
    candidates.push(PathBuf::from("C:\\Program Files (x86)\\Steam"));

    dedupe_paths(candidates)
}

pub fn resource_dll_dir(app: &tauri::AppHandle) -> Result<PathBuf> {
    resolve_dll_resource_dir_from_candidates(dll_resource_candidates(app))
        .ok_or_else(|| "Missing bundled DLL resources".to_string())
}

pub fn dll_resource_candidates(app: &tauri::AppHandle) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(path) = app.path().resolve("dlls", BaseDirectory::Resource) {
        candidates.push(path);
    }
    if let Ok(dir) = app.path().resource_dir() {
        candidates.push(dir.join("dlls"));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("dlls"));
            candidates.push(dir.join("resources").join("dlls"));
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("src-tauri").join("resources").join("dlls"));
        candidates.push(cwd.join("resources").join("dlls"));
        candidates.push(cwd.join("dlls"));
        if cwd.file_name().and_then(OsStr::to_str) == Some("src-tauri") {
            if let Some(parent) = cwd.parent() {
                candidates.push(parent.join("src-tauri").join("resources").join("dlls"));
                candidates.push(parent.join("dlls"));
            }
        }
    }
    dedupe_paths(candidates)
}

pub fn resolve_dll_resource_dir_from_candidates<I>(candidates: I) -> Option<PathBuf>
where
    I: IntoIterator<Item = PathBuf>,
{
    candidates
        .into_iter()
        .find(|candidate| missing_resources(Some(candidate)).is_empty())
}

pub fn load_settings_from_dir<P: AsRef<Path>>(steam_dir: P) -> Result<ManagerSettings> {
    let steam_dir = validate_steam_dir(steam_dir)?;
    let path = steam_dir.join("opensteamtool.toml");
    if !path.exists() {
        return Ok(ManagerSettings::default());
    }

    let text = fs::read_to_string(path).map_err(|err| err.to_string())?;
    let value: toml::Value = toml::from_str(&text).map_err(|err| err.to_string())?;
    let mut settings = ManagerSettings::default();

    if let Some(log) = value.get("log").and_then(|v| v.as_table()) {
        if let Some(level) = log.get("level").and_then(|v| v.as_str()) {
            settings.log_level = level.to_string();
        }
    }
    if let Some(manifest) = value.get("manifest").and_then(|v| v.as_table()) {
        if let Some(url) = manifest.get("url").and_then(|v| v.as_str()) {
            settings.manifest_url = url.to_string();
        }
        settings.timeout_resolve_ms =
            read_u64(manifest, "timeout_resolve_ms", settings.timeout_resolve_ms);
        settings.timeout_connect_ms =
            read_u64(manifest, "timeout_connect_ms", settings.timeout_connect_ms);
        settings.timeout_send_ms = read_u64(manifest, "timeout_send_ms", settings.timeout_send_ms);
        settings.timeout_recv_ms = read_u64(manifest, "timeout_recv_ms", settings.timeout_recv_ms);
    }
    if let Some(lua) = value.get("lua").and_then(|v| v.as_table()) {
        if let Some(paths) = lua.get("paths").and_then(|v| v.as_array()) {
            settings.lua_paths = paths
                .iter()
                .filter_map(|entry| entry.as_str().map(ToString::to_string))
                .collect();
        }
    }
    if let Some(pattern) = value.get("pattern").and_then(|v| v.as_table()) {
        if let Some(mirror) = pattern.get("mirror").and_then(|v| v.as_str()) {
            // Legacy key; migrate it to a url template below when the real key
            // is absent. The DLL never reads this section.
            settings.pattern_mirror = mirror.to_string();
        }
    }
    if let Some(remote) = value.get("remote").and_then(|v| v.as_table()) {
        if let Some(template) = remote.get("url_template").and_then(|v| v.as_str()) {
            settings.remote_url_template = template.to_string();
        }
    }
    if let Some(ui) = value.get("ui").and_then(|v| v.as_table()) {
        if let Some(flag) = ui.get("suppress_missing_popup").and_then(|v| v.as_bool()) {
            settings.suppress_missing_popup = flag;
        }
    }
    // G-OpenSteamTool-specific section; the DLL ignores unknown sections.
    if let Some(manager) = value.get("manager").and_then(|v| v.as_table()) {
        if let Some(flag) = manager.get("manage_manifest_lua").and_then(|v| v.as_bool()) {
            settings.manage_manifest_lua = flag;
        }
        if let Some(endpoint) = manager.get("manifest_lua_endpoint").and_then(|v| v.as_str()) {
            settings.manifest_lua_endpoint = endpoint.to_string();
        }
    }
    if settings.remote_url_template.trim().is_empty() {
        let legacy = settings.pattern_mirror.trim().trim_end_matches('/');
        if !legacy.is_empty() {
            // e.g. https://host/Org/repo/pattern -> https://host/Org/repo/pattern/{channel}/...
            settings.remote_url_template =
                format!("{legacy}/{{channel}}/{{component}}/{{sha256}}.toml");
        }
    }

    Ok(settings)
}

pub fn save_settings_to_dir<P: AsRef<Path>>(
    steam_dir: P,
    settings: &ManagerSettings,
) -> Result<()> {
    let steam_dir = validate_steam_dir(steam_dir)?;
    validate_settings(settings)?;
    let lua_paths = settings
        .lua_paths
        .iter()
        .map(|path| format!("\"{}\"", escape_toml(path)))
        .collect::<Vec<_>>()
        .join(", ");
    let mut text = format!(
        "[log]\nlevel = \"{}\"\n\n[manifest]\nurl = \"{}\"\ntimeout_resolve_ms = {}\ntimeout_connect_ms = {}\ntimeout_send_ms = {}\ntimeout_recv_ms = {}\n\n[lua]\npaths = [{}]\n",
        escape_toml(&settings.log_level),
        escape_toml(&settings.manifest_url),
        settings.timeout_resolve_ms,
        settings.timeout_connect_ms,
        settings.timeout_send_ms,
        settings.timeout_recv_ms,
        lua_paths
    );
    let template = settings.remote_url_template.trim();
    if !template.is_empty() {
        for placeholder in ["{channel}", "{component}", "{sha256}"] {
            if !template.contains(placeholder) {
                return Err(format!(
                    "remote url_template 缺少占位符 {placeholder}（必须同时包含 \
                     {{channel}}、{{component}}、{{sha256}}）"
                ));
            }
        }
        text.push_str(&format!(
            "\n[remote]\nurl_template = \"{}\"\n",
            escape_toml(template)
        ));
    }
    text.push_str(&format!(
        "\n[ui]\nsuppress_missing_popup = {}\n",
        settings.suppress_missing_popup
    ));
    text.push_str(&format!(
        "\n[manager]\nmanage_manifest_lua = {}\nmanifest_lua_endpoint = \"{}\"\n",
        settings.manage_manifest_lua,
        escape_toml(&settings.manifest_lua_endpoint)
    ));
    safe_replace_file(&steam_dir.join("opensteamtool.toml"), text.as_bytes())?;
    // The DLL only knows three provider names and all of them are dead; the
    // working download channel is delivered through config/lua/manifest.lua.
    sync_manifest_lua(&steam_dir, settings)
}

pub fn render_game_lua(game: &GameConfig) -> Result<String> {
    validate_game(game)?;
    let meta = serde_json::to_string(game).map_err(|err| err.to_string())?;
    let mut lines = vec![
        format!("-- G-OpenSteamTool: {} {}", game.appid, game.name.trim()),
        format!("{META_PREFIX}{meta}"),
    ];

    for entry in normalized_appid_entries(game) {
        match (entry.unlock_flag, clean_opt(&entry.depot_key)) {
            (Some(flag), Some(key)) => {
                lines.push(format!("addappid({}, {}, \"{}\")", entry.appid, flag, key))
            }
            (Some(flag), None) => lines.push(format!("addappid({}, {})", entry.appid, flag)),
            (None, Some(key)) => lines.push(format!("addappid({}, 0, \"{}\")", entry.appid, key)),
            (None, None) => lines.push(format!("addappid({})", entry.appid)),
        }
    }
    if let Some(token) = clean_opt(&game.access_token) {
        lines.push(format!("addtoken({}, \"{}\")", game.appid, token));
    }
    let manifest_entries = normalized_manifest_entries(game);
    if !manifest_entries.is_empty() {
        for entry in manifest_entries {
            lines.push(format!(
                "setManifestid({}, \"{}\")",
                entry.depot_id, entry.manifest_gid
            ));
        }
    } else if let Some(gid) = clean_opt(&game.manifest_gid) {
        lines.push(format!("setManifestid({}, \"{}\")", game.appid, gid));
    }
    if let Some(ticket) = clean_opt(&game.app_ticket_hex) {
        lines.push(format!("setAppTicket({}, \"{}\")", game.appid, ticket));
    }
    if let Some(ticket) = clean_opt(&game.e_ticket_hex) {
        lines.push(format!("setETicket({}, \"{}\")", game.appid, ticket));
    }
    if let Some(steam_id) = clean_opt(&game.stat_steam_id) {
        lines.push(format!("setStat({}, \"{}\")", game.appid, steam_id));
    }
    lines.push(String::new());
    Ok(lines.join("\n"))
}

const MANIFEST_LUA_SENTINEL: &str = "-- GOST-MANIFEST-LUA v1";
const MANIFEST_DEX_PREFIX: &str = "https://manifest.manifestdex.com/";
const MANIFEST_DEX_UA: &str = "ManifestDeX/1.0";
/// Providers the bundled DLL knows by name. All three stopped returning codes
/// after Valve added the ownership check, but they are still probed so the
/// diagnostics page can show *why* a build fails.
const LEGACY_MANIFEST_PROVIDERS: [(&str, &str); 3] = [
    ("opensteamtool", "https://manifest.opensteamtool.com/"),
    ("steamrun", "https://manifest.steam.run/api/manifest/"),
    ("wudrm", "http://gmrc.wudrm.com/manifest/"),
];

#[derive(Debug, Clone, Serialize)]
pub struct ManifestLuaStatus {
    pub path: String,
    pub present: bool,
    pub managed: bool,
    pub bytes: u64,
    pub modified_time: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ManifestProbe {
    pub provider: String,
    pub url: String,
    pub status: Option<u16>,
    pub latency_ms: u64,
    pub body: String,
    pub error: Option<String>,
    pub code_ok: bool,
    /// `x-manifest-cdn` hint returned by ManifestDeX, used for the CDN probe.
    pub cdn_hint: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DownloadProbe {
    pub provider: String,
    pub code: String,
    pub cdn_host: String,
    /// Cache group reported by Steam for this host (hkg1 = 33, tyo3 = 32 …).
    pub cdn_cell: Option<u32>,
    /// Where the host came from: `steam-cell` (CellID → Steam WebAPI),
    /// `steam-log` (recent content_log.txt), `provider-hint`, `default`.
    pub cdn_source: String,
    pub url: String,
    pub status: Option<u16>,
    pub bytes: u64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ManifestChannelProbe {
    pub appid: u32,
    pub game_name: String,
    pub depot_id: u32,
    pub manifest_gid: String,
    pub cell_id: Option<u32>,
    /// Hosts Steam itself used most recently (content_log.txt), including the
    /// mainland partner CDNs that need a per-license token.
    pub steam_hosts: Vec<String>,
    pub providers: Vec<ManifestProbe>,
    pub download: Option<DownloadProbe>,
}

fn lua_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

/// The DLL parses `.lua` files **line by line**: each time the accumulated
/// buffer compiles, it is executed and the buffer is cleared
/// (`LuaConfig::ParseFile`, see `luaL_loadstring` loop upstream). A top-level
/// `local` dies with its own chunk, so the previously generated file — which
/// declared `local SOURCES / memo / parse_code / fetch` and then a global
/// `fetch_manifest_code_ex` referring to them — failed on every call with
/// `attempt to index a nil value (global 'memo')`. The DLL then fell back to
/// `[manifest] url` (dead provider), Valve answered `Access Denied`, and Steam
/// showed 「无互联网连接」.
///
/// Everything therefore lives inside the function body, or in a global table
/// (`GOST_MANIFEST_SOURCES`), which survives between chunks.
pub fn render_manifest_lua(settings: &ManagerSettings) -> String {
    let mut sources = String::new();
    let custom = settings.manifest_lua_endpoint.trim();
    if !custom.is_empty() {
        let prefix = if custom.ends_with('/') {
            custom.to_string()
        } else {
            format!("{custom}/")
        };
        sources.push_str(&format!(
            "    {{ prefix = \"{}\", ua = \"\" }},\n",
            lua_escape(&prefix)
        ));
    }
    sources.push_str(&format!(
        "    {{ prefix = \"{MANIFEST_DEX_PREFIX}\", ua = \"{MANIFEST_DEX_UA}\" }},\n"
    ));

    format!(
        r#"{MANIFEST_LUA_SENTINEL}
-- 由 G-OpenSteamTool 生成：manifest request code 多源回退。
-- 注意：OST 的 Lua 解析器逐行累积成块执行，顶层 local 会在块结束时销毁，
-- 所以本文件不写顶层 local —— 状态放在函数体内或全局表里。
-- 删除首行标记即视为用户自管，工具不再覆盖本文件。

GOST_MANIFEST_SOURCES = {{
{sources}}}

function fetch_manifest_code_ex(app_id, depot_id, gid)
    local gid_s = tostring(gid)
    local memo = GOST_MANIFEST_MEMO
    if type(memo) ~= "table" then
        memo = {{}}
        GOST_MANIFEST_MEMO = memo
    end
    if memo[gid_s] then return memo[gid_s] end
    local sources = GOST_MANIFEST_SOURCES
    for i = 1, #sources do
        local src = sources[i]
        local headers = nil
        if src.ua ~= "" then headers = {{ ["User-Agent"] = src.ua }} end
        local ok, body, status = pcall(function()
            return http_get(src.prefix .. gid_s, headers)
        end)
        if ok and status == 200 and type(body) == "string" then
            local code = string.match(body, "^%s*(%d+)%s*$")
            if not code then code = string.match(body, '"content"%s*:%s*"(%d+)"') end
            if code then
                memo[gid_s] = code
                return code
            end
        end
    end
    return nil
end

function fetch_manifest_code(gid)
    return fetch_manifest_code_ex(0, 0, gid)
end
"#
    )
}

pub fn manifest_lua_path(steam_dir: &Path) -> PathBuf {
    lua_dir(steam_dir).join("manifest.lua")
}

pub fn manifest_lua_status<P: AsRef<Path>>(steam_dir: P) -> Result<ManifestLuaStatus> {
    let steam_dir = validate_steam_dir(steam_dir)?;
    let path = manifest_lua_path(&steam_dir);
    let present = path.is_file();
    let (managed, bytes, modified_time) = if present {
        let text = fs::read_to_string(&path).unwrap_or_default();
        (
            text.contains(MANIFEST_LUA_SENTINEL),
            fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0),
            fs::metadata(&path)
                .and_then(|meta| meta.modified())
                .ok()
                .and_then(system_time_secs),
        )
    } else {
        (false, 0, None)
    };
    Ok(ManifestLuaStatus {
        path: display_path(&path),
        present,
        managed,
        bytes,
        modified_time,
    })
}

pub fn install_manifest_lua<P: AsRef<Path>>(
    steam_dir: P,
    settings: &ManagerSettings,
) -> Result<ManifestLuaStatus> {
    let steam_dir = validate_steam_dir(steam_dir)?;
    let path = manifest_lua_path(&steam_dir);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    safe_replace_file(&path, render_manifest_lua(settings).as_bytes())?;
    manifest_lua_status(&steam_dir)
}

pub fn remove_manifest_lua<P: AsRef<Path>>(steam_dir: P) -> Result<ManifestLuaStatus> {
    let steam_dir = validate_steam_dir(steam_dir)?;
    let path = manifest_lua_path(&steam_dir);
    if path.exists() {
        fs::remove_file(&path).map_err(|err| err.to_string())?;
    }
    manifest_lua_status(&steam_dir)
}

/// Keeps `config/lua/manifest.lua` in sync with the settings. Files without the
/// managed sentinel belong to the user and are never touched.
fn sync_manifest_lua(steam_dir: &Path, settings: &ManagerSettings) -> Result<()> {
    let path = manifest_lua_path(steam_dir);
    if settings.manage_manifest_lua {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        safe_replace_file(&path, render_manifest_lua(settings).as_bytes())
    } else if path.exists() {
        let text = fs::read_to_string(&path).unwrap_or_default();
        if text.contains(MANIFEST_LUA_SENTINEL) {
            fs::remove_file(&path).map_err(|err| err.to_string())?;
        }
        Ok(())
    } else {
        Ok(())
    }
}

fn provider_url(prefix: &str, gid: &str) -> String {
    format!("{prefix}{gid}")
}

fn probe_provider(client: &reqwest::blocking::Client, provider: &str, url: &str, ua: Option<&str>) -> ManifestProbe {
    let started = Instant::now();
    let mut request = client.get(url);
    if let Some(ua) = ua {
        request = request.header(reqwest::header::USER_AGENT, ua);
    }
    match request.send() {
        Ok(response) => {
            let status = response.status().as_u16();
            let cdn_hint = response
                .headers()
                .get("x-manifest-cdn")
                .and_then(|value| value.to_str().ok())
                .map(str::to_string);
            let body = response.text().unwrap_or_default();
            let trimmed = body.trim();
            let code_ok = !trimmed.is_empty()
                && trimmed.len() <= 20
                && trimmed.chars().all(|ch| ch.is_ascii_digit())
                && trimmed.parse::<u64>().map(|code| code > 0).unwrap_or(false);
            let head = if trimmed.len() > 64 {
                format!("{}…", &trimmed[..64])
            } else {
                trimmed.to_string()
            };
            ManifestProbe {
                provider: provider.to_string(),
                url: url.to_string(),
                status: Some(status),
                latency_ms: started.elapsed().as_millis() as u64,
                body: head,
                error: None,
                code_ok,
                cdn_hint,
            }
        }
        Err(err) => ManifestProbe {
            provider: provider.to_string(),
            url: url.to_string(),
            status: None,
            latency_ms: started.elapsed().as_millis() as u64,
            body: String::new(),
            error: Some(err.to_string()),
            code_ok: false,
            cdn_hint: None,
        },
    }
}

fn cdn_host_from_header(value: &str) -> Option<String> {
    let host = value
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()?
        .trim();
    if host.is_empty() {
        None
    } else {
        Some(host.to_string())
    }
}

/// Steam's download region, as stored in `config/config.vdf`
/// (`Software/Valve/Steam/CurrentCellID`).
fn read_cell_id(steam_dir: &Path) -> Option<u32> {
    let text = fs::read_to_string(steam_dir.join("config").join("config.vdf")).ok()?;
    let key = "\"CurrentCellID\"";
    let index = text.find(key)?;
    let rest = &text[index + key.len()..];
    rest.split('"')
        .find(|part| !part.trim().is_empty())?
        .trim()
        .parse()
        .ok()
}

fn parse_cdn_hosts(body: &str) -> Vec<(String, Option<u32>)> {
    let Ok(value) = serde_json::from_str::<Value>(body) else {
        return Vec::new();
    };
    let Some(servers) = value
        .get("response")
        .and_then(|item| item.get("servers"))
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };
    let mut hosts: Vec<(String, Option<u32>)> = Vec::new();
    for server in servers {
        if server.get("type").and_then(Value::as_str) != Some("SteamCache") {
            continue;
        }
        if server.get("https_support").and_then(Value::as_str) == Some("none") {
            continue;
        }
        let Some(host) = server.get("host").and_then(Value::as_str) else {
            continue;
        };
        if host.ends_with(".steamcontent.com") && !hosts.iter().any(|item| item.0 == host) {
            let cell = server
                .get("cell_id")
                .and_then(Value::as_u64)
                .and_then(|value| u32::try_from(value).ok());
            hosts.push((host.to_string(), cell));
        }
        if hosts.len() >= 8 {
            break;
        }
    }
    hosts
}

/// Same list Steam's `ContentServerDirectoryService` uses for this CellID.
/// The WebAPI form works without a key, so the self-test can follow the
/// configured download region instead of a hard-coded node.
fn fetch_cdn_hosts(client: &reqwest::blocking::Client, cell_id: u32) -> Vec<(String, Option<u32>)> {
    let url = format!(
        "https://api.steampowered.com/IContentServerDirectoryService/GetServersForSteamPipe/v1/?cell_id={cell_id}&max_servers=20&format=json"
    );
    match client.get(&url).send() {
        Ok(response) => response
            .text()
            .map(|body| parse_cdn_hosts(&body))
            .unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

/// Hosts the local client actually connected to (manifest, chunks, redirects).
/// Kept for display: mainland partner CDNs are token-gated, so the self-test
/// cannot request manifests from them directly.
fn recent_log_hosts(steam_dir: &Path) -> Vec<String> {
    let Ok(text) = fs::read_to_string(steam_dir.join("logs").join("content_log.txt")) else {
        return Vec::new();
    };
    let mut hosts: Vec<String> = Vec::new();
    for line in text.lines().rev().take(4000) {
        let Some(index) = line.find("host: ") else {
            continue;
        };
        let rest = &line[index + 6..];
        let end = rest
            .find(|ch: char| ch == ')' || ch == ' ' || ch == ',')
            .unwrap_or(rest.len());
        let host = rest[..end].trim();
        if host.is_empty() || hosts.iter().any(|item| item == host) {
            continue;
        }
        hosts.push(host.to_string());
        if hosts.len() >= 3 {
            break;
        }
    }
    hosts
}

fn cdn_host_from_log_line(line: &str) -> Option<String> {
    let index = line.find("host: ")?;
    let rest = &line[index + 6..];
    let end = rest
        .find(|ch: char| ch == ')' || ch == ' ' || ch == ',')
        .unwrap_or(rest.len());
    let host = rest[..end].trim();
    if host.starts_with("cache") && host.ends_with(".steamcontent.com") {
        Some(host.to_string())
    } else {
        None
    }
}

fn recent_log_cdn_host(steam_dir: &Path) -> Option<String> {
    let text = fs::read_to_string(steam_dir.join("logs").join("content_log.txt")).ok()?;
    let mut fallback = None;
    for line in text.lines().rev().take(8000) {
        let Some(host) = cdn_host_from_log_line(line) else {
            continue;
        };
        if line.contains("manifest request") {
            return Some(host);
        }
        if fallback.is_none() {
            fallback = Some(host);
        }
    }
    fallback
}

pub fn probe_manifest_channel<P: AsRef<Path>>(
    steam_dir: P,
    appid: Option<u32>,
) -> Result<ManifestChannelProbe> {
    let steam_dir = validate_steam_dir(steam_dir)?;
    let settings = load_settings_from_dir(&steam_dir)?;
    let games = list_games_from_dir(&steam_dir)?;
    let game = match appid {
        Some(id) => games.iter().find(|game| game.appid == id),
        None => games
            .iter()
            .find(|game| game.manifest_entries.iter().any(|entry| !entry.manifest_gid.is_empty())),
    }
    .ok_or_else(|| "没有可测试的 App：请先保存一个带 Manifest 条目的 G-<AppId>.lua".to_string())?;
    let entry = game
        .manifest_entries
        .iter()
        .find(|entry| !entry.manifest_gid.trim().is_empty())
        .ok_or_else(|| format!("G-{}.lua 没有 Manifest 条目", game.appid))?;

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(8))
        .build()
        .map_err(|err| err.to_string())?;

    let mut providers = Vec::new();
    let endpoint = settings.manifest_lua_endpoint.trim();
    if !endpoint.is_empty() {
        let prefix = if endpoint.ends_with('/') {
            endpoint.to_string()
        } else {
            format!("{endpoint}/")
        };
        let url = provider_url(&prefix, &entry.manifest_gid);
        providers.push(probe_provider(&client, "self", &url, None));
    }
    let dex_url = provider_url(MANIFEST_DEX_PREFIX, &entry.manifest_gid);
    providers.push(probe_provider(&client, "manifestdex", &dex_url, Some(MANIFEST_DEX_UA)));
    for (name, prefix) in LEGACY_MANIFEST_PROVIDERS {
        let url = provider_url(prefix, &entry.manifest_gid);
        providers.push(probe_provider(&client, name, &url, None));
    }

    // Prefer a freshly fetched code, then the built-in ManifestDeX endpoint.
    let chosen = providers
        .iter()
        .find(|probe| probe.code_ok && probe.provider == "self")
        .or_else(|| providers.iter().find(|probe| probe.code_ok && probe.provider == "manifestdex"))
        .cloned();

    let mut download = None;
    let cell_id = read_cell_id(&steam_dir);
    let steam_hosts = recent_log_hosts(&steam_dir);
    if let Some(probe) = chosen {
        let code = probe.body.trim().to_string();
        let mut candidates: Vec<(String, &'static str, Option<u32>)> = Vec::new();
        if let Some(cell) = cell_id {
            for (host, host_cell) in fetch_cdn_hosts(&client, cell) {
                candidates.push((host, "steam-cell", host_cell));
            }
        }
        if let Some(host) = recent_log_cdn_host(&steam_dir) {
            candidates.push((host, "steam-log", None));
        }
        if let Some(host) = probe.cdn_hint.as_deref().and_then(cdn_host_from_header) {
            candidates.push((host, "provider-hint", None));
        }
        candidates.push(("cache9-tyo3.steamcontent.com".to_string(), "default", None));

        let mut tried: Vec<String> = Vec::new();
        for (cdn_host, source, cdn_cell) in candidates {
            if tried.iter().any(|item| item == &cdn_host) {
                continue;
            }
            tried.push(cdn_host.clone());
            let url = format!(
                "https://{cdn_host}/depot/{}/manifest/{}/5/{code}",
                entry.depot_id, entry.manifest_gid
            );
            let request = client
                .get(&url)
                .header(reqwest::header::USER_AGENT, "Valve/Steam HTTP Client 1.0");
            let result = match request.send() {
                Ok(response) => {
                    let status = response.status().as_u16();
                    let bytes = response.bytes().map(|data| data.len() as u64).unwrap_or(0);
                    DownloadProbe {
                        provider: probe.provider.clone(),
                        code: code.clone(),
                        cdn_host: cdn_host.clone(),
                        cdn_cell,
                        cdn_source: source.to_string(),
                        url,
                        status: Some(status),
                        bytes,
                        error: None,
                    }
                }
                Err(err) => DownloadProbe {
                    provider: probe.provider.clone(),
                    code: code.clone(),
                    cdn_host: cdn_host.clone(),
                    cdn_cell,
                    cdn_source: source.to_string(),
                    url,
                    status: None,
                    bytes: 0,
                    error: Some(err.to_string()),
                },
            };
            let ok = result.status == Some(200);
            download = Some(result);
            // Steam normally answers from the first healthy cache; keep a few
            // fallbacks for stale nodes but do not walk the whole list.
            if ok || tried.len() >= 4 {
                break;
            }
        }
    }

    Ok(ManifestChannelProbe {
        appid: game.appid,
        game_name: game.name.clone(),
        depot_id: entry.depot_id,
        manifest_gid: entry.manifest_gid.clone(),
        cell_id,
        steam_hosts,
        providers,
        download,
    })
}

pub fn upsert_game_in_dir<P: AsRef<Path>>(steam_dir: P, game: &GameConfig) -> Result<()> {
    let steam_dir = validate_steam_dir(steam_dir)?;
    let lua_dir = lua_dir(&steam_dir);
    fs::create_dir_all(&lua_dir).map_err(|err| err.to_string())?;
    let path = lua_dir.join(game_file_name(game.appid, game.enabled));
    let counterpart = lua_dir.join(game_file_name(game.appid, !game.enabled));
    let content = render_game_lua(game)?;
    safe_replace_file(&path, content.as_bytes())?;
    // One appid must own exactly one file. Writing the enabled copy while a
    // stale `.lua.disabled` (or the reverse) survives leaves both on disk, which
    // the loader cannot disambiguate.
    if counterpart.exists() {
        fs::remove_file(&counterpart).map_err(|err| err.to_string())?;
    }
    Ok(())
}

pub fn import_lua_file_from_path<P: AsRef<Path>, Q: AsRef<Path>>(
    steam_dir: P,
    source_path: Q,
) -> Result<GameConfig> {
    let steam_dir = validate_steam_dir(steam_dir)?;
    let source_path = source_path.as_ref();
    let file_name = source_path
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or_else(|| "Invalid Lua file name".to_string())?;
    let text = fs::read_to_string(source_path).map_err(|err| err.to_string())?;
    let mut game =
        parse_game_lua(file_name, &text).ok_or_else(|| "无法从 Lua 文件识别 AppId".to_string())?;
    game.enabled = true;
    upsert_game_in_dir(&steam_dir, &game)?;
    Ok(game)
}

/// 单个文件导入失败的记录：该文件不合法时其余文件继续导入。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImportFailure {
    pub file_name: String,
    pub path: String,
    pub error: String,
}

/// 批量导入结果。`failed` 为空表示全部成功；两份都为空表示没有选中任何文件。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImportReport {
    pub imported: Vec<GameConfig>,
    pub failed: Vec<ImportFailure>,
}

impl ImportReport {
    pub fn is_empty(&self) -> bool {
        self.imported.is_empty() && self.failed.is_empty()
    }
}

/// 批量导入：逐个解析并写入 Lua 目录，单个文件出错只记录失败，不影响其余文件。
pub fn import_lua_files_from_paths<P: AsRef<Path>, Q: AsRef<Path>>(
    steam_dir: P,
    source_paths: &[Q],
) -> Result<ImportReport> {
    let steam_dir = validate_steam_dir(steam_dir)?;
    let mut imported = Vec::new();
    let mut failed = Vec::new();
    for source_path in source_paths {
        let source_path = source_path.as_ref();
        match import_lua_file_from_path(&steam_dir, source_path) {
            Ok(game) => imported.push(game),
            Err(error) => failed.push(ImportFailure {
                file_name: source_path
                    .file_name()
                    .and_then(OsStr::to_str)
                    .unwrap_or_default()
                    .to_string(),
                path: display_path(source_path),
                error,
            }),
        }
    }
    Ok(ImportReport { imported, failed })
}

pub fn open_lua_dir_for_steam<P: AsRef<Path>>(steam_dir: P) -> Result<()> {
    let steam_dir = validate_steam_dir(steam_dir)?;
    let lua_dir = lua_dir(&steam_dir);
    fs::create_dir_all(&lua_dir).map_err(|err| err.to_string())?;
    open_dir(&lua_dir)
}

pub fn list_games_from_dir<P: AsRef<Path>>(steam_dir: P) -> Result<Vec<GameConfig>> {
    let steam_dir = validate_steam_dir(steam_dir)?;
    let lua_dir = lua_dir(&steam_dir);
    if !lua_dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut games = Vec::new();
    for entry in fs::read_dir(lua_dir).map_err(|err| err.to_string())? {
        let entry = entry.map_err(|err| err.to_string())?;
        let path = entry.path();
        let Some(name) = path.file_name().and_then(OsStr::to_str).map(str::to_string) else {
            continue;
        };
        if !is_managed_lua_name(&name) {
            continue;
        }
        let text = fs::read_to_string(path).map_err(|err| err.to_string())?;
        if let Some(mut game) = parse_game_lua(&name, &text) {
            game.enabled = is_enabled_lua_name(&name);
            games.push(game);
        }
    }
    games.sort_by_key(|game| game.appid);
    Ok(games)
}

pub fn delete_game_from_dir<P: AsRef<Path>>(steam_dir: P, appid: u32) -> Result<()> {
    let steam_dir = validate_steam_dir(steam_dir)?;
    let lua_dir = lua_dir(&steam_dir);
    for enabled in [true, false] {
        let path = lua_dir.join(game_file_name(appid, enabled));
        if path.exists() {
            fs::remove_file(path).map_err(|err| err.to_string())?;
        }
    }
    Ok(())
}

pub fn set_game_enabled_in_dir<P: AsRef<Path>>(
    steam_dir: P,
    appid: u32,
    enabled: bool,
) -> Result<()> {
    let steam_dir = validate_steam_dir(steam_dir)?;
    let lua_dir = lua_dir(&steam_dir);
    let source = lua_dir.join(game_file_name(appid, !enabled));
    let target = lua_dir.join(game_file_name(appid, enabled));
    if !source.exists() {
        if target.exists() {
            // Already in the requested state; nothing to rename.
            return Ok(());
        }
        return Err(format!("源 Lua 文件不存在：{}", source.display()));
    }
    if target.exists() {
        // Both states on disk (left behind by an older build). Keep the one the
        // user asked for and drop the duplicate instead of failing forever.
        fs::remove_file(&source).map_err(|err| err.to_string())?;
        return Ok(());
    }
    fs::rename(source, target).map_err(|err| err.to_string())
}

pub fn scan_state_with_assets<P: AsRef<Path>, Q: AsRef<Path>>(
    steam_dir: P,
    assets_dir: Option<Q>,
) -> Result<ScanState> {
    let steam_dir = validate_steam_dir(&steam_dir)?;
    let asset_path = assets_dir.as_ref().map(|path| path.as_ref().to_path_buf());
    let missing_dll_resources = missing_resources(asset_path.as_deref());
    let module_scan = scan_steam_modules();
    let dlls = DLL_NAMES
        .iter()
        .map(|name| dll_status(&steam_dir, asset_path.as_deref(), name, &module_scan))
        .collect::<Result<Vec<_>>>()?;
    let log_files = list_log_names(&steam_dir);
    Ok(ScanState {
        steam_dir: display_path(&steam_dir),
        steam_valid: true,
        steam_running: module_scan.steam_running,
        steam_version: steam_version(&steam_dir),
        config_exists: steam_dir.join("opensteamtool.toml").exists(),
        lua_count: list_games_from_dir(&steam_dir)?.len(),
        dlls,
        dll_resources_ready: missing_dll_resources.is_empty(),
        missing_dll_resources,
        log_files,
    })
}

pub fn scan_state_with_bundled_assets<P: AsRef<Path>>(steam_dir: P) -> Result<ScanState> {
    let steam_dir = validate_steam_dir(&steam_dir)?;
    let module_scan = scan_steam_modules();
    let dlls = DLL_NAMES
        .iter()
        .map(|name| {
            dll_status_with_resource_hash(
                &steam_dir,
                name,
                Some(bytes_hash(bundled_dll(name))),
                &module_scan,
            )
        })
        .collect::<Result<Vec<_>>>()?;
    let log_files = list_log_names(&steam_dir);
    Ok(ScanState {
        steam_dir: display_path(&steam_dir),
        steam_valid: true,
        steam_running: module_scan.steam_running,
        steam_version: steam_version(&steam_dir),
        config_exists: steam_dir.join("opensteamtool.toml").exists(),
        lua_count: list_games_from_dir(&steam_dir)?.len(),
        dlls,
        dll_resources_ready: true,
        missing_dll_resources: Vec::new(),
        log_files,
    })
}

pub fn install_bundled_dlls<P: AsRef<Path>>(steam_dir: P) -> Result<()> {
    let steam_dir = validate_steam_dir(steam_dir)?;
    let mut manifest = InstallManifest {
        dlls: BTreeMap::new(),
    };
    for name in DLL_NAMES {
        let dst = steam_dir.join(name);
        safe_replace_file(&dst, bundled_dll(name))?;
        manifest.dlls.insert(name.to_string(), file_hash(&dst)?);
    }
    write_manifest(&steam_dir, &manifest)
}

pub fn install_dlls_from_dir<P: AsRef<Path>, Q: AsRef<Path>>(
    steam_dir: P,
    assets_dir: Q,
) -> Result<()> {
    let steam_dir = validate_steam_dir(steam_dir)?;
    let assets_dir = assets_dir.as_ref();
    let missing = missing_resources(Some(assets_dir));
    if !missing.is_empty() {
        return Err(format!(
            "Missing bundled DLL resources: {}",
            missing.join(", ")
        ));
    }

    let mut manifest = InstallManifest {
        dlls: BTreeMap::new(),
    };
    for name in DLL_NAMES {
        let src = assets_dir.join(name);
        let dst = steam_dir.join(name);
        fs::copy(&src, &dst).map_err(|err| err.to_string())?;
        manifest.dlls.insert(name.to_string(), file_hash(&dst)?);
    }
    write_manifest(&steam_dir, &manifest)
}

pub fn remove_dlls_from_dir<P: AsRef<Path>>(steam_dir: P) -> Result<()> {
    let steam_dir = validate_steam_dir(steam_dir)?;
    let manifest = read_manifest(&steam_dir)?;
    for (name, expected_hash) in manifest.dlls {
        let path = steam_dir.join(&name);
        if path.exists() && file_hash(&path).ok() == Some(expected_hash) {
            fs::remove_file(path).map_err(|err| err.to_string())?;
        }
    }
    let manifest_path = steam_dir.join(INSTALL_MANIFEST);
    if manifest_path.exists() {
        fs::remove_file(manifest_path).map_err(|err| err.to_string())?;
    }
    Ok(())
}

pub fn fetch_app_metadata(appid: u32) -> Result<AppMetadata> {
    if appid == 0 {
        return Err("AppId must be greater than zero".into());
    }
    let url = format!("https://store.steampowered.com/api/appdetails?appids={appid}&filters=basic");
    let value: Value = reqwest::blocking::get(url)
        .map_err(|err| err.to_string())?
        .json()
        .map_err(|err| err.to_string())?;
    let name = value
        .get(appid.to_string())
        .and_then(|v| v.get("data"))
        .and_then(|v| v.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    if name.is_empty() {
        return Err("Steam Store did not return an app name".into());
    }
    Ok(AppMetadata {
        appid,
        name,
        source: "store.steampowered.com".into(),
    })
}

pub fn discover_depots_from_steam<P: AsRef<Path>>(
    steam_dir: P,
    appid: u32,
) -> Result<DepotDiscovery> {
    if appid == 0 {
        return Err("AppId must be greater than zero".into());
    }
    let steam_dir = validate_steam_dir(steam_dir)?;
    let appinfo_path = steam_dir.join("appcache").join("appinfo.vdf");
    let appinfo_data = fs::read(&appinfo_path)
        .map_err(|err| format!("无法读取 {}: {err}", appinfo_path.display()))?;
    let appinfo = steam_vdf_parser::parse_appinfo(&appinfo_data)
        .map_err(|err| format!("无法解析 appinfo.vdf: {err}"))?;
    let app_key = appid.to_string();
    let app_object = appinfo
        .get(&app_key)
        .and_then(|value| value.as_obj())
        .ok_or_else(|| {
            format!("appinfo.vdf 中没有 AppId {appid}，请先在 Steam 中打开该游戏页面")
        })?;
    let (name, mut depots) = depot_metadata_from_appinfo(app_object);
    if depots.is_empty() {
        return Err(format!(
            "appinfo.vdf 中没有找到 AppId {appid} 的 Depot 信息"
        ));
    }

    let config_path = steam_dir.join("config").join("config.vdf");
    let config_text = fs::read_to_string(&config_path).ok();
    let config = config_text
        .as_deref()
        .and_then(|text| steam_vdf_parser::parse_text(text).ok());
    let config_depots = config
        .as_ref()
        .and_then(|document| document.get_obj(&["Software", "Valve", "Steam", "depots"]));

    for depot in &mut depots {
        depot.depot_key =
            config_depots.and_then(|items| depot_key_from_config(items, depot.depot_id));
    }

    // DLC content depots for DLC the account holds come from packageinfo.vdf;
    // the file is optional and its absence only costs that fallback.
    let package_path = steam_dir.join("appcache").join("packageinfo.vdf");
    let package_data = fs::read(&package_path).ok();
    let packages = package_data.as_deref().and_then(|data| {
        steam_vdf_parser::parse_packageinfo_with(data, steam_vdf_parser::ParseOptions::default())
            .ok()
    });
    // Some builds ship no `extended.listofdlc` (new or partially cached apps),
    // so enumeration also scans appinfo for entries whose parent is this App.
    let dlc_ids = appinfo
        .as_obj()
        .map(|root| dlc_appids_from_appinfo(root, appid, app_object))
        .unwrap_or_default();
    let package_depots = packages
        .as_ref()
        .and_then(|document| document.as_obj())
        .map(|root| dlc_depots_from_packages(root, &dlc_ids))
        .unwrap_or_default();

    let dlc = appinfo
        .as_obj()
        .map(|root| {
            dlc_metadata_from_appinfo(root, app_object, &dlc_ids, config_depots, &package_depots)
        })
        .unwrap_or_default();

    Ok(DepotDiscovery {
        appid,
        name: name.unwrap_or_else(|| format!("App {appid}")),
        depots,
        dlc,
        app_source: "appcache/appinfo.vdf".into(),
        config_found: config_depots.is_some(),
    })
}

/// Labels every AppId / DepotId of `appid` with the part of the game it belongs
/// to. A saved `G-<AppId>.lua` only stores flat `addappid` rows, so the editor
/// calls this to show 「主内容 / DLC」 next to each row without re-running the
/// whole discovery flow.
pub fn depot_ownership_from_steam<P: AsRef<Path>>(
    steam_dir: P,
    appid: u32,
) -> Result<Vec<DepotOwnership>> {
    let discovery = discover_depots_from_steam(steam_dir, appid)?;
    Ok(ownership_rows(&discovery))
}

fn ownership_rows(discovery: &DepotDiscovery) -> Vec<DepotOwnership> {
    // 同一个 Depot 可能同时出现在主内容与 DLC 列表里；DLC 更具体，覆盖主内容。
    let mut by_appid: BTreeMap<u32, DepotOwnership> = BTreeMap::new();
    let base = |appid: u32| DepotOwnership {
        appid,
        group: "base".into(),
        dlc_appid: None,
        dlc_name: None,
    };
    by_appid.insert(discovery.appid, base(discovery.appid));
    for depot in &discovery.depots {
        by_appid.insert(depot.depot_id, base(depot.depot_id));
    }
    for dlc in &discovery.dlc {
        by_appid.insert(
            dlc.appid,
            DepotOwnership {
                appid: dlc.appid,
                group: "dlc".into(),
                dlc_appid: Some(dlc.appid),
                dlc_name: dlc.name.clone(),
            },
        );
        for depot in &dlc.depots {
            by_appid.insert(
                depot.depot_id,
                DepotOwnership {
                    appid: depot.depot_id,
                    group: "dlc".into(),
                    dlc_appid: Some(dlc.appid),
                    dlc_name: dlc.name.clone(),
                },
            );
        }
    }
    by_appid.into_values().collect()
}

/// `appinfo.vdf` entries wrap their payload in an `appinfo` object; a few
/// appids (and the unit-test fixtures) store it inline.
fn appinfo_payload<'a, 'text>(app_object: &'a VdfObject<'text>) -> &'a VdfObject<'text> {
    app_object
        .get("appinfo")
        .and_then(|value| value.as_obj())
        .unwrap_or(app_object)
}

/// Reads a numeric field addressed by `path` from a parsed VDF object.
fn u32_at(obj: &VdfObject<'_>, path: &[&str]) -> Option<u32> {
    let (last, keys) = path.split_last()?;
    let mut current = obj;
    for key in keys {
        current = current.get(key).and_then(|value| value.as_obj())?;
    }
    current
        .get(last)
        .and_then(vdf_decimal_string)
        .and_then(|text| text.trim().parse::<u32>().ok())
}

/// DLC AppIds for an App.
///
/// `appinfo.extended.listofdlc` is the primary source, but plenty of Apps ship
/// without it (new releases, partially cached entries). Those still carry the
/// DLC entries themselves, tagged `common.parent` / `common.type = DLC` or
/// `extended.dlcforappid`, so the whole appinfo root is scanned as a fallback.
fn dlc_appids_from_appinfo(
    appinfo_root: &VdfObject<'_>,
    appid: u32,
    app_object: &VdfObject<'_>,
) -> Vec<u32> {
    let mut ids = appinfo_payload(app_object)
        .get("extended")
        .and_then(|value| value.as_obj())
        .and_then(|extended| extended.get("listofdlc"))
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .split(',')
        .filter_map(|item| item.trim().parse::<u32>().ok())
        .filter(|candidate| *candidate > 0)
        .collect::<Vec<_>>();

    for (key, value) in appinfo_root.iter() {
        let Some(candidate) = key.parse::<u32>().ok() else {
            continue;
        };
        if candidate == appid {
            continue;
        }
        let Some(entry) = value.as_obj() else {
            continue;
        };
        let payload = appinfo_payload(entry);
        let parent = u32_at(payload, &["common", "parent"]);
        let dlc_for = u32_at(payload, &["extended", "dlcforappid"]);
        if parent == Some(appid) || dlc_for == Some(appid) {
            ids.push(candidate);
        }
    }

    ids.sort_unstable();
    ids.dedup();
    ids
}

/// Depots declared on the base App but owned by a DLC (`dlcappid`). Some games
/// ship DLC content under the base App node instead of the DLC's own entry.
fn dlc_depots_from_base_app(app_object: &VdfObject<'_>) -> Vec<(u32, u32)> {
    let Some(depots) = appinfo_payload(app_object)
        .get("depots")
        .and_then(|value| value.as_obj())
    else {
        return Vec::new();
    };
    depots
        .iter()
        .filter_map(|(key, value)| {
            let depot_id = key.parse::<u32>().ok()?;
            let dlc_appid = value
                .as_obj()?
                .get("dlcappid")
                .and_then(vdf_decimal_string)
                .and_then(|text| text.trim().parse::<u32>().ok())?;
            Some((depot_id, dlc_appid))
        })
        .collect()
}

/// Enumerates every DLC of the base App, merges the depots declared by the
/// DLC's own `appinfo` entry, the `dlcappid` depots on the base App, and the
/// depots of owned packages from `packageinfo.vdf`, then attaches local
/// `config.vdf` keys. DLC whose content is already present locally
/// (`key_found`) are returned first.
fn dlc_metadata_from_appinfo(
    appinfo_root: &VdfObject<'_>,
    app_object: &VdfObject<'_>,
    dlc_ids: &[u32],
    config_depots: Option<&VdfObject<'_>>,
    package_depots: &BTreeMap<u32, Vec<u32>>,
) -> Vec<DiscoveredDlc> {
    let base_dlc_depots = dlc_depots_from_base_app(app_object);
    let mut items = dlc_ids
        .iter()
        .copied()
        .map(|dlc_appid| {
            let (name, mut depots) = match appinfo_root
                .get(&dlc_appid.to_string())
                .and_then(|value| value.as_obj())
            {
                Some(entry) => depot_metadata_from_appinfo(entry),
                None => (None, Vec::new()),
            };
            for (depot_id, owner) in &base_dlc_depots {
                if *owner == dlc_appid && !depots.iter().any(|depot| depot.depot_id == *depot_id) {
                    depots.push(DiscoveredDepot {
                        depot_id: *depot_id,
                        depot_key: None,
                        manifest_gid: None,
                    });
                }
            }
            // Owned DLC usually has no `appinfo` entry until Steam fetches it;
            // the package the account holds still lists its depots.
            if let Some(owned) = package_depots.get(&dlc_appid) {
                for depot_id in owned {
                    if !depots.iter().any(|depot| depot.depot_id == *depot_id) {
                        depots.push(DiscoveredDepot {
                            depot_id: *depot_id,
                            depot_key: None,
                            manifest_gid: None,
                        });
                    }
                }
            }
            depots.sort_by_key(|depot| depot.depot_id);
            for depot in &mut depots {
                depot.depot_key =
                    config_depots.and_then(|items| depot_key_from_config(items, depot.depot_id));
            }
            let key_found = depots.iter().any(|depot| depot.depot_key.is_some());
            DiscoveredDlc {
                appid: dlc_appid,
                name,
                depots,
                key_found,
            }
        })
        .collect::<Vec<_>>();
    items.sort_by(|a, b| b.key_found.cmp(&a.key_found).then(a.appid.cmp(&b.appid)));
    items
}

/// Depot ids of every owned package, keyed by the DLC AppId that package
/// grants. `packageinfo.vdf` stores `appids` / `depotids` as index → number
/// objects, and the payload sits under the `0` (or `appinfo`) child.
fn dlc_depots_from_packages(
    packages_root: &VdfObject<'_>,
    dlc_ids: &[u32],
) -> BTreeMap<u32, Vec<u32>> {
    let dlc_set = dlc_ids.iter().copied().collect::<BTreeSet<_>>();
    let mut owned: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();
    for (_, package) in packages_root.iter() {
        let Some(package) = package.as_obj() else {
            continue;
        };
        let payload = package
            .get("appinfo")
            .and_then(|value| value.as_obj())
            .or_else(|| package.get("0").and_then(|value| value.as_obj()))
            .unwrap_or(package);
        let (Some(appids), Some(depotids)) = (
            payload.get("appids").and_then(|value| value.as_obj()),
            payload.get("depotids").and_then(|value| value.as_obj()),
        ) else {
            continue;
        };
        let granted = numeric_values(appids)
            .into_iter()
            .filter(|appid| dlc_set.contains(appid))
            .collect::<Vec<_>>();
        if granted.is_empty() {
            continue;
        }
        let depots = numeric_values(depotids);
        for appid in granted {
            owned
                .entry(appid)
                .or_default()
                .extend(depots.iter().copied());
        }
    }
    owned
        .into_iter()
        .map(|(appid, depots)| (appid, depots.into_iter().collect()))
        .collect()
}

/// `packageinfo.vdf` numbers are `I32`/`U64`, but a text fixture or a future
/// build may store them as decimal strings.
fn numeric_values(obj: &VdfObject<'_>) -> Vec<u32> {
    obj.iter()
        .filter_map(|(_, value)| match value {
            VdfValue::I32(number) => u32::try_from(*number).ok(),
            VdfValue::U64(number) => u32::try_from(*number).ok(),
            VdfValue::Str(text) => text.trim().parse::<u32>().ok(),
            _ => None,
        })
        .collect()
}

fn depot_key_from_config(config_depots: &VdfObject<'_>, depot_id: u32) -> Option<String> {
    config_depots
        .get(&depot_id.to_string())
        .and_then(|value| value.as_obj())
        .and_then(|item| item.get("DecryptionKey"))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|key| key.len() == 64 && key.chars().all(|ch| ch.is_ascii_hexdigit()))
        .map(ToString::to_string)
}

fn depot_metadata_from_appinfo(
    app_object: &VdfObject<'_>,
) -> (Option<String>, Vec<DiscoveredDepot>) {
    let payload = appinfo_payload(app_object);
    let name = payload
        .get("common")
        .and_then(|value| value.as_obj())
        .and_then(|common| common.get("name"))
        .and_then(|value| value.as_str())
        .map(ToString::to_string);
    let Some(depot_object) = payload.get("depots").and_then(|value| value.as_obj()) else {
        return (name, Vec::new());
    };

    let mut depots = depot_object
        .iter()
        .filter_map(|(key, value)| {
            let depot_id = key.parse::<u32>().ok()?;
            let item = value.as_obj()?;
            // 带 `dlcappid` 的 Depot 属于某个 DLC：主内容列表不收它，
            // 由 DLC 分组单独收录，避免同一个 Depot 被同时标成主体和 DLC。
            let owned_by_dlc = item
                .get("dlcappid")
                .and_then(vdf_decimal_string)
                .and_then(|text| text.trim().parse::<u32>().ok())
                .filter(|appid| *appid > 0);
            if owned_by_dlc.is_some() {
                return None;
            }
            let manifest_gid = item
                .get("manifests")
                .and_then(|value| value.as_obj())
                .and_then(|manifests| manifests.get("public"))
                .and_then(|value| value.as_obj())
                .and_then(|public| public.get("gid"))
                .and_then(vdf_decimal_string);
            Some(DiscoveredDepot {
                depot_id,
                depot_key: None,
                manifest_gid,
            })
        })
        .collect::<Vec<_>>();
    depots.sort_by_key(|depot| depot.depot_id);
    (name, depots)
}

fn vdf_decimal_string(value: &steam_vdf_parser::value::Value<'_>) -> Option<String> {
    value
        .as_str()
        .map(ToString::to_string)
        .or_else(|| value.as_u64().map(|number| number.to_string()))
        .or_else(|| value.as_i32().map(|number| number.to_string()))
}

pub fn read_logs_from_dir<P: AsRef<Path>>(steam_dir: P) -> Result<Vec<LogFile>> {
    let steam_dir = validate_steam_dir(steam_dir)?;
    let log_dir = steam_dir.join("opensteamtool");
    if !log_dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut logs = Vec::new();
    for entry in fs::read_dir(log_dir).map_err(|err| err.to_string())? {
        let entry = entry.map_err(|err| err.to_string())?;
        let path = entry.path();
        if path.extension().and_then(OsStr::to_str) != Some("log") {
            continue;
        }
        let name = path
            .file_name()
            .and_then(OsStr::to_str)
            .unwrap_or("unknown.log")
            .to_string();
        let metadata = fs::metadata(&path).map_err(|err| err.to_string())?;
        let size_bytes = metadata.len();
        let modified_time = metadata.modified().ok().and_then(system_time_secs);
        let mut content = fs::read_to_string(&path).unwrap_or_else(|_| String::new());
        if content.len() > 80_000 {
            content = content.split_off(content.len() - 80_000);
        }
        let line_count = content.lines().count();
        logs.push(LogFile {
            name,
            content,
            size_bytes,
            modified_time,
            line_count,
        });
    }
    logs.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(logs)
}

fn system_time_secs(time: SystemTime) -> Option<u64> {
    time.duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
}

pub fn close_steam() -> Result<()> {
    shutdown_steam(None)?;
    Ok(())
}

pub fn restart_steam<P: AsRef<Path>>(steam_dir: P) -> Result<()> {
    let steam_dir = validate_steam_dir(steam_dir)?;
    shutdown_steam(Some(&steam_dir))?;
    hidden_command(steam_dir.join("steam.exe"))
        .current_dir(steam_dir)
        .spawn()
        .map_err(|err| err.to_string())?;
    Ok(())
}

fn read_u64(table: &toml::map::Map<String, toml::Value>, key: &str, default: u64) -> u64 {
    table
        .get(key)
        .and_then(|v| v.as_integer())
        .and_then(|v| u64::try_from(v).ok())
        .unwrap_or(default)
}

fn validate_settings(settings: &ManagerSettings) -> Result<()> {
    if !["trace", "debug", "info", "warn", "error"].contains(&settings.log_level.as_str()) {
        return Err("Invalid log level".into());
    }
    if !["opensteamtool", "steamrun", "wudrm"].contains(&settings.manifest_url.as_str()) {
        return Err("Invalid manifest source".into());
    }
    let endpoint = settings.manifest_lua_endpoint.trim();
    if !endpoint.is_empty() && !(endpoint.starts_with("http://") || endpoint.starts_with("https://")) {
        return Err("Manifest endpoint must start with http:// or https://".into());
    }
    Ok(())
}

fn validate_game(game: &GameConfig) -> Result<()> {
    if game.appid == 0 {
        return Err("AppId must be greater than zero".into());
    }
    if let Some(key) = clean_opt(&game.depot_key) {
        if key.len() != 64 || !key.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err("Depot key must be 64 hex characters".into());
        }
    }
    for entry in &game.appid_entries {
        if entry.appid == 0 {
            return Err("addappid entry AppId must be greater than zero".into());
        }
        if let Some(key) = clean_opt(&entry.depot_key) {
            if key.len() != 64 || !key.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err("Depot key must be 64 hex characters".into());
            }
        }
    }
    for entry in &game.manifest_entries {
        if entry.depot_id == 0 {
            return Err("Manifest Depot ID must be greater than zero".into());
        }
        if !entry.manifest_gid.chars().all(|c| c.is_ascii_digit()) {
            return Err("manifest gid must contain digits only".into());
        }
    }
    for (label, value) in [
        ("access token", &game.access_token),
        ("manifest gid", &game.manifest_gid),
        ("stat steam id", &game.stat_steam_id),
    ] {
        if let Some(value) = clean_opt(value) {
            if !value.chars().all(|c| c.is_ascii_digit()) {
                return Err(format!("{label} must contain digits only"));
            }
        }
    }
    for (label, value) in [
        ("AppTicket", &game.app_ticket_hex),
        ("ETicket", &game.e_ticket_hex),
    ] {
        if let Some(value) = clean_opt(value) {
            if !value.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(format!("{label} must be hex"));
            }
        }
    }
    Ok(())
}

fn normalized_appid_entries(game: &GameConfig) -> Vec<AppIdEntry> {
    if !game.appid_entries.is_empty() {
        return game.appid_entries.clone();
    }
    vec![AppIdEntry {
        appid: game.appid,
        unlock_flag: clean_opt(&game.depot_key).map(|_| 0),
        depot_key: clean_opt(&game.depot_key),
    }]
}

fn normalized_manifest_entries(game: &GameConfig) -> Vec<ManifestEntry> {
    game.manifest_entries
        .iter()
        .filter_map(|entry| {
            clean_opt(&Some(entry.manifest_gid.clone())).map(|manifest_gid| ManifestEntry {
                depot_id: entry.depot_id,
                manifest_gid,
            })
        })
        .collect()
}

fn parse_game_lua(file_name: &str, text: &str) -> Option<GameConfig> {
    if let Some(mut game) = text.lines().find_map(|line| {
        line.strip_prefix(META_PREFIX)
            .and_then(|json| serde_json::from_str::<GameConfig>(json).ok())
    }) {
        let manifest_entries = find_manifest_entries(text);
        if game.manifest_entries.is_empty() && !manifest_entries.is_empty() {
            game.manifest_entries = manifest_entries;
        }
        return Some(game);
    }
    parse_legacy_game_lua(file_name, text)
}

fn parse_legacy_game_lua(file_name: &str, text: &str) -> Option<GameConfig> {
    let entries = find_addappid_entries(text);
    let appid = appid_from_g_lua_name(file_name)
        .or_else(|| find_comment_value(text, "AppId").and_then(|value| value.parse().ok()))
        .or_else(|| entries.first().map(|entry| entry.appid))?;
    let name = find_comment_value(text, "备注")
        .or_else(|| find_comment_value(text, "Name"))
        .unwrap_or_else(|| format!("App {appid}"));
    let depot_key = entries
        .iter()
        .find(|entry| entry.appid == appid)
        .and_then(|entry| entry.depot_key.clone());

    let manifest_entries = find_manifest_entries(text);
    let manifest_gid = manifest_entries
        .first()
        .map(|entry| entry.manifest_gid.clone())
        .or_else(|| find_first_quoted_arg(text, "setManifestid("));

    Some(GameConfig {
        appid,
        name,
        enabled: true,
        depot_key,
        access_token: find_first_quoted_arg(text, "addtoken("),
        manifest_gid,
        app_ticket_hex: find_first_quoted_arg(text, "setAppTicket("),
        e_ticket_hex: find_first_quoted_arg(text, "setETicket("),
        stat_steam_id: find_first_quoted_arg(text, "setStat("),
        appid_entries: entries,
        manifest_entries,
    })
}

fn appid_from_g_lua_name(file_name: &str) -> Option<u32> {
    file_name
        .strip_prefix("G-")
        .and_then(|name| {
            name.strip_suffix(".lua")
                .or_else(|| name.strip_suffix(".lua.disabled"))
        })
        .and_then(|appid| appid.parse().ok())
}

fn find_comment_value(text: &str, key: &str) -> Option<String> {
    let prefix = format!("-- {key}:");
    text.lines()
        .find_map(|line| line.trim().strip_prefix(&prefix).map(str::trim))
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn find_addappid_entries(text: &str) -> Vec<AppIdEntry> {
    let mut entries = Vec::new();
    for args in find_call_args(text, "addappid(") {
        let parts = split_lua_args(&args);
        let Some(appid) = parts.first().and_then(|value| value.trim().parse().ok()) else {
            continue;
        };
        entries.push(AppIdEntry {
            appid,
            unlock_flag: parts.get(1).and_then(|value| value.trim().parse().ok()),
            depot_key: parts
                .get(2)
                .map(|value| value.trim())
                .and_then(unquote_lua_string),
        });
    }
    entries
}

fn find_manifest_entries(text: &str) -> Vec<ManifestEntry> {
    let mut entries = Vec::new();
    for args in find_call_args(text, "setManifestid(") {
        let parts = split_lua_args(&args);
        let Some(depot_id) = parts.first().and_then(|value| value.trim().parse().ok()) else {
            continue;
        };
        let Some(manifest_gid) = parts
            .get(1)
            .map(|value| value.trim())
            .and_then(unquote_lua_string)
        else {
            continue;
        };
        entries.push(ManifestEntry {
            depot_id,
            manifest_gid,
        });
    }
    entries
}

fn find_call_args(text: &str, marker: &str) -> Vec<String> {
    text.split(marker)
        .skip(1)
        .filter_map(|segment| segment.split_once(')').map(|(args, _)| args.to_string()))
        .collect()
}

fn split_lua_args(args: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut escaped = false;
    for character in args.chars() {
        if escaped {
            current.push(character);
            escaped = false;
            continue;
        }
        if character == '\\' && in_string {
            current.push(character);
            escaped = true;
            continue;
        }
        if character == '"' {
            in_string = !in_string;
            current.push(character);
            continue;
        }
        if character == ',' && !in_string {
            result.push(current.trim().to_string());
            current.clear();
            continue;
        }
        current.push(character);
    }
    if !current.trim().is_empty() {
        result.push(current.trim().to_string());
    }
    result
}

fn unquote_lua_string(value: &str) -> Option<String> {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn find_first_quoted_arg(text: &str, marker: &str) -> Option<String> {
    for segment in text.split(marker).skip(1) {
        let mut quoted = segment.split('"').skip(1);
        if let Some(value) = quoted
            .next()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Some(value.to_string());
        }
    }
    None
}

fn clean_opt(value: &Option<String>) -> Option<String> {
    value
        .as_ref()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn lua_dir(steam_dir: &Path) -> PathBuf {
    steam_dir.join("config").join("lua")
}

fn game_file_name(appid: u32, enabled: bool) -> String {
    if enabled {
        format!("G-{appid}.lua")
    } else {
        format!("G-{appid}.lua.disabled")
    }
}

fn is_enabled_lua_name(name: &str) -> bool {
    name.starts_with("G-") && name.ends_with(".lua")
}

fn is_disabled_lua_name(name: &str) -> bool {
    name.starts_with("G-") && name.ends_with(".lua.disabled")
}

fn is_managed_lua_name(name: &str) -> bool {
    is_enabled_lua_name(name) || is_disabled_lua_name(name)
}

fn missing_resources(assets_dir: Option<&Path>) -> Vec<String> {
    DLL_NAMES
        .iter()
        .filter(|name| {
            assets_dir
                .map(|dir| !dir.join(name).is_file())
                .unwrap_or(true)
        })
        .map(|name| (*name).to_string())
        .collect()
}

fn dll_status(
    steam_dir: &Path,
    assets_dir: Option<&Path>,
    name: &str,
    module_scan: &SteamModuleScan,
) -> Result<DllStatus> {
    let resource_hash = assets_dir
        .map(|asset_dir| asset_dir.join(name))
        .filter(|asset| asset.exists())
        .and_then(|asset| file_hash_hex(&asset).ok());
    dll_status_with_resource_hash(steam_dir, name, resource_hash, module_scan)
}

fn dll_status_with_resource_hash(
    steam_dir: &Path,
    name: &str,
    resource_hash: Option<String>,
    module_scan: &SteamModuleScan,
) -> Result<DllStatus> {
    let target = steam_dir.join(name);
    let target_hash = target
        .exists()
        .then(|| file_hash_hex(&target))
        .transpose()?;
    let hash_matched = resource_hash.is_some() && resource_hash == target_hash;
    let state = if !target.exists() {
        DllState::Missing
    } else if hash_matched {
        DllState::Managed
    } else {
        DllState::Foreign
    };
    let load_state = dll_load_state_from_modules(
        steam_dir,
        name,
        module_scan.steam_running,
        module_scan.verify_failed,
        &module_scan.modules,
    );
    Ok(DllStatus {
        name: name.to_string(),
        state,
        target_path: display_path(target),
        resource_hash,
        target_hash,
        hash_matched,
        loaded_by_steam: load_state == DllLoadState::Loaded,
        load_state,
    })
}

pub fn dll_load_state_from_modules(
    steam_dir: &Path,
    dll_name: &str,
    steam_running: bool,
    verify_failed: bool,
    modules: &[(String, PathBuf)],
) -> DllLoadState {
    if !steam_running {
        return DllLoadState::SteamNotRunning;
    }
    if verify_failed {
        return DllLoadState::VerifyFailed;
    }
    let loaded = modules.iter().any(|(module_name, module_path)| {
        module_name.eq_ignore_ascii_case(dll_name) && path_is_inside_dir(module_path, steam_dir)
    });
    if loaded {
        DllLoadState::Loaded
    } else {
        DllLoadState::NotLoaded
    }
}

fn path_is_inside_dir(path: &Path, dir: &Path) -> bool {
    let base = comparable_path(dir);
    let candidate = comparable_path(path);
    candidate == base || candidate.starts_with(&format!("{base}\\"))
}

fn comparable_path(path: &Path) -> String {
    display_path(path)
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_ascii_lowercase()
}

fn file_hash(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path).map_err(|err| err.to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let read = file.read(&mut buffer).map_err(|err| err.to_string())?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn file_hash_hex(path: &Path) -> Result<String> {
    file_hash(path)
}

fn bundled_dll(name: &str) -> &'static [u8] {
    match name {
        "OpenSteamTool.dll" => OPENSTEAMTOOL_DLL,
        "dwmapi.dll" => DWMAPI_DLL,
        "xinput1_4.dll" => XINPUT1_4_DLL,
        _ => unreachable!("unknown bundled DLL: {name}"),
    }
}

fn bytes_hash(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn write_manifest(steam_dir: &Path, manifest: &InstallManifest) -> Result<()> {
    let text = serde_json::to_string_pretty(manifest).map_err(|err| err.to_string())?;
    safe_replace_file(&steam_dir.join(INSTALL_MANIFEST), text.as_bytes())
}

/// Replaces a managed text file with a staged write and rollback path.
///
/// Windows cannot rename a file over an existing destination reliably, so the
/// previous file is moved aside first and restored if committing the staged
/// file fails. Successful writes leave no temporary or backup artifact.
fn safe_replace_file(path: &Path, contents: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("Invalid managed file path: {}", path.display()))?;
    fs::create_dir_all(parent).map_err(|err| err.to_string())?;

    let name = path
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or_else(|| format!("Invalid managed file name: {}", path.display()))?;
    let staged = parent.join(format!(".{name}.gost.tmp"));
    let backup = parent.join(format!(".{name}.gost.bak"));

    if !path.exists() && backup.exists() {
        fs::rename(&backup, path)
            .map_err(|err| format!("Failed to recover previous {}: {err}", path.display()))?;
    }
    if staged.exists() {
        fs::remove_file(&staged).map_err(|err| err.to_string())?;
    }
    if backup.exists() {
        fs::remove_file(&backup).map_err(|err| err.to_string())?;
    }

    let mut file = fs::File::create(&staged).map_err(|err| err.to_string())?;
    file.write_all(contents).map_err(|err| err.to_string())?;
    file.sync_all().map_err(|err| err.to_string())?;
    drop(file);

    let had_previous = path.exists();
    if had_previous {
        fs::rename(path, &backup).map_err(|err| err.to_string())?;
    }

    if let Err(err) = fs::rename(&staged, path) {
        if had_previous && backup.exists() {
            let _ = fs::rename(&backup, path);
        }
        let _ = fs::remove_file(&staged);
        return Err(format!("Failed to commit {}: {err}", path.display()));
    }

    if backup.exists() {
        fs::remove_file(backup).map_err(|err| err.to_string())?;
    }
    Ok(())
}

fn read_manifest(steam_dir: &Path) -> Result<InstallManifest> {
    let path = steam_dir.join(INSTALL_MANIFEST);
    if !path.exists() {
        return Ok(InstallManifest {
            dlls: BTreeMap::new(),
        });
    }
    let text = fs::read_to_string(path).map_err(|err| err.to_string())?;
    serde_json::from_str(&text).map_err(|err| err.to_string())
}

fn list_log_names(steam_dir: &Path) -> Vec<String> {
    read_logs_from_dir(steam_dir)
        .map(|logs| logs.into_iter().map(|log| log.name).collect())
        .unwrap_or_default()
}

pub fn steam_shutdown_command_specs(steam_dir: Option<&Path>) -> Vec<CommandSpec> {
    let mut commands = Vec::new();
    if let Some(steam_dir) = steam_dir {
        commands.push(CommandSpec {
            program: display_path(steam_dir.join("steam.exe")),
            args: vec!["-shutdown".into()],
        });
    } else {
        commands.push(CommandSpec {
            program: "cmd".into(),
            args: vec![
                "/C".into(),
                "start".into(),
                "".into(),
                "steam://exit".into(),
            ],
        });
    }
    commands.push(force_steam_shutdown_command());
    commands
}

pub fn parse_steam_file_version_output(output: &str) -> Option<String> {
    output
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(ToString::to_string)
}

fn shutdown_steam(steam_dir: Option<&Path>) -> Result<()> {
    // Running `steam.exe -shutdown` while Steam is stopped can launch Steam just
    // to process the shutdown request. Skip the command sequence entirely when
    // no Steam process exists; restart can then launch it exactly once.
    if !is_steam_running() {
        return Ok(());
    }

    let commands = steam_shutdown_command_specs(steam_dir);
    if let Some(graceful) = commands.first() {
        let _ = run_command_spec(graceful);
    }
    wait_for_steam_exit(Duration::from_secs(4));
    if is_steam_running() {
        run_command_spec(&commands[1])?;
        wait_for_steam_exit(Duration::from_secs(4));
    }
    if is_steam_running() {
        return Err("Steam is still running after shutdown request".into());
    }
    Ok(())
}

fn wait_for_steam_exit(timeout: Duration) {
    let started = std::time::Instant::now();
    while started.elapsed() < timeout {
        if !is_steam_running() {
            break;
        }
        thread::sleep(Duration::from_millis(250));
    }
}

fn force_steam_shutdown_command() -> CommandSpec {
    CommandSpec {
        program: "taskkill".into(),
        args: vec!["/IM".into(), "steam.exe".into(), "/T".into(), "/F".into()],
    }
}

fn run_command_spec(spec: &CommandSpec) -> Result<()> {
    let status = hidden_command(&spec.program)
        .args(&spec.args)
        .status()
        .map_err(|err| err.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "Command failed: {} {}",
            spec.program,
            spec.args.join(" ")
        ))
    }
}

fn open_dir(path: &Path) -> Result<()> {
    #[cfg(windows)]
    {
        hidden_command("explorer")
            .arg(display_path(path))
            .spawn()
            .map_err(|err| err.to_string())?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        hidden_command("xdg-open")
            .arg(path)
            .spawn()
            .map_err(|err| err.to_string())?;
        Ok(())
    }
}

fn hidden_command<S: AsRef<OsStr>>(program: S) -> Command {
    let mut command = Command::new(program);
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

fn is_steam_running() -> bool {
    hidden_command("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "if (Get-Process -Name steam -ErrorAction SilentlyContinue) { exit 0 } else { exit 1 }",
        ])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn scan_steam_modules() -> SteamModuleScan {
    #[cfg(windows)]
    {
        scan_steam_modules_windows()
    }
    #[cfg(not(windows))]
    {
        SteamModuleScan {
            steam_running: false,
            verify_failed: false,
            modules: Vec::new(),
        }
    }
}

#[cfg(windows)]
fn scan_steam_modules_windows() -> SteamModuleScan {
    let mut scan = SteamModuleScan {
        steam_running: false,
        verify_failed: false,
        modules: Vec::new(),
    };

    let process_snapshot = match unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) } {
        Ok(handle) => handle,
        Err(_) => {
            scan.steam_running = is_steam_running();
            scan.verify_failed = scan.steam_running;
            return scan;
        }
    };

    let mut process = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };

    let mut has_process = unsafe { Process32FirstW(process_snapshot, &mut process).is_ok() };
    while has_process {
        let exe_name = wide_to_string(&process.szExeFile);
        if exe_name.eq_ignore_ascii_case("steam.exe") {
            scan.steam_running = true;
            match process_modules(process.th32ProcessID) {
                Ok(modules) => scan.modules.extend(modules),
                Err(_) => scan.verify_failed = true,
            }
        }
        has_process = unsafe { Process32NextW(process_snapshot, &mut process).is_ok() };
    }

    let _ = unsafe { CloseHandle(process_snapshot) };
    scan
}

#[cfg(windows)]
fn process_modules(process_id: u32) -> Result<Vec<(String, PathBuf)>> {
    let snapshot =
        unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, process_id) }
            .map_err(|err| err.to_string())?;

    let mut modules = Vec::new();
    let mut module = MODULEENTRY32W {
        dwSize: std::mem::size_of::<MODULEENTRY32W>() as u32,
        ..Default::default()
    };

    let mut has_module = unsafe { Module32FirstW(snapshot, &mut module).is_ok() };
    while has_module {
        modules.push((
            wide_to_string(&module.szModule),
            PathBuf::from(wide_to_string(&module.szExePath)),
        ));
        has_module = unsafe { Module32NextW(snapshot, &mut module).is_ok() };
    }

    let _ = unsafe { CloseHandle(snapshot) };
    Ok(modules)
}

#[cfg(windows)]
fn wide_to_string(value: &[u16]) -> String {
    let len = value
        .iter()
        .position(|character| *character == 0)
        .unwrap_or(value.len());
    String::from_utf16_lossy(&value[..len])
}

fn steam_version(steam_dir: &Path) -> Option<String> {
    steam_manifest_version(steam_dir)
        .or_else(|| steam_exe_file_version(&steam_dir.join("steam.exe")))
        .or_else(|| {
            let path = steam_dir.join("steam.cfg");
            fs::read_to_string(path).ok().and_then(|text| {
                text.lines()
                    .find(|line| line.to_ascii_lowercase().contains("version"))
                    .map(|line| line.trim().to_string())
            })
        })
}

fn steam_manifest_version(steam_dir: &Path) -> Option<String> {
    [
        steam_dir
            .join("package")
            .join("steam_client_win64.manifest"),
        steam_dir
            .join("package")
            .join("steam_client_win32.manifest"),
    ]
    .into_iter()
    .find_map(|path| {
        fs::read_to_string(path)
            .ok()
            .and_then(|text| parse_steam_manifest_version(&text))
    })
}

pub fn parse_steam_manifest_version(text: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let trimmed = line.trim();
        if !trimmed.starts_with("\"version\"") {
            return None;
        }
        trimmed
            .split('"')
            .nth(3)
            .map(str::trim)
            .filter(|version| !version.is_empty() && version.chars().all(|c| c.is_ascii_digit()))
            .map(ToString::to_string)
    })
}

fn steam_exe_file_version(path: &Path) -> Option<String> {
    let literal_path = escape_powershell_single_quoted(&display_path(path));
    let script = format!(
        "(Get-Item -LiteralPath '{}').VersionInfo.FileVersion",
        literal_path
    );
    hidden_command("powershell")
        .args(["-NoProfile", "-Command", &script])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .and_then(|version| parse_steam_file_version_output(&version))
}

fn escape_powershell_single_quoted(value: &str) -> String {
    value.replace('\'', "''")
}

fn escape_toml(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn dedupe_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut result = Vec::new();
    for path in paths {
        if !result.iter().any(|existing: &PathBuf| existing == &path) {
            result.push(path);
        }
    }
    result
}

#[cfg(windows)]
fn steam_registry_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Ok(key) = RegKey::predef(HKEY_CURRENT_USER).open_subkey("Software\\Valve\\Steam") {
        if let Ok(path) = key.get_value::<String, _>("SteamPath") {
            candidates.push(PathBuf::from(normalize_steam_registry_path(path)));
        }
        if let Ok(exe) = key.get_value::<String, _>("SteamExe") {
            let exe = PathBuf::from(normalize_steam_registry_path(exe));
            if let Some(parent) = exe.parent() {
                candidates.push(parent.to_path_buf());
            }
        }
    }

    for root in [
        "SOFTWARE\\WOW6432Node\\Valve\\Steam",
        "SOFTWARE\\Valve\\Steam",
    ] {
        if let Ok(key) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(root) {
            if let Ok(path) = key.get_value::<String, _>("InstallPath") {
                candidates.push(PathBuf::from(normalize_steam_registry_path(path)));
            }
        }
    }

    candidates
}

#[cfg(not(windows))]
fn steam_registry_candidates() -> Vec<PathBuf> {
    Vec::new()
}

fn normalize_steam_registry_path(path: String) -> String {
    path.replace('/', "\\")
}

#[cfg(test)]
mod depot_discovery_tests {
    use super::*;

    #[test]
    fn appinfo_metadata_extracts_numeric_depots_and_public_manifests() {
        let document = steam_vdf_parser::parse_text(
            r#""123"
            {
                "appinfo"
                {
                    "common" { "name" "Fixture Game" }
                    "depots"
                    {
                        "124" { "manifests" { "public" { "gid" "987654321" } } }
                        "125" { "config" { "oslist" "windows" } }
                        "126" { "dlcappid" "200" }
                        "branches" { "public" { "buildid" "1" } }
                    }
                }
            }"#,
        )
        .unwrap();
        let (name, depots) = depot_metadata_from_appinfo(document.as_obj().unwrap());

        assert_eq!(name.as_deref(), Some("Fixture Game"));
        assert_eq!(depots.len(), 2);
        assert_eq!(depots[0].depot_id, 124);
        assert_eq!(depots[0].manifest_gid.as_deref(), Some("987654321"));
        assert_eq!(depots[1].depot_id, 125);
        assert_eq!(depots[1].manifest_gid, None);
        // 126 带 dlcappid，属于 DLC，不能混进主内容
        assert!(depots.iter().all(|depot| depot.depot_id != 126));
    }

    #[test]
    fn dlc_metadata_lists_every_dlc_and_merges_local_keys() {
        let document = steam_vdf_parser::parse_text(
            r#""apps"
            {
                "100"
                {
                    "appinfo"
                    {
                        "common" { "name" "Base Game" }
                        "extended" { "listofdlc" "200, 201" }
                        "depots"
                        {
                            "101" { "manifests" { "public" { "gid" "111" } } }
                            "102" { "dlcappid" "200" }
                        }
                    }
                }
                "200"
                {
                    "appinfo"
                    {
                        "common" { "name" "DLC One" }
                        "depots" { "201" { "manifests" { "public" { "gid" "222" } } } }
                    }
                }
            }"#,
        )
        .unwrap();
        let root = document.as_obj().unwrap();
        let base = root.get("100").and_then(|value| value.as_obj()).unwrap();
        let local_key = "b".repeat(64);
        let config_text = format!(
            r#""depots" {{ "201" {{ "DecryptionKey" "{local_key}" }} }}"#
        );
        let config = steam_vdf_parser::parse_text(&config_text).unwrap();
        let config_depots = config.as_obj().unwrap();

        // packageinfo.vdf 里 201 这个 DLC 所在的包带着 depot 301。
        let packages = steam_vdf_parser::parse_text(
            r#""packages"
            {
                "77"
                {
                    "0"
                    {
                        "appids" { "0" "201" }
                        "depotids" { "0" "301" "1" "302" }
                    }
                }
                "78"
                {
                    "0"
                    {
                        "appids" { "0" "999" }
                        "depotids" { "0" "401" }
                    }
                }
            }"#,
        )
        .unwrap();
        let package_depots = dlc_depots_from_packages(packages.as_obj().unwrap(), &[200, 201]);
        assert_eq!(package_depots.get(&201), Some(&vec![301, 302]));
        assert_eq!(package_depots.get(&200), None);

        let dlc_ids = dlc_appids_from_appinfo(root, 100, base);
        assert_eq!(dlc_ids, vec![200, 201]);
        let dlc = dlc_metadata_from_appinfo(root, base, &dlc_ids, Some(config_depots), &package_depots);

        assert_eq!(dlc.len(), 2);
        // DLC with a local key sorts first.
        assert_eq!(dlc[0].appid, 200);
        assert_eq!(dlc[0].name.as_deref(), Some("DLC One"));
        assert!(dlc[0].key_found);
        // Depot 102 comes from the base App's `dlcappid`, 201 from the DLC entry.
        assert_eq!(
            dlc[0].depots.iter().map(|depot| depot.depot_id).collect::<Vec<_>>(),
            vec![102, 201]
        );
        assert_eq!(dlc[0].depots[1].depot_key.as_deref(), Some(local_key.as_str()));
        assert_eq!(dlc[0].depots[1].manifest_gid.as_deref(), Some("222"));
        // DLC 201 has no appinfo entry, so its depots come from packageinfo only.
        assert_eq!(dlc[1].appid, 201);
        assert_eq!(
            dlc[1].depots.iter().map(|depot| depot.depot_id).collect::<Vec<_>>(),
            vec![301, 302]
        );
        assert!(!dlc[1].key_found);
    }

    #[test]
    fn dlc_appids_falls_back_to_child_entries_when_listofdlc_is_missing() {
        // Escape Simulator 2 / 2879840 的本地 appinfo 就没有 listofdlc，
        // DLC 只能靠子条目的 common.parent 与 extended.dlcforappid 找到。
        let document = steam_vdf_parser::parse_text(
            r#""apps"
            {
                "2879840"
                {
                    "appinfo"
                    {
                        "common" { "name" "Escape Simulator 2" }
                        "depots" { "2879841" { } }
                    }
                }
                "4575720"
                {
                    "appinfo"
                    {
                        "common"
                        {
                            "name" "Escape Simulator 2: Apocalypse DLC"
                            "type" "DLC"
                            "parent" "2879840"
                        }
                    }
                }
                "4575721"
                {
                    "appinfo"
                    {
                        "common" { "name" "Escape Simulator 2 Soundtrack" }
                        "extended" { "dlcforappid" "2879840" }
                    }
                }
                "999"
                {
                    "appinfo"
                    {
                        "common" { "name" "Unrelated" "type" "DLC" "parent" "1234" }
                    }
                }
            }"#,
        )
        .unwrap();
        let root = document.as_obj().unwrap();
        let base = root.get("2879840").and_then(|value| value.as_obj()).unwrap();

        let ids = dlc_appids_from_appinfo(root, 2879840, base);
        assert_eq!(ids, vec![4575720, 4575721]);

        let dlc = dlc_metadata_from_appinfo(root, base, &ids, None, &BTreeMap::new());
        assert_eq!(dlc.len(), 2);
        let apocalypse = dlc
            .iter()
            .find(|item| item.appid == 4575720)
            .expect("apocalypse dlc");
        assert_eq!(
            apocalypse.name.as_deref(),
            Some("Escape Simulator 2: Apocalypse DLC")
        );
        assert!(!apocalypse.key_found);
    }

    #[test]
    fn ownership_rows_split_base_and_dlc() {
        let discovery = DepotDiscovery {
            appid: 100,
            name: "Fixture".into(),
            depots: vec![
                DiscoveredDepot {
                    depot_id: 101,
                    depot_key: None,
                    manifest_gid: None,
                },
                DiscoveredDepot {
                    depot_id: 102,
                    depot_key: Some("a".repeat(64)),
                    manifest_gid: None,
                },
            ],
            dlc: vec![DiscoveredDlc {
                appid: 200,
                name: Some("DLC One".into()),
                depots: vec![
                    DiscoveredDepot {
                        depot_id: 101,
                        depot_key: None,
                        manifest_gid: None,
                    },
                    DiscoveredDepot {
                        depot_id: 201,
                        depot_key: None,
                        manifest_gid: None,
                    },
                ],
                key_found: false,
            }],
            app_source: "test".into(),
            config_found: true,
        };

        let rows = ownership_rows(&discovery);
        let base = rows
            .iter()
            .filter(|row| row.group == "base")
            .map(|row| row.appid)
            .collect::<Vec<_>>();
        let dlc = rows
            .iter()
            .filter(|row| row.group == "dlc")
            .map(|row| row.appid)
            .collect::<Vec<_>>();
        // 101 同时出现在主内容与 DLC：DLC 覆盖主内容
        assert_eq!(base, vec![100, 102]);
        assert_eq!(dlc, vec![101, 200, 201]);
        assert!(rows
            .iter()
            .filter(|row| row.group == "dlc")
            .all(|row| row.dlc_appid == Some(200)
                && row.dlc_name.as_deref() == Some("DLC One")));
    }

    #[test]
    fn config_metadata_accepts_only_valid_decryption_keys() {
        let valid_key = "a".repeat(64);
        let text = format!(
            r#""depots"
            {{
                "124" {{ "DecryptionKey" "{valid_key}" }}
                "125" {{ "DecryptionKey" "not-a-key" }}
            }}"#
        );
        let document = steam_vdf_parser::parse_text(&text).unwrap();
        let depots = document.as_obj().unwrap();

        assert_eq!(
            depot_key_from_config(depots, 124).as_deref(),
            Some(valid_key.as_str())
        );
        assert_eq!(depot_key_from_config(depots, 125), None);
    }
}

#[cfg(test)]
mod manifest_lua_tests {
    use super::*;

    fn fixture_steam_dir() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("steam.exe"), b"stub").unwrap();
        dir
    }

    #[test]
    fn render_includes_manifestdex_and_custom_endpoint() {
        let mut settings = ManagerSettings::default();
        settings.manifest_lua_endpoint = "http://127.0.0.1:8787/manifest".into();
        let lua = render_manifest_lua(&settings);

        assert!(lua.starts_with(MANIFEST_LUA_SENTINEL));
        assert!(lua.contains("manifestdex"));
        assert!(lua.contains(MANIFEST_DEX_UA));
        assert!(lua.contains("http://127.0.0.1:8787/manifest/"));
        assert!(lua.contains("fetch_manifest_code_ex"));
        assert!(lua.contains("GOST_MANIFEST_SOURCES"));
    }

    /// The DLL executes every compilable prefix as its own chunk, so a top-level
    /// `local` is gone by the time the global fetch functions run. Guard against
    /// regressing to that shape.
    #[test]
    fn render_has_no_top_level_local() {
        let mut settings = ManagerSettings::default();
        settings.manifest_lua_endpoint = "http://127.0.0.1:8787/manifest".into();
        let lua = render_manifest_lua(&settings);

        let offenders: Vec<&str> = lua
            .lines()
            .filter(|line| line.starts_with("local "))
            .collect();
        assert!(offenders.is_empty(), "top-level local found: {offenders:?}");
    }

    #[test]
    fn install_and_remove_round_trip() {
        let steam = fixture_steam_dir();
        let settings = ManagerSettings::default();

        let status = install_manifest_lua(steam.path(), &settings).unwrap();
        assert!(status.present && status.managed && status.bytes > 0);
        assert!(steam
            .path()
            .join("config")
            .join("lua")
            .join("manifest.lua")
            .is_file());

        let after = remove_manifest_lua(steam.path()).unwrap();
        assert!(!after.present);
    }

    #[test]
    fn saving_settings_writes_and_respects_managed_flag() {
        let steam = fixture_steam_dir();
        let settings = ManagerSettings::default();
        save_settings_to_dir(steam.path(), &settings).unwrap();
        assert!(manifest_lua_status(steam.path()).unwrap().managed);

        let mut off = settings.clone();
        off.manage_manifest_lua = false;
        save_settings_to_dir(steam.path(), &off).unwrap();
        assert!(!manifest_lua_status(steam.path()).unwrap().present);
    }

    #[test]
    fn unmanaged_lua_file_is_never_removed() {
        let steam = fixture_steam_dir();
        let lua_dir = steam.path().join("config").join("lua");
        fs::create_dir_all(&lua_dir).unwrap();
        let path = lua_dir.join("manifest.lua");
        fs::write(&path, b"function fetch_manifest_code(gid) return nil end\n").unwrap();

        let mut settings = ManagerSettings::default();
        settings.manage_manifest_lua = false;
        save_settings_to_dir(steam.path(), &settings).unwrap();

        assert!(path.is_file());
        let status = manifest_lua_status(steam.path()).unwrap();
        assert!(status.present && !status.managed);
    }

    #[test]
    fn validation_accepts_opensteamtool_and_rejects_bad_endpoint() {
        let mut settings = ManagerSettings::default();
        settings.manifest_url = "opensteamtool".into();
        assert!(validate_settings(&settings).is_ok());

        settings.manifest_lua_endpoint = "ftp://nope".into();
        assert!(validate_settings(&settings).is_err());
    }
}

#[cfg(test)]
mod cdn_selection_tests {
    use super::*;
    use std::fs;

    fn fixture_steam_dir() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("steam.exe"), b"stub").unwrap();
        dir
    }

    #[test]
    fn reads_cell_id_from_config_vdf() {
        let steam = fixture_steam_dir();
        let config_dir = steam.path().join("config");
        fs::create_dir_all(&config_dir).unwrap();
        fs::write(
            config_dir.join("config.vdf"),
            "\"InstallConfigStore\"\n{\n\t\"Software\"\n\t{\n\t\t\"Valve\"\n\t\t{\n\t\t\t\"Steam\"\n\t\t\t{\n\t\t\t\t\"CurrentCellID\"\t\t\"166\"\n\t\t\t}\n\t\t}\n\t}\n}\n",
        )
        .unwrap();

        assert_eq!(read_cell_id(steam.path()), Some(166));
    }

    #[test]
    fn parses_steamcache_hosts_from_webapi() {
        let body = r#"{"response":{"servers":[
            {"type":"SteamCache","host":"cache5-hkg1.steamcontent.com","https_support":"mandatory"},
            {"type":"SteamCache","host":"cache4-hkg1.steamcontent.com","https_support":"mandatory"},
            {"type":"CDN","host":"dl.steam.clngaa.com","https_support":"optional"},
            {"type":"SteamCache","host":"cache6-hkg1.steamcontent.com","https_support":"none"}
        ]}}"#;

        assert_eq!(
            parse_cdn_hosts(body),
            vec![
                ("cache5-hkg1.steamcontent.com".to_string(), None),
                ("cache4-hkg1.steamcontent.com".to_string(), None)
            ]
        );
    }

    #[test]
    fn extracts_cache_host_from_content_log() {
        let steam = fixture_steam_dir();
        let logs = steam.path().join("logs");
        fs::create_dir_all(&logs).unwrap();
        fs::write(
            logs.join("content_log.txt"),
            concat!(
                "[2026-10-24 00:45:27] HTTPS (SteamCache,380) - cache9-tyo3.steamcontent.com ([fdfe:dcba:9876::158]:443 / 127.0.0.1:27897, host: cache9-tyo3.steamcontent.com): Negotiated HTTP/2\n",
                "[2026-10-24 01:49:46] HTTP (CDN,30) - dl.steam.clngaa.com (61.156.246.154:80 / 61.156.246.154:80, host: dl.steam.clngaa.com): manifest request received 200 (OK)\n"
            ),
        )
        .unwrap();

        assert_eq!(
            recent_log_cdn_host(steam.path()).as_deref(),
            Some("cache9-tyo3.steamcontent.com")
        );
    }
}
#[cfg(test)]
mod cdn_region_tests {
    use super::*;

    #[test]
    fn webapi_hosts_keep_their_cell_group() {
        let body = r#"{"response":{"servers":[
            {"type":"SteamCache","host":"cache6-hkg1.steamcontent.com","cell_id":33,"https_support":"mandatory"},
            {"type":"SteamCache","host":"cache9-tyo3.steamcontent.com","cell_id":32,"https_support":"mandatory"}
        ]}}"#;

        assert_eq!(
            parse_cdn_hosts(body),
            vec![
                ("cache6-hkg1.steamcontent.com".to_string(), Some(33)),
                ("cache9-tyo3.steamcontent.com".to_string(), Some(32))
            ]
        );
    }
}
