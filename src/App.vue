<script setup lang="ts">
import { onMounted, watch } from 'vue'
import { useRouter } from 'vue-router'
import { invoke } from '@tauri-apps/api/core'
import { RouterView } from 'vue-router'
import TitleBar from './components/TitleBar.vue'
import { useAppSettingsStore } from './stores/appSettings'
import { useEditorSchemaStore } from './stores/editorSchema'
import { applyFonts } from './typography'
import './themes/typography.css'

const router = useRouter()
const appSettings = useAppSettingsStore()
// 编辑器的版式表来自 Rust；先取回来，Step3 与重试弹窗都用它。
const editorSchema = useEditorSchemaStore()
watch(() => appSettings.settings.fonts, applyFonts, { deep: true, immediate: true, flush: 'sync' })

onMounted(async () => {
  await Promise.all([
    appSettings.load(),
    editorSchema.load().catch((e) => console.error('Failed to load editor schema:', e)),
  ])
  try {
    const firstRun = await invoke<boolean>('is_first_run')
    if (firstRun) {
      router.push('/wizard')
    }
  } catch (e) {
    console.error('Failed to check first run status:', e)
  }
})
</script>

<template>
  <TitleBar />
  <div class="app-body">
    <RouterView />
  </div>
</template>

<style>
.app-body {
  padding-top: var(--titlebar-height);
  height: 100vh;
  box-sizing: border-box;
  overflow: hidden;
}
</style>
