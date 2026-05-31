<script setup lang="ts">
import { computed, h, ref, watch } from 'vue'
import { NDataTable, NSpace, NTag, NText, NInput, NButton, NCheckbox, type DataTableColumns } from 'naive-ui'

import type { CatalogModel } from '@/api/types'
import { fmtContext, fmtCost, hasModality } from '@/utils/format'

const props = defineProps<{
  models: CatalogModel[]
  allModels: CatalogModel[]
  loading?: boolean
  search: string
  filters: {
    search: string
    support_image: boolean
    support_audio: boolean
    support_video: boolean
    tool_call: boolean
    reasoning: boolean
    min_context: number
  }
  total: number
}>()
const emit = defineEmits<{
  (e: 'select', ids: string[]): void
  (e: 'deselect', ids: string[]): void
  (e: 'open', model: CatalogModel): void
  (e: 'update:search', val: string): void
  (e: 'reset'): void
  (e: 'select-all', ids: string[]): void
  (e: 'deselect-all'): void
  (e: 'refresh'): void
}>()

const searchVal = computed({
  get: () => props.search,
  set: (v) => emit('update:search', v),
})

function onClearSearch() {
  emit('update:search', '')
}

const displayModels = computed(() => {
  const term = props.search.trim().toLowerCase()

  return props.models.filter((m) => {
    if (term) {
      const idMatch = m.id.toLowerCase().includes(term)
      const nameMatch = m.name?.toLowerCase().includes(term) ?? false
      if (!idMatch && !nameMatch) return false
    }
    if (props.filters.support_image && !hasModality(m, 'image')) return false
    if (props.filters.support_audio && !hasModality(m, 'audio')) return false
    if (props.filters.support_video && !hasModality(m, 'video')) return false
    if (props.filters.tool_call && !m.tool_call) return false
    if (props.filters.reasoning && !m.reasoning) return false
    if (props.filters.min_context && (m.limit?.context ?? 0) < props.filters.min_context) return false
    return true
  })
})

const displayMatched = computed(() => displayModels.value.length)

const modalityFilterOptions = computed(() => {
  const kinds = new Set<string>()
  const source = props.allModels?.length ? props.allModels : props.models
  source.forEach(m => {
    ['image', 'audio', 'video', 'pdf'].forEach(k => {
      if (hasModality(m, k)) {
        kinds.add(k)
      }
    })
  })
  
  return Array.from(kinds).map(k => ({
    label: getModalityCn(k),
    value: k
  }))
})

const contextFilterOptions = computed(() => {
  const vals = new Set<number>()
  const source = props.allModels?.length ? props.allModels : props.models
  source.forEach(m => {
    const ctx = m.limit?.context
    if (ctx && typeof ctx === 'number') {
      vals.add(ctx)
    }
  })
  
  return Array.from(vals)
    .sort((a, b) => a - b)
    .map(val => {
      let label = ''
      if (val >= 1_000_000) {
        label = `≥ ${val / 1_000_000}M`
      } else if (val >= 1000) {
        label = `≥ ${val / 1000}K`
      } else {
        label = `≥ ${val}`
      }
      return { label, value: val }
    })
})

const tableFilters = ref<Record<string, any>>({
  context: props.filters.min_context ? [props.filters.min_context] : [],
  modalities: [
    ...(props.filters.support_image ? ['image'] : []),
    ...(props.filters.support_audio ? ['audio'] : []),
    ...(props.filters.support_video ? ['video'] : []),
  ],
  tool_call: props.filters.tool_call ? ['true'] : [],
  reasoning: props.filters.reasoning ? ['true'] : [],
})

watch(
  () => props.filters,
  (newVal) => {
    tableFilters.value.context = newVal.min_context ? [newVal.min_context] : []
    tableFilters.value.modalities = [
      ...(newVal.support_image ? ['image'] : []),
      ...(newVal.support_audio ? ['audio'] : []),
      ...(newVal.support_video ? ['video'] : []),
    ]
    tableFilters.value.tool_call = newVal.tool_call ? ['true'] : []
    tableFilters.value.reasoning = newVal.reasoning ? ['true'] : []
  },
  { deep: true, immediate: true }
)

function handleUpdateFilters(filtersArg: Record<string, any>) {
  // 上下文过滤
  const contextVal = filtersArg.context
  let minCtx = 0
  if (contextVal !== null && contextVal !== undefined) {
    const list = Array.isArray(contextVal) ? contextVal : [contextVal]
    if (list.length > 0 && list[0] !== null && list[0] !== undefined) {
      minCtx = Number(list[0])
    }
  }
  props.filters.min_context = isNaN(minCtx) ? 0 : minCtx
  
  // 模态过滤
  const modalityVals = filtersArg.modalities || []
  props.filters.support_image = modalityVals.includes('image')
  props.filters.support_audio = modalityVals.includes('audio')
  props.filters.support_video = modalityVals.includes('video')
  
  // 工具过滤
  const toolVals = filtersArg.tool_call || []
  props.filters.tool_call = toolVals.includes('true')
  
  // 推理过滤
  const reasoningVals = filtersArg.reasoning || []
  props.filters.reasoning = reasoningVals.includes('true')
  
}

const checkedKeys = computed(() =>
  props.models.filter((m) => m._meta.is_selected).map((m) => m.id),
)

