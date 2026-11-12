//! Steam 创意工坊直连下载。
//!
//! 新版工坊物品是「工坊 Depot」里的一个 manifest，旧版物品则是一个 UGC 文件。
//! Steam 客户端下载新版物品时，会用工坊 Depot 的 Depot Key 解密文件名和分块；
//! 解锁工具生成的 Lua 如果没有这一行 Key，客户端就会报「内容仍然处于加密状态」
//! 或 `Missing decryption key`。
//!
//! 本模块绕开 Steam 客户端，按 SteamPipe 协议自行取 manifest、拉分块、解密并写进
//! `steamapps/workshop/content/<appid>/<itemid>`。Depot Key 的来源依次是：
//! 当前 Lua、本机 config.vdf、工具缓存、可配置的联网密钥库。这里不生成、不猜测
//! Key；密钥库里没有的 Depot 仍然会明确报错。

use crate::manager::{self, ManagerSettings};
use aes::cipher::{generic_array::GenericArray, BlockDecrypt, KeyInit};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use cbc::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};
use serde::Serialize;
use serde_json::Value;
use sha1::{Digest, Sha1};
use std::collections::{HashMap, VecDeque};
use std::fs::{self, File};
use std::io::{Cursor, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use steam_vdf_parser::value::{Obj, Value as VdfValue};
use steam_vdf_parser::Vdf;
use zip::ZipArchive;

type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;

/// 进度回调由 Tauri 命令层提供；核心下载逻辑不直接依赖 `AppHandle`。
pub type ProgressCallback = Arc<dyn Fn(WorkshopProgress) + Send + Sync>;

const Z_MAGIC: u32 = 0x71F6_17D0;
const KEY_DB_TTL_SECS: u64 = 7 * 24 * 60 * 60;
const MANIFEST_STEAMRUN: &str = "https://manifest.steam.run/api/manifest/";
const MANIFEST_WUDRM: &str = "http://gmrc.wudrm.com/manifest/";
const MANIFEST_DEX: &str = "https://manifest.manifestdex.com/";

#[derive(Debug, Clone, Serialize)]
pub struct WorkshopApp {
    pub appid: u32,
    pub name: String,
    pub workshop_depot: u32,
    pub key_ready: bool,
    pub key_source: String,
    pub installed_items: u32,
    pub content_dir: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkshopItem {
    pub published_file_id: u64,
    pub appid: u32,
    pub title: String,
    pub workshop_depot: u32,
    pub manifest_gid: String,
    pub size_bytes: u64,
    pub time_updated: u64,
    pub legacy: bool,
    pub legacy_file_url: Option<String>,
    pub legacy_file_name: Option<String>,
    pub key_ready: bool,
    pub key_source: String,
    pub installed: bool,
    pub content_dir: String,
    /// 游戏目录下是否存在 `Mods`；存在时下载后可直接安装成游戏模组。
    pub mods_available: bool,
    pub game_mods_dir: Option<String>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkshopProgress {
    pub item_id: u64,
    pub phase: String,
    pub message: String,
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub chunks_done: u64,
    pub chunks_total: u64,
    pub current_file: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkshopDownloadReport {
    pub item: WorkshopItem,
    pub files: u32,
    pub bytes: u64,
    pub elapsed_ms: u64,
    pub content_dir: String,
    /// 已复制到游戏 `Mods/<folder>` 的目录；未启用/没有 Mods 目录时为空。
    pub mods_dir: Option<String>,
    pub mods_files: u32,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone)]
struct PublishedFile {
    appid: u32,
    title: String,
    size_bytes: u64,
    time_updated: u64,
    hcontent_file: Option<String>,
    file_url: Option<String>,
    file_name: Option<String>,
}

#[derive(Debug, Clone)]
struct MetaItem {
    appid: u32,
    title: String,
    depot_id: Option<u32>,
    manifest_gid: String,
    size_bytes: u64,
    time_updated: u64,
}

#[derive(Debug, Clone)]
struct KeyInfo {
    key: String,
    source: &'static str,
}

#[derive(Debug, Clone)]
struct ManifestFile {
    name: String,
    size: u64,
    sha_content: Vec<u8>,
    chunks: Vec<ManifestChunk>,
    /// Steam manifest 用 flags=64 标记目录条目；这些不是实际文件。
    directory: bool,
}

#[derive(Debug, Clone)]
struct ManifestChunk {
    id: String,
    checksum: u32,
    offset: u64,
    uncompressed_length: u32,
    compressed_length: u32,
}

#[derive(Debug, Clone)]
struct ChunkTask {
    file_index: usize,
    offset: u64,
    chunk: ManifestChunk,
}

// ---------------------------------------------------------------------------
// 对外入口
// ---------------------------------------------------------------------------

/// 列出当前配置库 / 已下载工坊目录里，确实带创意工坊的游戏。
pub fn workshop_apps<P: AsRef<Path>>(steam_dir: P) -> manager::Result<Vec<WorkshopApp>> {
    let steam_dir = manager::validate_steam_dir(steam_dir)?;
    let games = manager::list_games_from_dir(&steam_dir)?;
    let app_meta = load_app_meta(&steam_dir);
    let mut appids: Vec<u32> = games.iter().map(|game| game.appid).collect();

    for steamapps in steam_library_roots(&steam_dir) {
        let workshop_root = steamapps.join("workshop");
        if let Ok(entries) = fs::read_dir(&workshop_root) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if let Some(text) = name
                    .strip_prefix("appworkshop_")
                    .and_then(|value| value.strip_suffix(".acf"))
                {
                    if let Ok(appid) = text.parse::<u32>() {
                        appids.push(appid);
                    }
                }
            }
        }
        if let Ok(entries) = fs::read_dir(&workshop_root.join("content")) {
            for entry in entries.flatten() {
                if let Ok(appid) = entry.file_name().to_string_lossy().parse::<u32>() {
                    appids.push(appid);
                }
            }
        }
    }
    appids.sort_unstable();
    appids.dedup();

    let mut apps = Vec::new();
    for appid in appids {
        let meta = app_meta.get(&appid);
        let workshop_depot = meta.and_then(|item| item.workshop_depot).or_else(|| {
            let dir = workshop_content_dir(&steam_dir, appid);
            dir.is_dir().then_some(appid)
        });
        let Some(workshop_depot) = workshop_depot else {
            continue;
        };
        let name = games
            .iter()
            .find(|game| game.appid == appid)
            .map(|game| game.name.clone())
            .filter(|value| !value.trim().is_empty())
            .or_else(|| meta.map(|item| item.name.clone()))
            .unwrap_or_else(|| format!("App {appid}"));
        let key = find_key(&steam_dir, &games, appid, workshop_depot, false)?;
        let content_dir = workshop_content_dir(&steam_dir, appid);
        apps.push(WorkshopApp {
            appid,
            name,
            workshop_depot,
            key_ready: key.is_some(),
            key_source: key.map(|item| item.source.to_string()).unwrap_or_default(),
            installed_items: count_subdirs(&content_dir),
            content_dir: manager::display_path(&content_dir),
        });
    }
    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(apps)
}

/// 解析工坊链接 / 物品 ID，返回元数据与 Key 状态（不触发密钥库下载）。
pub fn resolve_item<P: AsRef<Path>>(
    steam_dir: P,
    expected_appid: Option<u32>,
    item: &str,
) -> manager::Result<WorkshopItem> {
    let steam_dir = manager::validate_steam_dir(steam_dir)?;
    let published_file_id = parse_workshop_id(item)?;
    let client = http_client(Duration::from_secs(20))?;
    build_item(
        &steam_dir,
        &client,
        published_file_id,
        expected_appid,
        Vec::new(),
    )
}

/// 下载并安装一个工坊物品。
pub fn download_item<P: AsRef<Path>>(
    steam_dir: P,
    expected_appid: Option<u32>,
    item: &str,
    key_override: Option<String>,
    install_mods: bool,
    progress_cb: Option<ProgressCallback>,
) -> manager::Result<WorkshopDownloadReport> {
    let started = Instant::now();
    let steam_dir = manager::validate_steam_dir(steam_dir)?;
    let settings = manager::load_settings_from_dir(&steam_dir)?;
    let client = http_client(Duration::from_secs(120))?;
    let published_file_id = parse_workshop_id(item)?;
    let mut item = build_item(
        &steam_dir,
        &client,
        published_file_id,
        expected_appid,
        Vec::new(),
    )?;
    let progress = ProgressSink::new(progress_cb, published_file_id, item.size_bytes);
    progress.emit("resolve", "已解析工坊物品", true);

    let override_key = key_override
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let mut key = match override_key {
        Some(value) => Some(KeyInfo {
            key: validate_key(value)?,
            source: "手动",
        }),
        None => key_from_item(&steam_dir, &item)?,
    };
    if let Some(found) = &key {
        item.key_ready = true;
        item.key_source = found.source.to_string();
    }
    if key.is_none() && !item.legacy {
        progress.emit("key", "本地没有工坊 Depot Key，正在查询密钥库…", true);
        let games = manager::list_games_from_dir(&steam_dir)?;
        key = find_key(&steam_dir, &games, item.appid, item.workshop_depot, true)?;
        if key.is_none() {
            return Err(format!(
                "缺少创意工坊 Depot Key（App {} / Depot {}）。Steam 就是因此显示「内容仍然处于加密状态」。请在下载面板手动填入 Key，或在设置中配置可用的密钥库地址。",
                item.appid, item.workshop_depot
            ));
        }
        item.key_ready = true;
        item.key_source = key
            .as_ref()
            .map(|value| value.source.to_string())
            .unwrap_or_default();
    }

    let content_dir =
        workshop_content_dir(&steam_dir, item.appid).join(published_file_id.to_string());
    if content_dir.exists() {
        let stale = content_dir.with_extension("gost-stale");
        let _ = fs::remove_dir_all(&stale);
        fs::rename(&content_dir, &stale).map_err(|err| {
            format!(
                "无法替换已有的工坊目录 {}：{err}（请先关闭游戏/Steam）",
                manager::display_path(&content_dir)
            )
        })?;
        let _ = fs::remove_dir_all(&stale);
    }
    fs::create_dir_all(&content_dir).map_err(|err| {
        format!(
            "无法创建工坊目录 {}：{err}",
            manager::display_path(&content_dir)
        )
    })?;

    let (files, bytes) = if item.legacy {
        download_legacy(&client, &item, &content_dir, &progress)?
    } else {
        let key = key
            .as_ref()
            .ok_or_else(|| "缺少工坊 Depot Key".to_string())?;
        download_manifest_item(
            &steam_dir,
            &settings,
            &client,
            &item,
            &key.key,
            &content_dir,
            &progress,
        )?
    };

    progress.emit("installed", "文件已写入，正在更新 Steam 工坊记录…", true);
    let mut notes = Vec::new();
    if let Err(err) = upsert_workshop_acf(&steam_dir, &item, bytes) {
        notes.push(format!(
            "工坊文件已下载，但更新 appworkshop 记录失败：{err}"
        ));
    }

    let (mods_dir, mods_files) = if install_mods {
        progress.emit("mods", "正在安装到游戏 Mods 文件夹…", true);
        match install_to_game_mods(&steam_dir, &item, &content_dir) {
            Ok((path, count)) => {
                notes.push(format!(
                    "已安装到游戏 Mods：{}（{} 个文件），重启游戏后在模组菜单启用。",
                    manager::display_path(&path),
                    count
                ));
                (Some(manager::display_path(&path)), count)
            }
            Err(err) => {
                notes.push(format!("未能安装到游戏 Mods：{err}"));
                (None, 0)
            }
        }
    } else {
        (None, 0)
    };

    item.installed = true;
    item.notes = notes.clone();
    progress.emit("done", "工坊内容已就绪", true);
    Ok(WorkshopDownloadReport {
        item,
        files,
        bytes,
        elapsed_ms: started.elapsed().as_millis() as u64,
        content_dir: manager::display_path(&content_dir),
        mods_dir,
        mods_files,
        notes,
    })
}

/// 只刷新密钥库缓存，供工坊页手动重试。
pub fn refresh_key_db<P: AsRef<Path>>(steam_dir: P) -> manager::Result<usize> {
    let steam_dir = manager::validate_steam_dir(steam_dir)?;
    let settings = manager::load_settings_from_dir(&steam_dir)?;
    let db = load_key_db(&steam_dir, &settings, true)?;
    Ok(db.len())
}

// ---------------------------------------------------------------------------
// 物品元数据
// ---------------------------------------------------------------------------

fn build_item(
    steam_dir: &Path,
    client: &reqwest::blocking::Client,
    published_file_id: u64,
    expected_appid: Option<u32>,
    mut notes: Vec<String>,
) -> manager::Result<WorkshopItem> {
    let published = match fetch_published_file(client, published_file_id) {
        Ok(value) => Some(value),
        Err(err) => {
            notes.push(format!("Steam PublishedFileDetails 不可用：{err}"));
            None
        }
    };
    // 工坊索引只用于补充标题/Depot/大小等元数据；它有故障时 Steam
    // PublishedFileDetails 仍然足够完成下载，不要把索引错误当成下载警告。
    let meta = fetch_meta_item(client, published_file_id).ok();
    if published.is_none() && meta.is_none() {
        return Err(format!(
            "无法解析工坊物品 {published_file_id}：Steam API 与工坊索引都没有返回数据。请检查网络，或直接粘贴纯数字物品 ID 重试。"
        ));
    }

    let appid = published
        .as_ref()
        .map(|value| value.appid)
        .or_else(|| meta.as_ref().map(|value| value.appid))
        .or(expected_appid)
        .filter(|value| *value > 0)
        .ok_or_else(|| "工坊物品没有 consumer_app_id，无法确定所属游戏".to_string())?;
    if let Some(expected) = expected_appid.filter(|value| *value > 0) {
        if expected != appid {
            return Err(format!(
                "这个工坊物品属于 App {appid}，不是当前选中的 App {expected}"
            ));
        }
    }

    let app_meta = load_app_meta(steam_dir).remove(&appid);
    let workshop_depot = app_meta
        .as_ref()
        .and_then(|value| value.workshop_depot)
        .or_else(|| meta.as_ref().and_then(|value| value.depot_id))
        .unwrap_or(appid);
    let manifest_gid = published
        .as_ref()
        .and_then(|value| value.hcontent_file.clone())
        .or_else(|| meta.as_ref().map(|value| value.manifest_gid.clone()))
        .unwrap_or_default();
    let legacy = published
        .as_ref()
        .map(|value| value.file_url.is_some())
        .unwrap_or(false);
    let title = published
        .as_ref()
        .map(|value| value.title.clone())
        .or_else(|| meta.as_ref().map(|value| value.title.clone()))
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| format!("Workshop item {published_file_id}"));
    let size_bytes = published
        .as_ref()
        .map(|value| value.size_bytes)
        .or_else(|| meta.as_ref().map(|value| value.size_bytes))
        .unwrap_or(0);
    let time_updated = published
        .as_ref()
        .map(|value| value.time_updated)
        .or_else(|| meta.as_ref().map(|value| value.time_updated))
        .unwrap_or(0);
    let games = manager::list_games_from_dir(steam_dir)?;
    let key = find_key(steam_dir, &games, appid, workshop_depot, false)?;
    let content_dir = workshop_content_dir(steam_dir, appid).join(published_file_id.to_string());
    let key_ready = legacy || key.is_some();
    let key_source = if legacy {
        "旧版 UGC".to_string()
    } else {
        key.map(|value| value.source.to_string())
            .unwrap_or_default()
    };
    let game_mods = game_mods_dir(steam_dir, appid);

    Ok(WorkshopItem {
        published_file_id,
        appid,
        title,
        workshop_depot,
        manifest_gid,
        size_bytes,
        time_updated,
        legacy,
        legacy_file_url: published.as_ref().and_then(|value| value.file_url.clone()),
        legacy_file_name: published.as_ref().and_then(|value| value.file_name.clone()),
        key_ready,
        key_source,
        installed: content_dir.is_dir(),
        content_dir: manager::display_path(&content_dir),
        mods_available: game_mods.is_some(),
        game_mods_dir: game_mods.map(|path| manager::display_path(&path)),
        notes,
    })
}

