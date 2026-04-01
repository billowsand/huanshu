import { defineStore } from 'pinia'
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { SlideBlueprint } from './generation'

export interface ProjectSummary {
  id: number
  name: string
  slide_count: number
  created_at: number
  updated_at: number
}

export interface Project {
  id: number
  name: string
  md_content: string
  blueprints_json: string
  created_at: number
  updated_at: number
}

export const useProjectsStore = defineStore('projects', () => {
  const list = ref<ProjectSummary[]>([])
  const loading = ref(false)

  async function refresh() {
    loading.value = true
    try {
      list.value = await invoke<ProjectSummary[]>('list_projects')
    } finally {
      loading.value = false
    }
  }

  async function create(name: string, mdContent: string): Promise<number> {
    const id = await invoke<number>('create_project', { name, mdContent })
    await invoke('set_active_project', { id })
    await refresh()
    return id
  }

  async function open(id: number): Promise<Project> {
    return invoke<Project>('open_project', { id })
  }

  async function saveBlueprints(id: number, blueprints: SlideBlueprint[]) {
    await invoke('update_project_blueprints', {
      id,
      blueprintsJson: JSON.stringify(blueprints),
    })
  }

  async function updateContent(id: number, name: string, mdContent: string) {
    await invoke('update_project_content', { id, name, mdContent })
    await refresh()
  }

  async function remove(id: number) {
    await invoke('delete_project', { id })
    await refresh()
  }

  function formatDate(ts: number): string {
    return new Date(ts * 1000).toLocaleString('zh-CN', {
      year: 'numeric', month: '2-digit', day: '2-digit',
      hour: '2-digit', minute: '2-digit',
    })
  }

  return { list, loading, refresh, create, open, saveBlueprints, updateContent, remove, formatDate }
})
