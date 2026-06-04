<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import {
  NButton,
  NCard,
  NEmpty,
  NIcon,
  NPopconfirm,
  NSpace,
  NSpin,
  NTag,
  NTable,
  useMessage,
} from 'naive-ui'
import {
  RefreshCw,
  Download,
  Plus,
  Minus,
  Pencil,
  Trash2,
  List,
} from 'lucide-vue-next'

import { api } from '@/api/client'
import type { ModelsDevStatus } from '@/api/types'
import { useProviderStore } from '@/stores/providers'

const router = useRouter()
const message = useMessage()
const store = useProviderStore()
const modelsDevStatus = ref<ModelsDevStatus | null>(null)

const modelsDevRefreshText = computed(() => {
  if (!modelsDevStatus.value) return 'models.dev 尚未刷新'
  return `models.dev 上次刷新：${modelsDevStatus.value.refreshed_at}，${modelsDevStatus.value.model_count} 个模型`
})

onMounted(async () => {
  await Promise.all([store.load(), loadModelsDevStatus()])
})

async function loadModelsDevStatus() {
  try {
    modelsDevStatus.value = await api.modelsDevStatus()
  } catch {
    modelsDevStatus.value = null
  }
}

async function refreshModelsDev() {
  try {
    const { indexed_model_count, status } = await api.refreshModelsDev()
    modelsDevStatus.value = status
    message.success(`models.dev 已刷新 — 原始 ${status?.model_count ?? indexed_model_count} 个模型，索引 ${indexed_model_count} 个模型`)
  } catch (e) {
    message.error((e as Error).message)
  }
}

async function remove(id: string) {
  try {
    await api.deleteProvider(id)
    message.success(`已删除服务商 '${id}'`)
    await store.load()
  } catch (e) {
    message.error((e as Error).message)
  }
}

async function importConfig() {
  try {
    const report = await api.importConfig()
    message.success(`成功导入 ${report.providers_imported} 个服务商，${report.models_imported} 个模型配置`)
    await store.load()
  } catch (e) {
    message.error((e as Error).message)
  }
}

async function applyProvider(id: string) {
  try {
    const report = await api.applyProvider(id)
    message.success(`成功将服务商 ${id} 的配置应用到 ${report.path}`)
    await store.load()
  } catch (e) {
    message.error((e as Error).message)
  }
}

async function unapplyProvider(id: string) {
  try {
    const report = await api.unapplyProvider(id)
    message.success(`已取消应用服务商 ${id} 的配置`)
    await store.load()
  } catch (e) {
    message.error((e as Error).message)
  }
}
</script>