fn fetch_published_file(
    client: &reqwest::blocking::Client,
    published_file_id: u64,
) -> manager::Result<PublishedFile> {
    let form = [
        ("itemcount".to_string(), "1".to_string()),
        (
            "publishedfileids[0]".to_string(),
            published_file_id.to_string(),
        ),
    ];
    let mut errors = Vec::new();
    for endpoint in [
        "http://api.steampowered.com/ISteamRemoteStorage/GetPublishedFileDetails/v1/",
        "https://api.steampowered.com/ISteamRemoteStorage/GetPublishedFileDetails/v1/",
    ] {
        match client.post(endpoint).form(&form).send() {
            Ok(response) => {
                let status = response.status();
                if !status.is_success() {
                    errors.push(format!("{endpoint} -> HTTP {status}"));
                    continue;
                }
                let value = match response.json::<Value>() {
                    Ok(value) => value,
                    Err(err) => {
                        errors.push(format!("{endpoint} -> {err}"));
                        continue;
                    }
                };
                let Some(detail) = value
                    .get("response")
                    .and_then(|item| item.get("publishedfiledetails"))
                    .and_then(Value::as_array)
                    .and_then(|items| items.first())
                else {
                    errors.push(format!("{endpoint} -> 响应里没有 publishedfiledetails"));
                    continue;
                };
                let result = detail.get("result").and_then(Value::as_i64).unwrap_or(1);
                if result != 1 {
                    errors.push(format!("{endpoint} -> result={result}"));
                    continue;
                }
                return Ok(PublishedFile {
                    appid: parse_u32(detail.get("consumer_app_id")),
                    title: parse_string(detail.get("title")),
                    size_bytes: parse_u64(detail.get("file_size")),
                    time_updated: parse_u64(detail.get("time_updated")),
                    hcontent_file: parse_optional_string(detail.get("hcontent_file"))
                        .filter(|value| value != "0"),
                    file_url: parse_optional_string(detail.get("file_url")),
                    file_name: parse_optional_string(detail.get("filename")),
                });
            }
            Err(err) => errors.push(format!("{endpoint} -> {err}")),
        }
    }
    Err(errors.join("；"))
}