// The data table gives us the full next set of checked keys; diff against the
// current selection to decide what to add vs remove.
function onUpdateChecked(keys: Array<string | number>) {
  const next = new Set(keys.map(String))
  const cur = new Set(checkedKeys.value)
  const toAdd = [...next].filter((k) => !cur.has(k))
  const toRemove = [...cur].filter((k) => !next.has(k))
  if (toAdd.length) emit('select', toAdd)
  if (toRemove.length) emit('deselect', toRemove)
}

function getModalityCn(kind: string): string {
  const mapping: Record<string, string> = {
    image: '图片',
    audio: '音频',
    video: '视频',
    pdf: 'PDF',
  }
  return mapping[kind] || kind
}

const columns = computed<DataTableColumns<CatalogModel>>(() => [
  {
    title: '序号',
    key: 'index',
    width: 60,
    align: 'center',
    render: (_, rowIndex) => rowIndex + 1,
  },
  { type: 'selection', width: 50 },
  {
    title: '模型',
    key: 'id',
    minWidth: 280,
    render: (row) =>
      h(
        'div',
        { style: 'cursor: pointer; display: flex; flex-direction: column; gap: 4px; padding: 4px 0;', onClick: () => emit('open', row) },
        [
          h(
            'div',
            { style: 'display: flex; align-items: center; gap: 8px;' },
            [
              h(NText, { strong: true }, { default: () => row.id }),
              !row._meta.metadata_known
                ? h(
                    NTag,
                    { size: 'tiny', type: 'warning', bordered: false },
                    { default: () => '无元数据' },
                  )
                : null,
            ]
          ),
          row.name && row.name !== row.id
            ? h(
                NText,
                { depth: 3, style: 'font-size: 12px; line-height: 1.2;' },
                { default: () => row.name }
              )
            : null
        ]
      ),
  },
  {
    title: '上下文',
    key: 'context',
    width: 105,
    render: (row) => fmtContext(row.limit?.context),
    filter: true,
    filterMultiple: false,
    filterOptions: contextFilterOptions.value
  },
  {
    title: '模态',
    key: 'modalities',
    width: 110,
    render: (row) => {
      const kinds = ['image', 'audio', 'video', 'pdf'].filter((k) => hasModality(row, k))
      if (!kinds.length) return h(NText, { depth: 3 }, { default: () => '文本' })
      return h(
        NSpace,
        { size: 4 },
        { default: () => kinds.map((k) => h(NTag, { size: 'tiny', type: 'info' }, { default: () => getModalityCn(k) })) },
      )
    },
    filter: true,
    filterOptions: modalityFilterOptions.value
  },
  {
    title: () => h(NSpace, { align: 'center', size: 4 }, {
      default: () => [
        h('span', '工具'),
        h(NCheckbox, {
          checked: props.filters.tool_call,
          onUpdateChecked: (val: boolean) => {
            props.filters.tool_call = val
          }
        })
      ]
    }),
    key: 'tool_call',
    width: 90,
    render: (row) => (row.tool_call ? '✓' : ''),
  },
  {
    title: () => h(NSpace, { align: 'center', size: 4 }, {
      default: () => [
        h('span', '推理'),
        h(NCheckbox, {
          checked: props.filters.reasoning,
          onUpdateChecked: (val: boolean) => {
            props.filters.reasoning = val
          }
        })
      ]
    }),
    key: 'reasoning',
    width: 90,
    render: (row) => (row.reasoning ? '✓' : ''),
  },
  { title: '输入/输出单价', key: 'cost', width: 130, render: (row) => fmtCost(row) },
])

const rowKey = (row: CatalogModel) => row.id
</script>

<template>
  <div>
    <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 12px; flex-wrap: wrap; gap: 8px;">
      <n-space :size="8" align="center">
        <n-button size="small" @click="emit('reset')">重置</n-button>
        <n-button size="small" secondary type="primary" @click="emit('select-all', displayModels.map((m) => m.id))">
          选择所有过滤出的模型
        </n-button>
        <n-button size="small" secondary type="error" @click="emit('deselect-all')">
          取消选择全部
        </n-button>
        <n-button size="small" quaternary @click="emit('refresh')">
          ↻ 刷新
        </n-button>
        <n-text depth="3" style="font-size: 13px; margin-left: 8px; line-height: 28px;">
          显示 <strong>{{ displayMatched }}</strong> 个模型（共 {{ props.total }} 个）
        </n-text>
      </n-space>
      <n-input
        v-model:value="searchVal"
        placeholder="搜索模型 ID / 名称…"
        clearable
        style="width: 240px"
        size="small"
        @clear="onClearSearch"
      />
    </div>
    <n-data-table
      v-model:filters="tableFilters"
      :columns="columns"
      :data="displayModels"
      :row-key="rowKey"
      :checked-row-keys="checkedKeys"
      :loading="props.loading"
      :max-height="460"
      :scroll-x="1000"
      :filter-icon-popover-props="{ trigger: 'hover' }"
      virtual-scroll
      size="small"
      @update:checked-row-keys="onUpdateChecked"
      @update:filters="handleUpdateFilters"
    />
  </div>
</template>

<style scoped>
:deep(.n-data-table-th.n-data-table-th--filterable) {
  white-space: nowrap;
}
:deep(.n-data-table-th.n-data-table-th--filterable .n-data-table-th__title-wrapper) {
  display: inline-flex !important;
  vertical-align: middle;
  justify-content: flex-start !important;
}
:deep(.n-data-table-th.n-data-table-th--filterable .n-data-table-filter) {
  position: static !important;
  display: inline-flex !important;
  vertical-align: middle;
  margin-left: 6px !important;
  transform: none !important;
}
</style>
