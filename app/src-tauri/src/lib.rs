pub mod manager;
pub mod services;

use manager::{GameConfig, ManagerSettings};
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;

type CommandResult<T> = Result<T, String>;

#[tauri::command]
fn detect_steam_dir() -> CommandResult<Option<String>> {
    services::steam::detect()
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
    services::steam::scan(&app, steam_dir)
}

#[tauri::command]
fn install_dlls(app: tauri::AppHandle, steam_dir: String) -> CommandResult<()> {
    services::dlls::install(&app, steam_dir)
}

#[tauri::command]
fn remove_dlls(steam_dir: String) -> CommandResult<()> {
    services::dlls::remove(steam_dir)
}

#[tauri::command]
fn load_settings(steam_dir: String) -> CommandResult<ManagerSettings> {
    services::settings::load(steam_dir)
}

#[tauri::command]
fn save_settings(steam_dir: String, settings: ManagerSettings) -> CommandResult<()> {
    services::settings::save(steam_dir, &settings)
}

#[tauri::command]
fn list_games(steam_dir: String) -> CommandResult<Vec<GameConfig>> {
    services::games::list(steam_dir)
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
    services::games::import(steam_dir, path).map(Some)
}

#[tauri::command]
fn open_lua_dir(steam_dir: String) -> CommandResult<()> {
    services::games::open_directory(steam_dir)
}

#[tauri::command]
fn upsert_game(steam_dir: String, game: GameConfig) -> CommandResult<()> {
    services::games::upsert(steam_dir, &game)
}

#[tauri::command]
fn delete_game(steam_dir: String, appid: u32) -> CommandResult<()> {
    services::games::delete(steam_dir, appid)
}

#[tauri::command]
fn set_game_enabled(steam_dir: String, appid: u32, enabled: bool) -> CommandResult<()> {
    services::games::set_enabled(steam_dir, appid, enabled)
}

#[tauri::command]
fn fetch_app_metadata(appid: u32) -> CommandResult<manager::AppMetadata> {
    services::games::metadata(appid)
}

#[tauri::command]
async fn discover_depots(steam_dir: String, appid: u32) -> CommandResult<manager::DepotDiscovery> {
    tauri::async_runtime::spawn_blocking(move || services::games::discover_depots(steam_dir, appid))
        .await
        .map_err(|err| format!("Depot discovery task failed: {err}"))?
}

#[tauri::command]
fn read_logs(steam_dir: String) -> CommandResult<Vec<manager::LogFile>> {
    services::logs::read(steam_dir)
}

#[tauri::command]
async fn close_steam() -> CommandResult<()> {
    tauri::async_runtime::spawn_blocking(services::steam::close)
        .await
        .map_err(|err| format!("Steam shutdown task failed: {err}"))?
}

#[tauri::command]
async fn restart_steam(steam_dir: String) -> CommandResult<()> {
    tauri::async_runtime::spawn_blocking(move || services::steam::restart(steam_dir))
        .await
        .map_err(|err| format!("Steam restart task failed: {err}"))?
}

#[tauri::command]
fn minimize_window(window: tauri::Window) -> CommandResult<()> {
    window.minimize().map_err(|err| err.to_string())
}

#[tauri::command]
fn close_window(window: tauri::Window) -> CommandResult<()> {
    window.close().map_err(|err| err.to_string())
}

#[tauri::command]
fn start_window_drag(window: tauri::Window) -> CommandResult<()> {
    window.start_dragging().map_err(|err| err.to_string())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
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
            discover_depots,
            read_logs,
            close_steam,
            restart_steam,
            minimize_window,
            close_window,
            start_window_drag
        ])
        .run(tauri::generate_context!())
        .expect("error while running G-OpenSteamTool");
}
