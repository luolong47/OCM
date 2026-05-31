//! models.dev client.
//!
//! `https://models.dev/api.json` is one document keyed by provider, each holding a
//! `models` map of id → entry. We fetch it once, cache it, and flatten into a global
//! id → entry index for enrichment, while also keeping the per-provider grouping for
//! optional scoping.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::domain::ModelEntry;
use crate::error::AppError;

#[derive(Debug, Default)]
pub struct ModelsDevData {
    pub providers: Vec<ModelsDevProvider>,
    /// Flattened global index. First provider wins on id collision (see `note`).
    pub by_id: HashMap<String, ModelEntry>,
    /// Origin metadata for `by_id`, keyed by the same model id.
    pub by_id_origin: HashMap<String, ModelsDevOrigin>,
    /// Per-provider grouping, keyed by the models.dev provider key.
    pub by_provider: HashMap<String, HashMap<String, ModelEntry>>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ModelsDevProvider {
    pub key: String,
    pub id: Option<String>,
    pub name: Option<String>,
    pub npm: Option<String>,
    pub api: Option<Value>,
    pub doc: Option<Value>,
    pub env: Option<Value>,
    pub priority: i64,
    pub raw: Value,
    pub models: HashMap<String, ModelEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelsDevOrigin {
    pub provider: String,
    pub model_id: String,
}

#[derive(Debug, Clone)]
pub struct ModelsDevLookup<'a> {
    pub entry: &'a ModelEntry,
    pub origin: Option<ModelsDevOrigin>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct ProviderRecord {
    pub id: Option<String>,
    pub name: Option<String>,
    pub npm: Option<String>,
    pub api: Option<Value>,
    pub doc: Option<Value>,
    pub env: Option<Value>,
    #[serde(default)]
    pub models: HashMap<String, ModelEntry>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}

pub fn default_provider_priority(provider: &str) -> i64 {
    match provider {
        "openrouter" => 0,
        "xai" => 10,
        "vercel" => 20,
        "kilo" => 30,
        "302ai" => 40,
        "venice" => 50,
        "poe" => 1000,
        _ => 100,
    }
}

impl ModelsDevData {
    pub fn lookup_with_origin(&self, id: &str, scope: Option<&str>) -> Option<ModelsDevLookup<'_>> {
        if let Some(key) = scope {
            if let Some(found) = self.by_provider.get(key).and_then(|m| m.get(id)) {
                return Some(ModelsDevLookup {
                    entry: found,
                    origin: Some(ModelsDevOrigin {
                        provider: key.to_string(),
                        model_id: id.to_string(),
                    }),
                });
            }
            if let Some(provider_models) = self.by_provider.get(key) {
                if let Some((model_id, found)) = provider_models
                    .iter()
                    .find(|(model_id, _)| model_id.eq_ignore_ascii_case(id))
                {
                    return Some(ModelsDevLookup {
                        entry: found,
                        origin: Some(ModelsDevOrigin {
                            provider: key.to_string(),
                            model_id: model_id.clone(),
                        }),
                    });
                }
            }
        }
        self.by_id
            .get(id)
            .map(|entry| ModelsDevLookup {
                entry,
                origin: self.by_id_origin.get(id).cloned(),
            })
            .or_else(|| {
                self.by_id.iter().find_map(|(model_id, entry)| {
                    if model_id.eq_ignore_ascii_case(id) {
                        Some(ModelsDevLookup {
                            entry,
                            origin: self.by_id_origin.get(model_id).cloned(),
                        })
                    } else {
                        None
                    }
                })
            })
    }

    pub fn lookup_by_family_with_origin(
        &self,
        family: &str,
        scope: Option<&str>,
    ) -> Option<ModelsDevLookup<'_>> {
        let family_lower = family.to_lowercase();
        if let Some(key) = scope {
            if let Some(provider_models) = self.by_provider.get(key) {
                if let Some((model_id, found)) = provider_models.iter().find(|(_, m)| {
                    m.family.as_ref().map(|f| f.to_lowercase()) == Some(family_lower.clone())
                }) {
                    return Some(ModelsDevLookup {
                        entry: found,
                        origin: Some(ModelsDevOrigin {
                            provider: key.to_string(),
                            model_id: model_id.clone(),
                        }),
                    });
                }
            }
        }
        self.by_id.iter().find_map(|(model_id, m)| {
            if m.family.as_ref().map(|f| f.to_lowercase()) == Some(family_lower.clone()) {
                Some(ModelsDevLookup {
                    entry: m,
                    origin: self.by_id_origin.get(model_id).cloned(),
                })
            } else {
                None
            }
        })
    }

    pub fn model_count(&self) -> usize {
        self.by_id.len()
    }
}

pub async fn fetch_records(
    http: &reqwest::Client,
    url: &str,
) -> Result<HashMap<String, ProviderRecord>, AppError> {
    let resp = http
        .get(url)
        .send()
        .await
        .map_err(|e| AppError::Upstream(format!("models.dev fetch: {e}")))?;
    if !resp.status().is_success() {
        return Err(AppError::Upstream(format!(
            "models.dev returned {}",
            resp.status()
        )));
    }
    resp.json()
        .await
        .map_err(|e| AppError::Upstream(format!("models.dev parse: {e}")))
}

pub fn from_records(
    raw: HashMap<String, ProviderRecord>,
    priorities: Option<&HashMap<String, i64>>,
) -> ModelsDevData {
    let mut data = ModelsDevData::default();

    let mut providers: Vec<_> = raw.into_iter().collect();
    providers.sort_by(|(a_key, _), (b_key, _)| {
        priority_for(a_key, priorities)
            .cmp(&priority_for(b_key, priorities))
            .then_with(|| a_key.cmp(b_key))
    });

    for (provider_key, record) in providers {
        let mut provider_models = HashMap::new();
        for (model_id, mut entry) in record.models.clone() {
            // Trust the map key as the canonical id when the entry omits it.
            if entry.id.is_empty() {
                entry.id = model_id.clone();
            }
            provider_models.insert(model_id.clone(), entry.clone());
            if !data.by_id.contains_key(&model_id) {
                data.by_id.insert(model_id.clone(), entry);
                data.by_id_origin.insert(
                    model_id.clone(),
                    ModelsDevOrigin {
                        provider: provider_key.clone(),
                        model_id,
                    },
                );
            }
        }
        let priority = priority_for(&provider_key, priorities);
        data.providers.push(ModelsDevProvider {
            key: provider_key.clone(),
            id: record.id.clone(),
            name: record.name.clone(),
            npm: record.npm.clone(),
            api: record.api.clone(),
            doc: record.doc.clone(),
            env: record.env.clone(),
            priority,
            raw: serde_json::to_value(&record).unwrap_or(Value::Null),
            models: provider_models.clone(),
        });
        data.by_provider
            .insert(provider_key.clone(), provider_models);
    }
    data
}

fn priority_for(provider: &str, priorities: Option<&HashMap<String, i64>>) -> i64 {
    priorities
        .and_then(|m| m.get(provider).copied())
        .unwrap_or_else(|| default_provider_priority(provider))
}
