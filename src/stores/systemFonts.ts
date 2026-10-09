import { defineStore } from 'pinia'
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { SystemFont } from '../typography'

export const useSystemFontsStore = defineStore('systemFonts', () => {
  const fonts = ref<SystemFont[]>([])
  const loaded = ref(false)
  const loading = ref(false)
  const error = ref('')
  let pending: Promise<void> | null = null

  function load(refresh = false): Promise<void> {
    if (pending) return pending
    if (loaded.value && !refresh) return Promise.resolve()
    loading.value = true
    error.value = ''
    pending = (async () => {
      try {
        const result = await invoke<SystemFont[]>('list_system_fonts')
        if (!result.length) throw new Error('没有读取到可用字体，请刷新重试。')
        fonts.value = result.sort((a, b) => a.label.localeCompare(b.label, 'zh-CN'))
        loaded.value = true
      } catch (reason) {
        error.value = String(reason)
      } finally {
        loading.value = false
        pending = null
      }
    })()
    return pending
  }

  return { fonts, loaded, loading, error, load }
})
