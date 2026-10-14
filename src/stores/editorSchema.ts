import { defineStore } from 'pinia'
import { computed, ref, shallowRef } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { SlideBlueprint, SlideKind } from '../components/types'

/** 与 Rust `huanshu_core::editor_schema` 的结构一一对应。 */
export type EditorField = {
  key: string
  label: string
  type: 'text' | 'textarea' | 'select' | 'boolean'
  placeholder?: string
  rows?: number
  options?: string[]
}

export type StringListDef = {
  key: string
  label: string
  placeholder?: string
}

export type NestedListDef = {
  key: string
  label: string
  /** 「新增一项」时写进牌组的内容。 */
  new_item: Record<string, unknown>
  fields: EditorField[]
}

export type ListSectionDef = {
  key: string
  label: string
  emptyText: string
  new_item: Record<string, unknown>
  fields: EditorField[]
  stringLists?: StringListDef[]
  nestedLists?: NestedListDef[]
  iconQueryKeys?: string[]
}

export type ObjectSectionDef = {
  key: string
  label: string
  fields?: EditorField[]
  stringLists?: StringListDef[]
  listSections?: ListSectionDef[]
}

export type KindEntry = {
  kind: SlideKind
  label: string
  icon: string
  desc: string
  fields: string[]
  defaults: Partial<SlideBlueprint>
}

export type EditorSchema = {
  kinds: KindEntry[]
  simple_fields: EditorField[]
  list_sections: Array<[string, ListSectionDef[]]>
  object_sections: Array<[string, ObjectSectionDef[]]>
  tone_options: string[]
  direction_options: string[]
}

const EMPTY_SCHEMA: EditorSchema = {
  kinds: [],
  simple_fields: [],
  list_sections: [],
  object_sections: [],
  tone_options: [],
  direction_options: [],
}

/**
 * 编辑器的模式表。数据来自 Rust 的 `get_editor_schema`（见 `huanshu_core::editor_schema`），
 * 启动后取一次并缓存；界面不维护第二份字段定义。
 */
export const useEditorSchemaStore = defineStore('editorSchema', () => {
  const schema = shallowRef<EditorSchema | null>(null)
  const loading = ref(false)
  const error = ref('')
  let inflight: Promise<void> | null = null

  async function load() {
    if (schema.value) return
    if (inflight) return inflight
    loading.value = true
    inflight = invoke<EditorSchema>('get_editor_schema')
      .then((value) => {
        schema.value = value
        error.value = ''
      })
      .catch((e) => {
        error.value = String(e)
        throw e
      })
      .finally(() => {
        loading.value = false
        inflight = null
      })
    return inflight
  }

  const ready = computed(() => schema.value !== null)

  const kindMeta = computed(() => {
    const out: Partial<Record<SlideKind, { label: string; icon: string; desc: string }>> = {}
    for (const entry of schema.value?.kinds ?? []) {
      out[entry.kind] = { label: entry.label, icon: entry.icon, desc: entry.desc }
    }
    return out
  })

  const kindFields = computed(() => {
    const out: Partial<Record<SlideKind, string[]>> = {}
    for (const entry of schema.value?.kinds ?? []) out[entry.kind] = entry.fields
    return out
  })

  const kindDefaults = computed(() => {
    const out: Record<string, Partial<SlideBlueprint>> = {}
    for (const entry of schema.value?.kinds ?? []) out[entry.kind] = entry.defaults
    return out
  })

  const simpleFields = computed(() => schema.value?.simple_fields ?? [])
  const toneOptions = computed(() => schema.value?.tone_options ?? [])
  const directionOptions = computed(() => schema.value?.direction_options ?? [])

  const listSections = computed(() =>
    Object.fromEntries(schema.value?.list_sections ?? []),
  )
  const objectSections = computed(() =>
    Object.fromEntries(schema.value?.object_sections ?? []),
  )

  return {
    schema,
    loading,
    error,
    ready,
    load,
    kindMeta,
    kindFields,
    kindDefaults,
    simpleFields,
    toneOptions,
    directionOptions,
    listSections,
    objectSections,
    emptySchema: EMPTY_SCHEMA,
  }
})
