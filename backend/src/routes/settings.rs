use axum::extract::Json;
use serde::Deserialize;

use crate::autostart;
use crate::error::{ApiOk, ApiResult};

#[derive(Debug, Deserialize)]
pub struct AutostartInput {
    enabled: bool,
}

pub async fn get_autostart() -> ApiResult<autostart::AutostartStatus> {
    Ok(ApiOk(autostart::status()?))
}

pub async fn set_autostart(
    Json(input): Json<AutostartInput>,
) -> ApiResult<autostart::AutostartStatus> {
    Ok(ApiOk(autostart::set_enabled(input.enabled)?))
}
