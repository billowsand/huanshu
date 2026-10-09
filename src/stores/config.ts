import { defineStore } from 'pinia'
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'

export interface LlmSettings {
  llm: ModelServiceSettings
  embedding: ModelServiceSettings
  multimodal: ModelServiceSettings
  repair_rounds: number
  concurrency: number
}

export interface ModelServiceSettings {
  base_url: string
  api_key: string
  model: string
  proxy: Socks5Settings
}

export interface Socks5Settings {
  enabled: boolean
  url: string
  username: string
  password: string
}

function defaultProxy(): Socks5Settings {
  return { enabled: false, url: 'socks5://127.0.0.1:1080', username: '', password: '' }
}

export type ModelTarget = 'llm' | 'embedding' | 'multimodal'

export const useConfigStore = defineStore('config', () => {
  const settings = ref<LlmSettings>({
    llm: {
      base_url: 'http://127.0.0.1:1234',
      api_key: '',
      model: 'qwen/qwen3.5-9b',
      proxy: defaultProxy(),
    },
    embedding: {
      base_url: 'http://127.0.0.1:1234',
      api_key: '',
      model: 'text-embedding-bge-m3',
      proxy: defaultProxy(),
    },
    multimodal: {
      base_url: 'http://127.0.0.1:1234',
      api_key: '',
      model: 'qwen/qwen2.5-vl-7b-instruct',
      proxy: defaultProxy(),
    },
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

  async function listModels(target: ModelTarget, service?: ModelServiceSettings): Promise<string[]> {
    return invoke<string[]>('list_models', { target, service })
  }

  return { settings, loaded, saving, load, save, listModels }
})
