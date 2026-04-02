<script setup lang="ts">
import { onMounted, ref } from 'vue'

const themes = [
  { id: 'dark-yellow',   name: '暗·琥珀', bg: '#0b0c11', accent: '#f59e0b' },
  { id: 'dark-blue',     name: '暗·深海', bg: '#070c17', accent: '#38bdf8' },
  { id: 'light-blue',    name: '亮·蓝图', bg: '#eef4ff', accent: '#1d4ed8' },
  { id: 'light-chatgpt', name: '亮·翠绿', bg: '#f5f5f5', accent: '#059669' },
  { id: 'light-claude',  name: '亮·暖铜', bg: '#f2ece0', accent: '#b5601a' },
]

const currentTheme = ref('dark-yellow')

function setTheme(themeId: string) {
  currentTheme.value = themeId
  document.documentElement.setAttribute('data-theme', themeId)
  localStorage.setItem('app-theme', themeId)
}

onMounted(() => {
  const themeId = document.documentElement.getAttribute('data-theme') || 'dark-yellow'
  currentTheme.value = themeId
  document.documentElement.setAttribute('data-theme', themeId)
})
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
