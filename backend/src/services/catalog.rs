//! Catalog service — the corrected core data spine.
//!
//! A provider's catalog = its live `/v1/models` ids (ground truth of what the key can
//! call) *enriched by* models.dev metadata (joined by id), with OCM selection state
//! layered on top. `build_catalog` is pure and unit-tested; `fetch_catalog` wires it
//! to the live id-list, the models.dev cache, and the DB.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::clients::modelsdev::{ModelsDevData, ModelsDevLookup, ModelsDevOrigin};
use crate::db;
use crate::domain::{CatalogModel, ModelEntry, ModelMeta};
use crate::error::AppError;
use crate::services::selection;
use crate::state::AppState;

/// Filter parameters, parsed from query string on the `fetch` route.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct CatalogQuery {
    pub search: Option<String>,
    pub support_image: Option<bool>,
    pub support_audio: Option<bool>,
    pub support_video: Option<bool>,
    pub tool_call: Option<bool>,
    pub reasoning: Option<bool>,
    pub min_context: Option<u64>,
    pub max_context: Option<u64>,
    pub status: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CatalogResult {
    /// Total ids the provider key exposes (before filtering).
    pub total_available: usize,
    /// How many matched the filters.
    pub matched: usize,
    pub models: Vec<CatalogModel>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelMatchDetail {
    pub source: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidate: Option<String>,
    pub strategy: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelResolution {
    pub provider_id: String,
    pub id: String,
    pub available: bool,
    pub metadata_known: bool,
    pub source: &'static str,
    pub match_detail: ModelMatchDetail,
    pub entry: ModelEntry,
}

struct ResolvedMetadata {
    entry: ModelEntry,
    metadata_known: bool,
    source: &'static str,
    match_detail: ModelMatchDetail,
}

#[derive(Debug, Deserialize)]
struct CachedCatalogModel {
    #[serde(flatten)]
    entry: ModelEntry,
    #[serde(rename = "_meta")]
    meta: CachedModelMeta,
}

#[derive(Debug, Deserialize)]
struct CachedModelMeta {
    metadata_known: bool,
    source: String,
}

pub fn generate_candidates(id: &str) -> Vec<String> {
    let mut candidates = Vec::new();

    let mut add = |s: String| {
        if s.len() >= 3 && !candidates.contains(&s) {
            candidates.push(s);
        }
    };

    add(id.to_string());
    add(id.to_lowercase());

    let base = id.split('/').last().unwrap_or(id).to_string();
    add(base.clone());
    add(base.to_lowercase());

    // 1. Right-stripped from base
    let r_stripped = right_stripped(&base);
    for c in &r_stripped {
        add(c.clone());
        add(c.to_lowercase());
    }

    // 2. Left-stripped from base
    let l_stripped = left_stripped(&base);
    for c in &l_stripped {
        add(c.clone());
        add(c.to_lowercase());
    }

    // 3. Cross-combination
    for left in &l_stripped {
        for right in right_stripped(left) {
            add(right.clone());
            add(right.to_lowercase());
        }
    }

    candidates
}

fn right_stripped(s: &str) -> Vec<String> {
    let mut res = Vec::new();
    let mut curr = s.to_string();
    while let Some(pos) = curr.rfind(|c| c == '-' || c == '_' || c == '.') {
        curr = curr[..pos].to_string();
        if curr.len() >= 3 {
            res.push(curr.clone());
        } else {
            break;
        }
    }
    res
}

fn left_stripped(s: &str) -> Vec<String> {
    let mut res = Vec::new();
    let mut curr = s.to_string();
    while let Some(pos) = curr.find(|c| c == '-' || c == '_' || c == '.') {
        curr = curr[pos + 1..].to_string();
        if curr.len() >= 3 {
            res.push(curr.clone());
        } else {
            break;
        }
    }
    res
}

fn match_candidate_ids(c: &str, other_id: &str) -> bool {
    candidate_match_score(c, other_id).is_some()
}

fn candidate_match_score(c: &str, other_id: &str) -> Option<u8> {
    let c = c.to_lowercase();
    let other_lower = other_id.to_lowercase();
    let other_base = other_lower.split('/').last().unwrap_or(&other_lower);

    if c == other_lower || c == other_base {
        return Some(0);
    }

    let other_candidates = generate_candidates(other_base);
    if other_candidates.contains(&c.to_string()) {
        return Some(1);
    }

    None
}

fn modelsdev_provider_rank(provider: &str) -> usize {
    match provider {
        // OpenRouter entries tend to have the most complete normalized metadata for
        // cross-provider IDs exposed by OpenAI-compatible proxy providers.
        "openrouter" => 0,
        "xai" => 10,
        "vercel" => 20,
        "kilo" => 30,
        "302ai" => 40,
        "venice" => 50,
        // POE is kept last; its models.dev metadata is explicitly lower quality.
        "poe" => 1000,
        _ => 100,
    }
}

fn lookup_provider_rank(found: &ModelsDevLookup<'_>) -> usize {
    found
        .origin
        .as_ref()
        .map(|origin| modelsdev_provider_rank(&origin.provider))
        .unwrap_or(usize::MAX)
}

fn lookup_model_id(found: &ModelsDevLookup<'_>) -> String {
    found
        .origin
        .as_ref()
        .map(|origin| origin.model_id.clone())
        .unwrap_or_else(|| found.entry.id.clone())
}

fn lookup_modelsdev_by_candidate_with_origin<'a>(
    models_dev: &'a ModelsDevData,
    candidate: &str,
    scope: Option<&str>,
) -> Option<ModelsDevLookup<'a>> {
    let mut matches: Vec<(u8, ModelsDevLookup<'a>)> = Vec::new();

    if let Some(key) = scope {
        if let Some(provider_models) = models_dev.by_provider.get(key) {
            for (model_id, found) in provider_models {
                if let Some(score) = candidate_match_score(candidate, &found.id) {
                    matches.push((
                        score,
                        ModelsDevLookup {
                            entry: found,
                            origin: Some(ModelsDevOrigin {
                                provider: key.to_string(),
                                model_id: model_id.clone(),
                            }),
                        },
                    ));
                }
            }
        }
    } else {
        for (key, provider_models) in &models_dev.by_provider {
            for (model_id, found) in provider_models {
                if let Some(score) = candidate_match_score(candidate, &found.id) {
                    matches.push((
                        score,
                        ModelsDevLookup {
                            entry: found,
                            origin: Some(ModelsDevOrigin {
                                provider: key.clone(),
                                model_id: model_id.clone(),
                            }),
                        },
                    ));
                }
            }
        }
    }

    matches.sort_by(|(score_a, found_a), (score_b, found_b)| {
        score_a
            .cmp(score_b)
            .then_with(|| lookup_provider_rank(found_a).cmp(&lookup_provider_rank(found_b)))
            .then_with(|| lookup_model_id(found_a).cmp(&lookup_model_id(found_b)))
    });
    matches.into_iter().next().map(|(_, found)| found)
}

pub async fn resolve_model_metadata(
    state: &AppState,
    id: &str,
    scope: Option<&str>,
    models_dev: &ModelsDevData,
    local_library: &[db::ModelLibraryRow],
) -> (ModelEntry, bool, &'static str) {
    let resolved = resolve_model_metadata_detail(state, id, scope, models_dev, local_library).await;
    (resolved.entry, resolved.metadata_known, resolved.source)
}

async fn resolve_model_metadata_detail(
    state: &AppState,
    id: &str,
    scope: Option<&str>,
    models_dev: &ModelsDevData,
    local_library: &[db::ModelLibraryRow],
) -> ResolvedMetadata {
    let candidates = generate_candidates(id);

    // Candidate-by-candidate lookup (longest/most specific candidate first)
    for (i, candidate) in candidates.iter().enumerate() {
        // 1. Try local pattern match
        for row in local_library {
            if candidate.contains(&row.pattern.to_lowercase()) {
                let mut entry = ModelEntry::bare(id);
                entry.name = Some(row.name.clone());
                entry.family = row.family.clone();
                entry.attachment = Some(row.attachment);
                entry.reasoning = Some(row.reasoning);
                entry.tool_call = Some(row.tool_call);
                entry.temperature = Some(row.temperature);

                let mut modalities = crate::domain::model::Modalities::default();
                if row.attachment {
                    modalities.input = vec!["text".into(), "image".into()];
                } else {
                    modalities.input = vec!["text".into()];
                }
                modalities.output = vec!["text".into()];
                entry.modalities = modalities;

                entry.limit = crate::domain::model::Limit {
                    context: row.context.map(|c| c as u64),
                    output: row.max_output.map(|c| c as u64),
                    ..Default::default()
                };

                if row.cost_input.is_some() || row.cost_output.is_some() {
                    entry.cost = Some(crate::domain::model::Cost {
                        input: row.cost_input,
                        output: row.cost_output,
                        ..Default::default()
                    });
                }

                return ResolvedMetadata {
                    entry,
                    metadata_known: true,
                    source: "local-library",
                    match_detail: ModelMatchDetail {
                        source: "local-library",
                        provider: None,
                        model_id: None,
                        candidate: Some(candidate.clone()),
                        strategy: "local-pattern",
                        pattern: Some(row.pattern.clone()),
                    },
                };
            }
        }

        // 2. Try models.dev cache lookup
        if let Some(known) = models_dev.lookup_with_origin(candidate, scope) {
            let mut entry = known.entry.clone();
            entry.id = id.to_string();
            return ResolvedMetadata {
                entry,
                metadata_known: true,
                source: "models.dev",
                match_detail: modelsdev_detail(known.origin, candidate, "models.dev-exact-id"),
            };
        }
        if let Some(known) = lookup_modelsdev_by_candidate_with_origin(models_dev, candidate, scope)
        {
            let mut entry = known.entry.clone();
            entry.id = id.to_string();
            return ResolvedMetadata {
                entry,
                metadata_known: true,
                source: "models.dev",
                match_detail: modelsdev_detail(known.origin, candidate, "models.dev-candidate-id"),
            };
        }
        if let Some(known) = models_dev.lookup_by_family_with_origin(candidate, scope) {
            let mut entry = known.entry.clone();
            entry.id = id.to_string();
            return ResolvedMetadata {
                entry,
                metadata_known: true,
                source: "models.dev",
                match_detail: modelsdev_detail(known.origin, candidate, "models.dev-family"),
            };
        }

        // 3. Try OpenRouter API lookup
        if let Ok(openrouter_list) = state.catalog.openrouter_models().await {
            let allow_substring = i == 0;
            if let Some(or_model) = match_openrouter(candidate, &openrouter_list, allow_substring) {
                let mut entry = ModelEntry::bare(id);
                entry.name = Some(or_model.name.clone());

                let family = or_model.id.split('/').next().map(|s| s.to_string());
                entry.family = family;

                let has_image = or_model
                    .architecture
                    .input_modalities
                    .iter()
                    .any(|m| m == "image");
                entry.attachment = Some(has_image);

                let mut modalities = crate::domain::model::Modalities::default();
                modalities.input = or_model.architecture.input_modalities.clone();
                modalities.output = vec!["text".into()];
                entry.modalities = modalities;

                let has_tools = or_model.supported_parameters.iter().any(|p| p == "tools");
                entry.tool_call = Some(has_tools);

                let has_reasoning = or_model
                    .supported_parameters
                    .iter()
                    .any(|p| p == "reasoning" || p == "include_reasoning");
                entry.reasoning = Some(has_reasoning);

                entry.limit = crate::domain::model::Limit {
                    context: Some(or_model.context_length),
                    ..Default::default()
                };

                let cost_in = or_model
                    .pricing
                    .prompt
                    .parse::<f64>()
                    .ok()
                    .map(|c| c * 1_000_000.0);
                let cost_out = or_model
                    .pricing
                    .completion
                    .parse::<f64>()
                    .ok()
                    .map(|c| c * 1_000_000.0);
                let cache_read = or_model
                    .pricing
                    .input_cache_read
                    .as_ref()
                    .and_then(|s| s.parse::<f64>().ok())
                    .map(|c| c * 1_000_000.0);
                let cache_write = or_model
                    .pricing
                    .input_cache_write
                    .as_ref()
                    .and_then(|s| s.parse::<f64>().ok())
                    .map(|c| c * 1_000_000.0);

                if cost_in.is_some() || cost_out.is_some() {
                    entry.cost = Some(crate::domain::model::Cost {
                        input: cost_in,
                        output: cost_out,
                        cache_read,
                        cache_write,
                        ..Default::default()
                    });
                }

                return ResolvedMetadata {
                    entry,
                    metadata_known: true,
                    source: "openrouter",
                    match_detail: ModelMatchDetail {
                        source: "openrouter",
                        provider: Some("openrouter".to_string()),
                        model_id: Some(or_model.id.clone()),
                        candidate: Some(candidate.clone()),
                        strategy: if allow_substring {
                            "openrouter-candidate-or-substring"
                        } else {
                            "openrouter-candidate"
                        },
                        pattern: None,
                    },
                };
            }
        }
    }

    // 4. Bare fallback
    ResolvedMetadata {
        entry: ModelEntry::bare(id),
        metadata_known: false,
        source: "provider-only",
        match_detail: ModelMatchDetail {
            source: "provider-only",
            provider: None,
            model_id: None,
            candidate: None,
            strategy: "provider-only",
            pattern: None,
        },
    }
}

fn modelsdev_detail(
    origin: Option<ModelsDevOrigin>,
    candidate: &str,
    strategy: &'static str,
) -> ModelMatchDetail {
    ModelMatchDetail {
        source: "models.dev",
        provider: origin.as_ref().map(|o| o.provider.clone()),
        model_id: origin.map(|o| o.model_id),
        candidate: Some(candidate.to_string()),
        strategy,
        pattern: None,
    }
}

fn match_openrouter<'a>(
    our_id: &str,
    openrouter_models: &'a [crate::clients::openrouter::OpenRouterModel],
    allow_substring: bool,
) -> Option<&'a crate::clients::openrouter::OpenRouterModel> {
    let our_lower = our_id.to_lowercase();
    let our_base = our_lower.split('/').last().unwrap_or(&our_lower);

    // 1. Try exact match on ID
    if let Some(m) = openrouter_models
        .iter()
        .find(|m| m.id.to_lowercase() == our_lower)
    {
        return Some(m);
    }

    // 2. Try match on base name (last slash part)
    if let Some(m) = openrouter_models.iter().find(|m| {
        let m_lower = m.id.to_lowercase();
        let m_base = m_lower.split('/').last().unwrap_or(&m_lower);
        m_base == our_base
    }) {
        return Some(m);
    }

    // 3. Try cross-candidate match
    if let Some(m) = openrouter_models
        .iter()
        .find(|m| match_candidate_ids(our_base, &m.id))
    {
        return Some(m);
    }

    // 4. Try substring match (if our_id is inside openrouter_id or vice versa)
    if allow_substring {
        if let Some(m) = openrouter_models.iter().find(|m| {
            let m_lower = m.id.to_lowercase();
            m_lower.contains(our_base) || our_base.contains(&m_lower)
        }) {
            return Some(m);
        }
    }

    None
}

