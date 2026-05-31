//! Model catalog + selection handlers.
//!
//! Query params arrive as strings (robust across query encoders) and are parsed into
//! the typed `CatalogQuery` here.

use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::{ApiOk, ApiResult};
use crate::services::catalog::{self, CatalogQuery, CatalogResult};
use crate::services::selection::{self, SelectedUpdate};
use crate::state::AppState;

#[derive(Debug, Default, Deserialize)]
pub struct ModelQueryParams {
    search: Option<String>,
    support_image: Option<String>,
    support_audio: Option<String>,
    support_video: Option<String>,
    tool_call: Option<String>,
    reasoning: Option<String>,
    min_context: Option<String>,
    max_context: Option<String>,
    status: Option<String>,
}

fn parse_bool(s: &Option<String>) -> Option<bool> {
    s.as_deref()
        .map(|v| matches!(v.to_lowercase().as_str(), "true" | "1" | "yes" | "on"))
}
fn parse_u64(s: &Option<String>) -> Option<u64> {
    s.as_deref()
        .filter(|v| !v.is_empty())
        .and_then(|v| v.parse().ok())
}
fn non_empty(s: &Option<String>) -> Option<String> {
    s.clone().filter(|v| !v.is_empty())
}

impl ModelQueryParams {
    fn to_query(&self) -> CatalogQuery {
        CatalogQuery {
            search: non_empty(&self.search),
            support_image: parse_bool(&self.support_image),
            support_audio: parse_bool(&self.support_audio),
            support_video: parse_bool(&self.support_video),
            tool_call: parse_bool(&self.tool_call),
            reasoning: parse_bool(&self.reasoning),
            min_context: parse_u64(&self.min_context),
            max_context: parse_u64(&self.max_context),
            status: non_empty(&self.status),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct ModelIdsBody {
    #[serde(default)]
    model_ids: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct FilterBody {
    #[serde(default)]
    filters: CatalogQuery,
}

#[derive(Debug, Deserialize)]
pub struct ResolveQueryParams {
    model_id: String,
}

/// GET /providers/{id}/models/fetch — full filtered catalog (client virtual-scrolls).
pub async fn fetch(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<ModelQueryParams>,
) -> ApiResult<CatalogResult> {
    let result = catalog::fetch_catalog(&state, &id, &params.to_query()).await?;
    Ok(ApiOk(result))
}

/// GET /providers/{id}/models/resolve?model_id=... — explain metadata matching.
pub async fn resolve(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<ResolveQueryParams>,
) -> ApiResult<catalog::ModelResolution> {
    let model_id = params.model_id.trim();
    if model_id.is_empty() {
        return Err(crate::error::AppError::BadRequest(
            "model_id query param is required".into(),
        ));
    }
    let result = catalog::resolve_provider_model(&state, &id, model_id).await?;
    Ok(ApiOk(result))
}

/// POST /providers/{id}/models/refresh — force re-fetch of the live id list.
pub async fn refresh(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<CatalogResult> {
    state.catalog.invalidate_provider(&id).await;
    let result = catalog::refresh_catalog(&state, &id).await?;
    Ok(ApiOk(result))
}

/// GET /providers/{id}/models/selected — selected models with effective config.
pub async fn selected(State(state): State<AppState>, Path(id): Path<String>) -> ApiResult<Value> {
    let rows = selection::list_selected(&state.db, &id).await?;
    let models: Vec<Value> = rows.iter().map(|r| r.to_response()).collect();
    Ok(ApiOk(json!({ "total": models.len(), "models": models })))
}

/// POST /providers/{id}/models/select — batch select by id.
pub async fn select(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<ModelIdsBody>,
) -> ApiResult<Value> {
    let n = selection::select(&state, &id, &body.model_ids).await?;
    Ok(ApiOk(json!({ "selected": n })))
}

/// POST /providers/{id}/models/deselect — batch deselect by id.
pub async fn deselect(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<ModelIdsBody>,
) -> ApiResult<Value> {
    let n = selection::deselect(&state.db, &id, &body.model_ids).await?;
    Ok(ApiOk(json!({ "deselected": n })))
}

/// POST /providers/{id}/models/select-all-filtered — select the whole filtered set.
pub async fn select_all_filtered(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<FilterBody>,
) -> ApiResult<Value> {
    let n = selection::select_all_filtered(&state, &id, &body.filters).await?;
    Ok(ApiOk(json!({ "selected": n })))
}

/// POST /providers/{id}/models/deselect-all — clear the provider's selection.
pub async fn deselect_all(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Value> {
    let n = selection::deselect_all(&state.db, &id).await?;
    Ok(ApiOk(json!({ "deselected": n })))
}

/// PUT /providers/{id}/selected/{model_id} — update one selected model's overrides.
pub async fn update_model(
    State(state): State<AppState>,
    Path((id, model_id)): Path<(String, String)>,
    Json(update): Json<SelectedUpdate>,
) -> ApiResult<Value> {
    let row = selection::update_selected(&state.db, &id, &model_id, &update).await?;
    Ok(ApiOk(row.to_response()))
}

/// POST /models-dev/refresh — force re-fetch of the models.dev catalog.
pub async fn refresh_models_dev(State(state): State<AppState>) -> ApiResult<Value> {
    let data = state.catalog.refresh_models_dev(&state.db).await?;
    let status = crate::db::models_dev_status(&state.db).await?;
    Ok(ApiOk(json!({
        "indexed_model_count": data.model_count(),
        "status": status
    })))
}

/// GET /models-dev/status — last persisted models.dev refresh metadata.
pub async fn models_dev_status(State(state): State<AppState>) -> ApiResult<Value> {
    Ok(ApiOk(json!(crate::db::models_dev_status(&state.db).await?)))
}