fn fetch_meta_item(
    client: &reqwest::blocking::Client,
    published_file_id: u64,
) -> manager::Result<MetaItem> {
    let url = format!("https://manifest.steam.run/api/workshop/{published_file_id}");
    let response = client
        .get(&url)
        .header(reqwest::header::USER_AGENT, "G-OpenSteamTool/0.14")
        .send()
        .map_err(|err| format!("{url} -> {err}"))?;
    if !response.status().is_success() {
        return Err(format!("{url} -> HTTP {}", response.status()));
    }
    let value = response.json::<Value>().map_err(|err| err.to_string())?;
    if value.get("kind").and_then(Value::as_str) == Some("collection") {
        return Err("该链接是合集（collection），请分别下载合集里的物品".into());
    }
    let depot = value
        .get("depots")
        .and_then(Value::as_array)
        .and_then(|items| items.first());
    Ok(MetaItem {
        appid: parse_u32(value.get("appid")),
        title: parse_string(value.get("title")),
        depot_id: depot.map(|item| parse_u32(item.get("depotid"))),
        manifest_gid: depot
            .and_then(|item| item.get("manifestid"))
            .and_then(value_as_string)
            .unwrap_or_default(),
        size_bytes: depot
            .map(|item| parse_u64(item.get("size_bytes")))
            .unwrap_or(0),
        time_updated: value
            .get("timeupdated")
            .and_then(value_as_string)
            .and_then(|text| text.parse::<u64>().ok())
            .unwrap_or(0),
    })
}

// ---------------------------------------------------------------------------
// Depot Key
// ---------------------------------------------------------------------------

fn key_from_item(steam_dir: &Path, item: &WorkshopItem) -> manager::Result<Option<KeyInfo>> {
    let games = manager::list_games_from_dir(steam_dir)?;
    find_key(steam_dir, &games, item.appid, item.workshop_depot, false)
}

fn find_key(
    steam_dir: &Path,
    games: &[manager::GameConfig],
    appid: u32,
    depot: u32,
    allow_remote: bool,
) -> manager::Result<Option<KeyInfo>> {
    if let Some(game) = games.iter().find(|game| game.appid == appid) {
        if let Some(key) = lua_key(game, depot) {
            return Ok(Some(KeyInfo { key, source: "Lua" }));
        }
    }
    if let Some(key) = config_key(steam_dir, depot) {
        return Ok(Some(KeyInfo {
            key,
            source: "config.vdf",
        }));
    }
    if let Some(key) = cached_key(steam_dir, depot) {
        return Ok(Some(KeyInfo {
            key,
            source: "密钥库缓存",
        }));
    }
    if allow_remote {
        let settings = manager::load_settings_from_dir(steam_dir)?;
        if !settings.workshop_key_url.trim().is_empty() {
            if let Ok(db) = load_key_db(steam_dir, &settings, false) {
                if let Some(key) = db.get(&depot.to_string()).filter(|value| is_key(value)) {
                    return Ok(Some(KeyInfo {
                        key: key.to_lowercase(),
                        source: "密钥库",
                    }));
                }
            }
        }
    }
    Ok(None)
}

fn lua_key(game: &manager::GameConfig, depot: u32) -> Option<String> {
    if game.appid == depot {
        if let Some(key) = game.depot_key.as_deref().filter(|value| is_key(value)) {
            return Some(key.to_lowercase());
        }
    }
    game.appid_entries
        .iter()
        .filter(|entry| entry.appid == depot)
        .filter_map(|entry| entry.depot_key.as_deref())
        .find(|value| is_key(value))
        .map(|value| value.to_lowercase())
}

fn current_steam_id(steam_dir: &Path) -> Option<String> {
    let text = fs::read_to_string(steam_dir.join("config").join("config.vdf")).ok()?;
    let index = text.find("\"SteamID\"")?;
    let rest = &text[index + "\"SteamID\"".len()..];
    rest.split('"')
        .map(str::trim)
        .find(|part| !part.is_empty() && part.chars().all(|ch| ch.is_ascii_digit()))
        .map(ToString::to_string)
}

fn config_key(steam_dir: &Path, depot: u32) -> Option<String> {
    let text = fs::read_to_string(steam_dir.join("config").join("config.vdf")).ok()?;
    let vdf = steam_vdf_parser::parse_text(&text).ok()?;
    let depots = vdf.get_obj(&["Software", "Valve", "Steam", "depots"])?;
    let entry = depots.get(&depot.to_string())?.as_obj()?;
    let key = entry.get("DecryptionKey")?.as_str()?.trim();
    is_key(key).then(|| key.to_lowercase())
}

fn cached_key(steam_dir: &Path, depot: u32) -> Option<String> {
    let text = fs::read_to_string(key_cache_path(steam_dir)).ok()?;
    let db: HashMap<String, String> = serde_json::from_str(&text).ok()?;
    db.get(&depot.to_string())
        .filter(|value| is_key(value))
        .map(|value| value.to_lowercase())
}

fn load_key_db(
    steam_dir: &Path,
    settings: &ManagerSettings,
    force: bool,
) -> manager::Result<HashMap<String, String>> {
    let cache = key_cache_path(steam_dir);
    let stamp = key_stamp_path(steam_dir);
    let fresh = fs::metadata(&stamp)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|age| now_secs().saturating_sub(age.as_secs()) < KEY_DB_TTL_SECS)
        .unwrap_or(false);
    if !force {
        if let Ok(text) = fs::read_to_string(&cache) {
            if let Ok(db) = serde_json::from_str::<HashMap<String, String>>(&text) {
                if fresh {
                    return Ok(db);
                }
            }
        }
    }

    let url = settings.workshop_key_url.trim();
    if url.is_empty() {
        return Err("未配置创意工坊密钥库地址".into());
    }
    let client = http_client(Duration::from_secs(180))?;
    let response = client
        .get(url)
        .header(reqwest::header::USER_AGENT, "G-OpenSteamTool/0.14")
        .send()
        .map_err(|err| format!("无法下载密钥库 {url}：{err}"))?;
    if !response.status().is_success() {
        return Err(format!("密钥库 {url} 返回 HTTP {}", response.status()));
    }
    let bytes = response
        .bytes()
        .map_err(|err| format!("读取密钥库失败：{err}"))?;
    let db: HashMap<String, String> = serde_json::from_slice(&bytes)
        .map_err(|err| format!("密钥库格式不正确（应为 depotId -> key 的 JSON）：{err}"))?;
    if db.is_empty() {
        return Err("密钥库是空的".into());
    }
    let dir = cache.parent().unwrap_or(steam_dir);
    fs::create_dir_all(dir).map_err(|err| err.to_string())?;
    let tmp = cache.with_extension("json.tmp");
    fs::write(&tmp, &bytes).map_err(|err| err.to_string())?;
    fs::rename(&tmp, &cache).map_err(|err| err.to_string())?;
    let _ = fs::write(&stamp, now_secs().to_string());
    Ok(db)
}

fn key_cache_path(steam_dir: &Path) -> PathBuf {
    steam_dir.join("opensteamtool").join("workshop_keys.json")
}

fn key_stamp_path(steam_dir: &Path) -> PathBuf {
    steam_dir.join("opensteamtool").join("workshop_keys.stamp")
}

fn is_key(value: &str) -> bool {
    value.len() == 64 && value.chars().all(|ch| ch.is_ascii_hexdigit())
}

fn validate_key(value: &str) -> manager::Result<String> {
    let value = value.trim();
    if !is_key(value) {
        return Err("Depot Key 必须是 64 位十六进制".into());
    }
    Ok(value.to_lowercase())
}

// ---------------------------------------------------------------------------
// manifest 下载与解析
// ---------------------------------------------------------------------------

