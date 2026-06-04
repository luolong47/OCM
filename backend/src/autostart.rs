//! Autostart configuration - managed by Tauri plugin in Tauri mode.
//! Note: Frontend accesses this via HTTP API, not Tauri IPC.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct AutostartStatus {
    pub enabled: bool,
    pub message: String,
    // Legacy fields for compatibility with frontend types
    pub desktop_file: String,
    pub script_file: String,
    pub executable: String,
    pub working_dir: String,
}

pub fn status() -> Result<AutostartStatus, crate::error::AppError> {
    // Autostart is managed by Tauri plugin at the application level
    // This HTTP API is kept for compatibility but delegates to Tauri
    Ok(AutostartStatus {
        enabled: false,
        message: "Autostart is managed by the Tauri system tray menu".to_string(),
        desktop_file: String::new(),
        script_file: String::new(),
        executable: String::new(),
        working_dir: String::new(),
    })
}

pub fn set_enabled(_enabled: bool) -> Result<AutostartStatus, crate::error::AppError> {
    // Autostart is managed by Tauri plugin at the application level
    // This HTTP API is kept for compatibility but delegates to Tauri
    Ok(AutostartStatus {
        enabled: false,
        message: "Autostart is managed by the Tauri system tray menu".to_string(),
        desktop_file: String::new(),
        script_file: String::new(),
        executable: String::new(),
        working_dir: String::new(),
    })
}
