//! Provider CRUD handlers.

use axum::extract::{Path, State};
use axum::Json;
use serde_json::json;

use crate::db::{self, ProviderInput, ProviderRow};
use crate::error::{ApiOk, ApiResult, AppError};
use crate::state::AppState;

pub async fn list(State(state): State<AppState>) -> ApiResult<Vec<ProviderRow>> {
    Ok(ApiOk(db::list_providers(&state.db).await?))
}

pub async fn get(State(state): State<AppState>, Path(id): Path<String>) -> ApiResult<ProviderRow> {
    let provider = db::get_provider(&state.db, &id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("provider '{id}' not found")))?;
    Ok(ApiOk(provider))
}

pub async fn create(
    State(state): State<AppState>,
    Json(input): Json<ProviderInput>,
) -> ApiResult<ProviderRow> {
    Ok(ApiOk(db::insert_provider(&state.db, &input).await?))
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<ProviderInput>,
) -> ApiResult<ProviderRow> {
    let provider = db::update_provider(&state.db, &id, &input).await?;
    // Editing connection details can change which models the key sees.
    state.catalog.invalidate_provider(&id).await;
    Ok(ApiOk(provider))
}

pub async fn remove(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<serde_json::Value> {
    // Automatically remove the provider from opencode.json
    crate::services::apply::unapply(&state, &id).await?;
    db::delete_provider(&state.db, &id).await?;
    state.catalog.invalidate_provider(&id).await;
    Ok(ApiOk(json!({ "deleted": id })))
}