fn download_manifest_item(
    steam_dir: &Path,
    settings: &ManagerSettings,
    client: &reqwest::blocking::Client,
    item: &WorkshopItem,
    key: &str,
    content_dir: &Path,
    progress: &ProgressSink,
) -> manager::Result<(u32, u64)> {
    if item.manifest_gid.is_empty() {
        return Err("工坊物品没有 manifest GID，无法下载".into());
    }
    progress.emit("manifest", "正在获取 manifest 取码…", true);
    let code = fetch_manifest_code(client, settings, &item.manifest_gid)?;
    progress.emit("manifest", "正在下载 manifest…", true);
    let manifest = download_manifest_bytes(
        client,
        steam_dir,
        item.workshop_depot,
        &item.manifest_gid,
        &code,
    )?;
    let files = parse_manifest_zip(&manifest, key)?;
    if files.is_empty() {
        return Err("manifest 里没有文件，可能不是有效的工坊物品".into());
    }
    let bytes = files.iter().map(|file| file.size).sum::<u64>();
    progress.set_total(bytes);
    progress.emit("download", "开始下载工坊分块…", true);

    let file_handles = prepare_files(content_dir, &files)?;
    let mut tasks = Vec::new();
    for (file_index, file) in files.iter().enumerate() {
        for chunk in &file.chunks {
            tasks.push(ChunkTask {
                file_index,
                offset: chunk.offset,
                chunk: chunk.clone(),
            });
        }
    }
    progress.set_chunks(tasks.len() as u64);
    if tasks.is_empty() {
        return Ok((files.len() as u32, 0));
    }

    let queue = Arc::new(Mutex::new(VecDeque::from(tasks)));
    let handles = Arc::new(file_handles);
    let error: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let workers = worker_count();
    let hosts = cdn_candidates(steam_dir, client);
    let host_pool = Arc::new(Mutex::new(VecDeque::from(hosts)));
    let files = Arc::new(files);

    std::thread::scope(|scope| {
        for _ in 0..workers {
            let queue = Arc::clone(&queue);
            let handles = Arc::clone(&handles);
            let error = Arc::clone(&error);
            let host_pool = Arc::clone(&host_pool);
            let files = Arc::clone(&files);
            let client = client.clone();
            let progress = progress.clone();
            let key = key.to_string();
            let content_dir = content_dir.to_path_buf();
            scope.spawn(move || loop {
                if error.lock().ok().and_then(|value| value.clone()).is_some() {
                    break;
                }
                let task = queue.lock().ok().and_then(|mut value| value.pop_front());
                let Some(task) = task else {
                    break;
                };
                let file_name = files
                    .get(task.file_index)
                    .map(|file| file.name.clone())
                    .unwrap_or_default();
                progress.set_current_file(&file_name);
                let mut last_error = None;
                let mut written = None;
                for _ in 0..3 {
                    let host = host_pool
                        .lock()
                        .ok()
                        .and_then(|mut pool| pool.pop_front())
                        .unwrap_or_else(|| "cache9-tyo3.steamcontent.com".to_string());
                    match download_chunk(&client, &host, item.workshop_depot, &task.chunk, &key) {
                        Ok(data) => {
                            written = Some(data);
                            if let Ok(mut pool) = host_pool.lock() {
                                pool.push_back(host);
                            }
                            break;
                        }
                        Err(err) => {
                            last_error = Some(err);
                            if let Ok(mut pool) = host_pool.lock() {
                                pool.push_back(host);
                            }
                        }
                    }
                }
                let Some(data) = written else {
                    if let Ok(mut slot) = error.lock() {
                        *slot = last_error.or_else(|| Some("下载分块失败".to_string()));
                    }
                    break;
                };
                let write_result = handles
                    .get(task.file_index)
                    .ok_or_else(|| "文件索引越界".to_string())
                    .and_then(|handle| {
                        let mut file = handle.lock().map_err(|err| err.to_string())?;
                        file.seek(SeekFrom::Start(task.offset))
                            .map_err(|err| err.to_string())?;
                        file.write_all(&data).map_err(|err| err.to_string())
                    });
                if let Err(err) = write_result {
                    if let Ok(mut slot) = error.lock() {
                        *slot = Some(format!(
                            "写入 {} 失败：{err}",
                            manager::display_path(&content_dir.join(&file_name))
                        ));
                    }
                    break;
                }
                progress.add_bytes(data.len() as u64);
                progress.add_chunk();
            });
        }
    });

    if let Some(err) = error.lock().ok().and_then(|value| value.clone()) {
        return Err(err);
    }
    drop(handles);
    progress.emit("verify", "正在校验工坊文件…", true);
    for file in files.iter() {
        if file.size == 0 || file.sha_content.is_empty() {
            continue;
        }
        let path = content_dir.join(&file.name);
        let hash = file_sha1(&path)?;
        if !hash.eq_ignore_ascii_case(&hex(&file.sha_content)) {
            return Err(format!(
                "文件校验失败：{}（Depot Key 不对或下载不完整）",
                manager::display_path(&path)
            ));
        }
    }
    Ok((files.len() as u32, bytes))
}

fn fetch_manifest_code(
    client: &reqwest::blocking::Client,
    settings: &ManagerSettings,
    gid: &str,
) -> manager::Result<String> {
    let mut urls = Vec::new();
    let endpoint = settings.manifest_lua_endpoint.trim();
    if !endpoint.is_empty() {
        urls.push(format!(
            "{}{gid}",
            if endpoint.ends_with('/') {
                endpoint.to_string()
            } else {
                format!("{endpoint}/")
            }
        ));
    }
    urls.push(format!("{MANIFEST_STEAMRUN}{gid}"));
    urls.push(format!("{MANIFEST_WUDRM}{gid}"));
    urls.push(format!("{MANIFEST_DEX}{gid}"));
    let mut errors = Vec::new();
    for url in urls {
        match client
            .get(&url)
            .header(reqwest::header::USER_AGENT, "G-OpenSteamTool/0.14")
            .send()
        {
            Ok(response) if response.status().is_success() => {
                if let Ok(body) = response.text() {
                    if let Some(code) = parse_manifest_code_body(&body) {
                        return Ok(code);
                    }
                    errors.push(format!("{url} -> 响应里没有 code"));
                }
            }
            Ok(response) => errors.push(format!("{url} -> HTTP {}", response.status())),
            Err(err) => errors.push(format!("{url} -> {err}")),
        }
    }
    Err(format!("没有取到 manifest 取码：{}", errors.join("；")))
}

fn parse_manifest_code_body(body: &str) -> Option<String> {
    let trimmed = body.trim();
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        if let Some(content) = value.get("content").and_then(Value::as_str) {
            if !content.is_empty() && content.chars().all(|ch| ch.is_ascii_digit()) {
                return Some(content.to_string());
            }
        }
    }
    if !trimmed.is_empty() && trimmed.chars().all(|ch| ch.is_ascii_digit()) {
        return Some(trimmed.to_string());
    }
    None
}

fn download_manifest_bytes(
    client: &reqwest::blocking::Client,
    steam_dir: &Path,
    depot: u32,
    gid: &str,
    code: &str,
) -> manager::Result<Vec<u8>> {
    let mut errors = Vec::new();
    for host in cdn_candidates(steam_dir, client) {
        let url = format!("https://{host}/depot/{depot}/manifest/{gid}/5/{code}");
        match client
            .get(&url)
            .header(reqwest::header::USER_AGENT, "Valve/Steam HTTP Client 1.0")
            .send()
        {
            Ok(response) if response.status().is_success() => {
                let bytes = response.bytes().map_err(|err| err.to_string())?;
                if bytes.starts_with(b"PK\x03\x04") {
                    return Ok(bytes.to_vec());
                }
                errors.push(format!("{host} -> 不是 manifest zip"));
            }
            Ok(response) => errors.push(format!("{host} -> HTTP {}", response.status())),
            Err(err) => errors.push(format!("{host} -> {err}")),
        }
    }
    Err(format!(
        "manifest 下载失败（Depot {depot} / {gid}）：{}",
        errors.join("；")
    ))
}

fn parse_manifest_zip(bytes: &[u8], key: &str) -> manager::Result<Vec<ManifestFile>> {
    let mut archive = ZipArchive::new(Cursor::new(bytes))
        .map_err(|err| format!("manifest 不是有效的 zip：{err}"))?;
    let mut z = archive
        .by_name("z")
        .map_err(|err| format!("manifest zip 里没有 z 条目：{err}"))?;
    let mut data = Vec::new();
    z.read_to_end(&mut data).map_err(|err| err.to_string())?;
    let body = if data.len() >= 8 && u32::from_le_bytes(data[0..4].try_into().unwrap()) == Z_MAGIC {
        let length = u32::from_le_bytes(data[4..8].try_into().unwrap()) as usize;
        if data.len() < 8 + length {
            return Err("manifest z 条目被截断".into());
        }
        &data[8..8 + length]
    } else {
        &data[..]
    };
    let key = validate_key(key)?;
    let mut files = Vec::new();
    let mut pos = 0usize;
    while pos < body.len() {
        let (field, wire) = read_tag(body, &mut pos)?;
        match wire {
            0 => {
                let _ = read_varint(body, &mut pos)?;
            }
            2 => {
                let payload = read_len(body, &mut pos)?;
                if field == 1 {
                    let file = parse_file_mapping(payload, &key)?;
                    if !file.directory && !file.name.ends_with('/') && !file.name.ends_with('\\') {
                        files.push(file);
                    }
                }
            }
            5 => pos = pos.saturating_add(4),
            1 => pos = pos.saturating_add(8),
            _ => return Err(format!("不支持的 manifest 字段类型 {wire}")),
        }
    }
    Ok(files)
}

