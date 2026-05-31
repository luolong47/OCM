<script setup lang="ts">
import { onMounted, reactive, ref } from 'vue'
import { useRouter } from 'vue-router'
import {
  NButton,
  NForm,
  NFormItem,
  NIcon,
  NInput,
  NSelect,
  NSpace,
  NSwitch,
  NGrid,
  NGi,
  useMessage,
} from 'naive-ui'
import { ArrowLeft, Save, X } from 'lucide-vue-next'

import { api } from '@/api/client'
import type { ProviderInput } from '@/api/types'

const props = defineProps<{ id?: string }>()
const router = useRouter()
const message = useMessage()

const isEdit = ref(Boolean(props.id))
const saving = ref(false)

const form = reactive({
  id: '',
  name: '',
  npm: '@ai-sdk/openai-compatible',
  base_url: '',
  api_key_env: '',
  api_key: '',
  models_dev_key: '',
  headers: '',
  options: '',
  enabled: true,
  // Individual option fields
  setCacheKey: false,
  enterpriseUrl: '',
  timeout: '',
  headerTimeout: '',
  chunkTimeout: '',
})

const npmOptions = [
  { label: '@ai-sdk/openai-compatible (/chat/completions)', value: '@ai-sdk/openai-compatible' },
  { label: '@ai-sdk/openai (/responses)', value: '@ai-sdk/openai' },
  { label: '@ai-sdk/anthropic (/messages)', value: '@ai-sdk/anthropic' },
]

onMounted(async () => {
  if (!props.id) return
  try {
    const p = await api.getProvider(props.id)
    
    let optionsJson: Record<string, any> = {}
    try {
      if (p.options) {
        optionsJson = typeof p.options === 'string' ? JSON.parse(p.options) : p.options
      }
    } catch {}

    const setCacheKey = optionsJson.setCacheKey ?? false
    const enterpriseUrl = optionsJson.enterpriseUrl ?? ''
    
    let timeout = ''
    if (optionsJson.timeout === false) {
      timeout = 'false'
    } else if (typeof optionsJson.timeout === 'number') {
      timeout = optionsJson.timeout.toString()
    }
    
    let headerTimeout = ''
    if (optionsJson.headerTimeout === false) {
      headerTimeout = 'false'
    } else if (typeof optionsJson.headerTimeout === 'number') {
      headerTimeout = optionsJson.headerTimeout.toString()
    }
    
    let chunkTimeout = ''
    if (typeof optionsJson.chunkTimeout === 'number') {
      chunkTimeout = optionsJson.chunkTimeout.toString()
    }

    // Remove extracted keys to leave only extra custom options in the JSON editor
    delete optionsJson.setCacheKey
    delete optionsJson.enterpriseUrl
    delete optionsJson.timeout
    delete optionsJson.headerTimeout
    delete optionsJson.chunkTimeout

    const extraOptionsStr = Object.keys(optionsJson).length ? JSON.stringify(optionsJson, null, 2) : ''

    Object.assign(form, {
      id: p.id,
      name: p.name,
      npm: p.npm,
      base_url: p.base_url ?? '',
      api_key_env: p.api_key_env ?? '',
      api_key: p.api_key ?? '',
      models_dev_key: p.models_dev_key ?? '',
      headers: p.headers ?? '',
      options: extraOptionsStr,
      enabled: p.enabled,
      setCacheKey,
      enterpriseUrl,
      timeout,
      headerTimeout,
      chunkTimeout,
    })
  } catch (e) {
    message.error((e as Error).message)
  }
})

function parseJsonField(label: string, raw: string): Record<string, unknown> | null {
  const trimmed = raw.trim()
  if (!trimmed) return null
  try {
    return JSON.parse(trimmed) as Record<string, unknown>
  } catch {
    throw new Error(`${label} 不是有效的 JSON 格式`)
  }
}