/// Pure join + filter. No I/O — fully testable.
pub fn build_catalog(
    resolved_models: Vec<(ModelEntry, bool, &'static str)>,
    selected: &HashSet<String>,
    customized: &HashSet<String>,
    query: &CatalogQuery,
) -> Vec<CatalogModel> {
    let mut out: Vec<CatalogModel> = resolved_models
        .into_iter()
        .filter_map(|(entry, metadata_known, source)| {
            if !passes(&entry, metadata_known, query) {
                return None;
            }

            let id = entry.id.clone();
            Some(CatalogModel {
                meta: ModelMeta {
                    is_selected: selected.contains(&id),
                    has_custom_config: customized.contains(&id),
                    metadata_known,
                    source,
                },
                entry,
            })
        })
        .collect();

    // Selected models first, then alphabetical — stable, predictable for the UI.
    out.sort_by(|a, b| {
        b.meta
            .is_selected
            .cmp(&a.meta.is_selected)
            .then_with(|| a.entry.id.cmp(&b.entry.id))
    });
    out
}

/// Whether a model passes the active filters.
fn passes(entry: &ModelEntry, metadata_known: bool, q: &CatalogQuery) -> bool {
    if let Some(term) = &q.search {
        let term = term.to_lowercase();
        let id_match = entry.id.to_lowercase().contains(&term);
        let name_match = entry
            .name
            .as_deref()
            .map(|n| n.to_lowercase().contains(&term))
            .unwrap_or(false);
        if !id_match && !name_match {
            return false;
        }
    }

    let needs_meta = q.support_image.is_some()
        || q.support_audio.is_some()
        || q.support_video.is_some()
        || q.tool_call.is_some()
        || q.reasoning.is_some()
        || q.min_context.is_some()
        || q.max_context.is_some()
        || q.status.is_some();
    if needs_meta && !metadata_known {
        return false;
    }

    if let Some(want) = q.support_image {
        if entry.supports("image") != want {
            return false;
        }
    }
    if let Some(want) = q.support_audio {
        if entry.supports("audio") != want {
            return false;
        }
    }
    if let Some(want) = q.support_video {
        if entry.supports("video") != want {
            return false;
        }
    }
    if let Some(want) = q.tool_call {
        if entry.tool_call.unwrap_or(false) != want {
            return false;
        }
    }
    if let Some(want) = q.reasoning {
        if entry.reasoning.unwrap_or(false) != want {
            return false;
        }
    }
    if let Some(min) = q.min_context {
        if entry.context().unwrap_or(0) < min {
            return false;
        }
    }
    if let Some(max) = q.max_context {
        if entry.context().unwrap_or(u64::MAX) > max {
            return false;
        }
    }
    if let Some(status) = &q.status {
        if entry.status.as_deref() != Some(status.as_str()) {
            return false;
        }
    }
    true
}

/// Resolve filtered ids only — used by `select-all-filtered` so selection spans the
/// whole result set, not one page.
pub fn filtered_ids(
    resolved_models: Vec<(ModelEntry, bool, &'static str)>,
    query: &CatalogQuery,
) -> Vec<String> {
    let empty = HashSet::new();
    build_catalog(resolved_models, &empty, &empty, query)
        .into_iter()
        .map(|m| m.entry.id)
        .collect()
}

/// Live orchestration: provider id-list × models.dev × DB selection state.
pub async fn fetch_catalog(
    state: &AppState,
    provider_id: &str,
    query: &CatalogQuery,
) -> Result<CatalogResult, AppError> {
    let Some(cache) = db::get_catalog_cache(&state.db, provider_id).await? else {
        return Ok(CatalogResult {
            total_available: 0,
            matched: 0,
            models: Vec::new(),
        });
    };
    let resolved_models: Vec<(ModelEntry, bool, &'static str)> =
        serde_json::from_str::<Vec<CachedCatalogModel>>(&cache.models_json)
            .map_err(|e| AppError::Internal(format!("catalog cache parse failed: {e}")))?
            .into_iter()
            .map(|model| {
                let metadata_known = model.meta.metadata_known;
                let source = cached_source(&model.meta.source);
                let entry = model.entry;
                (entry, metadata_known, source)
            })
            .collect();
    let models = build_catalog(
        resolved_models,
        &selection::selected_ids(&state.db, provider_id).await?,
        &selection::customized_ids(&state.db, provider_id).await?,
        query,
    );
    Ok(CatalogResult {
        total_available: cache.total_available.max(0) as usize,
        matched: models.len(),
        models,
    })
}

fn cached_source(source: &str) -> &'static str {
    match source {
        "models.dev" => "models.dev",
        "local-library" => "local-library",
        "openrouter" => "openrouter",
        _ => "provider-only",
    }
}

pub async fn refresh_catalog(
    state: &AppState,
    provider_id: &str,
) -> Result<CatalogResult, AppError> {
    let ids = live_ids(state, provider_id).await?;
    let meta = state.catalog.models_dev(&state.db).await?;
    let provider = db::get_provider(&state.db, provider_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("provider '{provider_id}' not found")))?;

    let selected = selection::selected_ids(&state.db, provider_id).await?;
    let customized = selection::customized_ids(&state.db, provider_id).await?;

    let local_library = db::list_model_library(&state.db).await?;
    let mut resolved_models = Vec::new();
    for id in &ids {
        let res = resolve_model_metadata(
            state,
            id,
            provider.models_dev_key.as_deref(),
            &meta,
            &local_library,
        )
        .await;
        resolved_models.push(res);
    }

    let models = build_catalog(
        resolved_models,
        &selected,
        &customized,
        &CatalogQuery::default(),
    );

    if ids.len() > 4000 {
        tracing::info!(
            provider = provider_id,
            total = ids.len(),
            matched = models.len(),
            "large provider catalog (no results dropped; client virtual-scrolls)"
        );
    }

    Ok(CatalogResult {
        total_available: ids.len(),
        matched: models.len(),
        models,
    }
    .tap_save(state, provider_id)
    .await?)
}

trait SaveCatalogResult {
    async fn tap_save(self, state: &AppState, provider_id: &str) -> Result<Self, AppError>
    where
        Self: Sized;
}

impl SaveCatalogResult for CatalogResult {
    async fn tap_save(self, state: &AppState, provider_id: &str) -> Result<Self, AppError> {
        db::upsert_catalog_cache(&state.db, provider_id, self.total_available, &self.models)
            .await?;
        Ok(self)
    }
}

pub async fn resolve_provider_model(
    state: &AppState,
    provider_id: &str,
    model_id: &str,
) -> Result<ModelResolution, AppError> {
    let provider = db::get_provider(&state.db, provider_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("provider '{provider_id}' not found")))?;
    let base_url = provider
        .base_url
        .clone()
        .ok_or_else(|| AppError::BadRequest(format!("provider '{provider_id}' has no base_url")))?;
    let api_key = provider.resolve_api_key();
    let ids = state
        .catalog
        .provider_ids(provider_id, &base_url, api_key.as_deref())
        .await?;

    let meta = state.catalog.models_dev(&state.db).await?;
    let local_library = db::list_model_library(&state.db).await?;
    let resolved = resolve_model_metadata_detail(
        state,
        model_id,
        provider.models_dev_key.as_deref(),
        &meta,
        &local_library,
    )
    .await;

    Ok(ModelResolution {
        provider_id: provider_id.to_string(),
        id: model_id.to_string(),
        available: ids.iter().any(|id| id == model_id),
        metadata_known: resolved.metadata_known,
        source: resolved.source,
        match_detail: resolved.match_detail,
        entry: resolved.entry,
    })
}

