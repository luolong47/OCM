<script setup lang="ts">
import { ref } from 'vue'
import { NButton, NModal, NSpace, NText, useMessage } from 'naive-ui'

import { api } from '@/api/client'
import type { ModelFilters } from '@/api/types'

const props = defineProps<{ providerId: string }>()
const show = defineModel<boolean>('show', { required: true })
const emit = defineEmits<{ (e: 'changed'): void }>()

const message = useMessage()
const busy = ref(false)

const presets: Array<{ label: string; filters: ModelFilters }> = [
  { label: '所有图片 / 视觉模型', filters: { support_image: true } },
  { label: '所有工具调用模型', filters: { tool_call: true } },
  { label: '所有推理模型', filters: { reasoning: true } },
  { label: '所有 128K+ 上下文模型', filters: { min_context: 128_000 } },
  { label: '所有 1M+ 上下文模型', filters: { min_context: 1_000_000 } },
]

async function apply(filters: ModelFilters, label: string) {
  busy.value = true
  try {
    const { selected } = await api.selectAllFiltered(props.providerId, filters)
    message.success(`${label}: 已选择 ${selected} 个模型`)
    emit('changed')
  } catch (e) {
    message.error((e as Error).message)
  } finally {
    busy.value = false
  }
}

async function clearAll() {
  busy.value = true
  try {
    const { deselected } = await api.deselectAll(props.providerId)
    message.success(`已清除 ${deselected} 个已选模型`)
    emit('changed')
  } catch (e) {
    message.error((e as Error).message)
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <n-modal
    v-model:show="show"
    preset="card"
    title="批量选择"
    style="max-width: 460px"
  >
    <n-text depth="3">
      快速预设会选中整个目录中所有匹配的模型（而不仅仅是当前页）。
    </n-text>
    <n-space vertical :size="10" style="margin-top: 16px">
      <n-button
        v-for="p in presets"
        :key="p.label"
        block
        :loading="busy"
        @click="apply(p.filters, p.label)"
      >
        {{ p.label }}
      </n-button>
      <n-button block secondary type="error" :loading="busy" @click="clearAll">
        取消选择全部
      </n-button>
    </n-space>
  </n-modal>
</template>

