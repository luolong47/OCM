<script setup lang="ts">
import { onMounted, reactive, ref } from 'vue'
import { onBeforeRouteLeave, useRouter } from 'vue-router'
import { NAlert, NButton, NCard, NIcon, NSpace, useMessage } from 'naive-ui'
import { ArrowLeft } from 'lucide-vue-next'

import { api } from '@/api/client'
import type { CatalogModel, CatalogResult, Provider, SelectedModel } from '@/api/types'
import ModelFilterBar from '@/components/ModelFilterBar.vue'
import ModelTable from '@/components/ModelTable.vue'
import SelectedModelSection from '@/components/SelectedModelSection.vue'
import ModelDetailDrawer from '@/components/ModelDetailDrawer.vue'
import BatchSelectDialog from '@/components/BatchSelectDialog.vue'

const props = defineProps<{ id: string }>()
const router = useRouter()
const message = useMessage()

const providerName = ref(props.id)
const providerInfo = ref<Provider | null>(null)
const catalog = ref<CatalogResult | null>(null)
const selected = ref<SelectedModel[]>([])
const catalogLoading = ref(false)
const catalogError = ref('')

const drawerShow = ref(false)
const drawerModel = ref<CatalogModel | null>(null)
const drawerDetail = ref<SelectedModel | null>(null)
const batchShow = ref(false)

const defaultFilters = () => ({
  search: '',
  support_image: false,
  support_audio: false,
  support_video: false,
  tool_call: false,
  reasoning: false,
  min_context: 0,
  status: '',
})
const filters = reactive(defaultFilters())

const rawModels = ref<CatalogModel[]>([])

function markCatalogSelected(ids: string[], isSelected: boolean) {
  const idSet = new Set(ids)
  catalog.value?.models.forEach((model) => {
    if (idSet.has(model.id)) {
      model._meta.is_selected = isSelected
    }
  })
  rawModels.value.forEach((model) => {
    if (idSet.has(model.id)) {
      model._meta.is_selected = isSelected
    }
  })
}

async function loadSelected() {
  try {
    selected.value = (await api.getSelected(props.id)).models
    syncCatalogSelection()
  } catch (e) {
    message.error((e as Error).message)
  }
}

async function loadCatalogSnapshot() {
  catalogLoading.value = true
  catalogError.value = ''
  try {
    catalog.value = await api.fetchModels(props.id, {})
    rawModels.value = catalog.value.models
    syncCatalogSelection()
  } catch (e) {
    catalogError.value = (e as Error).message
    catalog.value = null
  } finally {
    catalogLoading.value = false
  }
}

async function reloadAll() {
  await loadSelected()
  syncDrawer()
}

onMounted(async () => {
  try {
    providerInfo.value = await api.getProvider(props.id)
    providerName.value = providerInfo.value.name
  } catch {
    /* header name is cosmetic */
  }
  await Promise.all([loadCatalogSnapshot(), loadSelected()])
})

// Auto-apply when leaving if the provider is already in applied state.
onBeforeRouteLeave(async () => {
  if (!providerInfo.value?.is_applied) return
  try {
    await api.applyProvider(props.id)
  } catch (e) {
    // Navigate away anyway; the backend's needs_reapply flag stays true,
    // so the yellow dot will appear in the provider list.
    message.warning(`配置未能自动写入，请在列表页点击重新应用。(${(e as Error).message})`)
  }
})

async function onSelect(ids: string[]) {
  try {
    await api.select(props.id, ids)
    markCatalogSelected(ids, true)
    await loadSelected()
    syncDrawer()
  } catch (e) {
    message.error((e as Error).message)
  }
}

async function onDeselect(ids: string[]) {
  try {
    await api.deselect(props.id, ids)
    markCatalogSelected(ids, false)
    await loadSelected()
    syncDrawer()
  } catch (e) {
    message.error((e as Error).message)
  }
}

async function selectAllFiltered(ids: string[]) {
  try {
    if (!ids.length) {
      message.info('当前没有可选择的模型')
      return
    }
    await api.select(props.id, ids)
    markCatalogSelected(ids, true)
    message.success(`已选择 ${ids.length} 个筛选出的模型`)
    await loadSelected()
    syncDrawer()
  } catch (e) {
    message.error((e as Error).message)
  }
}

