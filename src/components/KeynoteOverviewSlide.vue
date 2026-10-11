<script setup lang="ts">
import SlideTitle from './SlideTitle.vue'
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import type { AspectRatio } from './types'

const props = defineProps<{
  section: string
  title: string
  items: Array<{
    number: string
    title: string
    desc: string
  }>
  note?: string
  aspectRatio?: AspectRatio
}>()

const root = ref<HTMLElement>()
const grid = ref<HTMLElement>()
const fitScale = ref(1)
const count = computed(() => props.items.length)
// Wide canvases have more room for text; preserve their horizontal layouts.
const widthFactor = computed(() => props.aspectRatio === 'ratio_48x9' ? 3 : props.aspectRatio === 'ratio_32x9' ? 2 : 1)
const isLong = computed(() => props.items.some(item =>
  item.title.length > 22 * widthFactor.value || item.desc.length > 56 * widthFactor.value,
))
// Each row divides twelve tracks evenly, so incomplete rows have no empty cells.
const rows = computed(() => {
  const n = count.value
  if (n <= 2) return [Math.max(n, 1)]
  if (isLong.value) {
    if (n === 3) return [1, 1, 1]
    return Array.from({ length: Math.ceil(n / 2) }, (_, i) => Math.min(2, n - i * 2))
  }
  const presets: Record<number, number[]> = { 3: [3], 4: [2, 2], 5: [2, 3], 6: [3, 3], 7: [3, 4], 8: [4, 4] }
  return presets[n] ?? Array.from({ length: Math.ceil(n / 3) }, (_, i) => Math.min(3, n - i * 3))
})
const slots = computed(() => rows.value.flatMap((cols, row) =>
  Array.from({ length: cols }, (_, col) => ({ gridRow: row + 1, gridColumn: `${col * (12 / cols) + 1} / span ${12 / cols}` })),
))
const stacked = computed(() => !isLong.value && count.value >= 3)
const contentStyle = computed(() => ({
  '--overview-body': `${count.value <= 4 ? 1.2 : 1.05}rem`,
  '--overview-fit': fitScale.value,
  '--overview-rows': rows.value.length,
}))

let observer: ResizeObserver | undefined
let frame = 0
let disposed = false
// Measure the actual fonts and wraps, including the remaining height after title/note.
// A uniform scale keeps every chapter at the same size and titles 25% larger.
async function fitContent() {
  fitScale.value = 1
  await nextTick()
  if (disposed || !grid.value || grid.value.clientHeight === 0) return
  for (let attempt = 0; attempt < 8; attempt++) {
    const cards = [...grid.value.querySelectorAll<HTMLElement>('.overview-card')]
    const overflows = cards.some(card => {
      const bounds = card.getBoundingClientRect()
      const children = [...card.children] as HTMLElement[]
      return card.scrollHeight > card.clientHeight + 1 || card.scrollWidth > card.clientWidth + 1
        || children.some(child => {
          const rect = child.getBoundingClientRect()
          return rect.top < bounds.top || rect.bottom > bounds.bottom || rect.right > bounds.right
        })
    })
    if (!overflows) break
    const body = grid.value.querySelector<HTMLElement>('.overview-desc')
    const size = body ? parseFloat(getComputedStyle(body).fontSize) : 20
    // Never shrink body text below 16 logical pixels.
    if (size <= 16.1) break
    fitScale.value *= Math.max(16 / size, 0.92)
    await nextTick()
    if (disposed || !grid.value) return
  }
}
function scheduleFit() {
  cancelAnimationFrame(frame)
  frame = requestAnimationFrame(() => { void fitContent() })
}
watch(() => [props.items, props.title, props.note, props.aspectRatio], scheduleFit, { deep: true, flush: 'post' })
onMounted(() => {
  observer = new ResizeObserver(scheduleFit)
  if (root.value) observer.observe(root.value)
  if (grid.value) observer.observe(grid.value)
  document.fonts.addEventListener('loadingdone', scheduleFit)
  void document.fonts.ready.then(() => { if (!disposed) scheduleFit() })
  scheduleFit()
})
onBeforeUnmount(() => {
  disposed = true
  observer?.disconnect()
  cancelAnimationFrame(frame)
  document.fonts.removeEventListener('loadingdone', scheduleFit)
})
</script>