fn parse_file_mapping(data: &[u8], key: &str) -> manager::Result<ManifestFile> {
    let mut pos = 0usize;
    let mut name = String::new();
    let mut size = 0u64;
    let mut flags = 0u64;
    let mut sha_content = Vec::new();
    let mut chunks = Vec::new();
    while pos < data.len() {
        let (field, wire) = read_tag(data, &mut pos)?;
        match wire {
            0 => {
                let value = read_varint(data, &mut pos)?;
                if field == 2 {
                    size = value;
                } else if field == 3 {
                    flags = value;
                }
            }
            2 => {
                let payload = read_len(data, &mut pos)?;
                match field {
                    1 => {
                        let encoded = String::from_utf8_lossy(payload).to_string();
                        name = decrypt_filename(&encoded, key)
                            .or_else(|| {
                                (!encoded.contains('=')
                                    && (encoded.contains('/')
                                        || encoded.contains('\\')
                                        || encoded.contains('.')))
                                .then_some(encoded)
                            })
                            .ok_or_else(|| {
                                "无法解密工坊文件名：Depot Key 不正确或 manifest 已损坏".to_string()
                            })?;
                    }
                    5 => sha_content = payload.to_vec(),
                    6 => chunks.push(parse_chunk(payload)?),
                    _ => {}
                }
            }
            5 => pos = pos.saturating_add(4),
            1 => pos = pos.saturating_add(8),
            _ => return Err(format!("不支持的 manifest 文件字段类型 {wire}")),
        }
    }
    let name = name.trim_end_matches('\0').to_string();
    if name.trim().is_empty() || name.contains('\0') {
        return Err("manifest 文件名为空或包含 NUL".into());
    }
    Ok(ManifestFile {
        name,
        size,
        sha_content,
        chunks,
        directory: flags & 0x40 != 0,
    })
}

fn parse_chunk(data: &[u8]) -> manager::Result<ManifestChunk> {
    let mut pos = 0usize;
    let mut id = String::new();
    let mut checksum = 0u32;
    let mut offset = 0u64;
    let mut uncompressed_length = 0u32;
    let mut compressed_length = 0u32;
    while pos < data.len() {
        let (field, wire) = read_tag(data, &mut pos)?;
        match wire {
            0 => {
                let value = read_varint(data, &mut pos)?;
                match field {
                    2 => checksum = value as u32,
                    3 => offset = value,
                    4 => uncompressed_length = value as u32,
                    5 => compressed_length = value as u32,
                    _ => {}
                }
            }
            2 => {
                let payload = read_len(data, &mut pos)?;
                if field == 1 {
                    id = hex(payload);
                }
            }
            5 => pos = pos.saturating_add(4),
            1 => pos = pos.saturating_add(8),
            _ => return Err(format!("不支持的 manifest 分块字段类型 {wire}")),
        }
    }
    if id.is_empty() || compressed_length == 0 {
        return Err("manifest 分块缺少 ChunkID / 长度".into());
    }
    Ok(ManifestChunk {
        id,
        checksum,
        offset,
        uncompressed_length,
        compressed_length,
    })
}

fn read_tag(data: &[u8], pos: &mut usize) -> manager::Result<(u32, u8)> {
    let value = read_varint(data, pos)?;
    Ok(((value >> 3) as u32, (value & 7) as u8))
}

fn read_varint(data: &[u8], pos: &mut usize) -> manager::Result<u64> {
    let mut value = 0u64;
    let mut shift = 0u32;
    loop {
        let byte = *data
            .get(*pos)
            .ok_or_else(|| "manifest 数据被截断".to_string())?;
        *pos += 1;
        value |= ((byte & 0x7f) as u64) << shift;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
        shift += 7;
        if shift >= 64 {
            return Err("manifest varint 过长".into());
        }
    }
}

fn read_len<'a>(data: &'a [u8], pos: &mut usize) -> manager::Result<&'a [u8]> {
    let length = read_varint(data, pos)? as usize;
    let end = pos
        .checked_add(length)
        .ok_or_else(|| "manifest 长度溢出".to_string())?;
    if end > data.len() {
        return Err("manifest 字段被截断".into());
    }
    let slice = &data[*pos..end];
    *pos = end;
    Ok(slice)
}

// ---------------------------------------------------------------------------
// 分块下载 / 解密
// ---------------------------------------------------------------------------

fn download_chunk(
    client: &reqwest::blocking::Client,
    host: &str,
    depot: u32,
    chunk: &ManifestChunk,
    key: &str,
) -> manager::Result<Vec<u8>> {
    let url = format!("https://{host}/depot/{depot}/chunk/{}", chunk.id);
    let response = client
        .get(&url)
        .header(reqwest::header::USER_AGENT, "Valve/Steam HTTP Client 1.0")
        .send()
        .map_err(|err| format!("{host} -> {err}"))?;
    if !response.status().is_success() {
        return Err(format!("{host} -> HTTP {}", response.status()));
    }
    let data = response.bytes().map_err(|err| err.to_string())?;
    if chunk.compressed_length != 0 && data.len() != chunk.compressed_length as usize {
        return Err(format!(
            "分块 {} 长度不符（{} != {}）",
            chunk.id,
            data.len(),
            chunk.compressed_length
        ));
    }
    let plain = decrypt_chunk(&data, key)?;
    if plain.len() != chunk.uncompressed_length as usize {
        return Err(format!(
            "分块 {} 解压后长度不符（{} != {}）",
            chunk.id,
            plain.len(),
            chunk.uncompressed_length
        ));
    }
    let checksum = adler32(&plain);
    if chunk.checksum != 0 && checksum != chunk.checksum {
        return Err(format!(
            "分块 {} 校验失败（Adler32 {:08x} != {:08x}）",
            chunk.id, checksum, chunk.checksum
        ));
    }
    Ok(plain)
}

fn decrypt_chunk(data: &[u8], key: &str) -> manager::Result<Vec<u8>> {
    if data.len() < 32 {
        return Err("工坊分块太短".into());
    }
    let key = decode_key(key)?;
    let cipher = aes::Aes256::new_from_slice(&key).map_err(|err| err.to_string())?;
    let mut iv = GenericArray::clone_from_slice(&data[..16]);
    cipher.decrypt_block(&mut iv);
    let mut buffer = data[16..].to_vec();
    let plain = Aes256CbcDec::new(GenericArray::from_slice(&key), &iv)
        .decrypt_padded_mut::<Pkcs7>(&mut buffer)
        .map_err(|err| format!("工坊分块解密失败：{err}"))?;
    if plain.len() < 4 {
        return Err("工坊分块解密后为空".into());
    }
    match &plain[..4] {
        b"VSZa" => {
            if plain.len() < 8 + 15 {
                return Err("VZstd 分块太短".into());
            }
            let expected =
                u32::from_le_bytes(plain[plain.len() - 11..plain.len() - 7].try_into().unwrap())
                    as usize;
            let frame = &plain[8..plain.len() - 15];
            zstd::bulk::decompress(frame, expected).map_err(|err| format!("Zstd 解压失败：{err}"))
        }
        b"VZa" => Err("该工坊分块使用 LZMA 压缩，当前版本暂不支持".into()),
        [0x50, 0x4b, 0x03, 0x04] => {
            let mut archive = ZipArchive::new(Cursor::new(plain))
                .map_err(|err| format!("Zip 分块解析失败：{err}"))?;
            let mut entry = archive
                .by_index(0)
                .map_err(|err| format!("Zip 分块没有条目：{err}"))?;
            let mut out = Vec::new();
            entry.read_to_end(&mut out).map_err(|err| err.to_string())?;
            Ok(out)
        }
        other => Err(format!("未知的工坊分块压缩格式：{}", hex(other))),
    }
}

fn decode_key(key: &str) -> manager::Result<Vec<u8>> {
    let key = validate_key(key)?;
    (0..32)
        .map(|index| u8::from_str_radix(&key[index * 2..index * 2 + 2], 16))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| err.to_string())
}

fn decrypt_filename(encoded: &str, key: &str) -> Option<String> {
    // 部分工坊 manifest 会把 base64 每 64 个字符硬换行；先去掉所有空白。
    let compact: String = encoded.chars().filter(|ch| !ch.is_whitespace()).collect();
    let data = BASE64.decode(compact).ok()?;
    if data.len() < 32 || (data.len() - 16) % 16 != 0 {
        return None;
    }
    let key = decode_key(key).ok()?;
    let cipher = aes::Aes256::new_from_slice(&key).ok()?;
    let mut iv = GenericArray::clone_from_slice(&data[..16]);
    cipher.decrypt_block(&mut iv);
    let mut buffer = data[16..].to_vec();
    let plain = Aes256CbcDec::new(GenericArray::from_slice(&key), &iv)
        .decrypt_padded_mut::<Pkcs7>(&mut buffer)
        .ok()?;
    let end = plain
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(plain.len());
    let name = String::from_utf8(plain[..end].to_vec()).ok()?;
    (!name.is_empty()).then_some(name)
}

