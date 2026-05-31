<script setup lang="ts">
import { reactive, ref, watch } from 'vue'
import {
  NButton,
  NCode,
  NDescriptions,
  NDescriptionsItem,
  NDrawer,
  NDrawerContent,
  NFormItem,
  NInput,
  NInputNumber,
  NSelect,
  NCheckboxGroup,
  NCheckbox,
  NGrid,
  NGi,
  NCollapse,
  NCollapseItem,
  NSpace,
  NSwitch,
  NTag,
  useMessage,
} from 'naive-ui'

import { api } from '@/api/client'
import type { CatalogModel, SelectedModel } from '@/api/types'
import { fmtContext, fmtCost } from '@/utils/format'

const props = defineProps<{
  providerId: string
  model: CatalogModel | null
  detail: SelectedModel | null
}>()
const show = defineModel<boolean>('show', { required: true })
const emit = defineEmits<{ (e: 'changed'): void }>()

const message = useMessage()
const saving = ref(false)

const displayName = ref('')
const enabled = ref(true)

type OverridePatch = {
  family?: string
  status?: string | null
  attachment?: boolean
  reasoning?: boolean
  tool_call?: boolean
  temperature?: boolean
  experimental?: boolean
  limit?: {
    context?: number
    input?: number
    output?: number
  }
  cost?: {
    input?: number
    output?: number
    cache_read?: number
    cache_write?: number
  }
  modalities?: {
    input?: string[]
    output?: string[]
  }
  interleaved?: boolean | string
}

const familyOptions = [
  { label: 'GPT-4', value: 'gpt-4' },
  { label: 'GPT-3.5', value: 'gpt-3.5' },
  { label: 'Claude 3', value: 'claude-3' },
  { label: 'Gemini 2.5', value: 'gemini-2.5' },
  { label: 'DeepSeek', value: 'deepseek' },
  { label: 'Qwen', value: 'qwen' },
  { label: 'Llama', value: 'llama' },
  { label: 'Mistral', value: 'mistral' },
]

const statusOptions = [
  { label: '活跃 (active)', value: 'active' },
  { label: '测试 (beta)', value: 'beta' },
  { label: '内测 (alpha)', value: 'alpha' },
  { label: '已废弃 (deprecated)', value: 'deprecated' },
]

const interleavedOptions = [
  { label: '默认/不开启', value: '' },
  { label: '开启交错 (true)', value: 'true' },
  { label: 'reasoning_content', value: 'reasoning_content' },
  { label: 'reasoning_details', value: 'reasoning_details' },
]

const modalityOptions = [
  { label: '文本 (text)', value: 'text' },
  { label: '图片 (image)', value: 'image' },
  { label: '音频 (audio)', value: 'audio' },
  { label: '视频 (video)', value: 'video' },
  { label: 'PDF (pdf)', value: 'pdf' },
]

const triStateOptions = [
  { label: '默认 (不重写)', value: 'null' },
  { label: '支持 (true)', value: 'true' },
  { label: '不支持 (false)', value: 'false' },
]

const nonWritableModelFields = [
  'id',
  'knowledge',
  'last_updated',
  'open_weights',
  'structured_output',
  'experimental',
  'interleaved',
  'provider',
  'name',
  '_meta',
]

const nonWritableCostFields = [
  'context_over_200k',
  'tiers',
  'input_audio',
  'output_audio',
  'reasoning',
]

const form = reactive({
  family: '',
  status: null as string | null,
  attachment: 'null',
  reasoning: 'null',
  tool_call: 'null',
  temperature: 'null',
  experimental: 'null',
  context: null as number | null,
  input_limit: null as number | null,
  output_limit: null as number | null,
  cost_input: null as number | null,
  cost_output: null as number | null,
  cost_cache_read: null as number | null,
  cost_cache_write: null as number | null,
  input_modalities: [] as string[],
  output_modalities: [] as string[],
  interleaved: null as string | null,
})