/// Fetch a provider's live model ids (via cache), resolving its api key.
pub async fn live_ids(state: &AppState, provider_id: &str) -> Result<Vec<String>, AppError> {
    let provider = db::get_provider(&state.db, provider_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("provider '{provider_id}' not found")))?;
    let base_url = provider
        .base_url
        .clone()
        .ok_or_else(|| AppError::BadRequest(format!("provider '{provider_id}' has no base_url")))?;
    let api_key = provider.resolve_api_key();

    let ids = state
        .catalog
        .provider_ids(provider_id, &base_url, api_key.as_deref())
        .await?;
    Ok((*ids).clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::{Limit, Modalities};
    use std::collections::HashMap;

    fn meta_with(models: Vec<ModelEntry>) -> ModelsDevData {
        let mut data = ModelsDevData::default();
        for m in models {
            data.by_id.insert(m.id.clone(), m);
        }
        data
    }

    fn lookup_meta<'a>(meta: &'a ModelsDevData, id: &str) -> Option<&'a ModelEntry> {
        meta.lookup_with_origin(id, None).map(|found| found.entry)
    }

    fn lookup_family<'a>(meta: &'a ModelsDevData, family: &str) -> Option<&'a ModelEntry> {
        meta.lookup_by_family_with_origin(family, None)
            .map(|found| found.entry)
    }

    fn entry(id: &str, ctx: u64, img: bool, tools: bool) -> ModelEntry {
        ModelEntry {
            id: id.into(),
            name: Some(id.into()),
            tool_call: Some(tools),
            modalities: Modalities {
                input: if img {
                    vec!["text".into(), "image".into()]
                } else {
                    vec!["text".into()]
                },
                output: vec!["text".into()],
            },
            limit: Limit {
                context: Some(ctx),
                ..Default::default()
            },
            status: Some("active".into()),
            ..Default::default()
        }
    }

    #[test]
    fn joins_and_marks_provider_only_ids() {
        let meta = meta_with(vec![entry("gpt-4o", 128_000, true, true)]);
        let ids = vec!["gpt-4o".to_string(), "mystery-model".to_string()];

        let mut resolved = Vec::new();
        for id in &ids {
            let known = lookup_meta(&meta, id);
            let entry = known.cloned().unwrap_or_else(|| ModelEntry::bare(id));
            resolved.push((
                entry,
                known.is_some(),
                if known.is_some() {
                    "models.dev"
                } else {
                    "provider-only"
                },
            ));
        }

        let sel = HashSet::new();
        let cust = HashSet::new();

        let out = build_catalog(resolved, &sel, &cust, &CatalogQuery::default());
        assert_eq!(out.len(), 2);
        let mystery = out.iter().find(|m| m.entry.id == "mystery-model").unwrap();
        assert!(!mystery.meta.metadata_known);
        assert_eq!(mystery.meta.source, "provider-only");
        let known = out.iter().find(|m| m.entry.id == "gpt-4o").unwrap();
        assert!(known.meta.metadata_known);
        assert_eq!(known.entry.context(), Some(128_000));
    }

    #[test]
    fn modelsdev_id_lookup_ignores_ascii_case() {
        let meta = meta_with(vec![entry(
            "stepfun-ai/step-3.7-flash",
            128_000,
            false,
            true,
        )]);

        let found = lookup_meta(&meta, "stepfun-ai/Step-3.7-Flash").unwrap();

        assert_eq!(found.id, "stepfun-ai/step-3.7-flash");
        assert_eq!(found.context(), Some(128_000));
    }

    #[test]
    fn image_and_context_filters_exclude_unknown_metadata() {
        let meta = meta_with(vec![
            entry("vision-big", 200_000, true, true),
            entry("text-small", 8_000, false, true),
        ]);
        let ids = vec![
            "vision-big".to_string(),
            "text-small".to_string(),
            "mystery".to_string(),
        ];

        let mut resolved = Vec::new();
        for id in &ids {
            let known = lookup_meta(&meta, id);
            let entry = known.cloned().unwrap_or_else(|| ModelEntry::bare(id));
            resolved.push((
                entry,
                known.is_some(),
                if known.is_some() {
                    "models.dev"
                } else {
                    "provider-only"
                },
            ));
        }

        let (sel, cust) = (HashSet::new(), HashSet::new());

        let q = CatalogQuery {
            support_image: Some(true),
            min_context: Some(128_000),
            ..Default::default()
        };
        let out = build_catalog(resolved, &sel, &cust, &q);
        let names: Vec<_> = out.iter().map(|m| m.entry.id.as_str()).collect();
        assert_eq!(names, vec!["vision-big"]);
    }

    #[test]
    fn search_matches_provider_only_ids() {
        let meta = meta_with(vec![]);
        let ids = vec!["grok-4-fast".to_string(), "claude".to_string()];

        let mut resolved = Vec::new();
        for id in &ids {
            let known = lookup_meta(&meta, id);
            let entry = known.cloned().unwrap_or_else(|| ModelEntry::bare(id));
            resolved.push((
                entry,
                known.is_some(),
                if known.is_some() {
                    "models.dev"
                } else {
                    "provider-only"
                },
            ));
        }

        let (sel, cust) = (HashSet::new(), HashSet::new());
        let q = CatalogQuery {
            search: Some("GROK".into()),
            ..Default::default()
        };
        let out = build_catalog(resolved, &sel, &cust, &q);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].entry.id, "grok-4-fast");
    }

    #[test]
    fn selected_sort_first() {
        let meta = meta_with(vec![
            entry("aaa", 1000, false, false),
            entry("zzz", 1000, false, false),
        ]);
        let ids = vec!["aaa".to_string(), "zzz".to_string()];

        let mut resolved = Vec::new();
        for id in &ids {
            let known = lookup_meta(&meta, id);
            let entry = known.cloned().unwrap_or_else(|| ModelEntry::bare(id));
            resolved.push((
                entry,
                known.is_some(),
                if known.is_some() {
                    "models.dev"
                } else {
                    "provider-only"
                },
            ));
        }

        let sel: HashSet<String> = ["zzz".to_string()].into_iter().collect();
        let cust = HashSet::new();
        let out = build_catalog(resolved, &sel, &cust, &CatalogQuery::default());
        assert_eq!(out[0].entry.id, "zzz");
        assert!(out[0].meta.is_selected);
    }

    #[test]
    fn test_candidate_generation() {
        let candidates = generate_candidates("grok-4.20-0309-non-reasoning-console");
        assert!(candidates.contains(&"grok-4.20-0309-non-reasoning".to_string()));
        assert!(candidates.contains(&"grok".to_string()));
        assert!(candidates.contains(&"grok-4.20-0309-non-reasoning-console".to_string()));

        let candidates_with_slash =
            generate_candidates("x-ai/grok-4.20-0309-non-reasoning-console");
        assert!(candidates_with_slash.contains(&"grok-4.20-0309-non-reasoning".to_string()));
        assert!(candidates_with_slash.contains(&"grok".to_string()));
    }

    #[test]
    fn test_match_openrouter_candidates() {
        let openrouter_models = vec![crate::clients::openrouter::OpenRouterModel {
            id: "x-ai/grok-4.20-0309-non-reasoning".to_string(),
            name: "Grok 4.20 (Non-Reasoning)".to_string(),
            architecture: crate::clients::openrouter::OpenRouterModelArchitecture {
                input_modalities: vec!["text".into(), "image".into()],
            },
            supported_parameters: vec!["tools".into()],
            context_length: 2_000_000,
            pricing: crate::clients::openrouter::OpenRouterModelPricing {
                prompt: "0.00000125".to_string(),
                completion: "0.000005".to_string(),
                input_cache_read: None,
                input_cache_write: None,
            },
        }];

        // 1. match_openrouter with exact base match on stripped candidate
        let candidates = generate_candidates("grok-4.20-0309-non-reasoning-console");
        let mut matched = None;
        for (i, candidate) in candidates.iter().enumerate() {
            let allow_substring = i == 0;
            if let Some(m) = match_openrouter(candidate, &openrouter_models, allow_substring) {
                matched = Some(m);
                break;
            }
        }
        assert!(matched.is_some());
        assert_eq!(matched.unwrap().id, "x-ai/grok-4.20-0309-non-reasoning");
    }

    #[test]
    fn modelsdev_candidate_lookup_prefers_openrouter_over_peer_matches() {
        let mut meta = ModelsDevData::default();
        meta.by_provider.insert(
            "kilo".to_string(),
            HashMap::from([(
                "x-ai/grok-4.20-multi-agent".to_string(),
                entry("x-ai/grok-4.20-multi-agent", 2_000_000, true, false),
            )]),
        );
        meta.by_provider.insert(
            "xai".to_string(),
            HashMap::from([(
                "grok-4.20-multi-agent-0309".to_string(),
                entry("grok-4.20-multi-agent-0309", 2_000_000, true, false),
            )]),
        );
        meta.by_provider.insert(
            "openrouter".to_string(),
            HashMap::from([(
                "x-ai/grok-4.20-multi-agent".to_string(),
                entry("x-ai/grok-4.20-multi-agent", 2_000_000, true, false),
            )]),
        );

        let found = lookup_modelsdev_by_candidate_with_origin(&meta, "grok-4.20-multi-agent", None)
            .unwrap();

        let origin = found.origin.unwrap();
        assert_eq!(origin.provider, "openrouter");
        assert_eq!(origin.model_id, "x-ai/grok-4.20-multi-agent");
    }

    #[test]
    fn test_family_lookup_models_dev() {
        let mut model = entry("grok-build-0-1", 256_000, true, true);
        model.family = Some("grok-build".to_string());
        let meta = meta_with(vec![model]);

        let id = "grok-build-console";
        let candidates = generate_candidates(id);

        let mut resolved = None;
        for candidate in &candidates {
            if let Some(known) = lookup_meta(&meta, candidate) {
                resolved = Some(known.clone());
                break;
            }
            if let Some(known) = lookup_family(&meta, candidate) {
                let mut entry = known.clone();
                entry.id = id.to_string();
                resolved = Some(entry);
                break;
            }
        }

        assert!(resolved.is_some());
        let entry = resolved.unwrap();
        assert_eq!(entry.id, "grok-build-console");
        assert_eq!(entry.family.as_deref(), Some("grok-build"));
        assert_eq!(entry.context(), Some(256_000));
    }

    #[test]
    fn test_specific_candidate_precedence() {
        let meta = meta_with(vec![
            entry("grok-4", 256_000, false, true), // Generic match, should be avoided if a more specific one matches in OpenRouter
        ]);

        let openrouter_models = vec![crate::clients::openrouter::OpenRouterModel {
            id: "x-ai/grok-4.20-0309-non-reasoning".to_string(),
            name: "Grok 4.20 (Non-Reasoning)".to_string(),
            architecture: crate::clients::openrouter::OpenRouterModelArchitecture {
                input_modalities: vec!["text".into(), "image".into()],
            },
            supported_parameters: vec!["tools".into()],
            context_length: 2_000_000,
            pricing: crate::clients::openrouter::OpenRouterModelPricing {
                prompt: "0.00000125".to_string(),
                completion: "0.000005".to_string(),
                input_cache_read: None,
                input_cache_write: None,
            },
        }];

        let id = "grok-4.20-0309-console";
        let candidates = generate_candidates(id);

        let mut matched_source = None;
        let mut matched_entry = None;

        for (i, candidate) in candidates.iter().enumerate() {
            // 1. models.dev exact check
            if let Some(known) = lookup_meta(&meta, candidate) {
                matched_source = Some("models.dev");
                let mut entry = known.clone();
                entry.id = id.to_string();
                matched_entry = Some(entry);
                break;
            }
            // 2. models.dev cross-candidate check
            if let Some(known) = lookup_modelsdev_by_candidate_with_origin(&meta, candidate, None) {
                matched_source = Some("models.dev");
                let mut entry = known.entry.clone();
                entry.id = id.to_string();
                matched_entry = Some(entry);
                break;
            }
            // 3. OpenRouter check
            let allow_substring = i == 0;
            if let Some(or_model) = match_openrouter(candidate, &openrouter_models, allow_substring)
            {
                matched_source = Some("openrouter");
                let mut entry = ModelEntry::bare(id);
                entry.name = Some(or_model.name.clone());
                entry.limit = crate::domain::model::Limit {
                    context: Some(or_model.context_length),
                    ..Default::default()
                };
                matched_entry = Some(entry);
                break;
            }
        }

        assert_eq!(matched_source, Some("openrouter"));
        let entry = matched_entry.unwrap();
        assert_eq!(entry.id, "grok-4.20-0309-console");
        assert_eq!(entry.context(), Some(2_000_000));
    }
}
