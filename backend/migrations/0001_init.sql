-- OCM initial schema.
--
-- Design note: rich model metadata (context window, modalities, cost, tool/reasoning
-- support) is NOT stored as the source of truth. It is fetched live from models.dev
-- (the same dataset opencode uses) and joined against each provider's live /v1/models
-- id list. We persist ONLY the user's selection + their override patch, plus a snapshot
-- of the models.dev entry at selection time so the choice survives metadata drift.

PRAGMA foreign_keys = ON;

-- A configured provider. `id` is the opencode provider key (e.g. "grok2api").
CREATE TABLE providers (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    -- AI SDK package: "@ai-sdk/openai-compatible" (/chat/completions) or
    -- "@ai-sdk/openai" (/responses).
    npm             TEXT NOT NULL DEFAULT '@ai-sdk/openai-compatible',
    base_url        TEXT,
    -- Prefer env-var indirection; opencode writes this as "{env:NAME}".
    api_key_env     TEXT,
    -- Raw key (optional; discouraged — kept for convenience).
    api_key         TEXT,
    -- Optional hint to scope models.dev enrichment to one provider key.
    models_dev_key  TEXT,
    -- JSON object of custom headers and extra provider options.
    headers         TEXT,
    options         TEXT,
    enabled         INTEGER NOT NULL DEFAULT 1,
    is_applied      INTEGER NOT NULL DEFAULT 0,
    -- 'ocm' (managed here) | 'external' (discovered in opencode.json on startup).
    source          TEXT NOT NULL DEFAULT 'ocm',
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Only the user's SELECTED models live here.
CREATE TABLE selected_models (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    provider_id      TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
    model_id         TEXT NOT NULL,
    display_name     TEXT,
    is_enabled       INTEGER NOT NULL DEFAULT 1,

    -- JSON snapshot of the models.dev ModelEntry at selection time
    -- (or a minimal {"id": ...} when metadata was unknown).
    snapshot         TEXT NOT NULL,
    -- true when `snapshot` came from models.dev rather than being a bare id.
    metadata_known   INTEGER NOT NULL DEFAULT 0,
    -- JSON merge-patch (RFC 7386) of user overrides, applied over `snapshot` on export.
    override_patch   TEXT,

    -- Extracted columns for cheap filtering/sorting of the selected set.
    context          INTEGER,
    has_image        INTEGER NOT NULL DEFAULT 0,
    tool_call        INTEGER NOT NULL DEFAULT 0,

    selected_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at       TEXT NOT NULL DEFAULT (datetime('now')),
    api_snapshot_at  TEXT,

    UNIQUE (provider_id, model_id)
);

CREATE INDEX idx_selected_provider ON selected_models (provider_id);
CREATE INDEX idx_selected_context  ON selected_models (context);
CREATE INDEX idx_selected_image    ON selected_models (has_image);
