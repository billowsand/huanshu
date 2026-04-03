<script setup lang="ts">
import { ref, computed, watch, onMounted, onBeforeUnmount } from 'vue'
import { getCurrentWindow } from '@tauri-apps/api/window'
import SlideRenderer from '../components/SlideRenderer.vue'
import ThemeToggle from '../components/ThemeToggle.vue'
import type { SlideBlueprint, AspectRatio } from '../components/types'
import { ASPECT_DIMENSIONS } from '../components/types'

const props = defineProps<{
  blueprints: SlideBlueprint[]
  mediaMap: Record<string, string>
  initialSlide?: number
}>()

const emit = defineEmits<{
  'exit': []
}>()

const appWindow = getCurrentWindow()
const presenting = ref(true)
const presentSlide = ref(0)
const showShortcuts = ref(false)
const isFullscreen = ref(false)
const showOverview = ref(false)
const presOverlayRef = ref<HTMLElement | null>(null)
const viewportWidth = ref(window.innerWidth)
const viewportHeight = ref(window.innerHeight)
let shortcutTimer: ReturnType<typeof setTimeout> | null = null
let resizeObserver: ResizeObserver | null = null


const currentAspectRatio = computed<AspectRatio>(() => {
  return props.blueprints[presentSlide.value]?.aspect_ratio ?? 'ratio_16x9'
})

const slideDimensions = computed(() => {
  return ASPECT_DIMENSIONS[currentAspectRatio.value]
})

const uniformScale = computed(() => {
  const padH = isFullscreen.value ? 0 : 96
  const padV = isFullscreen.value ? 0 : 88
  const vw = Math.max(viewportWidth.value - padH, 1)
  const vh = Math.max(viewportHeight.value - padV, 1)
  const sx = vw / slideDimensions.value.w
  const sy = vh / slideDimensions.value.h
  return Math.min(sx, sy)
})

watch(
  () => props.initialSlide,
  (value) => {
    const fallback = 0
    const next = Number.isFinite(value) ? Number(value) : fallback
    presentSlide.value = Math.min(Math.max(next, 0), Math.max(props.blueprints.length - 1, 0))
  },
  { immediate: true },
)

function updateViewport() {
  const el = presOverlayRef.value
  if (el) {
    viewportWidth.value = Math.round(el.clientWidth || window.innerWidth)
    viewportHeight.value = Math.round(el.clientHeight || window.innerHeight)
    return
  }
  viewportWidth.value = window.innerWidth
  viewportHeight.value = window.innerHeight
}

async function enterWindowFullscreen() {
  // Window-level fullscreen removed — using DOM fullscreen only
}

async function exitWindowFullscreen() {
  // Window-level fullscreen removed
}

async function toggleFullscreen() {
  if (!isFullscreen.value) {
    await presOverlayRef.value?.requestFullscreen().catch(() => {})
    isFullscreen.value = true
  } else {
    await document.exitFullscreen().catch(() => {})
    isFullscreen.value = false
  }
  updateViewport()
}

async function onFullscreenChange() {
  isFullscreen.value = !!document.fullscreenElement
  updateViewport()
}

function presNext() {
  if (presentSlide.value < props.blueprints.length - 1) presentSlide.value++
}

function presPrev() {
  if (presentSlide.value > 0) presentSlide.value--
}

function toggleOverview() {
  showOverview.value = !showOverview.value
}

function selectSlide(idx: number) {
  presentSlide.value = idx
  showOverview.value = false
}

async function exitPresentation() {
  presenting.value = false
  showShortcuts.value = false
  showOverview.value = false
  if (shortcutTimer) clearTimeout(shortcutTimer)
  if (document.fullscreenElement) {
    await document.exitFullscreen().catch(() => {})
  }
  emit('exit')
}

function handlePresKey(e: KeyboardEvent) {
  if (!presenting.value) return
  switch (e.key) {
    case 'o':
    case 'O':
      toggleOverview()
      break
    case 'ArrowRight':
    case 'ArrowDown':
    case ' ':
      if (!showOverview.value) presNext()
      break
    case 'ArrowLeft':
    case 'ArrowUp':
      if (!showOverview.value) presPrev()
      break
    case 'Escape':
      if (showOverview.value) {
        showOverview.value = false
      } else if (!document.fullscreenElement) {
        exitPresentation()
      }
      break
    case 'f':
    case 'F':
      toggleFullscreen()
      break
    case 'Home':
      presentSlide.value = 0
      break
    case 'End':
      presentSlide.value = props.blueprints.length - 1
      break
  }
}

