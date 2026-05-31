pub mod manager;

use manager::{GameConfig, ManagerSettings};
use serde::Serialize;
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_updater::UpdaterExt;

type CommandResult<T> = Result<T, String>;

#[derive(Debug, Clone, Serialize)]
struct UpdateCheckInfo {
    channel: String,
    available: bool,
    version: String,
    current_version: String,
    date: Option<String>,
    body: Option<String>,
    release: Option<manager::GitHubReleaseInfo>,
}

#[tauri::command]
fn detect_steam_dir() -> CommandResult<Option<String>> {
    manager::detect_steam_dir().map(|path| path.map(manager::display_path))
}

#[tauri::command]
async fn select_steam_dir(app: tauri::AppHandle) -> CommandResult<Option<String>> {
    Ok(app
        .dialog()
        .file()
        .blocking_pick_folder()
        .and_then(|path| path.into_path().ok())
        .map(manager::display_path))
}

#[tauri::command]
fn scan_state(app: tauri::AppHandle, steam_dir: String) -> CommandResult<manager::ScanState> {
    let assets =
        manager::resolve_dll_resource_dir_from_candidates(manager::dll_resource_candidates(&app));
    manager::scan_state_with_assets(steam_dir, assets.as_deref())
}

#[tauri::command]
fn install_dlls(app: tauri::AppHandle, steam_dir: String) -> CommandResult<()> {
    let assets = manager::resource_dll_dir(&app)?;
    manager::install_dlls_from_dir(steam_dir, assets)
}

#[tauri::command]
fn remove_dlls(steam_dir: String) -> CommandResult<()> {
    manager::remove_dlls_from_dir(steam_dir)
}

#[tauri::command]
fn load_settings(steam_dir: String) -> CommandResult<ManagerSettings> {
    manager::load_settings_from_dir(steam_dir)
}

#[tauri::command]
fn save_settings(steam_dir: String, settings: ManagerSettings) -> CommandResult<()> {
    manager::save_settings_to_dir(steam_dir, &settings)
}

#[tauri::command]
fn list_games(steam_dir: String) -> CommandResult<Vec<GameConfig>> {
    manager::list_games_from_dir(steam_dir)
}

#[tauri::command]
async fn import_lua_file(
    app: tauri::AppHandle,
    steam_dir: String,
) -> CommandResult<Option<GameConfig>> {
    let Some(path) = app
        .dialog()
        .file()
        .add_filter("Lua", &["lua"])
        .blocking_pick_file()
        .and_then(|path| path.into_path().ok())
    else {
        return Ok(None);
    };
    manager::import_lua_file_from_path(steam_dir, path).map(Some)
}

#[tauri::command]
fn open_lua_dir(steam_dir: String) -> CommandResult<()> {
    manager::open_lua_dir_for_steam(steam_dir)
}

#[tauri::command]
fn upsert_game(steam_dir: String, game: GameConfig) -> CommandResult<()> {
    manager::upsert_game_in_dir(steam_dir, &game)
}

#[tauri::command]
fn delete_game(steam_dir: String, appid: u32) -> CommandResult<()> {
    manager::delete_game_from_dir(steam_dir, appid)
}

#[tauri::command]
fn set_game_enabled(steam_dir: String, appid: u32, enabled: bool) -> CommandResult<()> {
    manager::set_game_enabled_in_dir(steam_dir, appid, enabled)
}

#[tauri::command]
fn fetch_app_metadata(appid: u32) -> CommandResult<manager::AppMetadata> {
    manager::fetch_app_metadata(appid)
}

#[tauri::command]
fn read_logs(steam_dir: String) -> CommandResult<Vec<manager::LogFile>> {
    manager::read_logs_from_dir(steam_dir)
}

#[tauri::command]
async fn check_github_release(dot_enabled: bool) -> CommandResult<manager::GitHubReleaseInfo> {
    manager::check_github_release(dot_enabled).await
}

#[tauri::command]
async fn resolve_github_domain_with_dot(host: String) -> CommandResult<Vec<String>> {
    manager::resolve_github_domain_with_dot(&host).await
}

#[tauri::command]
async fn test_github_dns_latency(host: String) -> CommandResult<manager::DnsLatencyReport> {
    manager::test_github_dns_latency(&host).await
}

