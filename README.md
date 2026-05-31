# OpenCode Config Manager (OCM)

A local tool to browse a provider's models, filter the haystack down, batch-select the
ones you want, optionally override their config, and write them into your
`opencode.json` — without hand-editing JSON or clobbering the rest of your config.

## How it works (the important bit)

A provider's catalog is built from **two** sources, joined by model id:

| Source | Gives you | Why |
|---|---|---|
| Provider `GET {base_url}/models` | the **ids the key can actually call** | ground truth, but OpenAI-compatible endpoints return *only* ids |
| [`models.dev`](https://models.dev) `api.json` | **rich metadata** (context, modalities, cost, tool/reasoning) | the same dataset opencode itself uses |

So capability filtering ("supports image", "≥128K context") works even though the
provider endpoint returns bare ids. Ids present at the provider but missing from
models.dev are surfaced as **provider-only** (no metadata) and can still be selected.

OCM persists **only your selection + overrides** (plus a metadata snapshot taken at
selection time). The full model list is never stored — it's fetched live and cached.

```
provider /v1/models ─┐
                     ├─ join by id ─► filter ─► select ─► merge(snapshot, override) ─► opencode.json
models.dev/api.json ─┘                                    (preserves your other providers + keys)
```

## Layout

```
backend/    Rust + Axum + SQLx (SQLite) + reqwest + moka
frontend/   Vue 3 + Vite + Naive UI + Pinia + vue-router
```

## Prerequisites

- **Rust** (stable) — `cargo`
- **Node 20+** and **pnpm** — e.g. via nvm + corepack:
  ```sh
  nvm use default       # or: nvm install --lts
  corepack enable pnpm  # if pnpm isn't on PATH
  ```

## Run the backend

```sh
cd backend
cp .env.example .env        # optional; sane defaults otherwise
cargo run
```

Listens on `http://127.0.0.1:8787` by default. Migrations run automatically; the
SQLite db is created at `backend/data/ocm.db`.

> ⚠️ Apply writes to `~/.config/opencode/opencode.json` (it backs up to `.json.bak`
> first and preserves any providers/keys it didn't create). To target a throwaway file
> while experimenting, set `OCM_OPENCODE_CONFIG=/tmp/opencode.json`.

Key env vars (all optional — see `.env.example`):

| Var | Default | Meaning |
|---|---|---|
| `DATABASE_URL` | `sqlite:data/ocm.db?mode=rwc` | SQLite location |
| `OCM_BIND` | `127.0.0.1:8787` | listen address |
| `OCM_MODELS_DEV_URL` | `https://models.dev/api.json` | metadata source |
| `OCM_MODELS_DEV_TTL_SECS` | `86400` | metadata cache TTL |
| `OCM_PROVIDER_LIST_TTL_SECS` | `300` | per-provider id-list cache TTL |
| `OCM_OPENCODE_CONFIG` | `~/.config/opencode/opencode.json` | apply target |

## Run the frontend

```sh
cd frontend
pnpm install
pnpm dev        # http://localhost:5174  (proxies /api → backend on :8787)
```

`pnpm build` for a production bundle, `pnpm typecheck` to run `vue-tsc`.

## HTTP API

All responses are `{ "code": 0, "data": ... }` on success, or
`{ "code": <nonzero>, "message": "...", "data": null }` on error.

```
GET    /health
GET    /providers
POST   /providers                                   {id,name,npm,base_url,api_key_env,...}
GET    /providers/{id}
PUT    /providers/{id}
DELETE /providers/{id}

GET    /providers/{id}/models/fetch?search=&support_image=&min_context=&tool_call=&...
GET    /providers/{id}/models/resolve?model_id=...       # explain metadata match source
POST   /providers/{id}/models/refresh               # force re-fetch live id list
GET    /providers/{id}/models/selected
POST   /providers/{id}/models/select                {model_ids:[...]}
POST   /providers/{id}/models/deselect              {model_ids:[...]}
POST   /providers/{id}/models/select-all-filtered   {filters:{...}}   # spans whole result set
POST   /providers/{id}/models/deselect-all
PUT    /providers/{id}/selected/{model_id}          {display_name?,is_enabled?,override_patch?}

GET    /providers/{id}/apply/preview
POST   /providers/{id}/apply
POST   /apply
POST   /models-dev/refresh
```

## Tests

```sh
cd backend && cargo test     # covers the join/filter core + merge-patch
```