// Re-seed the override form whenever a different model is opened.
watch(
  () => props.model?.id,
  () => {
    displayName.value = props.detail?.display_name ?? ''
    enabled.value = props.detail?.is_enabled ?? true
    
    const patchObj = (props.detail?.override_patch || {}) as OverridePatch
    
    form.family = patchObj.family ?? ''
    form.status = patchObj.status ?? null
    
    form.attachment = patchObj.attachment === true ? 'true' : patchObj.attachment === false ? 'false' : 'null'
    form.reasoning = patchObj.reasoning === true ? 'true' : patchObj.reasoning === false ? 'false' : 'null'
    form.tool_call = patchObj.tool_call === true ? 'true' : patchObj.tool_call === false ? 'false' : 'null'
    form.temperature = patchObj.temperature === true ? 'true' : patchObj.temperature === false ? 'false' : 'null'
    form.experimental = patchObj.experimental === true ? 'true' : patchObj.experimental === false ? 'false' : 'null'
    
    form.context = patchObj.limit?.context ?? null
    form.input_limit = patchObj.limit?.input ?? null
    form.output_limit = patchObj.limit?.output ?? null
    
    form.cost_input = patchObj.cost?.input ?? null
    form.cost_output = patchObj.cost?.output ?? null
    form.cost_cache_read = patchObj.cost?.cache_read ?? null
    form.cost_cache_write = patchObj.cost?.cache_write ?? null
    
    form.input_modalities = patchObj.modalities?.input ?? []
    form.output_modalities = patchObj.modalities?.output ?? []
    
    if (patchObj.interleaved === true) {
      form.interleaved = 'true'
    } else if (typeof patchObj.interleaved === 'string') {
      form.interleaved = patchObj.interleaved
    } else {
      form.interleaved = null
    }
  },
  { immediate: true }
)

// Translate model status to Chinese
function fmtStatus(status?: string): string {
  if (!status) return ''
  const mapping: Record<string, string> = {
    active: '活跃',
    beta: '测试',
    alpha: '内测',
    deprecated: '废弃',
  }
  return mapping[status] || status
}

function fmtSource(src: string): string {
  const mapping: Record<string, string> = {
    'models.dev': 'models.dev',
    'local-library': '本地模型库',
    'openrouter': 'OpenRouter',
    'provider-only': '仅服务商',
  }
  return mapping[src] || src
}

function effectivePreview(): string {
  if (!props.model) return ''
  const source = props.detail?.effective ?? props.model
  const entry = { ...source } as Record<string, unknown>
  for (const key of nonWritableModelFields) {
    delete entry[key]
  }
  entry.name = props.model.id
  const cost = entry.cost
  if (cost && typeof cost === 'object' && !Array.isArray(cost)) {
    const cleanCost = { ...(cost as Record<string, unknown>) }
    for (const key of nonWritableCostFields) {
      delete cleanCost[key]
    }
    entry.cost = cleanCost
  }
  return JSON.stringify(entry, null, 2)
}

async function selectModel() {
  if (!props.model) return
  try {
    await api.select(props.providerId, [props.model.id])
    message.success(`已选择 ${props.model.id}`)
    emit('changed')
  } catch (e) {
    message.error((e as Error).message)
  }
}

async function deselectModel() {
  if (!props.model) return
  try {
    await api.deselect(props.providerId, [props.model.id])
    message.success(`已移除 ${props.model.id}`)
    emit('changed')
    show.value = false
  } catch (e) {
    message.error((e as Error).message)
  }
}

async function saveConfig() {
  if (!props.model) return
  saving.value = true
  try {
    const patch: Record<string, any> = {}
    
    if (form.family) patch.family = form.family
    if (form.status) patch.status = form.status
    
    if (form.attachment === 'true') patch.attachment = true
    else if (form.attachment === 'false') patch.attachment = false
    
    if (form.reasoning === 'true') patch.reasoning = true
    else if (form.reasoning === 'false') patch.reasoning = false
    
    if (form.tool_call === 'true') patch.tool_call = true
    else if (form.tool_call === 'false') patch.tool_call = false
    
    if (form.temperature === 'true') patch.temperature = true
    else if (form.temperature === 'false') patch.temperature = false
    
    if (form.experimental === 'true') patch.experimental = true
    else if (form.experimental === 'false') patch.experimental = false
    
    // limit
    const limit: Record<string, any> = {}
    if (form.context !== null) limit.context = form.context
    if (form.input_limit !== null) limit.input = form.input_limit
    if (form.output_limit !== null) limit.output = form.output_limit
    if (Object.keys(limit).length > 0) patch.limit = limit

    // cost
    const cost: Record<string, any> = {}
    if (form.cost_input !== null) cost.input = form.cost_input
    if (form.cost_output !== null) cost.output = form.cost_output
    if (form.cost_cache_read !== null) cost.cache_read = form.cost_cache_read
    if (form.cost_cache_write !== null) cost.cache_write = form.cost_cache_write
    if (Object.keys(cost).length > 0) patch.cost = cost

    // modalities
    const modalities: Record<string, any> = {}
    if (form.input_modalities.length > 0) modalities.input = form.input_modalities
    if (form.output_modalities.length > 0) modalities.output = form.output_modalities
    if (Object.keys(modalities).length > 0) patch.modalities = modalities

    // interleaved
    if (form.interleaved) {
      if (form.interleaved === 'true') {
        patch.interleaved = true
      } else {
        patch.interleaved = form.interleaved
      }
    }

    const update = {
      display_name: displayName.value,
      is_enabled: enabled.value,
      override_patch: patch,
    }
    
    await api.updateSelected(props.providerId, props.model.id, update)
    message.success('重写配置已保存')
    emit('changed')
  } catch (e) {
    message.error((e as Error).message)
  } finally {
    saving.value = false
  }
}

