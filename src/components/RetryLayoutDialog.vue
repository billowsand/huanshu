<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { SlideKind } from './types'
import { KIND_META } from '../composables/useSlideEditor'

const emit = defineEmits<{ confirm: [index: number, kind: SlideKind] }>()
const dialog = ref<HTMLDialogElement | null>(null)
const pageIndex = ref(0)
const choices = ref<SlideKind[]>([])
const selectedKind = ref<SlideKind | ''>('')
const loading = ref(false)
const error = ref('')
const description = computed(() => selectedKind.value ? KIND_META[selectedKind.value]?.desc : '')
let requestVersion = 0

function close() {
  requestVersion++
  dialog.value?.close()
}

async function open(runId: number | undefined, index: number) {
  const version = ++requestVersion
  pageIndex.value = index
  choices.value = []
  selectedKind.value = ''
  loading.value = true
  error.value = ''
  await nextTick()
  dialog.value?.showModal()
  try {
    if (!runId) throw new Error('缺少生成记录，请重新生成')
    const options = await invoke<{ current_kind: SlideKind, kinds: SlideKind[] }>('get_retry_layouts', {
      runId, pageIndex: index,
    })
    if (version !== requestVersion) return
    choices.value = options.kinds
    selectedKind.value = options.kinds.includes(options.current_kind) ? options.current_kind : options.kinds[0] ?? ''
    if (!choices.value.length) error.value = '这一页暂无可用布局'
  } catch (cause) {
    if (version === requestVersion) error.value = String(cause)
  } finally {
    if (version === requestVersion) loading.value = false
  }
}

function confirm() {
  if (loading.value || !selectedKind.value || !choices.value.includes(selectedKind.value)) return
  emit('confirm', pageIndex.value, selectedKind.value)
  close()
}

onBeforeUnmount(() => { requestVersion++ })
defineExpose({ open, close })
</script>

<template>
  <dialog ref="dialog" class="retry-layout-dialog" aria-labelledby="retry-layout-title" @cancel.prevent="close">
    <form @submit.prevent="confirm">
      <h2 id="retry-layout-title">选择重试布局</h2>
      <p>第 {{ pageIndex + 1 }} 页将按所选布局重新生成。</p>
      <p v-if="loading" role="status">正在读取可用布局…</p>
      <p v-else-if="error" class="retry-layout-error" role="alert">{{ error }}</p>
      <template v-else>
        <label for="retry-layout-kind">页面布局</label>
        <select id="retry-layout-kind" v-model="selectedKind" autofocus>
          <option v-for="kind in choices" :key="kind" :value="kind">{{ KIND_META[kind]?.label ?? kind }}</option>
        </select>
        <p class="retry-layout-description">{{ description }}</p>
      </template>
      <div class="retry-layout-actions">
        <button type="button" class="btn" @click="close">取消</button>
        <button type="submit" class="btn btn-primary" :disabled="loading || !!error || !selectedKind">按此布局重试</button>
      </div>
    </form>
  </dialog>
</template>

<style scoped>
.retry-layout-dialog { width: min(440px, calc(100vw - 48px)); padding: 24px; border: 1px solid var(--studio-border); border-radius: 14px; background: var(--studio-surface); color: var(--text); }
.retry-layout-dialog::backdrop { background: rgb(0 0 0 / 50%); }
h2 { margin: 0 0 12px; font-size: 1.3rem; }
p { margin: 12px 0; line-height: 1.6; }
label { display: block; margin-top: 20px; margin-bottom: 8px; }
select { width: 100%; padding: 10px; border: 1px solid var(--studio-border); border-radius: 6px; background: var(--studio-surface); color: inherit; font: inherit; }
.retry-layout-description { min-height: 3em; opacity: .8; }
.retry-layout-error { color: var(--danger); }
.retry-layout-actions { display: flex; justify-content: flex-end; gap: 12px; margin-top: 24px; }
</style>