onMounted(async () => {
  updateViewport()
  showShortcuts.value = true
  if (shortcutTimer) clearTimeout(shortcutTimer)
  shortcutTimer = setTimeout(() => { showShortcuts.value = false }, 3000)
  await new Promise(r => setTimeout(r, 0))
  try {
    await presOverlayRef.value?.requestFullscreen()
  } catch { /* user may deny — gracefully continue windowed */ }
  isFullscreen.value = !!document.fullscreenElement
  if (presOverlayRef.value) {
    resizeObserver = new ResizeObserver(() => {
      updateViewport()
    })
    resizeObserver.observe(presOverlayRef.value)
  }
  window.addEventListener('keydown', handlePresKey)
  window.addEventListener('resize', updateViewport)
  document.addEventListener('fullscreenchange', onFullscreenChange)
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', handlePresKey)
  window.removeEventListener('resize', updateViewport)
  document.removeEventListener('fullscreenchange', onFullscreenChange)
  resizeObserver?.disconnect()
  resizeObserver = null
  if (shortcutTimer) clearTimeout(shortcutTimer)
})
</script>

<template>
  <div v-if="presenting" ref="presOverlayRef" class="presentation-overlay">
    <!-- Keyboard shortcuts hint -->
    <Transition name="shortcut-fade">
      <div v-if="showShortcuts" class="shortcut-hint">
        <div class="shortcut-item"><kbd>←</kbd><kbd>→</kbd> 翻页</div>
        <div class="shortcut-item"><kbd>O</kbd> 全览</div>
        <div class="shortcut-item"><kbd>F</kbd> 全屏</div>
        <div class="shortcut-item"><kbd>Esc</kbd> 退出</div>
      </div>
    </Transition>

    <!-- Nav arrows -->
    <div v-if="!showOverview" class="pres-nav pres-prev" @click="presPrev"><span class="i-carbon:chevron-left" /></div>
    <div v-if="!showOverview" class="pres-nav pres-next" @click="presNext"><span class="i-carbon:chevron-right" /></div>

    <!-- Slide stage: click to advance -->
    <div v-if="!showOverview" class="pres-stage" @click="presNext">
      <div
        class="pres-slide-wrapper"
        :style="{
          width:  `${slideDimensions.w * uniformScale}px`,
          height: `${slideDimensions.h * uniformScale}px`,
        }"
      >
        <div
          class="pres-slide"
          :style="{
            width: `${slideDimensions.w}px`,
            height: `${slideDimensions.h}px`,
            transform: `scale(${uniformScale})`,
            transformOrigin: 'top left',
          }"
        >
          <SlideRenderer v-if="blueprints[presentSlide]" :slide="(blueprints[presentSlide] as unknown as SlideBlueprint)" :slide-index="presentSlide" :media-map="mediaMap" />
        </div>
      </div>
    </div>

    <div v-else class="pres-overview">
      <div class="pres-overview-head">
        <div class="pres-overview-title">Keynote 全览</div>
        <div class="pres-overview-subtitle">点击任意缩略图进入对应页</div>
      </div>
      <div class="pres-overview-grid">
        <button
          v-for="(bp, idx) in blueprints"
          :key="idx"
          class="pres-overview-card"
          :class="{ active: idx === presentSlide }"
          :title="`${idx + 1}. ${bp.title}`"
          @click="selectSlide(idx)"
        >
          <div class="pres-overview-thumb">
            <div class="pres-overview-slide">
              <SlideRenderer :slide="(bp as unknown as SlideBlueprint)" :slide-index="idx" :media-map="mediaMap" />
            </div>
          </div>
          <div class="pres-overview-meta">
            <span class="pres-overview-index">{{ idx + 1 }}</span>
            <span class="pres-overview-name">{{ bp.title }}</span>
          </div>
        </button>
      </div>
    </div>

    <!-- Bottom controls bar -->
    <div class="pres-controls" @click.stop>
      <!-- Filmstrip -->
      <div class="pres-filmstrip">
        <div
          v-for="(bp, idx) in blueprints"
          :key="idx"
          class="filmstrip-item"
          :class="{ active: idx === presentSlide }"
          :title="`${idx + 1}. ${bp.title}`"
          @click="presentSlide = idx"
        >
          <span class="filmstrip-num">{{ idx + 1 }}</span>
        </div>
      </div>

      <div class="pres-controls-right">
        <div class="pres-center-info">
          <div class="pres-counter">{{ presentSlide + 1 }} / {{ blueprints.length }}</div>
          <div class="pres-progress-wrap">
            <div class="pres-progress">
              <div
                class="pres-progress-fill"
                :style="{ width: ((presentSlide + 1) / blueprints.length * 100) + '%' }"
              />
            </div>
          </div>
        </div>

        <ThemeToggle class="pres-theme-toggle" />

        <button class="btn pres-ctrl-btn" :title="showOverview ? '关闭全览 (O)' : '打开全览 (O)'" @click="toggleOverview">
          <span class="i-carbon:app-switcher" />
        </button>

        <button class="btn pres-ctrl-btn" :title="isFullscreen ? '退出全屏' : '全屏'" @click="toggleFullscreen">
          <span :class="isFullscreen ? 'i-carbon:minimize' : 'i-carbon:maximize'" />
        </button>

        <button class="btn pres-ctrl-btn" title="退出演示 (Esc)" @click="exitPresentation">
          <span class="i-carbon:close" />
        </button>
      </div>
    </div>
  </div>
</template>
