import { defineStore } from 'pinia'
import { ref } from 'vue'

import { api } from '@/api/client'
import type { Provider } from '@/api/types'

export const useProviderStore = defineStore('providers', () => {
  const providers = ref<Provider[]>([])
  const loading = ref(false)

  async function load() {
    loading.value = true
    try {
      providers.value = await api.listProviders()
    } finally {
      loading.value = false
    }
  }

  return { providers, loading, load }
})
