-- Migration: Add model_library for local model base metadata mapping

CREATE TABLE model_library (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    pattern         TEXT NOT NULL UNIQUE,
    name            TEXT NOT NULL,
    family          TEXT,
    attachment      INTEGER NOT NULL DEFAULT 0,
    reasoning       INTEGER NOT NULL DEFAULT 0,
    tool_call       INTEGER NOT NULL DEFAULT 0,
    temperature     INTEGER NOT NULL DEFAULT 1,
    context         INTEGER,
    max_output      INTEGER,
    cost_input      REAL,
    cost_output     REAL,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Populate with some popular base models for default matching
INSERT INTO model_library (pattern, name, family, attachment, reasoning, tool_call, context, max_output, cost_input, cost_output)
VALUES ('deepseek', 'DeepSeek', 'deepseek', 0, 1, 1, 64000, 8192, 0.14, 0.28);

INSERT INTO model_library (pattern, name, family, attachment, reasoning, tool_call, context, max_output, cost_input, cost_output)
VALUES ('qwen', 'Qwen', 'qwen', 1, 0, 1, 128000, 4096, 0.40, 1.00);

INSERT INTO model_library (pattern, name, family, attachment, reasoning, tool_call, context, max_output, cost_input, cost_output)
VALUES ('gpt-4o', 'GPT-4o', 'gpt-4o', 1, 0, 1, 128000, 4096, 5.00, 15.00);

INSERT INTO model_library (pattern, name, family, attachment, reasoning, tool_call, context, max_output, cost_input, cost_output)
VALUES ('gpt-4', 'GPT-4', 'gpt-4', 0, 0, 1, 128000, 4096, 30.00, 60.00);

INSERT INTO model_library (pattern, name, family, attachment, reasoning, tool_call, context, max_output, cost_input, cost_output)
VALUES ('claude-3', 'Claude 3', 'claude-3', 1, 0, 1, 200000, 4096, 3.00, 15.00);

INSERT INTO model_library (pattern, name, family, attachment, reasoning, tool_call, context, max_output, cost_input, cost_output)
VALUES ('llama', 'Llama', 'llama', 0, 0, 1, 128000, 4096, 0.20, 0.20);
