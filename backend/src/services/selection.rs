//! Selection service: which models the user has chosen, plus per-model overrides.
//!
//! On select we snapshot the models.dev entry (so the choice survives metadata drift)
//! and extract a few columns for cheap filtering. Re-selecting refreshes the snapshot
//! but preserves the user's overrides, display name, and enabled flag.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};

use crate::db;
use crate::domain::merge_patch;
use crate::error::AppError;
use crate::services::catalog::{self, CatalogQuery};
use crate::state::AppState;

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct SelectedRow {
    pub provider_id: String,
    pub model_id: String,
    pub display_name: Option<String>,
    pub is_enabled: bool,
    #[serde(skip_serializing)]
    pub snapshot: String,
    pub metadata_known: bool,
    #[serde(skip_serializing)]
    pub override_patch: Option<String>,
    pub context: Option<i64>,
    pub has_image: bool,
    pub tool_call: bool,
    pub selected_at: String,
    pub updated_at: String,
    pub api_snapshot_at: Option<String>,
}

impl SelectedRow {
    /// snapshot with the user's override merge-patch applied.
    pub fn effective(&self) -> serde_json::Value {
        let mut base: serde_json::Value = serde_json::from_str(&self.snapshot)
            .unwrap_or_else(|_| serde_json::json!({ "id": self.model_id }));
        if let Some(patch) = &self.override_patch {
            if let Ok(p) = serde_json::from_str::<serde_json::Value>(patch) {
                merge_patch(&mut base, &p);
            }
        }
        base
    }

    pub fn has_custom_config(&self) -> bool {
        self.override_patch.is_some()
    }

    pub fn to_response(&self) -> serde_json::Value {
        serde_json::json!({
            "model_id": self.model_id,
            "display_name": self.display_name,
            "is_enabled": self.is_enabled,
            "metadata_known": self.metadata_known,
            "has_custom_config": self.has_custom_config(),
            "selected_at": self.selected_at,
            "api_snapshot_at": self.api_snapshot_at,
            "effective": self.effective(),
            "override_patch": self.override_patch.as_ref().and_then(|p| serde_json::from_str::<serde_json::Value>(p).ok()),
        })
    }
}

#[derive(Debug, Deserialize)]
pub struct SelectedUpdate {
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub is_enabled: Option<bool>,
    #[serde(default)]
    pub override_patch: Option<serde_json::Value>,
}

const SELECTED_COLS: &str = "provider_id, model_id, display_name, is_enabled, snapshot, \
     metadata_known, override_patch, context, has_image, tool_call, selected_at, updated_at, \
     api_snapshot_at";

pub async fn selected_ids(
    pool: &SqlitePool,
    provider_id: &str,
) -> Result<HashSet<String>, AppError> {
    let rows: Vec<(String,)> =
        sqlx::query_as("SELECT model_id FROM selected_models WHERE provider_id = ?")
            .bind(provider_id)
            .fetch_all(pool)
            .await?;
    Ok(rows.into_iter().map(|(id,)| id).collect())
}

pub async fn customized_ids(
    pool: &SqlitePool,
    provider_id: &str,
) -> Result<HashSet<String>, AppError> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT model_id FROM selected_models WHERE provider_id = ? AND override_patch IS NOT NULL",
    )
    .bind(provider_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|(id,)| id).collect())
}

pub async fn list_selected(
    pool: &SqlitePool,
    provider_id: &str,
) -> Result<Vec<SelectedRow>, AppError> {
    let sql = format!(
        "SELECT {SELECTED_COLS} FROM selected_models WHERE provider_id = ? ORDER BY model_id"
    );
    Ok(sqlx::query_as::<_, SelectedRow>(&sql)
        .bind(provider_id)
        .fetch_all(pool)
        .await?)
}

/// Snapshot + upsert the given ids. Returns how many rows were written.
pub async fn select(
    state: &AppState,
    provider_id: &str,
    ids: &[String],
) -> Result<usize, AppError> {
    if ids.is_empty() {
        return Ok(0);
    }
    let meta = state.catalog.models_dev(&state.db).await?;
    let provider = db::get_provider(&state.db, provider_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("provider '{provider_id}' not found")))?;
    let scope = provider.models_dev_key.as_deref();

    let local_library = db::list_model_library(&state.db).await?;

    let mut tx = state.db.begin().await?;
    for id in ids {
        let (entry, metadata_known, _) =
            catalog::resolve_model_metadata(state, id, scope, &meta, &local_library).await;

        let snapshot = serde_json::to_string(&entry)?;
        let context = entry.context().map(|c| c as i64);
        let has_image = entry.supports("image");
        let tool_call = entry.tool_call.unwrap_or(false);

        sqlx::query(
            "INSERT INTO selected_models \
             (provider_id, model_id, snapshot, metadata_known, context, has_image, tool_call, api_snapshot_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, datetime('now')) \
             ON CONFLICT(provider_id, model_id) DO UPDATE SET \
               snapshot = excluded.snapshot, metadata_known = excluded.metadata_known, \
               context = excluded.context, has_image = excluded.has_image, \
               tool_call = excluded.tool_call, api_snapshot_at = excluded.api_snapshot_at, \
               updated_at = datetime('now')",
        )
        .bind(provider_id)
        .bind(id)
        .bind(&snapshot)
        .bind(metadata_known)
        .bind(context)
        .bind(has_image)
        .bind(tool_call)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    // Mark provider as needing re-apply if it was already applied.
    if let Ok(Some(p)) = db::get_provider(&state.db, provider_id).await {
        if p.is_applied {
            let _ = db::set_needs_reapply(&state.db, provider_id, true).await;
        }
    }
    Ok(ids.len())
}

