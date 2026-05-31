//! Apply handlers — write selected models into opencode.json.

use axum::extract::{Path, State};
use serde_json::Value;

use crate::error::{ApiOk, ApiResult};
use crate::services::apply::{self, ApplyReport};
use crate::state::AppState;

/// GET /providers/{id}/apply/preview — show the block that would be written.
pub async fn preview(State(state): State<AppState>, Path(id): Path<String>) -> ApiResult<Value> {
    Ok(ApiOk(apply::preview(&state, &id).await?))
}

/// POST /providers/{id}/apply — write just this provider into opencode.json.
pub async fn apply_one(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<ApplyReport> {
    Ok(ApiOk(apply::apply(&state, Some(&[id])).await?))
}

/// POST /providers/{id}/unapply — remove just this provider from opencode.json.
pub async fn unapply_one(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<ApplyReport> {
    Ok(ApiOk(apply::unapply(&state, &id).await?))
}
