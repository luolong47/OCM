<script setup lang="ts">
import {
  NConfigProvider,
  NDialogProvider,
  NLayout,
  NLayoutContent,
  NLayoutHeader,
  NMessageProvider,
  zhCN,
  dateZhCN,
} from 'naive-ui'
</script>

<template>
  <n-config-provider :locale="zhCN" :date-locale="dateZhCN">
    <n-message-provider>
      <n-dialog-provider>
        <n-layout position="absolute">
          <n-layout-header
            bordered
            style="height: 56px; display: flex; align-items: center; gap: 10px; padding: 0 20px"
          >
            <strong style="font-size: 16px">OpenCode 配置管理器</strong>

          </n-layout-header>
          <n-layout-content
            style="padding: 20px"
            content-style="max-width: 1180px; margin: 0 auto;"
          >
            <router-view />
          </n-layout-content>
        </n-layout>
      </n-dialog-provider>
    </n-message-provider>
  </n-config-provider>
</template>

