use crate::manager::{self, Result, ScanState};

pub fn detect() -> Result<Option<String>> {
    manager::detect_steam_dir().map(|path| path.map(manager::display_path))
}

pub fn scan(steam_dir: String) -> Result<ScanState> {
    manager::scan_state_with_bundled_assets(steam_dir)
}

pub fn close() -> Result<()> {
    manager::close_steam()
}

pub fn restart(steam_dir: String) -> Result<()> {
    manager::restart_steam(steam_dir)
}
