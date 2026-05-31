//! Database access: pool setup + provider CRUD. Selection/apply queries live in
//! their respective services.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use sqlx::FromRow;

use crate::clients::modelsdev::{
    default_provider_priority, from_records, ModelsDevData, ProviderRecord,
};
use crate::domain::{CatalogModel, ModelEntry};
use crate::error::AppError;

pub async fn connect(database_url: &str) -> Result<SqlitePool, AppError> {
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await?;
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .map_err(|e| AppError::Internal(format!("migration failed: {e}")))?;
    Ok(pool)
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct ProviderRow {
    pub id: String,
    pub name: String,
    pub npm: String,
    pub base_url: Option<String>,
    pub api_key_env: Option<String>,
    pub api_key: Option<String>,
    pub models_dev_key: Option<String>,
    pub headers: Option<String>,
    pub options: Option<String>,
    pub enabled: bool,
    pub is_applied: bool,
    pub needs_reapply: bool,
    pub source: String,
    pub created_at: String,
    pub updated_at: String,
}

impl ProviderRow {
    /// Raw key wins; otherwise resolve from the named env var; else none.
    pub fn resolve_api_key(&self) -> Option<String> {
        if let Some(k) = self.api_key.as_ref().filter(|k| !k.is_empty()) {
            return Some(k.clone());
        }
        self.api_key_env
            .as_ref()
            .and_then(|name| std::env::var(name).ok())
    }
}

#[derive(Debug, Deserialize)]
pub struct ProviderInput {
    pub id: String,
    pub name: String,
    #[serde(default = "default_npm")]
    pub npm: String,
    pub base_url: Option<String>,
    pub api_key_env: Option<String>,
    pub api_key: Option<String>,
    pub models_dev_key: Option<String>,
    pub headers: Option<serde_json::Value>,
    pub options: Option<serde_json::Value>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_npm() -> String {
    "@ai-sdk/openai-compatible".to_string()
}
fn default_true() -> bool {
    true
}

const PROVIDER_COLS: &str = "id, name, npm, base_url, api_key_env, api_key, models_dev_key, \
     headers, options, enabled, is_applied, needs_reapply, source, created_at, updated_at";

pub async fn list_providers(pool: &SqlitePool) -> Result<Vec<ProviderRow>, AppError> {
    let sql = format!("SELECT {PROVIDER_COLS} FROM providers ORDER BY id");
    Ok(sqlx::query_as::<_, ProviderRow>(&sql)
        .fetch_all(pool)
        .await?)
}

pub async fn get_provider(pool: &SqlitePool, id: &str) -> Result<Option<ProviderRow>, AppError> {
    let sql = format!("SELECT {PROVIDER_COLS} FROM providers WHERE id = ?");
    Ok(sqlx::query_as::<_, ProviderRow>(&sql)
        .bind(id)
        .fetch_optional(pool)
        .await?)
}

pub async fn insert_provider(
    pool: &SqlitePool,
    input: &ProviderInput,
) -> Result<ProviderRow, AppError> {
    sqlx::query(
        "INSERT INTO providers \
         (id, name, npm, base_url, api_key_env, api_key, models_dev_key, headers, options, enabled) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&input.id)
    .bind(&input.name)
    .bind(&input.npm)
    .bind(&input.base_url)
    .bind(&input.api_key_env)
    .bind(&input.api_key)
    .bind(&input.models_dev_key)
    .bind(input.headers.as_ref().map(|v| v.to_string()))
    .bind(input.options.as_ref().map(|v| v.to_string()))
    .bind(input.enabled)
    .execute(pool)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db) if db.is_unique_violation() => {
            AppError::BadRequest(format!("provider '{}' already exists", input.id))
        }
        _ => AppError::Db(e),
    })?;

    get_provider(pool, &input.id)
        .await?
        .ok_or_else(|| AppError::Internal("provider vanished after insert".into()))
}

