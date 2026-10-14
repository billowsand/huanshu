<script setup lang="ts">
import { onBeforeUnmount, ref, watch } from 'vue'

/**
 * SlideFrame — 幻灯片位图的统一容器（Step2 看板与详情、演示全览与主画面、Step3 编辑器预览）。
 *
 * 尺寸由外层决定（三处都已经有按页面比例算好的舞台），本组件只负责铺满它。
 * 「正在出图」要持续 300ms 才淡入，避免每次翻页都闪一下。
 */
const props = withDefaults(defineProps<{
  /** 有值时显示图片，铺满本组件。 */
  src?: string | null
  /** 正在出图。 */
  pending?: boolean
  /** 最近一次渲染命令失败（`useTypstThumbnails` 的 `failed`）。旧图仍然会照常显示。 */
  error?: boolean
  alt?: string
  /** 「渲染中…」延迟出现的毫秒数。 */
  pendingDelay?: number
}>(), {
  src: null,
  pending: false,
  error: false,
  alt: '',
  pendingDelay: 300,
})

const emit = defineEmits<{
  retry: []
}>()

const showPending = ref(false)
let pendingTimer: ReturnType<typeof setTimeout> | null = null

function clearPendingTimer() {
  if (pendingTimer) {
    clearTimeout(pendingTimer)
    pendingTimer = null
  }
}

watch(
  () => [props.pending, !!props.src] as const,
  ([pending, hasImage]) => {
    clearPendingTimer()
    if (!pending || hasImage) {
      showPending.value = false
      return
    }
    pendingTimer = setTimeout(() => {
      pendingTimer = null
      showPending.value = true
    }, Math.max(props.pendingDelay, 0))
  },
  { immediate: true },
)

onBeforeUnmount(clearPendingTimer)
</script>

<template>
  <div class="slide-frame">
    <img
      v-if="src"
      class="slide-frame__img"
      :src="src"
      :alt="alt"
      draggable="false"
    >

    <template v-else>
      <div class="slide-frame__body">
        <span v-if="showPending && !error" class="slide-frame__hint" role="status">渲染中…</span>
        <template v-else-if="error">
          <span class="slide-frame__hint slide-frame__hint--error">渲染失败</span>
          <button type="button" class="slide-frame__retry" @click="emit('retry')">重试</button>
        </template>
      </div>
    </template>

    <!-- 有旧图但最近一次渲染失败：不遮挡画面，只在角上给一个重试入口。 -->
    <button
      v-if="src && error"
      type="button"
      class="slide-frame__retry slide-frame__retry--overlay"
      @click="emit('retry')"
    >
      渲染失败 · 重试
    </button>
  </div>
</template>

<style scoped>
.slide-frame {
  position: relative;
  width: 100%;
  height: 100%;
  overflow: hidden;
  background: var(--studio-bg);
}

.slide-frame__img {
  display: block;
  width: 100%;
  height: 100%;
  object-fit: fill;
}

.slide-frame__body {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 0.5rem;
  padding: 0.75rem;
  text-align: center;
}

.slide-frame__hint {
  font-size: 0.78rem;
  color: var(--studio-muted);
  animation: slide-frame-fade-in 240ms ease-out;
}

.slide-frame__hint--error {
  color: var(--studio-error);
}

.slide-frame__retry {
  border: 1px solid var(--studio-border-hover);
  background: var(--studio-surface);
  color: var(--studio-text);
  border-radius: 6px;
  padding: 0.2rem 0.6rem;
  font-size: 0.72rem;
  cursor: pointer;
  transition: border-color 120ms ease, background 120ms ease;
}

.slide-frame__retry:hover {
  border-color: var(--studio-primary-border);
  background: var(--studio-panel-2);
}

.slide-frame__retry--overlay {
  position: absolute;
  right: 0.4rem;
  bottom: 0.4rem;
  background: color-mix(in srgb, var(--studio-panel) 88%, transparent);
  backdrop-filter: blur(2px);
}

@keyframes slide-frame-fade-in {
  from { opacity: 0; }
  to { opacity: 1; }
}
</style>
