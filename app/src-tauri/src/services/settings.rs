use crate::manager::{self, ManagerSettings, Result};

pub fn load(steam_dir: String) -> Result<ManagerSettings> {
    manager::load_settings_from_dir(steam_dir)
}

pub fn save(steam_dir: String, settings: &ManagerSettings) -> Result<()> {
    manager::save_settings_to_dir(steam_dir, settings)
}