function getModalityCn(kind: string): string {
  const mapping: Record<string, string> = {
    text: '文本',
    image: '图片',
    audio: '音频',
    video: '视频',
    pdf: 'PDF',
  }
  return mapping[kind] || kind
}
</script>

<template>
  <n-drawer v-model:show="show" :width="460">
    <n-drawer-content v-if="model" :title="model.id" closable>
      <n-space align="center" :size="8" style="margin-bottom: 12px">
        <n-tag :type="model._meta.is_selected ? 'success' : 'default'" size="small">
          {{ model._meta.is_selected ? '已选择' : '未选择' }}
        </n-tag>
        <n-tag size="small" :type="model._meta.metadata_known ? 'info' : 'warning'">
          {{ fmtSource(model._meta.source) }}
        </n-tag>

        <n-tag v-if="model.status" size="small">{{ fmtStatus(model.status) }}</n-tag>
      </n-space>

      <n-descriptions label-placement="left" :column="1" size="small" bordered>
        <n-descriptions-item label="上下文">
          {{ fmtContext(model.limit?.context) }}
        </n-descriptions-item>
        <n-descriptions-item label="最大输出">
          {{ model.limit?.output ?? '—' }}
        </n-descriptions-item>
        <n-descriptions-item label="输入模态">
          <n-space :size="4" v-if="model.modalities?.input?.length">
            <n-tag
              v-for="k in model.modalities.input"
              :key="k"
              size="tiny"
              type="info"
              :bordered="false"
            >
              {{ getModalityCn(k) }}
            </n-tag>
          </n-space>
          <n-text depth="3" v-else>文本</n-text>
        </n-descriptions-item>
        <n-descriptions-item label="工具调用">{{ model.tool_call ? '是' : '否' }}</n-descriptions-item>
        <n-descriptions-item label="推理能力">{{ model.reasoning ? '是' : '否' }}</n-descriptions-item>
        <n-descriptions-item label="附件支持">{{ model.attachment ? '是' : '否' }}</n-descriptions-item>
        <n-descriptions-item label="输入/输出单价">{{ fmtCost(model) }}</n-descriptions-item>
      </n-descriptions>

      <template v-if="model._meta.is_selected">
        <h4 style="margin: 18px 0 8px">用户自定义重写</h4>
        <n-form-item label="显示名称" :show-feedback="false" style="margin-bottom: 12px">
          <n-input v-model:value="displayName" placeholder="(默认使用 models.dev 名称)" />
        </n-form-item>
        <n-form-item label="已启用" :show-feedback="false" style="margin-bottom: 12px">
          <n-switch v-model:value="enabled" />
        </n-form-item>

        <n-collapse style="margin-top: 12px">
          <n-collapse-item title="高级属性重写 (可选项)" name="advanced">
            <n-space vertical :size="12">
              <n-form-item label="模型系列 (family)" :show-feedback="false">
                <n-select
                  v-model:value="form.family"
                  :options="familyOptions"
                  placeholder="选择或输入模型系列"
                  filterable
                  tag
                />
              </n-form-item>

              <n-form-item label="模型状态 (status)" :show-feedback="false">
                <n-select
                  v-model:value="form.status"
                  :options="statusOptions"
                  placeholder="选择模型状态"
                  clearable
                />
              </n-form-item>

              <n-form-item label="功能支持 (attachment / reasoning / tools 等)" :show-feedback="false">
                <n-grid :cols="1" :y-gap="8">
                  <n-gi>
                    <n-space justify="space-between" align="center" style="width: 100%">
                      <span>支持附件 (attachment)</span>
                      <n-select v-model:value="form.attachment" :options="triStateOptions" style="width: 140px" />
                    </n-space>
                  </n-gi>
                  <n-gi>
                    <n-space justify="space-between" align="center" style="width: 100%">
                      <span>支持推理 (reasoning)</span>
                      <n-select v-model:value="form.reasoning" :options="triStateOptions" style="width: 140px" />
                    </n-space>
                  </n-gi>
                  <n-gi>
                    <n-space justify="space-between" align="center" style="width: 100%">
                      <span>工具调用 (tool_call)</span>
                      <n-select v-model:value="form.tool_call" :options="triStateOptions" style="width: 140px" />
                    </n-space>
                  </n-gi>
                  <n-gi>
                    <n-space justify="space-between" align="center" style="width: 100%">
                      <span>温度参数 (temperature)</span>
                      <n-select v-model:value="form.temperature" :options="triStateOptions" style="width: 140px" />
                    </n-space>
                  </n-gi>
                  <n-gi>
                    <n-space justify="space-between" align="center" style="width: 100%">
                      <span>实验性模型 (experimental)</span>
                      <n-select v-model:value="form.experimental" :options="triStateOptions" style="width: 140px" />
                    </n-space>
                  </n-gi>
                </n-grid>
              </n-form-item>

              <n-form-item label="限制配置 (limit)" :show-feedback="false">
                <n-grid :cols="3" :x-gap="8">
                  <n-gi>
                    <n-form-item label="最大上下文" :show-feedback="false">
                      <n-input-number v-model:value="form.context" placeholder="无" :show-button="false" clearable />
                    </n-form-item>
                  </n-gi>
                  <n-gi>
                    <n-form-item label="最大输入" :show-feedback="false">
                      <n-input-number v-model:value="form.input_limit" placeholder="无" :show-button="false" clearable />
                    </n-form-item>
                  </n-gi>
                  <n-gi>
                    <n-form-item label="最大输出" :show-feedback="false">
                      <n-input-number v-model:value="form.output_limit" placeholder="无" :show-button="false" clearable />
                    </n-form-item>
                  </n-gi>
                </n-grid>
              </n-form-item>

              <n-form-item label="单价配置 (cost，每百万 tokens 美元)" :show-feedback="false">
                <n-grid :cols="2" :x-gap="8" :y-gap="8">
                  <n-gi>
                    <n-form-item label="输入单价" :show-feedback="false">
                      <n-input-number v-model:value="form.cost_input" placeholder="无" :show-button="false" :precision="4" clearable />
                    </n-form-item>
                  </n-gi>
                  <n-gi>
                    <n-form-item label="输出单价" :show-feedback="false">
                      <n-input-number v-model:value="form.cost_output" placeholder="无" :show-button="false" :precision="4" clearable />
                    </n-form-item>
                  </n-gi>
                  <n-gi>
                    <n-form-item label="缓存读取单价" :show-feedback="false">
                      <n-input-number v-model:value="form.cost_cache_read" placeholder="无" :show-button="false" :precision="4" clearable />
                    </n-form-item>
                  </n-gi>
                  <n-gi>
                    <n-form-item label="缓存写入单价" :show-feedback="false">
                      <n-input-number v-model:value="form.cost_cache_write" placeholder="无" :show-button="false" :precision="4" clearable />
                    </n-form-item>
                  </n-gi>
                </n-grid>
              </n-form-item>

              <n-form-item label="输入模态 (input modalities)" :show-feedback="false">
                <n-checkbox-group v-model:value="form.input_modalities">
                  <n-space>
                    <n-checkbox v-for="opt in modalityOptions" :key="opt.value" :value="opt.value">
                      {{ opt.label }}
                    </n-checkbox>
                  </n-space>
                </n-checkbox-group>
              </n-form-item>

              <n-form-item label="输出模态 (output modalities)" :show-feedback="false">
                <n-checkbox-group v-model:value="form.output_modalities">
                  <n-space>
                    <n-checkbox v-for="opt in modalityOptions" :key="opt.value" :value="opt.value">
                      {{ opt.label }}
                    </n-checkbox>
                  </n-space>
                </n-checkbox-group>
              </n-form-item>

              <n-form-item label="交错推理内容 (interleaved)" :show-feedback="false">
                <n-select
                  v-model:value="form.interleaved"
                  :options="interleavedOptions"
                  placeholder="选择推理内容是否交错"
                  clearable
                />
              </n-form-item>
            </n-space>
          </n-collapse-item>
        </n-collapse>
      </template>

      <h4 style="margin: 18px 0 8px">生效配置 (将写入 opencode.json)</h4>
      <n-code :code="effectivePreview()" language="json" word-wrap />

      <template #footer>
        <n-space>
          <template v-if="model._meta.is_selected">
            <n-button type="primary" :loading="saving" @click="saveConfig">保存配置</n-button>
            <n-button tertiary type="error" @click="deselectModel">取消选择</n-button>
          </template>
          <n-button v-else type="primary" @click="selectModel">选择此模型</n-button>
        </n-space>
      </template>
    </n-drawer-content>
  </n-drawer>
</template>
