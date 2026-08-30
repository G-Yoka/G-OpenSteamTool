use crate::manager::{self, Result};

pub fn install(steam_dir: String) -> Result<()> {
    manager::install_bundled_dlls(steam_dir)
}

pub fn remove(steam_dir: String) -> Result<()> {
    manager::remove_dlls_from_dir(steam_dir)
}