async function deselectAll() {
  try {
    const { deselected } = await api.deselectAll(props.id)
    markCatalogSelected(rawModels.value.map((model) => model.id), false)
    message.success(`已取消选择 ${deselected} 个模型`)
    await loadSelected()
    syncDrawer()
  } catch (e) {
    message.error((e as Error).message)
  }
}

async function refresh() {
  try {
    catalog.value = await api.refreshModels(props.id)
    rawModels.value = catalog.value.models
    message.success('已重新拉取模型列表')
  } catch (e) {
    message.error((e as Error).message)
  }
}

function resetFilters() {
  Object.assign(filters, defaultFilters())
}

function syncCatalogSelection() {
  const selectedIds = new Set(selected.value.map((model) => model.model_id))
  const customizedIds = new Set(
    selected.value
      .filter((model) => model.has_custom_config)
      .map((model) => model.model_id),
  )
  catalog.value?.models.forEach((model) => {
    model._meta.is_selected = selectedIds.has(model.id)
    model._meta.has_custom_config = customizedIds.has(model.id)
  })
  rawModels.value.forEach((model) => {
    model._meta.is_selected = selectedIds.has(model.id)
    model._meta.has_custom_config = customizedIds.has(model.id)
  })
}

function openFromTable(model: CatalogModel) {
  drawerModel.value = model
  drawerDetail.value = selected.value.find((s) => s.model_id === model.id) ?? null
  drawerShow.value = true
}

function selectedToCatalog(s: SelectedModel): CatalogModel {
  return {
    ...s.effective,
    id: s.model_id,
    _meta: {
      is_selected: true,
      has_custom_config: s.has_custom_config,
      metadata_known: s.metadata_known,
      source: s.metadata_known ? 'models.dev' : 'provider-only',
    },
  }
}

function openFromSelected(s: SelectedModel) {
  drawerModel.value = selectedToCatalog(s)
  drawerDetail.value = s
  drawerShow.value = true
}

// After a select/deselect/override change, refresh the drawer's bound objects.
function syncDrawer() {
  if (!drawerModel.value) return
  const id = drawerModel.value.id
  const fromCatalog = catalog.value?.models.find((m) => m.id === id)
  const fromSelected = selected.value.find((s) => s.model_id === id) ?? null
  if (fromCatalog) drawerModel.value = fromCatalog
  else if (fromSelected) drawerModel.value = selectedToCatalog(fromSelected)
  drawerDetail.value = fromSelected
}
</script>

<template>
  <div>
    <n-space align="center" justify="space-between" style="margin-bottom: 16px">
      <n-space align="center">
        <n-button
          quaternary
          circle
          title="返回"
          @click="router.push('/providers')"
        >
          <template #icon>
            <n-icon :component="ArrowLeft" />
          </template>
        </n-button>
        <h2 style="margin: 0">{{ providerName }} — 模型管理</h2>
        <code style="color: #999">{{ props.id }}</code>
      </n-space>
    </n-space>

    <n-card size="small" style="margin-bottom: 16px">
      <selected-model-section
        :selected="selected"
        @deselect="(id: string) => onDeselect([id])"
        @open="openFromSelected"
        @clear-all="deselectAll"
      />
    </n-card>

    <n-card size="small" title="模型目录">
      <n-alert
        v-if="catalogError"
        type="error"
        title="无法获取模型列表"
        style="margin-bottom: 16px"
      >
        {{ catalogError }}
        <div style="margin-top: 6px; font-size: 12px">
          请检查服务商的接口地址 (Base URL) 和 API 密钥，然后点击刷新。
        </div>
      </n-alert>

      <div v-else>
        <model-table
          v-model:search="filters.search"
          :filters="filters"
          :all-models="rawModels"
          :total="catalog?.total_available ?? 0"
          :models="catalog?.models ?? []"
          :loading="catalogLoading"
          @select="onSelect"
          @deselect="onDeselect"
          @open="openFromTable"
          @reset="resetFilters"
          @select-all="selectAllFiltered"
          @deselect-all="deselectAll"
          @refresh="refresh"
        />
      </div>
    </n-card>

    <model-detail-drawer
      v-model:show="drawerShow"
      :provider-id="props.id"
      :model="drawerModel"
      :detail="drawerDetail"
      @changed="reloadAll"
    />

    <batch-select-dialog
      v-model:show="batchShow"
      :provider-id="props.id"
      @changed="reloadAll"
    />
  </div>
</template>