async function save() {
  saving.value = true
  try {
    let mergedOptions: Record<string, any> = parseJsonField('额外服务商配置', form.options) || {}
    
    if (form.setCacheKey) {
      mergedOptions.setCacheKey = true
    }
    
    const entUrl = form.enterpriseUrl.trim()
    if (entUrl) {
      mergedOptions.enterpriseUrl = entUrl
    }
    
    const tVal = form.timeout.trim()
    if (tVal) {
      if (tVal.toLowerCase() === 'false') {
        mergedOptions.timeout = false
      } else {
        const parsed = parseInt(tVal, 10)
        if (!isNaN(parsed)) {
          mergedOptions.timeout = parsed
        }
      }
    }
    
    const htVal = form.headerTimeout.trim()
    if (htVal) {
      if (htVal.toLowerCase() === 'false') {
        mergedOptions.headerTimeout = false
      } else {
        const parsed = parseInt(htVal, 10)
        if (!isNaN(parsed)) {
          mergedOptions.headerTimeout = parsed
        }
      }
    }
    
    const ctVal = form.chunkTimeout.trim()
    if (ctVal) {
      const parsed = parseInt(ctVal, 10)
      if (!isNaN(parsed)) {
        mergedOptions.chunkTimeout = parsed
      }
    }

    const optionsVal = Object.keys(mergedOptions).length ? mergedOptions : null

    const input: ProviderInput = {
      id: form.id.trim(),
      name: form.name.trim(),
      npm: form.npm,
      base_url: form.base_url.trim() || null,
      api_key_env: form.api_key_env.trim() || null,
      api_key: form.api_key.trim() || null,
      models_dev_key: form.models_dev_key.trim() || null,
      headers: parseJsonField('自定义请求头', form.headers),
      options: optionsVal,
      enabled: form.enabled,
    }
    if (!input.id || !input.name) {
      throw new Error('ID 和名称为必填项')
    }
    if (isEdit.value) {
      await api.updateProvider(input.id, input)
      message.success('服务商已更新')
    } else {
      await api.createProvider(input)
      message.success('服务商已创建')
    }
    router.push('/providers')
  } catch (e) {
    message.error((e as Error).message)
  } finally {
    saving.value = false
  }
}
</script>

<template>
  <div style="max-width: 680px">
    <n-space align="center" style="margin-bottom: 16px">
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
      <h2 style="margin: 0">{{ isEdit ? '编辑服务商' : '添加服务商' }}</h2>
    </n-space>

    <n-form label-placement="top">
      <n-form-item label="服务商 ID (opencode 键)">
        <n-input
          v-model:value="form.id"
          :disabled="isEdit"
          placeholder="例如 grok2api"
        />
      </n-form-item>
      <n-form-item label="显示名称">
        <n-input v-model:value="form.name" placeholder="例如 Grok (proxy)" />
      </n-form-item>
      <n-form-item label="SDK 包 (npm)">
        <n-select v-model:value="form.npm" :options="npmOptions" />
      </n-form-item>
      <n-form-item label="接口地址 (Base URL)">
        <n-input v-model:value="form.base_url" placeholder="https://api.example.com/v1" />
      </n-form-item>
      <n-form-item label="API 密钥 (明文 — 可选，本地存储)">
        <n-input
          v-model:value="form.api_key"
          type="text"
          placeholder="不推荐使用；建议优先使用环境变量"
        />
      </n-form-item>
      <n-form-item label="启用 promptCacheKey (setCacheKey)" label-placement="left">
        <n-switch v-model:value="form.setCacheKey" />
      </n-form-item>


      <n-space style="margin-top: 24px">
        <n-button
          type="primary"
          circle
          :loading="saving"
          :title="isEdit ? '保存修改' : '创建服务商'"
          @click="save"
        >
          <template #icon>
            <n-icon :component="Save" />
          </template>
        </n-button>
        <n-button
          circle
          title="取消"
          @click="router.push('/providers')"
        >
          <template #icon>
            <n-icon :component="X" />
          </template>
        </n-button>
      </n-space>
    </n-form>
  </div>
</template>

