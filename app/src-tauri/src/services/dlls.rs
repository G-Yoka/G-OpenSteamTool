use crate::manager::{self, Result};

pub fn install(app: &tauri::AppHandle, steam_dir: String) -> Result<()> {
    let assets = manager::resource_dll_dir(app)?;
    manager::install_dlls_from_dir(steam_dir, assets)
}

pub fn remove(steam_dir: String) -> Result<()> {
    manager::remove_dlls_from_dir(steam_dir)
}
