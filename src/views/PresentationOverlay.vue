<script setup lang="ts">
import { ref, computed, watch, onMounted, onBeforeUnmount } from 'vue'
import { isTauri } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import SlideFrame from '../components/SlideFrame.vue'
import ThemeToggle from '../components/ThemeToggle.vue'
import type { SlideBlueprint, AspectRatio } from '../components/types'
import { ASPECT_DIMENSIONS } from '../components/types'
import { useTypstThumbnails } from '../composables/useTypstThumbnails'

const props = defineProps<{
  blueprints: SlideBlueprint[]
  initialSlide?: number
  projectId?: number | null
}>()

const emit = defineEmits<{
  'exit': []
}>()

const presenting = ref(true)
const presentSlide = ref(0)
const showShortcuts = ref(false)
const isFullscreen = ref(false)
const showOverview = ref(false)
const stretchToFill = ref(true)
const presOverlayRef = ref<HTMLElement | null>(null)
const viewportWidth = ref(window.innerWidth)
const viewportHeight = ref(window.innerHeight)
let shortcutTimer: ReturnType<typeof setTimeout> | null = null
let resizeObserver: ResizeObserver | null = null

// Slide transition direction
const transitionDirection = ref<'forward' | 'backward' | 'jump'>('forward')
const slideTransitionName = computed(() => `slide-${transitionDirection.value}`)

// Touch handling
let touchStartX = 0
let touchStartY = 0
let didSwipe = false


const currentAspectRatio = computed<AspectRatio>(() => {
  return props.blueprints[presentSlide.value]?.aspect_ratio ?? 'ratio_16x9'
})

const slideDimensions = computed(() => {
  return ASPECT_DIMENSIONS[currentAspectRatio.value]
})

const stretchedViewport = computed(() => {
  const padH = isFullscreen.value ? 0 : 96
  const padV = isFullscreen.value ? 0 : 88
  return {
    width: Math.max(viewportWidth.value - padH, 1),
    height: Math.max(viewportHeight.value - padV, 1),
  }
})

const stretchScale = computed(() => {
  return {
    scaleX: stretchedViewport.value.width / slideDimensions.value.w,
    scaleY: stretchedViewport.value.height / slideDimensions.value.h,
  }
})

const uniformScale = computed(() => {
  return Math.min(stretchScale.value.scaleX, stretchScale.value.scaleY)
})

