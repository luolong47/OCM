//! Cache layer.
//!
//! Two caches, sized to how the data actually behaves:
//!   - models.dev: one ~2 MB document for *all* providers, slow-changing → cache daily.
//!   - provider id-list: small, reflects the live key's access → cache briefly.

use std::sync::Arc;

use moka::future::Cache;

use crate::clients::modelsdev::{self, ModelsDevData};
use crate::clients::provider;
use crate::config::Config;
use crate::db;
use crate::error::AppError;

#[derive(Clone)]
pub struct Catalog {
    http: reqwest::Client,
    config: Arc<Config>,
    models_dev: Cache<(), Arc<ModelsDevData>>,
    provider_ids: Cache<String, Arc<Vec<String>>>,
    openrouter_models: Cache<(), Arc<Vec<crate::clients::openrouter::OpenRouterModel>>>,
}

impl Catalog {
    pub fn new(http: reqwest::Client, config: Arc<Config>) -> Self {
        let models_dev = Cache::builder()
            .max_capacity(1)
            .time_to_live(config.models_dev_ttl)
            .build();
        let provider_ids = Cache::builder()
            .max_capacity(256)
            .time_to_live(config.provider_list_ttl)
            .build();
        let openrouter_models = Cache::builder()
            .max_capacity(1)
            .time_to_live(config.models_dev_ttl)
            .build();
        Catalog {
            http,
            config,
            models_dev,
            provider_ids,
            openrouter_models,
        }
    }

    /// models.dev metadata index, loaded from SQLite and cached in memory.
    pub async fn models_dev(&self, db: &sqlx::SqlitePool) -> Result<Arc<ModelsDevData>, AppError> {
        let db = db.clone();
        self.models_dev
            .try_get_with((), async move {
                db::load_models_dev(&db)
                    .await?
                    .map(Arc::new)
                    .ok_or_else(|| {
                        AppError::BadRequest(
                            "models.dev 尚未刷新，请先在服务商列表页点击刷新 models.dev".into(),
                        )
                    })
            })
            .await
            .map_err(unwrap_cache_err)
    }

    pub async fn refresh_models_dev(
        &self,
        db: &sqlx::SqlitePool,
    ) -> Result<Arc<ModelsDevData>, AppError> {
        self.models_dev.invalidate(&()).await;
        let records = modelsdev::fetch_records(&self.http, &self.config.models_dev_url).await?;
        let data = db::save_models_dev(db, &self.config.models_dev_url, &records).await?;
        let data = Arc::new(data);
        self.models_dev.insert((), data.clone()).await;
        Ok(data)
    }

    pub async fn openrouter_models(
        &self,
    ) -> Result<Arc<Vec<crate::clients::openrouter::OpenRouterModel>>, AppError> {
        let http = self.http.clone();
        self.openrouter_models
            .try_get_with((), async move {
                crate::clients::openrouter::fetch_openrouter_models(&http)
                    .await
                    .map(Arc::new)
            })
            .await
            .map_err(unwrap_cache_err)
    }

    /// A provider's live model-id list, cached briefly per provider id.
    pub async fn provider_ids(
        &self,
        provider_id: &str,
        base_url: &str,
        api_key: Option<&str>,
    ) -> Result<Arc<Vec<String>>, AppError> {
        let http = self.http.clone();
        let base_url = base_url.to_string();
        let api_key = api_key.map(str::to_string);
        self.provider_ids
            .try_get_with(provider_id.to_string(), async move {
                provider::fetch_model_ids(&http, &base_url, api_key.as_deref())
                    .await
                    .map(Arc::new)
            })
            .await
            .map_err(unwrap_cache_err)
    }

    pub async fn invalidate_provider(&self, provider_id: &str) {
        self.provider_ids.invalidate(provider_id).await;
    }
}

/// moka returns `Arc<E>` from `try_get_with`; rebuild a fresh owned error that keeps
/// the original variant (and thus the right HTTP status).
fn unwrap_cache_err(err: Arc<AppError>) -> AppError {
    match &*err {
        AppError::Upstream(m) => AppError::Upstream(m.clone()),
        AppError::NotFound(m) => AppError::NotFound(m.clone()),
        AppError::BadRequest(m) => AppError::BadRequest(m.clone()),
        other => AppError::Internal(other.to_string()),
    }
}