pub async fn update_provider(
    pool: &SqlitePool,
    id: &str,
    input: &ProviderInput,
) -> Result<ProviderRow, AppError> {
    let affected = sqlx::query(
        "UPDATE providers SET name = ?, npm = ?, base_url = ?, api_key_env = ?, api_key = ?, \
         models_dev_key = ?, headers = ?, options = ?, enabled = ?, updated_at = datetime('now') \
         WHERE id = ?",
    )
    .bind(&input.name)
    .bind(&input.npm)
    .bind(&input.base_url)
    .bind(&input.api_key_env)
    .bind(&input.api_key)
    .bind(&input.models_dev_key)
    .bind(input.headers.as_ref().map(|v| v.to_string()))
    .bind(input.options.as_ref().map(|v| v.to_string()))
    .bind(input.enabled)
    .bind(id)
    .execute(pool)
    .await?
    .rows_affected();

    if affected == 0 {
        return Err(AppError::NotFound(format!("provider '{id}' not found")));
    }
    get_provider(pool, id)
        .await?
        .ok_or_else(|| AppError::Internal("provider vanished after update".into()))
}

pub async fn delete_provider(pool: &SqlitePool, id: &str) -> Result<(), AppError> {
    let affected = sqlx::query("DELETE FROM providers WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?
        .rows_affected();
    if affected == 0 {
        return Err(AppError::NotFound(format!("provider '{id}' not found")));
    }
    Ok(())
}

pub async fn set_applied(pool: &SqlitePool, id: &str, applied: bool) -> Result<(), AppError> {
    sqlx::query("UPDATE providers SET is_applied = ?, updated_at = datetime('now') WHERE id = ?")
        .bind(applied)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn set_needs_reapply(pool: &SqlitePool, id: &str, flag: bool) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE providers SET needs_reapply = ?, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(flag)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

#[derive(Debug, Clone, FromRow)]
#[allow(dead_code)]
pub struct ModelLibraryRow {
    pub id: i64,
    pub pattern: String,
    pub name: String,
    pub family: Option<String>,
    pub attachment: bool,
    pub reasoning: bool,
    pub tool_call: bool,
    pub temperature: bool,
    pub context: Option<i64>,
    pub max_output: Option<i64>,
    pub cost_input: Option<f64>,
    pub cost_output: Option<f64>,
}

pub async fn list_model_library(pool: &SqlitePool) -> Result<Vec<ModelLibraryRow>, AppError> {
    Ok(sqlx::query_as::<_, ModelLibraryRow>(
        "SELECT id, pattern, name, family, attachment, reasoning, tool_call, temperature, context, max_output, cost_input, cost_output FROM model_library ORDER BY length(pattern) DESC"
    )
    .fetch_all(pool)
    .await?)
}

#[derive(Debug, Clone, FromRow)]
pub struct CatalogCacheRow {
    pub total_available: i64,
    pub models_json: String,
}

pub async fn get_catalog_cache(
    pool: &SqlitePool,
    provider_id: &str,
) -> Result<Option<CatalogCacheRow>, AppError> {
    Ok(sqlx::query_as::<_, CatalogCacheRow>(
        "SELECT total_available, models_json FROM provider_model_catalog_cache WHERE provider_id = ?",
    )
    .bind(provider_id)
    .fetch_optional(pool)
    .await?)
}

pub async fn upsert_catalog_cache(
    pool: &SqlitePool,
    provider_id: &str,
    total_available: usize,
    models: &[CatalogModel],
) -> Result<(), AppError> {
    let models_json = serde_json::to_string(models)
        .map_err(|e| AppError::Internal(format!("catalog cache serialize failed: {e}")))?;
    sqlx::query(
        "INSERT INTO provider_model_catalog_cache (provider_id, total_available, models_json) \
         VALUES (?, ?, ?) \
         ON CONFLICT(provider_id) DO UPDATE SET \
           total_available = excluded.total_available, \
           models_json = excluded.models_json, \
           refreshed_at = datetime('now'), \
           updated_at = datetime('now')",
    )
    .bind(provider_id)
    .bind(total_available as i64)
    .bind(models_json)
    .execute(pool)
    .await?;
    Ok(())
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct ModelsDevRefreshRow {
    pub source_url: String,
    pub provider_count: i64,
    pub model_count: i64,
    pub refreshed_at: String,
}

pub async fn models_dev_status(pool: &SqlitePool) -> Result<Option<ModelsDevRefreshRow>, AppError> {
    Ok(sqlx::query_as::<_, ModelsDevRefreshRow>(
        "SELECT source_url, provider_count, model_count, refreshed_at FROM models_dev_refresh WHERE id = 1",
    )
    .fetch_optional(pool)
    .await?)
}

pub async fn load_models_dev(pool: &SqlitePool) -> Result<Option<ModelsDevData>, AppError> {
    let provider_rows: Vec<(String, String, i64)> = sqlx::query_as(
        "SELECT provider_key, raw_json, priority FROM models_dev_providers ORDER BY priority, provider_key",
    )
    .fetch_all(pool)
    .await?;

    if provider_rows.is_empty() {
        return Ok(None);
    }

    let mut records = HashMap::new();
    let mut priorities = HashMap::new();
    for (provider_key, raw_json, priority) in provider_rows {
        let record: ProviderRecord = serde_json::from_str(&raw_json).map_err(|e| {
            AppError::Internal(format!(
                "models.dev provider cache parse failed for '{provider_key}': {e}"
            ))
        })?;
        priorities.insert(provider_key.clone(), priority);
        records.insert(provider_key, record);
    }
    Ok(Some(from_records(records, Some(&priorities))))
}

pub async fn save_models_dev(
    pool: &SqlitePool,
    source_url: &str,
    records: &HashMap<String, ProviderRecord>,
) -> Result<ModelsDevData, AppError> {
    let mut tx = pool.begin().await?;

    let existing_priorities: Vec<(String, i64)> =
        sqlx::query_as("SELECT provider_key, priority FROM models_dev_providers")
            .fetch_all(&mut *tx)
            .await?;
    let existing_priorities: HashMap<String, i64> = existing_priorities.into_iter().collect();

    sqlx::query("DELETE FROM models_dev_model_modalities")
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM models_dev_models")
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM models_dev_providers")
        .execute(&mut *tx)
        .await?;

    let mut priorities = HashMap::new();
    let mut provider_count = 0usize;
    let mut model_count = 0usize;

    for (provider_key, record) in records {
        provider_count += 1;
        model_count += record.models.len();
        let priority = existing_priorities
            .get(provider_key)
            .copied()
            .unwrap_or_else(|| default_provider_priority(provider_key));
        priorities.insert(provider_key.clone(), priority);

        let raw_json = serde_json::to_string(record).map_err(|e| {
            AppError::Internal(format!(
                "models.dev provider serialize failed for '{provider_key}': {e}"
            ))
        })?;
        sqlx::query(
            "INSERT INTO models_dev_providers \
             (provider_key, provider_id, name, npm, api_json, doc_json, env_json, priority, raw_json) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(provider_key)
        .bind(&record.id)
        .bind(&record.name)
        .bind(&record.npm)
        .bind(record.api.as_ref().map(|v| v.to_string()))
        .bind(record.doc.as_ref().map(|v| v.to_string()))
        .bind(record.env.as_ref().map(|v| v.to_string()))
        .bind(priority)
        .bind(raw_json)
        .execute(&mut *tx)
        .await?;

        for (model_id, model) in &record.models {
            insert_models_dev_model(&mut tx, provider_key, model_id, model).await?;
        }
    }

    sqlx::query(
        "INSERT INTO models_dev_refresh (id, source_url, provider_count, model_count) \
         VALUES (1, ?, ?, ?) \
         ON CONFLICT(id) DO UPDATE SET \
           source_url = excluded.source_url, \
           provider_count = excluded.provider_count, \
           model_count = excluded.model_count, \
           refreshed_at = datetime('now'), \
           updated_at = datetime('now')",
    )
    .bind(source_url)
    .bind(provider_count as i64)
    .bind(model_count as i64)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(from_records(records.clone(), Some(&priorities)))
}

async fn insert_models_dev_model(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    provider_key: &str,
    model_id: &str,
    model: &ModelEntry,
) -> Result<(), AppError> {
    let raw_json = serde_json::to_string(model).map_err(|e| {
        AppError::Internal(format!(
            "models.dev model serialize failed for '{provider_key}/{model_id}': {e}"
        ))
    })?;
    let interleaved_bool = model.interleaved.as_ref().and_then(|v| v.as_bool());

    sqlx::query(
        "INSERT INTO models_dev_models \
         (provider_key, model_id, name, family, attachment, reasoning, tool_call, temperature, \
          structured_output, open_weights, release_date, last_updated, knowledge, status, \
          limit_context, limit_input, limit_output, cost_input, cost_output, cost_cache_read, \
          cost_cache_write, cost_input_audio, cost_output_audio, cost_reasoning, \
          cost_context_over_200k_json, cost_tiers_json, experimental_json, interleaved_bool, \
          interleaved_json, provider_json, raw_json) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(provider_key)
    .bind(model_id)
    .bind(&model.name)
    .bind(&model.family)
    .bind(model.attachment)
    .bind(model.reasoning)
    .bind(model.tool_call)
    .bind(model.temperature)
    .bind(model.structured_output)
    .bind(model.open_weights)
    .bind(&model.release_date)
    .bind(&model.last_updated)
    .bind(&model.knowledge)
    .bind(&model.status)
    .bind(model.limit.context.map(|v| v as i64))
    .bind(model.limit.input.map(|v| v as i64))
    .bind(model.limit.output.map(|v| v as i64))
    .bind(model.cost.as_ref().and_then(|c| c.input))
    .bind(model.cost.as_ref().and_then(|c| c.output))
    .bind(model.cost.as_ref().and_then(|c| c.cache_read))
    .bind(model.cost.as_ref().and_then(|c| c.cache_write))
    .bind(model.cost.as_ref().and_then(|c| c.input_audio))
    .bind(model.cost.as_ref().and_then(|c| c.output_audio))
    .bind(model.cost.as_ref().and_then(|c| c.reasoning))
    .bind(
        model
            .cost
            .as_ref()
            .and_then(|c| c.context_over_200k.as_ref())
            .map(|v| v.to_string()),
    )
    .bind(
        model
            .cost
            .as_ref()
            .and_then(|c| c.tiers.as_ref())
            .map(|v| v.to_string()),
    )
    .bind(model.experimental.as_ref().map(|v| v.to_string()))
    .bind(interleaved_bool)
    .bind(
        model
            .interleaved
            .as_ref()
            .filter(|v| !v.is_boolean())
            .map(|v| v.to_string()),
    )
    .bind(model.provider.as_ref().map(|v| v.to_string()))
    .bind(raw_json)
    .execute(&mut **tx)
    .await?;

    for modality in &model.modalities.input {
        insert_model_modality(tx, provider_key, model_id, "input", modality).await?;
    }
    for modality in &model.modalities.output {
        insert_model_modality(tx, provider_key, model_id, "output", modality).await?;
    }

    Ok(())
}

async fn insert_model_modality(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    provider_key: &str,
    model_id: &str,
    direction: &str,
    modality: &str,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO models_dev_model_modalities (provider_key, model_id, direction, modality) \
         VALUES (?, ?, ?, ?)",
    )
    .bind(provider_key)
    .bind(model_id)
    .bind(direction)
    .bind(modality)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