fn adler32(data: &[u8]) -> u32 {
    let mut a = 1u32;
    let mut b = 0u32;
    for byte in data {
        a = (a + *byte as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

// ---------------------------------------------------------------------------
// 旧版 UGC 物品
// ---------------------------------------------------------------------------

fn download_legacy(
    client: &reqwest::blocking::Client,
    item: &WorkshopItem,
    content_dir: &Path,
    progress: &ProgressSink,
) -> manager::Result<(u32, u64)> {
    let url = item
        .legacy_file_url
        .as_deref()
        .ok_or_else(|| "旧版工坊物品没有 file_url".to_string())?;
    let file_name = item
        .legacy_file_name
        .as_deref()
        .and_then(|value| Path::new(value).file_name())
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("workshop_item.bin");
    let safe_name = safe_relative_path(file_name)
        .and_then(|path| path.to_str().map(ToString::to_string))
        .ok_or_else(|| "旧版工坊文件名不安全".to_string())?;
    progress.emit("download", "正在下载旧版工坊文件…", true);
    let response = client
        .get(url)
        .header(reqwest::header::USER_AGENT, "G-OpenSteamTool/0.14")
        .send()
        .map_err(|err| format!("下载 {url} 失败：{err}"))?;
    if !response.status().is_success() {
        return Err(format!("{url} 返回 HTTP {}", response.status()));
    }
    let bytes = response.bytes().map_err(|err| err.to_string())?;
    let path = content_dir.join(&safe_name);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    fs::write(&path, &bytes)
        .map_err(|err| format!("写入 {} 失败：{err}", manager::display_path(&path)))?;
    progress.add_bytes(bytes.len() as u64);
    progress.add_chunk();
    Ok((1, bytes.len() as u64))
}

// ---------------------------------------------------------------------------
// appworkshop_<appid>.acf
// ---------------------------------------------------------------------------

fn upsert_workshop_acf(steam_dir: &Path, item: &WorkshopItem, bytes: u64) -> manager::Result<()> {
    let workshop_root = steam_library_for_app(steam_dir, item.appid).join("workshop");
    fs::create_dir_all(&workshop_root).map_err(|err| err.to_string())?;
    let path = workshop_root.join(format!("appworkshop_{}.acf", item.appid));
    let mut vdf = if path.exists() {
        let text = fs::read_to_string(&path).map_err(|err| err.to_string())?;
        steam_vdf_parser::parse_text(&text)
            .map_err(|err| format!("解析 {} 失败：{err}", manager::display_path(&path)))?
            .into_owned()
    } else {
        Vdf::new("AppWorkshop", VdfValue::Obj(Obj::new()))
    };
    let root = vdf
        .value_mut()
        .as_obj_mut()
        .ok_or_else(|| "AppWorkshop 根节点不是对象".to_string())?;
    set_str(root, "appid", item.appid.to_string());
    set_str(root, "NeedsUpdate", "0");
    set_str(root, "NeedsDownload", "0");
    set_str(root, "TimeLastUpdated", now_secs().to_string());
    set_str(root, "TimeLastFullCheck", now_secs().to_string());
    set_str(root, "TimeLastAppRan", "0");
    set_str(root, "LastBuildID", "0");
    let total = directory_size(&workshop_root.join("content").join(item.appid.to_string()));
    set_str(root, "SizeOnDisk", total.max(bytes).to_string());

    let installed = ensure_obj(root, "WorkshopItemsInstalled");
    let mut entry = Obj::new();
    entry.insert("size", VdfValue::Str(bytes.to_string().into()));
    entry.insert(
        "timeupdated",
        VdfValue::Str(item.time_updated.to_string().into()),
    );
    if item.legacy {
        entry.insert("manifest", VdfValue::Str("-1".into()));
        if let Some(handle) = item
            .legacy_file_url
            .as_deref()
            .and_then(ugc_handle_from_url)
        {
            entry.insert("ugchandle", VdfValue::Str(handle.into()));
        }
    } else {
        entry.insert("manifest", VdfValue::Str(item.manifest_gid.clone().into()));
    }
    installed.insert(item.published_file_id.to_string(), VdfValue::Obj(entry));

    let details = ensure_obj(root, "WorkshopItemDetails");
    let mut detail = Obj::new();
    detail.insert(
        "manifest",
        VdfValue::Str(
            if item.legacy {
                "-1".to_string()
            } else {
                item.manifest_gid.clone()
            }
            .into(),
        ),
    );
    detail.insert(
        "timeupdated",
        VdfValue::Str(item.time_updated.to_string().into()),
    );
    detail.insert("timetouched", VdfValue::Str("0".into()));
    detail.insert(
        "subscribedby",
        VdfValue::Str(
            current_steam_id(steam_dir)
                .unwrap_or_else(|| "0".to_string())
                .into(),
        ),
    );
    detail.insert(
        "latest_timeupdated",
        VdfValue::Str(item.time_updated.to_string().into()),
    );
    detail.insert(
        "latest_manifest",
        VdfValue::Str(item.manifest_gid.clone().into()),
    );
    details.insert(item.published_file_id.to_string(), VdfValue::Obj(detail));

    let mut out = String::new();
    write_vdf(&mut out, vdf.key(), vdf.value(), 0);
    if path.exists() {
        let backup = path.with_extension("acf.bak");
        let _ = fs::copy(&path, &backup);
    }
    let tmp = path.with_extension("acf.tmp");
    fs::write(&tmp, out.as_bytes()).map_err(|err| err.to_string())?;
    fs::rename(&tmp, &path).map_err(|err| err.to_string())?;
    Ok(())
}

fn set_str(obj: &mut Obj<'static>, key: &str, value: impl Into<String>) {
    obj.insert(key.to_string(), VdfValue::Str(value.into().into()));
}

fn ensure_obj<'a>(obj: &'a mut Obj<'static>, key: &str) -> &'a mut Obj<'static> {
    if !obj.contains_key(key) {
        obj.insert(key.to_string(), VdfValue::Obj(Obj::new()));
    }
    obj.get_mut(key)
        .and_then(VdfValue::as_obj_mut)
        .expect("just inserted object")
}

fn write_vdf(out: &mut String, key: &str, value: &VdfValue<'_>, depth: usize) {
    let pad = "\t".repeat(depth);
    match value {
        VdfValue::Obj(obj) => {
            out.push_str(&format!("{pad}\"{}\"\n{pad}{{\n", vdf_escape(key)));
            for (child_key, child) in obj.iter() {
                write_vdf(out, child_key, child, depth + 1);
            }
            out.push_str(&format!("{pad}}}\n"));
        }
        VdfValue::Str(text) => {
            out.push_str(&format!(
                "{pad}\"{}\"\t\t\"{}\"\n",
                vdf_escape(key),
                vdf_escape(text)
            ));
        }
        VdfValue::I32(number) => {
            out.push_str(&format!("{pad}\"{}\"\t\t\"{}\"\n", vdf_escape(key), number));
        }
        VdfValue::U64(number) => {
            out.push_str(&format!("{pad}\"{}\"\t\t\"{}\"\n", vdf_escape(key), number));
        }
        VdfValue::Float(number) => {
            out.push_str(&format!("{pad}\"{}\"\t\t\"{}\"\n", vdf_escape(key), number));
        }
        VdfValue::Pointer(number) => {
            out.push_str(&format!("{pad}\"{}\"\t\t\"{}\"\n", vdf_escape(key), number));
        }
        VdfValue::Color(color) => {
            out.push_str(&format!(
                "{pad}\"{}\"\t\t\"{}\"\n",
                vdf_escape(key),
                color
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>()
            ));
        }
    }
}

fn vdf_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn ugc_handle_from_url(url: &str) -> Option<String> {
    let rest = url.split("/ugc/").nth(1)?;
    rest.split('/')
        .next()
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

// ---------------------------------------------------------------------------
// Steam 路径
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct AppMeta {
    name: String,
    workshop_depot: Option<u32>,
}

fn load_app_meta(steam_dir: &Path) -> HashMap<u32, AppMeta> {
    let Ok(data) = fs::read(steam_dir.join("appcache").join("appinfo.vdf")) else {
        return HashMap::new();
    };
    let Ok(appinfo) = steam_vdf_parser::parse_appinfo(&data) else {
        return HashMap::new();
    };
    let Some(root) = appinfo.as_obj() else {
        return HashMap::new();
    };
    let mut result = HashMap::new();
    for (key, value) in root.iter() {
        let Ok(appid) = key.parse::<u32>() else {
            continue;
        };
        let Some(app) = value.as_obj() else {
            continue;
        };
        let payload = app.get("appinfo").and_then(VdfValue::as_obj).unwrap_or(app);
        let name = payload
            .get("common")
            .and_then(VdfValue::as_obj)
            .and_then(|common| common.get("name"))
            .and_then(VdfValue::as_str)
            .unwrap_or("")
            .to_string();
        let workshop_depot = payload
            .get("depots")
            .and_then(VdfValue::as_obj)
            .and_then(|depots| depots.get("workshopdepot"))
            .and_then(vdf_u32);
        result.insert(
            appid,
            AppMeta {
                name,
                workshop_depot,
            },
        );
    }
    result
}

fn vdf_u32(value: &VdfValue<'_>) -> Option<u32> {
    value
        .as_str()
        .and_then(|text| text.trim().parse::<u32>().ok())
        .or_else(|| value.as_u64().and_then(|number| u32::try_from(number).ok()))
}

fn steam_library_roots(steam_dir: &Path) -> Vec<PathBuf> {
    let main = steam_dir.join("steamapps");
    let mut roots = vec![main.clone()];
    let Ok(text) = fs::read_to_string(main.join("libraryfolders.vdf")) else {
        return roots;
    };
    let Ok(vdf) = steam_vdf_parser::parse_text(&text) else {
        return roots;
    };
    let Some(folders) = vdf.as_obj() else {
        return roots;
    };
    for (_key, value) in folders.iter() {
        let Some(folder) = value.as_obj() else {
            continue;
        };
        let Some(path) = folder.get("path").and_then(VdfValue::as_str) else {
            continue;
        };
        let steamapps = PathBuf::from(path).join("steamapps");
        if !roots.iter().any(|existing| existing == &steamapps) {
            roots.push(steamapps);
        }
    }
    roots
}

fn steam_library_for_app(steam_dir: &Path, appid: u32) -> PathBuf {
    let roots = steam_library_roots(steam_dir);
    if appid > 0 {
        for steamapps in &roots {
            if steamapps.join(format!("appmanifest_{appid}.acf")).exists() {
                return steamapps.clone();
            }
        }
    }
    roots
        .into_iter()
        .next()
        .unwrap_or_else(|| steam_dir.join("steamapps"))
}

fn workshop_content_dir(steam_dir: &Path, appid: u32) -> PathBuf {
    steam_library_for_app(steam_dir, appid)
        .join("workshop")
        .join("content")
        .join(appid.to_string())
}

fn game_install_dir(steam_dir: &Path, appid: u32) -> Option<PathBuf> {
    let steamapps = steam_library_for_app(steam_dir, appid);
    let text = fs::read_to_string(steamapps.join(format!("appmanifest_{appid}.acf"))).ok()?;
    let vdf = steam_vdf_parser::parse_text(&text).ok()?;
    let installdir = vdf.get_str(&["installdir"])?;
    let dir = steamapps.join("common").join(installdir);
    dir.is_dir().then_some(dir)
}

/// 游戏自己的模组目录（`<game>/Mods`），不是 Steam 的 workshop 目录。
fn game_mods_dir(steam_dir: &Path, appid: u32) -> Option<PathBuf> {
    let dir = game_install_dir(steam_dir, appid)?.join("Mods");
    dir.is_dir().then_some(dir)
}

fn install_to_game_mods(
    steam_dir: &Path,
    item: &WorkshopItem,
    content_dir: &Path,
) -> manager::Result<(PathBuf, u32)> {
    let mods_root = game_mods_dir(steam_dir, item.appid)
        .ok_or_else(|| "游戏目录下没有 Mods 文件夹".to_string())?;
    let folder = mod_folder_name(content_dir)
        .unwrap_or_else(|| format!("Workshop-{}", item.published_file_id));
    let dest = mods_root.join(&folder);

    // 先备份同名旧模组；复制时只覆盖包内文件，保留用户自己放的配置。
    if dest.exists() {
        // 备份放到 Mods 外面，避免游戏把备份目录也当成一个模组扫描。
        let backup_root = mods_root
            .parent()
            .unwrap_or(&mods_root)
            .join("Mods.gost-backup");
        let backup = backup_root.join(&folder);
        let _ = fs::remove_dir_all(&backup);
        let _ = fs::create_dir_all(&backup_root);
        let _ = copy_tree(&dest, &backup);
    }
    let count = copy_tree(content_dir, &dest)?;
    Ok((dest, count))
}

fn mod_folder_name(content_dir: &Path) -> Option<String> {
    let manifest = fs::read_to_string(content_dir.join("manifest.xml")).ok()?;
    let assembly = xml_tag(&manifest, "Assembly").or_else(|| xml_tag(&manifest, "ID"))?;
    let stem = assembly
        .trim()
        .strip_suffix(".dll")
        .or_else(|| assembly.trim().strip_suffix(".DLL"))
        .unwrap_or(assembly.trim());
    sanitize_folder_name(stem)
}

fn xml_tag(text: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = text.find(&open)? + open.len();
    let end = text[start..].find(&close)? + start;
    Some(text[start..end].trim().to_string())
}

fn sanitize_folder_name(value: &str) -> Option<String> {
    let cleaned: String = value
        .chars()
        .map(|ch| {
            if ch.is_control() || matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
            {
                '_'
            } else {
                ch
            }
        })
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').trim();
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned.chars().take(96).collect())
    }
}

fn copy_tree(src: &Path, dst: &Path) -> manager::Result<u32> {
    fs::create_dir_all(dst)
        .map_err(|err| format!("创建 {} 失败：{err}", manager::display_path(dst)))?;
    let mut count = 0u32;
    for entry in fs::read_dir(src)
        .map_err(|err| format!("读取 {} 失败：{err}", manager::display_path(src)))?
        .flatten()
    {
        let path = entry.path();
        let target = dst.join(entry.file_name());
        if path.is_dir() {
            count += copy_tree(&path, &target)?;
        } else {
            fs::copy(&path, &target).map_err(|err| {
                format!(
                    "复制到 {} 失败：{err}（请先关闭游戏）",
                    manager::display_path(&target)
                )
            })?;
            count += 1;
        }
    }
    Ok(count)
}

fn count_subdirs(dir: &Path) -> u32 {
    fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .filter(|entry| entry.path().is_dir())
                .count() as u32
        })
        .unwrap_or(0)
}

