//! Import config handler — load providers and models from opencode.json.

use axum::extract::State;

use crate::error::{ApiOk, ApiResult};
use crate::services::import_config::{self, ImportReport};
use crate::state::AppState;

/// POST /import — import providers and selected models from opencode.json.
pub async fn import_all(State(state): State<AppState>) -> ApiResult<ImportReport> {
    Ok(ApiOk(import_config::import_from_config(&state).await?))
}
