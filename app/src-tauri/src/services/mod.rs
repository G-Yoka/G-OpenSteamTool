//! Application service boundary.
//!
//! Tauri commands call these modules instead of reaching into filesystem and
//! Windows implementation details directly. The legacy `manager` module remains
//! the infrastructure implementation while it is split incrementally.

pub mod dlls;
pub mod games;
pub mod logs;
pub mod settings;
pub mod steam;
