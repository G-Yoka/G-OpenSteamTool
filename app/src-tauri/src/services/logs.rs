use crate::manager::{self, LogFile, Result};

pub fn read(steam_dir: String) -> Result<Vec<LogFile>> {
    manager::read_logs_from_dir(steam_dir)
}