fn directory_size(dir: &Path) -> u64 {
    let mut total = 0u64;
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            total += directory_size(&path);
        } else if let Ok(meta) = path.metadata() {
            total += meta.len();
        }
    }
    total
}

fn prepare_files(
    content_dir: &Path,
    files: &[ManifestFile],
) -> manager::Result<Vec<Arc<Mutex<File>>>> {
    let mut handles = Vec::with_capacity(files.len());
    for file in files {
        let relative = safe_relative_path(&file.name)
            .ok_or_else(|| format!("manifest 里的路径不安全：{}", file.name))?;
        let path = content_dir.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        let handle = File::create(&path)
            .map_err(|err| format!("创建 {} 失败：{err}", manager::display_path(&path)))?;
        if file.size > 0 {
            handle
                .set_len(file.size)
                .map_err(|err| format!("预分配 {} 失败：{err}", manager::display_path(&path)))?;
        }
        handles.push(Arc::new(Mutex::new(handle)));
    }
    Ok(handles)
}

fn safe_relative_path(name: &str) -> Option<PathBuf> {
    let normalized = name.replace('\\', "/");
    let mut path = PathBuf::new();
    for part in normalized.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." || part.contains(':') || part.contains('\0') {
            return None;
        }
        path.push(part);
    }
    (!path.as_os_str().is_empty()).then_some(path)
}

fn file_sha1(path: &Path) -> manager::Result<String> {
    let mut file = File::open(path).map_err(|err| err.to_string())?;
    let mut hasher = Sha1::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|err| err.to_string())?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex(&hasher.finalize()))
}

// ---------------------------------------------------------------------------
// 通用工具
// ---------------------------------------------------------------------------

fn http_client(timeout: Duration) -> manager::Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .timeout(timeout)
        .connect_timeout(Duration::from_secs(15))
        .user_agent("G-OpenSteamTool/0.14")
        .build()
        .map_err(|err| err.to_string())
}

fn worker_count() -> usize {
    std::thread::available_parallelism()
        .map(|count| count.get().clamp(2, 8))
        .unwrap_or(4)
        .min(6)
}

fn cdn_candidates(steam_dir: &Path, client: &reqwest::blocking::Client) -> Vec<String> {
    // 先用 Steam 日志里刚刚连过的节点，再退到固定节点；WebAPI 只作补充。
    // 这样在 api.steampowered.com 不可达的网络里不会每次先等一轮超时。
    let mut hosts = Vec::new();
    if let Some(host) = manager::recent_log_cdn_host(steam_dir) {
        hosts.push(host);
    }
    for host in [
        "cache9-tyo3.steamcontent.com",
        "cache7-hkg1.steamcontent.com",
        "cache12-hkg1.steamcontent.com",
    ] {
        if !hosts.contains(&host.to_string()) {
            hosts.push(host.to_string());
        }
    }
    if let Some(cell) = manager::read_cell_id(steam_dir) {
        for (host, _) in manager::fetch_cdn_hosts(client, cell) {
            if !hosts.contains(&host) {
                hosts.push(host);
            }
        }
    }
    hosts
}

