<script setup lang="ts">
import { onMounted, ref } from 'vue'
import {
  NButton,
  NCard,
  NForm,
  NFormItem,
  NInput,
  NPopconfirm,
  NSpace,
  NSpin,
  NSwitch,
  useMessage,
} from 'naive-ui'

import { api } from '@/api/client'
import type { NutstoreSettings } from '@/api/types'

const message = useMessage()
const autostartEnabled = ref(false)
const autostartLoading = ref(false)
const nutstoreLoading = ref(false)
const nutstoreBackingUp = ref(false)
const nutstoreRestoring = ref(false)
const nutstoreForm = ref<NutstoreSettings>({
  enabled: false,
  server_url: 'https://dav.jianguoyun.com/dav/',
  username: '',
  password: '',
  remote_dir: '',
})

onMounted(async () => {
  await Promise.all([loadAutostart(), loadNutstoreSettings()])
})

async function loadAutostart() {
  try {
    autostartEnabled.value = (await api.getAutostart()).enabled
  } catch (e) {
    message.error((e as Error).message)
  }
}

async function setAutostart(enabled: boolean) {
  autostartLoading.value = true
  try {
    autostartEnabled.value = (await api.setAutostart(enabled)).enabled
    message.success(autostartEnabled.value ? '已开启开机自启' : '已关闭开机自启')
  } catch (e) {
    autostartEnabled.value = !enabled
    message.error((e as Error).message)
  } finally {
    autostartLoading.value = false
  }
}

async function loadNutstoreSettings() {
  nutstoreLoading.value = true
  try {
    nutstoreForm.value = await api.getNutstoreSettings()
  } catch (e) {
    message.error((e as Error).message)
  } finally {
    nutstoreLoading.value = false
  }
}

async function saveNutstoreSettings() {
  nutstoreLoading.value = true
  try {
    nutstoreForm.value = await api.setNutstoreSettings(nutstoreForm.value)
    message.success('坚果云设置已保存')
  } catch (e) {
    message.error((e as Error).message)
  } finally {
    nutstoreLoading.value = false
  }
}

async function backupToNutstore() {
  nutstoreBackingUp.value = true
  try {
    const report = await api.backupToNutstore()
    message.success(`已备份 OCM 数据库到坚果云：${report.remote_archive_url}`)
  } catch (e) {
    message.error((e as Error).message)
  } finally {
    nutstoreBackingUp.value = false
  }
}

async function restoreFromNutstore() {
  nutstoreRestoring.value = true
  try {
    const report = await api.restoreFromNutstore()
    message.success(
      `已从最近备份恢复：${report.providers_restored} 个服务商，${report.selected_models_restored} 个已选模型；当前均为未应用状态`,
    )
  } catch (e) {
    message.error((e as Error).message)
  } finally {
    nutstoreRestoring.value = false
  }
}
</script>

<template>
  <div>
    <h2 style="margin: 0 0 16px">设置</h2>

    <n-space vertical :size="16">
      <n-card title="本地设置">
        <n-form label-placement="left" label-width="110">
          <n-form-item label="开机自启">
            <n-switch
              :value="autostartEnabled"
              :loading="autostartLoading"
              @update:value="setAutostart"
            />
          </n-form-item>
        </n-form>
      </n-card>

      <n-card title="坚果云备份与恢复">
        <n-spin :show="nutstoreLoading">
          <n-form label-placement="left" label-width="120" :model="nutstoreForm">
            <n-form-item label="启用备份">
              <n-switch v-model:value="nutstoreForm.enabled" />
            </n-form-item>
            <n-form-item label="WebDAV 地址">
              <n-input
                v-model:value="nutstoreForm.server_url"
                placeholder="默认 https://dav.jianguoyun.com/dav/"
              />
            </n-form-item>
            <n-form-item label="用户名">
              <n-input v-model:value="nutstoreForm.username" placeholder="坚果云用户名" />
            </n-form-item>
            <n-form-item label="应用密码">
              <n-input
                v-model:value="nutstoreForm.password"
                type="password"
                show-password-on="click"
                placeholder="坚果云应用密码"
              />
            </n-form-item>
            <n-form-item label="远程目录">
              <n-input
                v-model:value="nutstoreForm.remote_dir"
                placeholder="可选，例如 backups/ocm"
              />
            </n-form-item>
            <n-space>
              <n-button type="primary" :loading="nutstoreLoading" @click="saveNutstoreSettings">
                保存设置
              </n-button>
              <n-button
                type="success"
                ghost
                :loading="nutstoreBackingUp"
                :disabled="!nutstoreForm.enabled"
                @click="backupToNutstore"
              >
                备份 OCM 数据
              </n-button>
              <n-popconfirm
                positive-text="确认恢复"
                negative-text="取消"
                @positive-click="restoreFromNutstore"
              >
                <template #trigger>
                  <n-button
                    type="warning"
                    ghost
                    :loading="nutstoreRestoring"
                    :disabled="!nutstoreForm.enabled"
                  >
                    恢复最近备份
                  </n-button>
                </template>
                将从坚果云下载最近一次 OCM 数据库备份并覆盖当前本地数据，恢复后所有服务商都会变成“取消应用”状态。确认继续？
              </n-popconfirm>
            </n-space>
          </n-form>
        </n-spin>
      </n-card>
    </n-space>
  </div>
</template>
