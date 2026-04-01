import { defineStore } from 'pinia'
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'

export interface AppGlobalSettings {
  data_dir: string
  media_dir: string
  llm_configured: boolean
  embeddings_ready: boolean
}

export const useAppSettingsStore = defineStore('appSettings', () => {
  const settings = ref<AppGlobalSettings>({
    data_dir: '',
    media_dir: '',
    llm_configured: false,
    embeddings_ready: false,
  })
  const loaded = ref(false)

  async function load() {
    try {
      const s = await invoke<AppGlobalSettings>('get_app_settings')
      settings.value = s
      loaded.value = true
    } catch (e) {
      console.error('Failed to load app settings:', e)
    }
  }

  async function save() {
    try {
      await invoke('save_app_settings', { settings: settings.value })
    } catch (e) {
      console.error('Failed to save app settings:', e)
      throw e
    }
  }

  return { settings, loaded, load, save }
})