<template>
  <div>
    <n-space justify="space-between" align="center" style="margin-bottom: 16px">
      <h2 style="margin: 0">服务商</h2>
      <n-space>
        <span style="font-size: 13px; color: #888; line-height: 34px">
          {{ modelsDevRefreshText }}
        </span>
        <n-button
          circle
          title="刷新 models.dev"
          @click="refreshModelsDev"
        >
          <template #icon>
            <n-icon :component="RefreshCw" />
          </template>
        </n-button>
        <n-button
          type="info"
          ghost
          circle
          title="从配置导入"
          @click="importConfig"
        >
          <template #icon>
            <n-icon :component="Download" />
          </template>
        </n-button>
        <n-button
          type="primary"
          circle
          title="添加服务商"
          @click="router.push('/providers/new')"
        >
          <template #icon>
            <n-icon :component="Plus" />
          </template>
        </n-button>
      </n-space>
    </n-space>

    <n-spin :show="store.loading">
      <n-empty
        v-if="!store.providers.length"
        description="暂无服务商，请先添加服务商以选择模型。"
        style="padding: 48px 0"
      />
      <n-card v-else content-style="padding: 0; overflow-x: auto;" :bordered="true">
        <n-table :bordered="false" :single-line="false" style="border-collapse: collapse; min-width: 640px;">
          <thead>
            <tr>
              <th style="width: 180px; font-weight: 600; padding: 12px 16px; white-space: nowrap;">服务商 ID</th>
              <th style="width: 130px; font-weight: 600; padding: 12px 16px; white-space: nowrap;">显示名称</th>
              <th style="font-weight: 600; padding: 12px 16px; min-width: 180px;">接口地址 (Base URL)</th>
              <th style="width: 160px; font-weight: 600; padding: 12px 16px; text-align: right; white-space: nowrap;">操作</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="p in store.providers" :key="p.id">
              <td style="padding: 12px 16px; vertical-align: middle; white-space: nowrap;">
                <span style="display: inline-flex; align-items: center; gap: 8px;">
                  <span :style="{
                    display: 'inline-block',
                    width: '8px',
                    height: '8px',
                    borderRadius: '50%',
                    backgroundColor: p.is_applied && !p.needs_reapply ? '#18a058'
                                   : p.is_applied && p.needs_reapply  ? '#f0a020'
                                   : '#c2c2c2'
                  }" :title="p.is_applied && !p.needs_reapply ? '已应用'
                            : p.is_applied && p.needs_reapply  ? '配置已变更，需要重新应用'
                            : '未应用'"
                  ></span>
                  <code>{{ p.id }}</code>
                  <n-tag v-if="p.source === 'external'" size="tiny" type="warning" :bordered="false" style="font-size: 11px; padding: 0 4px; height: 16px; line-height: 16px;">
                    外部
                  </n-tag>
                </span>
              </td>
              <td style="padding: 12px 16px; vertical-align: middle; white-space: nowrap;">
                <strong>{{ p.name }}</strong>
              </td>
              <td style="padding: 12px 16px; vertical-align: middle; max-width: 220px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">
                <span style="font-size: 13px; color: #666;" :title="p.base_url || ''">
                  {{ p.base_url || '— 无 —' }}
                </span>
              </td>
              <td style="width: 160px; padding: 12px 16px; vertical-align: middle; text-align: right; white-space: nowrap;">
                <div style="display: flex; justify-content: flex-end; align-items: center; gap: 8px;">
                  <n-button
                    size="small"
                    quaternary
                    circle
                    type="primary"
                    title="管理模型"
                    @click="router.push(`/providers/${encodeURIComponent(p.id)}/models`)"
                  >
                    <template #icon>
                      <n-icon :component="List" />
                    </template>
                  </n-button>
                  <!-- 未应用 → 应用 -->
                  <n-button
                    v-if="!p.is_applied"
                    size="small"
                    quaternary
                    circle
                    type="success"
                    title="应用配置"
                    @click="applyProvider(p.id)"
                  >
                    <template #icon>
                      <n-icon :component="Plus" />
                    </template>
                  </n-button>
                  <!-- 已应用且配置有变更 → 重新应用 -->
                  <n-button
                    v-else-if="p.needs_reapply"
                    size="small"
                    quaternary
                    circle
                    type="warning"
                    title="配置已变更，点击重新应用"
                    @click="applyProvider(p.id)"
                  >
                    <template #icon>
                      <n-icon :component="RefreshCw" />
                    </template>
                  </n-button>
                  <!-- 已应用且无变更 → 取消应用 -->
                  <n-button
                    v-else
                    size="small"
                    quaternary
                    circle
                    type="warning"
                    title="取消应用"
                    @click="unapplyProvider(p.id)"
                  >
                    <template #icon>
                      <n-icon :component="Minus" />
                    </template>
                  </n-button>
                  <n-button
                    size="small"
                    quaternary
                    circle
                    type="info"
                    title="编辑"
                    @click="router.push(`/providers/${encodeURIComponent(p.id)}/edit`)"
                  >
                    <template #icon>
                      <n-icon :component="Pencil" />
                    </template>
                  </n-button>
                  <n-popconfirm
                    negative-text="取消"
                    positive-text="确认"
                    @positive-click="remove(p.id)"
                  >
                    <template #trigger>
                      <n-button size="small" quaternary circle type="error" title="删除">
                        <template #icon>
                          <n-icon :component="Trash2" />
                        </template>
                      </n-button>
                    </template>
                    确定要删除服务商 '{{ p.id }}' 及其已选模型吗？
                  </n-popconfirm>
                </div>
              </td>
            </tr>
          </tbody>
        </n-table>
      </n-card>
    </n-spin>
  </div>
</template>