const stageSize = computed(() => {
  if (stretchToFill.value) {
    return {
      width: stretchedViewport.value.width,
      height: stretchedViewport.value.height,
    }
  }

  return {
    width: slideDimensions.value.w * uniformScale.value,
    height: slideDimensions.value.h * uniformScale.value,
  }
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

// Tauri already turns DOM fullscreen into native window fullscreen (WebView2
// ContainsFullScreenElementChanged → setFullscreen). But tao clamps a *maximized* borderless
// window's client area to the monitor work area (WM_NCCALCSIZE) even while fullscreen, so a
// maximized window leaves the taskbar strip black. Unmaximize before entering, restore on exit.
let restoreMaximized = false

async function unmaximizeForFullscreen() {
  if (!isTauri()) return
  try {
    const win = getCurrentWindow()
    if (await win.isMaximized()) {
      restoreMaximized = true
      await win.unmaximize()
    }
  } catch (e) {
    console.warn('[presentation] unmaximize failed', e)
  }
}

async function restoreMaximizedWindow() {
  if (!isTauri() || !restoreMaximized) return
  restoreMaximized = false
  try {
    const win = getCurrentWindow()
    // Make sure fullscreen is left first, otherwise tao's saved placement overrides maximize
    await win.setFullscreen(false)
    await win.maximize()
  } catch (e) {
    console.warn('[presentation] re-maximize failed', e)
  }
}

async function enterFullscreen() {
  await unmaximizeForFullscreen()
  await presOverlayRef.value?.requestFullscreen().catch(() => {})
  isFullscreen.value = !!document.fullscreenElement
  if (!isFullscreen.value) await restoreMaximizedWindow()
}

async function toggleFullscreen() {
  if (!isFullscreen.value) {
    await enterFullscreen()
  } else {
    await document.exitFullscreen().catch(() => {})
    isFullscreen.value = false
    await restoreMaximizedWindow()
  }
  updateViewport()
}

async function onFullscreenChange() {
  isFullscreen.value = !!document.fullscreenElement
  if (!isFullscreen.value) await restoreMaximizedWindow()
  updateViewport()
}

function presNext() {
  if (presentSlide.value < props.blueprints.length - 1) {
    transitionDirection.value = 'forward'
    presentSlide.value++
  }
}

function presPrev() {
  if (presentSlide.value > 0) {
    transitionDirection.value = 'backward'
    presentSlide.value--
  }
}

function toggleOverview() {
  showOverview.value = !showOverview.value
}

function toggleStretchMode() {
  stretchToFill.value = !stretchToFill.value
}

function selectSlide(idx: number) {
  transitionDirection.value = 'jump'
  presentSlide.value = idx
  showOverview.value = false
}

// 全览里的缩略图：256×144 的框装不下完整幻灯片组件（原来写死 1280×720 scale(0.2)，32:9 / 48:9
// 的页面直接被裁掉），改用 Typst 光栅化的 PNG，按各页自己的比例定框。只在全览打开时渲染。
const OVERVIEW_WIDTH = 256
const OVERVIEW_HEIGHT = 144

/** 全览里这一页缩略框的像素尺寸（按本页比例装进 256×144）。 */
function overviewBoxSize(slide: SlideBlueprint | undefined) {
  const dims = ASPECT_DIMENSIONS[slide?.aspect_ratio ?? 'ratio_16x9']
  const scale = Math.min(OVERVIEW_WIDTH / dims.w, OVERVIEW_HEIGHT / dims.h)
  return { width: `${Math.round(dims.w * scale)}px`, height: `${Math.round(dims.h * scale)}px` }
}

const thumbnails = useTypstThumbnails({
  slides: () => props.blueprints,
  projectId: () => props.projectId ?? null,
  widthPx: () => Math.ceil(OVERVIEW_WIDTH * (window.devicePixelRatio || 1)),
  enabled: () => showOverview.value,
})

/** 该页的 Typst 缩略图；还没有图时返回 null，由加载框占位。 */
function overviewThumb(idx: number) {
  return thumbnails.thumbs.value[idx] ?? null
}

// ── 演示主画面 ────────────────────────────────────────────────────────────────
// 与全览那个实例分开：全览只要 256px 宽、一次要整副，主画面要覆盖整个舞台、
// 只要当前页前一页到后三页（往后翻比往前翻多得多），两者的宽度档位和缓存都不同。

/**
 * 要覆盖舞台所需的位图宽度。页面比例固定，拉伸模式下舞台可能非等比，所以取
 * 「舞台宽」和「按页高铺满所需的宽」的较大者，再乘 devicePixelRatio；Rust 侧夹到 3840。
 */
function stageImageWidth(): number {
  const size = stageSize.value
  const dims = slideDimensions.value
  return (Math.max(size.width, (size.height * dims.w) / dims.h) * (window.devicePixelRatio || 1)) || 1
}

const stage = useTypstThumbnails({
  slides: () => props.blueprints,
  projectId: () => props.projectId ?? null,
  widthPx: stageImageWidth,
  // 演示中、且不在全览模式（全览时主画面根本不在 DOM 里）。
  enabled: () => !showOverview.value,
  wanted: () => {
    const last = props.blueprints.length - 1
    return [presentSlide.value - 1, presentSlide.value, presentSlide.value + 1, presentSlide.value + 2, presentSlide.value + 3]
      .filter(index => index >= 0 && index <= last)
  },
  maxCached: 10,
})

watch(presentSlide, () => {
  void stage.refresh()
})

/** 当前页的位图；还没有图时返回 null，由加载框占位。 */
function stageThumb() {
  return stage.thumbs.value[presentSlide.value] ?? null
}

function handleStageClick() {
  if (showOverview.value) return
  if (didSwipe) { didSwipe = false; return }
  presNext()
}

function handleTouchStart(e: TouchEvent) {
  touchStartX = e.touches[0].clientX
  touchStartY = e.touches[0].clientY
  didSwipe = false
}

function handleTouchEnd(e: TouchEvent) {
  const dx = e.changedTouches[0].clientX - touchStartX
  const dy = e.changedTouches[0].clientY - touchStartY
  if (Math.abs(dx) > Math.abs(dy) && Math.abs(dx) > 50) {
    didSwipe = true
    if (dx < 0) presNext()
    else presPrev()
  }
}

async function exitPresentation() {
  presenting.value = false
  showShortcuts.value = false
  showOverview.value = false
  if (shortcutTimer) clearTimeout(shortcutTimer)
  if (document.fullscreenElement) {
    await document.exitFullscreen().catch(() => {})
  }
  await restoreMaximizedWindow()
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
    case 's':
    case 'S':
      if (!showOverview.value) toggleStretchMode()
      break
    case 'Home':
      transitionDirection.value = 'jump'
      presentSlide.value = 0
      break
    case 'End':
      transitionDirection.value = 'jump'
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
  // user may deny DOM fullscreen — enterFullscreen then continues windowed
  await enterFullscreen()
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
  // Unmounting without going through exitPresentation must still give back the maximized window
  void restoreMaximizedWindow()
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
        <div class="shortcut-item"><kbd>S</kbd> 拉伸</div>
        <div class="shortcut-item"><kbd>Esc</kbd> 退出</div>
      </div>
    </Transition>

    <!-- Nav arrows -->
    <div v-if="!showOverview" class="pres-nav pres-prev" @click="presPrev"><span class="i-carbon:chevron-left" /></div>
    <div v-if="!showOverview" class="pres-nav pres-next" @click="presNext"><span class="i-carbon:chevron-right" /></div>

    <!-- Slide stage: click/swipe to advance -->
    <div v-if="!showOverview" class="pres-stage" @click="handleStageClick" @touchstart="handleTouchStart" @touchend="handleTouchEnd">
      <div
        class="pres-transition-container"
        :style="{
          width: `${stageSize.width}px`,
          height: `${stageSize.height}px`,
        }"
      >
        <Transition :name="slideTransitionName">
          <div :key="presentSlide" class="pres-slide-wrapper">
            <!--
              Typst 光栅化的当前页，尺寸就是舞台尺寸。拉伸模式本来就要非等比铺满，所以铺满整个 wrapper
              （`SlideFrame` 里 `object-fit: fill`）。还没有图时显示加载框，主画面不放空白。
            -->
            <SlideFrame
              :src="stageThumb()?.url ?? null"
              :pending="stage.pending.value"
              :error="stage.failed.value"
              alt=""
              @retry="stage.refresh()"
            />
          </div>
        </Transition>
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
          <div class="pres-overview-thumb" :style="overviewBoxSize(bp)">
            <SlideFrame
              :src="overviewThumb(idx)?.url ?? null"
              :pending="thumbnails.pending.value"
              :error="thumbnails.failed.value"
              alt=""
              @retry="thumbnails.refresh()"
            />
          </div>
          <div class="pres-overview-meta">
            <span class="pres-overview-index">{{ idx + 1 }}</span>
            <span class="pres-overview-name">{{ bp.title }}</span>
          </div>
        </button>
      </div>
    </div>

    <!-- Bottom controls bar -->
    <div class="pres-controls" :class="{ 'pres-controls--floating': isFullscreen }" @click.stop>
      <!-- Filmstrip -->
      <div class="pres-filmstrip">
        <div
          v-for="(bp, idx) in blueprints"
          :key="idx"
          class="filmstrip-item"
          :class="{ active: idx === presentSlide }"
          :title="`${idx + 1}. ${bp.title}`"
          @click="selectSlide(idx)"
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

        <button class="btn pres-ctrl-btn" :title="stretchToFill ? '切换为等比显示 (S)' : '切换为拉伸铺满 (S)'" @click="toggleStretchMode">
          <span :class="stretchToFill ? 'i-carbon:fit-to-screen' : 'i-carbon:crop'" />
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
