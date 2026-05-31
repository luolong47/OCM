import axios from 'axios'
import type {
  ApiEnvelope,
  AutostartStatus,
  ApplyReport,
  CatalogResult,
  ModelFilters,
  Provider,
  ProviderInput,
  SelectedList,
  SelectedModel,
  SelectedUpdate,
  ImportReport,
  ModelsDevRefreshResult,
  ModelsDevStatus,
} from './types'

const http = axios.create({
  baseURL: import.meta.env.VITE_API_BASE ?? '/api',
  timeout: 30000,
})

// Surface the backend's `{ code, message }` envelope as a normal Error.
http.interceptors.response.use(
  (res) => res,
  (err) => {
    const env = err?.response?.data
    if (env && typeof env.message === 'string') {
      return Promise.reject(new Error(env.message))
    }
    return Promise.reject(err)
  },
)

async function unwrap<T>(p: Promise<{ data: ApiEnvelope<T> }>): Promise<T> {
  const { data: env } = await p
  if (env.code !== 0) {
    throw new Error(env.message || `request failed (code ${env.code})`)
  }
  return env.data
}

const enc = encodeURIComponent

export const api = {
  // Providers
  listProviders: () => unwrap<Provider[]>(http.get('/providers')),
  getProvider: (id: string) => unwrap<Provider>(http.get(`/providers/${enc(id)}`)),
  createProvider: (input: ProviderInput) =>
      unwrap<Provider>(http.post('/providers', input)),
  updateProvider: (id: string, input: ProviderInput) =>
      unwrap<Provider>(http.put(`/providers/${enc(id)}`, input)),
  deleteProvider: (id: string) =>
      unwrap<{ deleted: string }>(http.delete(`/providers/${enc(id)}`)),

  // Catalog + selection
  fetchModels: (id: string, filters: ModelFilters) =>
      unwrap<CatalogResult>(http.get(`/providers/${enc(id)}/models/fetch`, { params: filters })),
  refreshModels: (id: string) =>
      unwrap<CatalogResult>(http.post(`/providers/${enc(id)}/models/refresh`)),
  getSelected: (id: string) =>
      unwrap<SelectedList>(http.get(`/providers/${enc(id)}/models/selected`)),
  select: (id: string, modelIds: string[]) =>
      unwrap<{ selected: number }>(http.post(`/providers/${enc(id)}/models/select`, { model_ids: modelIds })),
  deselect: (id: string, modelIds: string[]) =>
      unwrap<{ deselected: number }>(http.post(`/providers/${enc(id)}/models/deselect`, { model_ids: modelIds })),
  selectAllFiltered: (id: string, filters: ModelFilters) =>
      unwrap<{ selected: number }>(http.post(`/providers/${enc(id)}/models/select-all-filtered`, { filters })),
  deselectAll: (id: string) =>
      unwrap<{ deselected: number }>(http.post(`/providers/${enc(id)}/models/deselect-all`)),
  updateSelected: (id: string, modelId: string, update: SelectedUpdate) =>
      unwrap<SelectedModel>(http.put(`/providers/${enc(id)}/selected/${enc(modelId)}`, update)),

  // Apply
  applyProvider: (id: string) =>
      unwrap<ApplyReport>(http.post(`/providers/${enc(id)}/apply`)),
  unapplyProvider: (id: string) =>
      unwrap<ApplyReport>(http.post(`/providers/${enc(id)}/unapply`)),
  previewProvider: (id: string) =>
      unwrap<unknown>(http.get(`/providers/${enc(id)}/apply/preview`)),

  // Import
  importConfig: () => unwrap<ImportReport>(http.post('/import')),

  // Local settings
  getAutostart: () =>
      unwrap<AutostartStatus>(http.get('/settings/autostart')),
  setAutostart: (enabled: boolean) =>
      unwrap<AutostartStatus>(http.put('/settings/autostart', { enabled })),

  // models.dev maintenance
  modelsDevStatus: () =>
      unwrap<ModelsDevStatus | null>(http.get('/models-dev/status')),
  refreshModelsDev: () =>
      unwrap<ModelsDevRefreshResult>(http.post('/models-dev/refresh')),
}
