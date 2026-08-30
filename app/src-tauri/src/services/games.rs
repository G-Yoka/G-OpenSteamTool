use std::path::PathBuf;

use crate::manager::{self, AppMetadata, DepotDiscovery, GameConfig, Result};

pub fn list(steam_dir: String) -> Result<Vec<GameConfig>> {
    manager::list_games_from_dir(steam_dir)
}

pub fn import(steam_dir: String, path: PathBuf) -> Result<GameConfig> {
    manager::import_lua_file_from_path(steam_dir, path)
}

pub fn open_directory(steam_dir: String) -> Result<()> {
    manager::open_lua_dir_for_steam(steam_dir)
}

pub fn upsert(steam_dir: String, game: &GameConfig) -> Result<()> {
    manager::upsert_game_in_dir(steam_dir, game)
}

pub fn delete(steam_dir: String, appid: u32) -> Result<()> {
    manager::delete_game_from_dir(steam_dir, appid)
}

pub fn set_enabled(steam_dir: String, appid: u32, enabled: bool) -> Result<()> {
    manager::set_game_enabled_in_dir(steam_dir, appid, enabled)
}

pub fn metadata(appid: u32) -> Result<AppMetadata> {
    manager::fetch_app_metadata(appid)
}

pub fn discover_depots(steam_dir: String, appid: u32) -> Result<DepotDiscovery> {
    manager::discover_depots_from_steam(steam_dir, appid)
}
