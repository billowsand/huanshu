<script setup lang="ts">
import { nextTick, onMounted, onUnmounted, ref, watch } from 'vue'

const props = defineProps<{
  syntax: string
  fallbackClass?: string
}>()

const containerRef = ref<HTMLDivElement>()
let infographicInstance: any = null
let themeObserver: MutationObserver | null = null

/** Read a CSS custom property from :root and return its value */
function getCssVar(name: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim()
}

/** Determine if current theme is dark based on data-theme attribute */
function isDarkTheme(): boolean {
  const theme = document.documentElement.getAttribute('data-theme') || 'dark-yellow'
  return theme.startsWith('dark')
}

/** Build themeConfig for @antv/infographic based on current app theme */
function buildThemeConfig() {
  const bg = getCssVar('--bg')
  const primary = getCssVar('--primary')
  const text = getCssVar('--text')
  const primaryLight = getCssVar('--primary-light')
  const info = getCssVar('--info')
  const success = getCssVar('--success')
  const warning = getCssVar('--warning')
  const danger = getCssVar('--danger')

  const dark = isDarkTheme()

  return {
    theme: dark ? 'dark' : 'light',
    themeConfig: {
      colorBg: bg || (dark ? '#09090f' : '#ffffff'),
      colorPrimary: primary || (dark ? '#f59e0b' : '#1d4ed8'),
      palette: [
        primary || '#f59e0b',
        info || '#0ea5e9',
        success || '#10b981',
        warning || '#f59e0b',
        danger || '#ef4444',
        primaryLight || '#38bdf8',
      ],
      base: {
        text: {
          fill: text || (dark ? '#f5edda' : '#111827'),
        },
      },
    },
  }
}

function showFallback(text: string) {
  if (containerRef.value) {
    const pre = document.createElement('pre')
    pre.textContent = text
    pre.className = props.fallbackClass ?? ''
    pre.style.whiteSpace = 'pre-wrap'
    pre.style.wordBreak = 'break-word'
    containerRef.value.innerHTML = ''
    containerRef.value.appendChild(pre)
  }
}

async function initInfographic(syntax: string) {
  if (!containerRef.value || !syntax?.trim()) return

  try {
    const { Infographic } = await import('@antv/infographic')

    if (infographicInstance) {
      infographicInstance.destroy()
      infographicInstance = null
    }

    const themeOpts = buildThemeConfig()

    infographicInstance = new Infographic({
      container: containerRef.value,
      width: containerRef.value.clientWidth || 800,
      height: containerRef.value.clientHeight || 450,
      ...themeOpts,
    })

    // Listen for error events (emitted when template not found, etc.)
    infographicInstance.on('error', (err: any) => {
      console.error('[InfographicCanvas] render error:', err)
      showFallback(syntax)
    })

    infographicInstance.render(syntax)
  }
  catch (err) {
    console.error('[InfographicCanvas] render failed:', err)
    showFallback(syntax)
  }
}

function onThemeChange() {
  if (props.syntax) {
    initInfographic(props.syntax)
  }
}

onMounted(async () => {
  await nextTick()
  if (props.syntax) {
    initInfographic(props.syntax)
  }

  // Watch for data-theme attribute changes via MutationObserver
  themeObserver = new MutationObserver((mutations) => {
    for (const m of mutations) {
      if (m.type === 'attributes' && m.attributeName === 'data-theme') {
        onThemeChange()
        break
      }
    }
  })
  themeObserver.observe(document.documentElement, {
    attributes: true,
    attributeFilter: ['data-theme'],
  })
})

onUnmounted(() => {
  if (infographicInstance) {
    infographicInstance.destroy()
    infographicInstance = null
  }
  if (themeObserver) {
    themeObserver.disconnect()
    themeObserver = null
  }
})

watch(() => props.syntax, (newSyntax) => {
  if (newSyntax) {
    initInfographic(newSyntax)
  }
})
</script>

<template>
  <div ref="containerRef" class="h-full w-full" />
</template>
