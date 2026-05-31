<script setup lang="ts">
import {
  NButton,
  NCollapse,
  NCollapseItem,
  NEmpty,
  NSpace,
  NTag,
  NText,
  NScrollbar,
} from 'naive-ui'

import type { SelectedModel } from '@/api/types'

const props = defineProps<{ selected: SelectedModel[] }>()
const emit = defineEmits<{
  (e: 'deselect', id: string): void
  (e: 'open', model: SelectedModel): void
  (e: 'clear-all'): void
}>()
</script>

<template>
  <n-collapse :default-expanded-names="['selected']">
    <n-collapse-item name="selected">
      <template #header>
        <n-space align="center" :size="8">
          <strong>已选模型</strong>
          <n-tag round size="small" type="primary">{{ props.selected.length }}</n-tag>
        </n-space>
      </template>
      <template #header-extra>
        <n-button
          v-if="props.selected.length"
          size="tiny"
          tertiary
          type="error"
          @click.stop="emit('clear-all')"
        >
          清除全部
        </n-button>
      </template>

      <n-empty v-if="!props.selected.length" size="small" description="暂无已选模型" />
      <n-scrollbar v-else style="max-height: 280px; padding-right: 12px;">
        <div
          v-for="m in props.selected"
          :key="m.model_id"
          style="
            display: flex;
            align-items: center;
            justify-content: space-between;
            padding: 6px 0;
            border-bottom: 1px solid var(--n-border-color, #eee);
          "
        >
          <n-space align="center" :size="8">
            <n-text strong>{{ m.display_name || m.model_id }}</n-text>
            <n-text v-if="m.display_name" depth="3" style="font-size: 12px">{{ m.model_id }}</n-text>
            <n-tag v-if="!m.is_enabled" size="tiny">已禁用</n-tag>
            <n-tag v-if="m.has_custom_config" size="tiny" type="info">自定义</n-tag>
            <n-tag v-if="!m.metadata_known" size="tiny" type="warning">无元数据</n-tag>
          </n-space>
          <n-space :size="6">
            <n-button size="tiny" @click="emit('open', m)">编辑</n-button>
            <n-button size="tiny" tertiary type="error" @click="emit('deselect', m.model_id)">
              移除
            </n-button>
          </n-space>
        </div>
      </n-scrollbar>
    </n-collapse-item>
  </n-collapse>
</template>