#[tauri::command]
async fn check_update_channel(
    app: tauri::AppHandle,
    channel: String,
    dot_enabled: bool,
) -> CommandResult<UpdateCheckInfo> {
    let (endpoint, release) = update_endpoint_for_channel(&channel, dot_enabled).await?;
    let mut builder = app
        .updater_builder()
        .endpoints(vec![
            reqwest::Url::parse(&endpoint).map_err(|err| err.to_string())?
        ])
        .map_err(|err| err.to_string())?;

    if channel == "beta" {
        builder = builder.version_comparator(|current, update| update.version != current);
    }

    let update = builder
        .build()
        .map_err(|err| err.to_string())?
        .check()
        .await
        .map_err(|err| describe_update_error(&err.to_string()))?;

    Ok(if let Some(update) = update {
        UpdateCheckInfo {
            channel,
            available: true,
            version: update.version,
            current_version: update.current_version,
            date: update.date.map(|date| date.to_string()),
            body: update.body,
            release,
        }
    } else {
        let version = release
            .as_ref()
            .map(|release| release.version.clone())
            .unwrap_or_else(|| "0.2.0-beta.1".to_string());
        UpdateCheckInfo {
            channel,
            available: false,
            version,
            current_version: app.package_info().version.to_string(),
            date: release
                .as_ref()
                .and_then(|release| release.published_at.clone()),
            body: release.as_ref().map(|release| release.body.clone()),
            release,
        }
    })
}

#[tauri::command]
async fn install_update_channel(
    app: tauri::AppHandle,
    channel: String,
    dot_enabled: bool,
) -> CommandResult<()> {
    let (endpoint, _) = update_endpoint_for_channel(&channel, dot_enabled).await?;
    let mut builder = app
        .updater_builder()
        .endpoints(vec![
            reqwest::Url::parse(&endpoint).map_err(|err| err.to_string())?
        ])
        .map_err(|err| err.to_string())?;

    if channel == "beta" {
        builder = builder.version_comparator(|current, update| update.version != current);
    }

    let Some(update) = builder
        .build()
        .map_err(|err| err.to_string())?
        .check()
        .await
        .map_err(|err| describe_update_error(&err.to_string()))?
    else {
        return Err("当前通道没有可安装更新".into());
    };

    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|err| err.to_string())
}

async fn update_endpoint_for_channel(
    channel: &str,
    dot_enabled: bool,
) -> CommandResult<(String, Option<manager::GitHubReleaseInfo>)> {
    match channel {
        "stable" => Ok((manager::STABLE_UPDATE_ENDPOINT.to_string(), None)),
        "beta" => {
            let release = manager::check_github_beta_release(dot_enabled).await?;
            let endpoint = manager::release_asset_url(&release, manager::BETA_UPDATE_METADATA)
                .ok_or_else(|| "GitHub Pre-release 中未找到 beta-latest.json".to_string())?;
            Ok((endpoint, Some(release)))
        }
        other => Err(format!("Unsupported update channel: {other}")),
    }
}

fn describe_update_error(error: &str) -> String {
    if error.contains("Could not fetch a valid release JSON")
        || error.contains("latest.json")
        || error.contains("404")
    {
        "未找到 latest.json，请在 GitHub Release 上传 Tauri updater metadata 后再使用正式更新"
            .into()
    } else if error.contains("beta-latest.json") {
        "未找到 beta-latest.json，请在 GitHub Pre-release 上传测试版 updater metadata".into()
    } else {
        error.to_string()
    }
}

#[tauri::command]
fn close_steam() -> CommandResult<()> {
    manager::close_steam()
}

#[tauri::command]
fn restart_steam(steam_dir: String) -> CommandResult<()> {
    manager::restart_steam(steam_dir)
}

#[tauri::command]
fn minimize_window(window: tauri::Window) -> CommandResult<()> {
    window.minimize().map_err(|err| err.to_string())
}

#[tauri::command]
fn toggle_maximize_window(window: tauri::Window) -> CommandResult<()> {
    if window.is_maximized().map_err(|err| err.to_string())? {
        window.unmaximize().map_err(|err| err.to_string())
    } else {
        window.maximize().map_err(|err| err.to_string())
    }
}

#[tauri::command]
fn close_window(window: tauri::Window) -> CommandResult<()> {
    window.close().map_err(|err| err.to_string())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let _ = app.path().resource_dir();
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            detect_steam_dir,
            select_steam_dir,
            scan_state,
            install_dlls,
            remove_dlls,
            load_settings,
            save_settings,
            list_games,
            import_lua_file,
            open_lua_dir,
            upsert_game,
            delete_game,
            set_game_enabled,
            fetch_app_metadata,
            read_logs,
            check_github_release,
            resolve_github_domain_with_dot,
            test_github_dns_latency,
            check_update_channel,
            install_update_channel,
            close_steam,
            restart_steam,
            minimize_window,
            toggle_maximize_window,
            close_window
        ])
        .run(tauri::generate_context!())
        .expect("error while running G-OpenSteamTool");
}
