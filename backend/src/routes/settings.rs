use axum::extract::Json;
use axum::extract::State;
use serde::Deserialize;
use serde::Serialize;

use crate::error::{ApiOk, ApiResult};
use crate::services::{backup, settings};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct AutostartInput {
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct NutstoreSettingsInput {
    pub enabled: bool,
    pub server_url: String,
    pub username: String,
    pub password: String,
    pub remote_dir: String,
}

#[derive(Debug, Serialize)]
pub struct NutstoreSettingsView {
    pub enabled: bool,
    pub server_url: String,
    pub username: String,
    pub password: String,
    pub remote_dir: String,
}

fn to_view(input: settings::NutstoreSettings) -> NutstoreSettingsView {
    NutstoreSettingsView {
        enabled: input.enabled,
        server_url: input.server_url,
        username: input.username,
        password: input.password,
        remote_dir: input.remote_dir,
    }
}

pub async fn get_nutstore(State(state): State<AppState>) -> ApiResult<NutstoreSettingsView> {
    let settings = settings::load(&state)?;
    Ok(ApiOk(to_view(settings.nutstore)))
}

pub async fn set_nutstore(
    State(state): State<AppState>,
    Json(input): Json<NutstoreSettingsInput>,
) -> ApiResult<NutstoreSettingsView> {
    let mut settings_file = settings::load(&state)?;
    settings_file.nutstore = settings::NutstoreSettings {
        enabled: input.enabled,
        server_url: input.server_url.trim().to_string(),
        username: input.username.trim().to_string(),
        password: input.password,
        remote_dir: input.remote_dir.trim().to_string(),
    };
    settings::save(&state, &settings_file)?;
    Ok(ApiOk(to_view(settings_file.nutstore)))
}

pub async fn backup_nutstore(
    State(state): State<AppState>,
) -> ApiResult<backup::NutstoreBackupReport> {
    Ok(ApiOk(backup::backup_to_nutstore(&state).await?))
}

pub async fn restore_nutstore(
    State(state): State<AppState>,
) -> ApiResult<backup::NutstoreRestoreReport> {
    Ok(ApiOk(backup::restore_from_nutstore(&state).await?))
}

use crate::autostart;

pub async fn get_autostart() -> ApiResult<autostart::AutostartStatus> {
    Ok(ApiOk(autostart::status()?))
}

pub async fn set_autostart(
    Json(input): Json<AutostartInput>,
) -> ApiResult<autostart::AutostartStatus> {
    Ok(ApiOk(autostart::set_enabled(input.enabled)?))
}
