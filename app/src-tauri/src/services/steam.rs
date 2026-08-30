use crate::manager::{self, Result, ScanState};

pub fn detect() -> Result<Option<String>> {
    manager::detect_steam_dir().map(|path| path.map(manager::display_path))
}

pub fn scan(app: &tauri::AppHandle, steam_dir: String) -> Result<ScanState> {
    let assets =
        manager::resolve_dll_resource_dir_from_candidates(manager::dll_resource_candidates(app));
    manager::scan_state_with_assets(steam_dir, assets.as_deref())
}

pub fn close() -> Result<()> {
    manager::close_steam()
}

pub fn restart(steam_dir: String) -> Result<()> {
    manager::restart_steam(steam_dir)
}
