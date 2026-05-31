CREATE TABLE models_dev_refresh (
    id             INTEGER PRIMARY KEY CHECK (id = 1),
    source_url     TEXT NOT NULL,
    provider_count INTEGER NOT NULL,
    model_count    INTEGER NOT NULL,
    refreshed_at   TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at     TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE models_dev_providers (
    provider_key TEXT PRIMARY KEY,
    provider_id  TEXT,
    name         TEXT,
    npm          TEXT,
    api_json     TEXT,
    doc_json     TEXT,
    env_json     TEXT,
    priority     INTEGER NOT NULL DEFAULT 100,
    raw_json     TEXT NOT NULL,
    refreshed_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE models_dev_models (
    provider_key                 TEXT NOT NULL REFERENCES models_dev_providers(provider_key) ON DELETE CASCADE,
    model_id                     TEXT NOT NULL,
    name                         TEXT,
    family                       TEXT,
    attachment                   BOOLEAN,
    reasoning                    BOOLEAN,
    tool_call                    BOOLEAN,
    temperature                  BOOLEAN,
    structured_output            BOOLEAN,
    open_weights                 BOOLEAN,
    release_date                 TEXT,
    last_updated                 TEXT,
    knowledge                    TEXT,
    status                       TEXT,
    limit_context                INTEGER,
    limit_input                  INTEGER,
    limit_output                 INTEGER,
    cost_input                   REAL,
    cost_output                  REAL,
    cost_cache_read              REAL,
    cost_cache_write             REAL,
    cost_input_audio             REAL,
    cost_output_audio            REAL,
    cost_reasoning               REAL,
    cost_context_over_200k_json  TEXT,
    cost_tiers_json              TEXT,
    experimental_json            TEXT,
    interleaved_bool             BOOLEAN,
    interleaved_json             TEXT,
    provider_json                TEXT,
    raw_json                     TEXT NOT NULL,
    refreshed_at                 TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at                   TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (provider_key, model_id)
);

CREATE TABLE models_dev_model_modalities (
    provider_key TEXT NOT NULL,
    model_id     TEXT NOT NULL,
    direction    TEXT NOT NULL CHECK (direction IN ('input', 'output')),
    modality     TEXT NOT NULL,
    PRIMARY KEY (provider_key, model_id, direction, modality),
    FOREIGN KEY (provider_key, model_id) REFERENCES models_dev_models(provider_key, model_id) ON DELETE CASCADE
);

CREATE INDEX idx_models_dev_providers_priority ON models_dev_providers(priority, provider_key);
CREATE INDEX idx_models_dev_models_model_id ON models_dev_models(model_id);
CREATE INDEX idx_models_dev_models_family ON models_dev_models(family);
CREATE INDEX idx_models_dev_models_context ON models_dev_models(limit_context);
CREATE INDEX idx_models_dev_modalities ON models_dev_model_modalities(direction, modality);
