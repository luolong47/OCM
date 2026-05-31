# OpenCode Config Manager（OCM）

OCM 是一个本地工具，用来管理 `opencode.json` 里的模型提供商和模型配置。它可以从提供商接口读取可用模型，结合 `models.dev` 的模型元数据进行筛选、批量选择、覆盖配置，并把结果写回 OpenCode 配置文件，避免手工编辑 JSON。

## 工作方式

OCM 的模型列表来自两个数据源，并按模型 ID 合并：

| 数据源 | 提供内容 | 用途 |
| --- | --- | --- |
| 提供商 `GET {base_url}/models` | 当前 API Key 实际可调用的模型 ID | 真实可用性来源，但 OpenAI 兼容接口通常只返回 ID |
| [`models.dev`](https://models.dev) `api.json` | 上下文长度、输入输出模态、价格、工具调用、推理能力等元数据 | 用于能力筛选，也是 opencode 使用的数据集 |

因此，即使提供商接口只返回模型 ID，OCM 仍然可以筛选“支持图片”“上下文大于 128K”“支持工具调用”等条件。对于提供商返回但 `models.dev` 中不存在的模型，OCM 会标记为“仅提供商存在”，仍然允许选择。

OCM 只持久化你选择的模型、覆盖配置，以及选择时的元数据快照。完整模型列表不会写入数据库，只会实时拉取并缓存。

```text
provider /v1/models ─┐
                     ├─ 按模型 ID 合并 ─► 筛选 ─► 选择 ─► 合并快照和覆盖配置 ─► opencode.json
models.dev/api.json ─┘                                      保留其他 provider 和 key
```

## 项目结构

```text
backend/    Rust + Axum + SQLx(SQLite) + reqwest + moka
frontend/   Vue 3 + Vite + Naive UI + Pinia + vue-router
docs/       字段说明和 models.dev API 参考
```

## 环境要求

- Rust stable，需要 `cargo`
- Node.js 20+ 和 pnpm

如果本机还没有 pnpm，可以使用：

```sh
corepack enable pnpm
```

## 启动后端

```sh
cd backend
cp .env.example .env
cargo run
```

默认监听：

```text
http://127.0.0.1:8787
```

数据库默认创建在：

```text
backend/data/ocm.db
```

后端启动时会自动执行数据库迁移。

### 写入 OpenCode 配置

应用配置时，OCM 默认写入：

```text
~/.config/opencode/opencode.json
```

写入前会先备份为 `.json.bak`，并保留不属于 OCM 管理的 provider、key 和其他配置。

如果只是测试，不想改真实配置，可以指定临时文件：

```sh
OCM_OPENCODE_CONFIG=/tmp/opencode.json cargo run
```

## 后端环境变量

所有环境变量都是可选的，默认值见 `backend/.env.example`。

| 变量 | 默认值 | 说明 |
| --- | --- | --- |
| `DATABASE_URL` | `sqlite:data/ocm.db?mode=rwc` | SQLite 数据库位置 |
| `OCM_BIND` | `127.0.0.1:8787` | 后端监听地址 |
| `OCM_MODELS_DEV_URL` | `https://models.dev/api.json` | 模型元数据来源 |
| `OCM_MODELS_DEV_TTL_SECS` | `86400` | `models.dev` 元数据缓存时间，单位秒 |
| `OCM_PROVIDER_LIST_TTL_SECS` | `300` | 每个提供商模型 ID 列表缓存时间，单位秒 |
| `OCM_OPENCODE_CONFIG` | `~/.config/opencode/opencode.json` | OpenCode 配置写入目标 |
| `RUST_LOG` | `ocm_backend=debug,tower_http=info,info` | 后端日志级别 |

## 启动前端

```sh
cd frontend
pnpm install
pnpm dev
```

前端默认地址：

```text
http://localhost:5174
```

开发环境会把 `/api` 代理到后端 `http://127.0.0.1:8787`。

常用命令：

```sh
pnpm build      # 构建生产版本
pnpm preview    # 预览生产构建
pnpm typecheck  # TypeScript 类型检查
```

## 主要功能

- 管理 OpenCode provider：新增、编辑、删除和查看。
- 从提供商实时拉取模型 ID，并与 `models.dev` 元数据合并。
- 按搜索词、上下文长度、图片输入、工具调用等条件筛选模型。
- 支持单选、批量选择、按筛选条件全选、全部取消。
- 支持为选中的模型设置显示名称、启用状态和覆盖配置。
- 预览将要写入的 OpenCode 配置。
- 将选中的 provider 和模型应用到 `opencode.json`。

## HTTP API

成功响应格式：

```json
{ "code": 0, "data": {} }
```

失败响应格式：

```json
{ "code": 1, "message": "错误信息", "data": null }
```

主要接口：

```text
GET    /health

GET    /providers
POST   /providers
GET    /providers/{id}
PUT    /providers/{id}
DELETE /providers/{id}

GET    /providers/{id}/models/fetch
GET    /providers/{id}/models/resolve
POST   /providers/{id}/models/refresh
GET    /providers/{id}/models/selected
POST   /providers/{id}/models/select
POST   /providers/{id}/models/deselect
POST   /providers/{id}/models/select-all-filtered
POST   /providers/{id}/models/deselect-all
PUT    /providers/{id}/selected/{model_id}

GET    /providers/{id}/apply/preview
POST   /providers/{id}/apply
POST   /apply
POST   /models-dev/refresh
```

## 测试

后端测试：

```sh
cd backend
cargo test
```

前端类型检查：

```sh
cd frontend
pnpm typecheck
```

## Git 忽略规则

仓库不会提交本地环境、数据库、构建产物和依赖目录，例如：

- `backend/.env`
- `backend/data/`
- `backend/target/`
- `frontend/node_modules/`
- `frontend/dist/`
