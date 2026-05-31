pub mod apply;
pub mod import;
pub mod models;
pub mod providers;
pub mod settings;

use axum::routing::{get, post, put};
use axum::Router;

use crate::error::{ApiOk, ApiResult};
use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    // All API routes live under /api so the root can serve the embedded frontend SPA.
    let api = Router::new()
        .route("/health", get(health))
        .route("/providers", get(providers::list).post(providers::create))
        .route(
            "/providers/{id}",
            get(providers::get)
                .put(providers::update)
                .delete(providers::remove),
        )
        .route("/providers/{id}/models/fetch", get(models::fetch))
        .route("/providers/{id}/models/resolve", get(models::resolve))
        .route("/providers/{id}/models/refresh", post(models::refresh))
        .route("/providers/{id}/models/selected", get(models::selected))
        .route("/providers/{id}/models/select", post(models::select))
        .route("/providers/{id}/models/deselect", post(models::deselect))
        .route(
            "/providers/{id}/models/select-all-filtered",
            post(models::select_all_filtered),
        )
        .route(
            "/providers/{id}/models/deselect-all",
            post(models::deselect_all),
        )
        .route(
            "/providers/{id}/selected/{model_id}",
            put(models::update_model),
        )
        .route("/providers/{id}/apply", post(apply::apply_one))
        .route("/providers/{id}/unapply", post(apply::unapply_one))
        .route("/providers/{id}/apply/preview", get(apply::preview))
        .route("/import", post(import::import_all))
        .route(
            "/settings/autostart",
            get(settings::get_autostart).put(settings::set_autostart),
        )
        .route("/models-dev/status", get(models::models_dev_status))
        .route("/models-dev/refresh", post(models::refresh_models_dev))
        .with_state(state);

    Router::new()
        .nest("/api", api)
        .fallback(super::embedded::handler)
}

async fn health() -> ApiResult<serde_json::Value> {
    Ok(ApiOk(
        serde_json::json!({ "status": "ok", "service": "ocm-backend" }),
    ))
}
