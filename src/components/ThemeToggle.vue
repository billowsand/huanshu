<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue'

const themes = [
  { id: 'dark-yellow',   name: '暗·琥珀', bg: '#0b0c11', accent: '#f59e0b' },
  { id: 'dark-blue',     name: '暗·星图', bg: '#020810', accent: '#0EA5E9' },
  { id: 'dark-glass-blue', name: '暗·极光', bg: 'linear-gradient(135deg, #3b1fdc 0%, #0694a2 50%, #065f46 100%)', accent: '#BAE6FD' },
  { id: 'dark-terminal', name: '终端·墨', bg: '#151614', accent: '#ed9b62' },
  { id: 'light-terminal', name: '终端·纸', bg: '#f4f3eb', accent: '#9d4a1c' },
]

const currentTheme = ref('dark-yellow')
let themeObserver: MutationObserver | undefined

function setTheme(themeId: string) {
  currentTheme.value = themeId
  document.documentElement.setAttribute('data-theme', themeId)
  localStorage.setItem('app-theme', themeId)
}

onMounted(() => {
  const syncTheme = () => {
    currentTheme.value = document.documentElement.getAttribute('data-theme') || 'dark-yellow'
  }
  syncTheme()
  themeObserver = new MutationObserver(syncTheme)
  themeObserver.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] })
})

onUnmounted(() => themeObserver?.disconnect())
</script>

<template>
  <div class="theme-toggle">
    <span class="toggle-label">主题</span>
    <div class="swatches">
      <button
        v-for="theme in themes"
        :key="theme.id"
        class="swatch"
        :class="{ active: currentTheme === theme.id }"
        :style="{ background: theme.bg }"
        :title="theme.name"
        :aria-label="theme.name"
        :aria-pressed="currentTheme === theme.id"
        @click="setTheme(theme.id)"
      >
        <span class="swatch-dot" :style="{ background: theme.accent }" />
      </button>
    </div>
  </div>
</template>

<style scoped>
.theme-toggle {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.5rem 0.75rem;
  border-top: 1px solid var(--studio-border);
}

.toggle-label {
  font-size: 0.65rem;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  color: var(--studio-muted);
  white-space: nowrap;
}

.swatches {
  display: flex;
  gap: 0.3rem;
  flex-wrap: wrap;
}

.swatch {
  width: 20px;
  height: 20px;
  border-radius: 50%;
  border: 2px solid transparent;
  cursor: pointer;
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: transform 0.15s ease, border-color 0.15s ease;
  padding: 0;
}

.swatch:hover {
  transform: scale(1.15);
}

.swatch.active {
  border-color: var(--studio-text);
  box-shadow: 0 0 0 1px var(--studio-border-hover);
}

.swatch-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  display: block;
}
</style>