<template>
  <div class="section-num">{{ section }}</div>

  <div ref="root" class="overview-layout" relative z-10 flex flex-col h-full py-4>
    <div class="slide-header">
      <SlideTitle class="slide-title slide-compact-title" :title="title" />
      <div class="slide-title-divider" />
    </div>

    <div class="overview-content" :style="contentStyle">
      <div ref="grid" class="overview-grid" :class="{ 'overview-grid--stacked': stacked }" :data-count="count">
        <div
          v-for="(item, idx) in items"
          :key="`${item.number}-${idx}`"
          class="overview-item-animated"
          :style="{ ...slots[idx], animationDelay: `${idx * 80 + 150}ms` }"
        >
          <GlassCard
            no-padding
            class="overview-card"
          >
            <div class="overview-number slide-number" text="var(--primary)">{{ item.number }}</div>
            <div class="overview-copy">
              <h2 class="overview-title slide-card-title">{{ item.title }}</h2>
              <div v-if="item.desc" class="overview-desc">{{ item.desc }}</div>
            </div>
          </GlassCard>
        </div>
      </div>

      <div v-if="note" class="overview-note glass-card px-5 py-3 flex items-center gap-3">
        <div i-carbon:idea class="slide-icon-xl" text="var(--primary)" />
        <span class="slide-text-note" opacity-80 v-html="note" />
      </div>
    </div>
  </div>
</template>

<style scoped>
.overview-layout {
  position: relative;
  z-index: 10;
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  padding: 1rem 0;
}
.overview-layout > .slide-header { flex-shrink: 0; }
.overview-content {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 1rem;
}
.overview-grid {
  display: grid;
  grid-template-columns: repeat(12, minmax(0, 1fr));
  grid-template-rows: repeat(var(--overview-rows), minmax(0, 1fr));
  gap: calc(1rem * var(--overview-fit));
  flex: 1;
  min-height: 0;
}
.overview-item-animated { min-width: 0; min-height: 0; }
.overview-card {
  height: 100%;
  min-width: 0;
  min-height: 0;
  display: flex;
  align-items: center;
  gap: calc(1.2rem * var(--overview-fit));
  padding: calc(1.35rem * var(--overview-fit));
}
.overview-grid--stacked .overview-card {
  flex-direction: column;
  align-items: flex-start;
  justify-content: center;
}
.overview-copy { min-width: 0; width: 100%; }
.overview-number {
  flex-shrink: 0;
  font-size: calc(2.2rem * var(--type-scale) * var(--overview-fit));
  line-height: 1;
  opacity: 0.55;
}
.overview-title {
  font-size: calc(var(--overview-body) * var(--type-scale) * var(--overview-fit) * 1.25);
  margin: 0 0 0.5em;
  line-height: 1.3;
  overflow-wrap: anywhere;
}
.overview-desc {
  font-size: calc(var(--overview-body) * var(--type-scale) * var(--overview-fit));
  line-height: 1.45;
  color: var(--text);
  opacity: 0.7;
  overflow-wrap: anywhere;
}
.overview-note { flex-shrink: 0; }
.overview-note > span { min-width: 0; overflow-wrap: anywhere; }
.overview-note > div { flex-shrink: 0; }
.overview-item-animated {
  opacity: 0;
  animation: overview-slide-in 0.4s cubic-bezier(0.16, 1, 0.3, 1) forwards;
  will-change: transform, opacity;
}
@media (prefers-reduced-motion: reduce) {
  .overview-item-animated { animation: none; opacity: 1; }
}

@keyframes overview-slide-in {
  from {
    opacity: 0;
    transform: translateX(-16px);
  }
  to {
    opacity: 1;
    transform: translateX(0);
  }
}
</style>
