CREATE TABLE provider_model_catalog_cache (
    provider_id     TEXT PRIMARY KEY REFERENCES providers(id) ON DELETE CASCADE,
    total_available INTEGER NOT NULL,
    models_json     TEXT NOT NULL,
    refreshed_at    TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