fn parse_workshop_id(input: &str) -> manager::Result<u64> {
    let text = input.trim();
    if text.is_empty() {
        return Err("请输入创意工坊链接或物品 ID".into());
    }
    if text.chars().all(|ch| ch.is_ascii_digit()) {
        return text
            .parse::<u64>()
            .map_err(|_| "工坊物品 ID 超出范围".to_string());
    }
    for marker in ["id=", "/filedetails/?id=", "CommunityFilePage/"] {
        if let Some(index) = text.find(marker) {
            let rest = &text[index + marker.len()..];
            let digits = rest
                .chars()
                .take_while(|ch| ch.is_ascii_digit())
                .collect::<String>();
            if let Ok(value) = digits.parse::<u64>() {
                if value > 0 {
                    return Ok(value);
                }
            }
        }
    }
    let tail = text
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or_default();
    tail.parse::<u64>()
        .map_err(|_| format!("无法从「{text}」里识别工坊物品 ID"))
}

fn parse_string(value: Option<&Value>) -> String {
    value.and_then(value_as_string).unwrap_or_default()
}

fn parse_optional_string(value: Option<&Value>) -> Option<String> {
    value
        .and_then(value_as_string)
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

fn parse_u32(value: Option<&Value>) -> u32 {
    value
        .and_then(|item| {
            item.as_u64().or_else(|| {
                item.as_str()
                    .and_then(|text| text.trim().parse::<u64>().ok())
            })
        })
        .unwrap_or(0) as u32
}

fn parse_u64(value: Option<&Value>) -> u64 {
    value
        .and_then(|item| {
            item.as_u64().or_else(|| {
                item.as_str()
                    .and_then(|text| text.trim().parse::<u64>().ok())
            })
        })
        .unwrap_or(0)
}

fn value_as_string(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(ToString::to_string)
        .or_else(|| value.as_u64().map(|number| number.to_string()))
        .or_else(|| value.as_i64().map(|number| number.to_string()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// 进度
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct ProgressSink {
    callback: Option<ProgressCallback>,
    item_id: u64,
    bytes_done: Arc<AtomicU64>,
    bytes_total: Arc<AtomicU64>,
    chunks_done: Arc<AtomicU64>,
    chunks_total: Arc<AtomicU64>,
    current_file: Arc<Mutex<String>>,
    last_emit: Arc<Mutex<Instant>>,
}

impl ProgressSink {
    fn new(callback: Option<ProgressCallback>, item_id: u64, bytes_total: u64) -> Self {
        Self {
            callback,
            item_id,
            bytes_done: Arc::new(AtomicU64::new(0)),
            bytes_total: Arc::new(AtomicU64::new(bytes_total)),
            chunks_done: Arc::new(AtomicU64::new(0)),
            chunks_total: Arc::new(AtomicU64::new(0)),
            current_file: Arc::new(Mutex::new(String::new())),
            last_emit: Arc::new(Mutex::new(Instant::now() - Duration::from_secs(1))),
        }
    }

    fn set_total(&self, bytes: u64) {
        self.bytes_total.store(bytes, Ordering::Relaxed);
    }

    fn set_chunks(&self, chunks: u64) {
        self.chunks_total.store(chunks, Ordering::Relaxed);
    }

    fn add_bytes(&self, bytes: u64) {
        self.bytes_done.fetch_add(bytes, Ordering::Relaxed);
        self.emit("download", "", false);
    }

    fn add_chunk(&self) {
        self.chunks_done.fetch_add(1, Ordering::Relaxed);
    }

    fn set_current_file(&self, name: &str) {
        if let Ok(mut slot) = self.current_file.lock() {
            *slot = name.to_string();
        }
    }

    fn emit(&self, phase: &str, message: &str, force: bool) {
        let Some(callback) = &self.callback else {
            return;
        };
        let now = Instant::now();
        if let Ok(mut last) = self.last_emit.lock() {
            if !force && now.duration_since(*last) < Duration::from_millis(160) {
                return;
            }
            *last = now;
        }
        let payload = WorkshopProgress {
            item_id: self.item_id,
            phase: phase.to_string(),
            message: message.to_string(),
            bytes_done: self.bytes_done.load(Ordering::Relaxed),
            bytes_total: self.bytes_total.load(Ordering::Relaxed),
            chunks_done: self.chunks_done.load(Ordering::Relaxed),
            chunks_total: self.chunks_total.load(Ordering::Relaxed),
            current_file: self
                .current_file
                .lock()
                .map(|value| value.clone())
                .unwrap_or_default(),
        };
        callback(payload);
    }
}

// ---------------------------------------------------------------------------
// 测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_workshop_ids_from_urls() {
        assert_eq!(parse_workshop_id("3829206666").unwrap(), 3_829_206_666);
        assert_eq!(
            parse_workshop_id(
                "https://steamcommunity.com/sharedfiles/filedetails/?id=3829206666&searchtext="
            )
            .unwrap(),
            3_829_206_666
        );
        assert_eq!(
            parse_workshop_id("steam://url/CommunityFilePage/3829206666").unwrap(),
            3_829_206_666
        );
        assert_eq!(
            parse_workshop_id("https://steamcommunity.com/workshop/filedetails/?id=1417465272")
                .unwrap(),
            1_417_465_272
        );
    }

    #[test]
    fn decrypts_known_steam_filename() {
        let name = decrypt_filename(
            "6lhRjKTk96yCZiPKO9dVcF7t89t28BL3KmHHArFHops=",
            "71c85102ab69ddbd68700653b9e01f46b28add2ff2c0b8c9c5e3a141973322d2",
        )
        .unwrap();
        assert_eq!(name, "LICENSE");
    }

    #[test]
    fn decrypts_wrapped_base64_filename() {
        // 3829696854 的 manifest 把 base64 每 64 字符硬换行；换行必须忽略。
        let name = decrypt_filename(
            "/dPF+40s/nlVLnpmWM5cjs/Kf043A0Ho9OohT1JsfvQvpE0xwaLiclBajHrkzpSQordVMPgj\njBrZLm0R0Sm36w==",
            "71c85102ab69ddbd68700653b9e01f46b28add2ff2c0b8c9c5e3a141973322d2",
        )
        .unwrap();
        assert_eq!(name, "Textures\\Items\\cp_portrait_femaleRev7.png");
    }

    #[test]
    fn adler_matches_known_value() {
        assert_eq!(adler32(b"hello"), 0x062C_0215);
    }

    /// 手动联调：设置 GST_STEAM_DIR 后列出工坊游戏。
    #[test]
    #[ignore]
    fn workshop_apps_probe() {
        let steam_dir = std::env::var("GST_STEAM_DIR").expect("steam dir");
        let apps = workshop_apps(&steam_dir).expect("apps");
        for app in apps {
            println!(
                "{} appid={} depot={} key={} source={} items={}",
                app.name,
                app.appid,
                app.workshop_depot,
                app.key_ready,
                app.key_source,
                app.installed_items
            );
        }
        if let Ok(item) = std::env::var("GST_WORKSHOP_ITEM") {
            let item = resolve_item(&steam_dir, None, &item).expect("resolve");
            println!(
                "item={} appid={} depot={} manifest={} key={} source={}",
                item.title,
                item.appid,
                item.workshop_depot,
                item.manifest_gid,
                item.key_ready,
                item.key_source
            );
            if let Ok(appid) = std::env::var("GST_DISCOVER_APPID") {
                let discovery =
                    manager::discover_depots_from_steam(&steam_dir, appid.parse().unwrap())
                        .unwrap();
                for depot in discovery.depots {
                    println!(
                        "depot={} key={} manifest={:?}",
                        depot.depot_id,
                        depot.depot_key.is_some(),
                        depot.manifest_gid
                    );
                }
            }
        }
    }

    /// 手动联调：把已下载的工坊物品复制到游戏 Mods 文件夹。
    #[test]
    #[ignore]
    fn mods_install_probe() {
        let steam_dir = std::env::var("GST_STEAM_DIR").expect("steam dir");
        let item = std::env::var("GST_WORKSHOP_ITEM").expect("workshop item");
        let item = resolve_item(&steam_dir, None, &item).expect("resolve");
        let content_dir = workshop_content_dir(Path::new(&steam_dir), item.appid)
            .join(item.published_file_id.to_string());
        println!("folder={:?}", mod_folder_name(&content_dir));
        let (dir, count) =
            install_to_game_mods(Path::new(&steam_dir), &item, &content_dir).expect("mods install");
        println!("dir={} files={count}", manager::display_path(&dir));
    }

    /// 手动联调：设置 GST_WORKSHOP_STEAM_DIR 与 GST_WORKSHOP_ITEM 后运行
    /// `cargo test workshop_network_probe -- --ignored --nocapture`。
    #[test]
    #[ignore]
    fn workshop_network_probe() {
        let steam_dir = std::env::var("GST_WORKSHOP_STEAM_DIR").expect("steam dir");
        let item = std::env::var("GST_WORKSHOP_ITEM").expect("workshop item");
        let report = download_item(&steam_dir, None, &item, None, false, None).expect("download");
        println!(
            "{} files={} bytes={} dir={}",
            report.item.title, report.files, report.bytes, report.content_dir
        );
        for note in &report.notes {
            println!("note: {note}");
        }
    }
}
