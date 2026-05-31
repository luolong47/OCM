// Types mirroring the backend's JSON shapes (which mirror models.dev / opencode).

export interface ApiEnvelope<T> {
  code: number
  data: T
  message?: string
}

export interface Modalities {
  input?: string[]
  output?: string[]
}

export interface Limit {
  context?: number
  input?: number
  output?: number
}

export interface Cost {
  input?: number
  output?: number
  cache_read?: number
  cache_write?: number
}

export interface ModelEntry {
  id: string
  name?: string
  family?: string
  attachment?: boolean
  reasoning?: boolean
  tool_call?: boolean
  temperature?: boolean
  release_date?: string
  status?: string
  modalities?: Modalities
  limit?: Limit
  cost?: Cost
}

export interface ModelMeta {
  is_selected: boolean
  has_custom_config: boolean
  metadata_known: boolean
  source: string
}

export type CatalogModel = ModelEntry & { _meta: ModelMeta }

export interface CatalogResult {
  total_available: number
  matched: number
  models: CatalogModel[]
}

export interface SelectedModel {
  model_id: string
  display_name: string | null
  is_enabled: boolean
  metadata_known: boolean
  has_custom_config: boolean
  selected_at: string
  api_snapshot_at: string | null
  effective: ModelEntry
  override_patch?: Record<string, unknown> | null
}

export interface SelectedList {
  total: number
  models: SelectedModel[]
}

export interface Provider {
  id: string
  name: string
  npm: string
  base_url: string | null
  api_key_env: string | null
  api_key: string | null
  models_dev_key: string | null
  headers: string | null
  options: string | null
  enabled: boolean
  is_applied: boolean
  needs_reapply: boolean
  source: string
  created_at: string
  updated_at: string
}

export interface ProviderInput {
  id: string
  name: string
  npm?: string
  base_url?: string | null
  api_key_env?: string | null
  api_key?: string | null
  models_dev_key?: string | null
  headers?: Record<string, unknown> | null
  options?: Record<string, unknown> | null
  enabled?: boolean
}

export interface ModelFilters {
  search?: string
  support_image?: boolean
  support_audio?: boolean
  support_video?: boolean
  tool_call?: boolean
  reasoning?: boolean
  min_context?: number
  max_context?: number
  status?: string
}

export interface SelectedUpdate {
  display_name?: string
  is_enabled?: boolean
  override_patch?: Record<string, unknown>
}

export interface ApplyReport {
  path: string
  providers_written: string[]
  models_written: number
  backup: string | null
}

export interface ImportReport {
  providers_imported: number
  models_imported: number
}

export interface ModelsDevStatus {
  source_url: string
  provider_count: number
  model_count: number
  refreshed_at: string
}

export interface ModelsDevRefreshResult {
  indexed_model_count: number
  status: ModelsDevStatus | null
}

export interface AutostartStatus {
  enabled: boolean
  desktop_file: string
  script_file: string
  executable: string
  working_dir: string
}
