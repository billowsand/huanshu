<script setup lang="ts">
import { onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { invoke } from '@tauri-apps/api/core'
import { RouterView } from 'vue-router'
import TitleBar from './components/TitleBar.vue'

const router = useRouter()

onMounted(async () => {
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
