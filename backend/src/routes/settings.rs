use axum::extract::Json;
use serde::Deserialize;

use crate::error::{ApiOk, ApiResult};

#[derive(Debug, Deserialize)]
pub struct AutostartInput {
    pub enabled: bool,
}

#[cfg(target_os = "linux")]
use crate::autostart;

#[cfg(target_os = "linux")]
pub async fn get_autostart() -> ApiResult<autostart::AutostartStatus> {
    Ok(ApiOk(autostart::status()?))
}

#[cfg(target_os = "linux")]
pub async fn set_autostart(
    Json(input): Json<AutostartInput>,
) -> ApiResult<autostart::AutostartStatus> {
    Ok(ApiOk(autostart::set_enabled(input.enabled)?))
}

#[cfg(not(target_os = "linux"))]
pub async fn get_autostart() -> ApiResult<crate::autostart::AutostartStatus> {
    // Return a default status indicating autostart is not supported on this platform
    Ok(ApiOk(crate::autostart::AutostartStatus {
        enabled: false,
        desktop_file: String::new(),
        script_file: String::new(),
        executable: String::new(),
        working_dir: String::new(),
    }))
}

#[cfg(not(target_os = "linux"))]
pub async fn set_autostart(
    _input: Json<AutostartInput>,
) -> ApiResult<crate::autostart::AutostartStatus> {
    // On non-Linux platforms, setting autostart is not supported
    Ok(ApiOk(crate::autostart::AutostartStatus {
        enabled: false,
        desktop_file: String::new(),
        script_file: String::new(),
        executable: String::new(),
        working_dir: String::new(),
    }))
}
