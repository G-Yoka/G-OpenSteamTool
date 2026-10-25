use crate::manager::{self, ManagerSettings, Result};

pub fn load(steam_dir: String) -> Result<ManagerSettings> {
    manager::load_settings_from_dir(steam_dir)
}

pub fn save(steam_dir: String, settings: &ManagerSettings) -> Result<()> {
    manager::save_settings_to_dir(steam_dir, settings)
}

pub fn manifest_status(steam_dir: String) -> Result<manager::ManifestLuaStatus> {
    manager::manifest_lua_status(steam_dir)
}

pub fn install_manifest(steam_dir: String) -> Result<manager::ManifestLuaStatus> {
    let settings = manager::load_settings_from_dir(&steam_dir)?;
    manager::install_manifest_lua(steam_dir, &settings)
}

pub fn remove_manifest(steam_dir: String) -> Result<manager::ManifestLuaStatus> {
    manager::remove_manifest_lua(steam_dir)
}

pub fn probe_manifest(
    steam_dir: String,
    appid: Option<u32>,
) -> Result<manager::ManifestChannelProbe> {
    manager::probe_manifest_channel(steam_dir, appid)
}
