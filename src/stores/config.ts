import { defineStore } from 'pinia'
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'

export interface LlmSettings {
  base_url: string
  api_key: string
  model: string
  embedding_model: string
  repair_rounds: number
  concurrency: number
}

export const useConfigStore = defineStore('config', () => {
  const settings = ref<LlmSettings>({
    base_url: 'http://127.0.0.1:1234',
    api_key: '',
    model: 'qwen/qwen3.5-9b',
    embedding_model: 'text-embedding-bge-m3',
    repair_rounds: 4,
    concurrency: 4,
  })
  const loaded = ref(false)
  const saving = ref(false)

  async function load() {
    try {
      const s = await invoke<LlmSettings>('get_settings')
      settings.value = s
      loaded.value = true
    } catch (e) {
      console.error('Failed to load settings:', e)
    }
  }

  async function save() {
    saving.value = true
    try {
      await invoke('save_settings', { settings: settings.value })
    } finally {
      saving.value = false
    }
  }

  async function listModels(): Promise<string[]> {
    return invoke<string[]>('list_models')
  }

  return { settings, loaded, saving, load, save, listModels }
})
