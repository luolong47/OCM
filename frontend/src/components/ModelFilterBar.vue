<script setup lang="ts">
import {
  NButton,
  NCheckbox,
  NSelect,
  NSpace,
  NText,
} from 'naive-ui'

import type { ModelFilters } from '@/api/types'

// `filters` is a shared reactive object owned by the parent; binding v-model to its
// fields here mutates it in place, and the parent re-fetches on `apply`.
const props = defineProps<{
  filters: ModelFilters
  matched: number
  total: number
  loading?: boolean
}>()

const emit = defineEmits<{
  (e: 'apply'): void
  (e: 'reset'): void
  (e: 'select-all'): void
  (e: 'deselect-all'): void
  (e: 'refresh'): void
}>()

const contextOptions = [
  { label: '任意上下文长度', value: 0 },
  { label: '≥ 32K', value: 32_000 },
  { label: '≥ 128K', value: 128_000 },
  { label: '≥ 200K', value: 200_000 },
  { label: '≥ 1M', value: 1_000_000 },
]

</script>

<template>
  <div>
    <n-space align="center" :wrap="true" :size="12">
      <n-checkbox v-model:checked="props.filters.support_image">图片</n-checkbox>
      <n-checkbox v-model:checked="props.filters.tool_call">工具调用</n-checkbox>
      <n-checkbox v-model:checked="props.filters.reasoning">推理能力</n-checkbox>
      <n-select
        v-model:value="props.filters.min_context"
        :options="contextOptions"
        style="width: 150px"
      />
    </n-space>

    <n-space align="center" justify="space-between" style="margin-top: 12px">
      <n-space :size="8">
        <n-button type="primary" :loading="props.loading" @click="emit('apply')">
          应用筛选
        </n-button>
        <n-button @click="emit('reset')">重置</n-button>
        <n-button secondary type="primary" @click="emit('select-all')">
          选择所有过滤出的模型
        </n-button>
        <n-button secondary type="error" @click="emit('deselect-all')">取消选择全部</n-button>
        <n-button quaternary @click="emit('refresh')">↻ 刷新列表</n-button>
      </n-space>
      <n-text depth="3">
        显示 <strong>{{ props.matched }}</strong> 个模型（共 {{ props.total }} 个）
      </n-text>
    </n-space>
  </div>
</template>