pub async fn deselect(
    pool: &SqlitePool,
    provider_id: &str,
    ids: &[String],
) -> Result<usize, AppError> {
    if ids.is_empty() {
        return Ok(0);
    }
    let mut tx = pool.begin().await?;
    let mut removed = 0u64;
    for id in ids {
        removed +=
            sqlx::query("DELETE FROM selected_models WHERE provider_id = ? AND model_id = ?")
                .bind(provider_id)
                .bind(id)
                .execute(&mut *tx)
                .await?
                .rows_affected();
    }
    tx.commit().await?;
    // Mark provider as needing re-apply if it was already applied.
    if let Ok(Some(p)) = db::get_provider(pool, provider_id).await {
        if p.is_applied {
            let _ = db::set_needs_reapply(pool, provider_id, true).await;
        }
    }
    Ok(removed as usize)
}

pub async fn deselect_all(pool: &SqlitePool, provider_id: &str) -> Result<usize, AppError> {
    let removed = sqlx::query("DELETE FROM selected_models WHERE provider_id = ?")
        .bind(provider_id)
        .execute(pool)
        .await?
        .rows_affected();
    // Mark provider as needing re-apply if it was already applied.
    if let Ok(Some(p)) = db::get_provider(pool, provider_id).await {
        if p.is_applied {
            let _ = db::set_needs_reapply(pool, provider_id, true).await;
        }
    }
    Ok(removed as usize)
}

/// Select every id matching the filters — across the whole result set, not one page.
pub async fn select_all_filtered(
    state: &AppState,
    provider_id: &str,
    query: &CatalogQuery,
) -> Result<usize, AppError> {
    let ids = catalog::live_ids(state, provider_id).await?;
    let meta = state.catalog.models_dev(&state.db).await?;
    let provider = db::get_provider(&state.db, provider_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("provider '{provider_id}' not found")))?;

    let local_library = db::list_model_library(&state.db).await?;
    let mut resolved_models = Vec::new();
    for id in &ids {
        let res = catalog::resolve_model_metadata(
            state,
            id,
            provider.models_dev_key.as_deref(),
            &meta,
            &local_library,
        )
        .await;
        resolved_models.push(res);
    }

    let filtered = catalog::filtered_ids(resolved_models, query);
    select(state, provider_id, &filtered).await
}

pub async fn update_selected(
    pool: &SqlitePool,
    provider_id: &str,
    model_id: &str,
    update: &SelectedUpdate,
) -> Result<SelectedRow, AppError> {
    let patch_text = match &update.override_patch {
        Some(v) => Some(serde_json::to_string(v)?),
        None => None,
    };
    let affected = sqlx::query(
        "UPDATE selected_models SET \
           display_name = COALESCE(?, display_name), \
           is_enabled = COALESCE(?, is_enabled), \
           override_patch = COALESCE(?, override_patch), \
           updated_at = datetime('now') \
         WHERE provider_id = ? AND model_id = ?",
    )
    .bind(&update.display_name)
    .bind(update.is_enabled)
    .bind(patch_text)
    .bind(provider_id)
    .bind(model_id)
    .execute(pool)
    .await?
    .rows_affected();

    if affected == 0 {
        return Err(AppError::NotFound(format!(
            "model '{model_id}' is not selected for provider '{provider_id}'"
        )));
    }

    let row = sqlx::query_as::<_, SelectedRow>(&format!(
        "SELECT {SELECTED_COLS} FROM selected_models WHERE provider_id = ? AND model_id = ?"
    ))
    .bind(provider_id)
    .bind(model_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::Internal("selected model vanished after update".into()))?;
    // Mark provider as needing re-apply if it was already applied.
    if let Ok(Some(p)) = db::get_provider(pool, provider_id).await {
        if p.is_applied {
            let _ = db::set_needs_reapply(pool, provider_id, true).await;
        }
    }
    Ok(row)
}
