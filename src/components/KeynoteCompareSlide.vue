<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import SlideTitle from './SlideTitle.vue'
import { toneVars, type Tone } from './keynoteTheme'

type CompareSide = {
  title: string
  tone?: Tone
  icon?: string
  items: Array<{ label: string; desc?: string; highlight?: boolean }>
  conclusion?: string
}
const props = defineProps<{
  section?: string
  title: string
  subtitle?: string
  mode?: 'side-by-side' | 'pros-cons' | 'versus'
  left: CompareSide
  right: CompareSide
  footer?: string
}>()

const sides = computed(() => [props.left, props.right].map((side, index) => ({
  ...side, tone: side.tone || (index === 0 ? 'blue' : 'amber') as Tone,
})))
// Preserve authored order. A shared row keeps the next pair aligned even when
// one description wraps; unequal lists retain an explicit empty cell.
const rows = computed(() => Array.from({ length: Math.max(props.left.items.length, props.right.items.length) },
  (_, index) => [props.left.items[index], props.right.items[index]]))
const clean = (text?: string) => (text || '').replace(/\s/g, '')
const footerText = computed(() => clean(props.footer) && !sides.value.some(side => clean(side.conclusion) === clean(props.footer)) ? props.footer : '')
const root = ref<HTMLElement>()
const body = ref<HTMLElement>()
const fit = ref(1)
const layoutStyle = computed(() => ({
  '--compare-fit': fit.value,
  '--compare-copy': `${rows.value.length <= 2 ? 1.35 : rows.value.length === 3 ? 1.2 : 1.05}rem`,
  '--compare-gap': rows.value.length >= 4 ? '6px' : '12px',
  '--compare-pad': rows.value.length >= 5 ? '2px' : rows.value.length === 4 ? '4px' : '18px',
  '--compare-desc-gap': rows.value.length >= 4 ? '4px' : '8px',
  '--compare-leading': rows.value.length >= 4 ? 1.4 : 1.5,
}))
let observer: ResizeObserver | undefined
let frame = 0
let disposed = false
async function fitContent() {
  fit.value = 1
  await nextTick()
  if (disposed || !root.value || !body.value) return
  for (let attempt = 0; attempt < 12; attempt++) {
    const bounds = root.value.getBoundingClientRect()
    const overflowing = body.value.scrollHeight > body.value.clientHeight + 1
      || root.value.scrollHeight > root.value.clientHeight + 1
      || [...root.value.querySelectorAll<HTMLElement>('.cp-copy,.cp-footer,.compare-footnote,.cp-header')].some(el =>
        el.scrollWidth > el.clientWidth + 1 || el.getBoundingClientRect().bottom > bounds.bottom + 1)
    if (!overflowing) break
    const copy = root.value.querySelector<HTMLElement>('.cp-desc')
    const size = copy ? parseFloat(getComputedStyle(copy).fontSize) : 20
    if (size <= 16.1) break
    fit.value *= Math.max(16 / size, 0.92)
    await nextTick()
    if (disposed || !root.value || !body.value) return
  }
}
function scheduleFit() {
  cancelAnimationFrame(frame)
  frame = requestAnimationFrame(() => { void fitContent() })
}
watch(() => [props.left, props.right, props.title, props.subtitle, props.footer, props.mode], scheduleFit, { deep: true, flush: 'post' })
onMounted(() => {
  observer = new ResizeObserver(scheduleFit)
  if (root.value) observer.observe(root.value)
  if (body.value) observer.observe(body.value)
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
  <div v-if="section" class="section-num">{{ section }}</div>
  <div ref="root" class="compare-slide" :style="layoutStyle" relative z-10 py-4 flex flex-col h-full>
    <div class="slide-header">
      <SlideTitle class="slide-title" :title="title" />
      <div class="slide-title-divider" />
    </div>
    <TagBadge v-if="subtitle" inline class="slide-subtitle mt-2">{{ subtitle }}</TagBadge>

    <div class="compare-layout">
      <div class="cp-heading-row cp-pair">
        <div v-for="(side, sideIndex) in sides" :key="sideIndex" class="cp-header cp-panel glass-card" :data-tone="side.tone"
          :style="{ background: toneVars(side.tone).bg, borderColor: toneVars(side.tone).border, color: toneVars(side.tone).text }">
          <div v-if="side.icon" :class="side.icon" class="slide-icon-lg cp-icon" />
          <span v-if="mode === 'pros-cons'" class="cp-mode-badge">{{ sideIndex === 0 ? '优势' : '劣势' }}</span>
          <span class="slide-card-title cp-heading">{{ side.title }}</span>
        </div>
        <div class="cp-axis-label" :class="{ 'cp-vs-badge': mode === 'versus', 'cp-balance-badge': mode === 'pros-cons' }">
          {{ mode === 'versus' ? 'VS' : mode === 'pros-cons' ? '±' : '↔' }}
        </div>
      </div>

      <div ref="body" class="cp-body">
        <div v-for="(pair, index) in rows" :key="index" class="cp-pair cp-row">
          <div v-for="(item, sideIndex) in pair" :key="sideIndex" class="cp-item cp-panel glass-card"
            :class="{ 'cp-item--empty': !item }" :data-tone="sides[sideIndex].tone" :data-highlight="!!item?.highlight"
            :style="{ borderColor: toneVars(sides[sideIndex].tone).border,
              background: item?.highlight ? toneVars(sides[sideIndex].tone).bg : undefined,
              borderLeftColor: toneVars(sides[sideIndex].tone).solid }">
            <template v-if="item">
              <div :class="item.highlight ? 'i-carbon:star-filled' : mode === 'pros-cons' && sideIndex === 1 ? 'i-carbon:close' : 'i-carbon:checkmark-filled'"
                class="slide-icon-sm cp-icon" :style="{ color: toneVars(sides[sideIndex].tone).text }" />
              <div class="cp-copy">
                <div class="slide-text-meta cp-label">{{ item.label }}</div>
                <div v-if="item.desc" class="slide-text-caption slide-font-body cp-desc">{{ item.desc }}</div>
              </div>
            </template>
            <span v-else class="cp-empty">—</span>
          </div>
          <span class="cp-row-index">{{ String(index + 1).padStart(2, '0') }}</span>
        </div>
      </div>

      <div v-if="sides.some(side => side.conclusion)" class="cp-pair cp-conclusions">
        <div v-for="(side, sideIndex) in sides" :key="sideIndex" class="cp-footer" :data-tone="side.tone"
          :class="{ 'cp-footer--empty': !side.conclusion }"
          :style="{ background: toneVars(side.tone).bg, borderColor: toneVars(side.tone).border, color: toneVars(side.tone).text }">
          <div v-if="side.conclusion" i-carbon:idea class="slide-icon-sm cp-icon" />
          <span class="slide-text-caption cp-conclusion">{{ side.conclusion }}</span>
        </div>
      </div>
    </div>
    <div v-if="footerText" class="slide-footnote compare-footnote">{{ footerText }}</div>
  </div>
</template>

<style scoped>
.compare-slide { display: flex; flex-direction: column; height: 100%; min-height: 0; position: relative; z-index: 10; padding-block: 16px; }
.compare-layout { display: flex; flex-direction: column; flex: 1; min-height: 0; gap: var(--compare-gap); margin-top: 20px; }
.cp-pair { display: grid; grid-template-columns: minmax(0, 1fr) 44px minmax(0, 1fr); align-items: stretch; }
.cp-pair > :nth-child(1) { grid-column: 1; grid-row: 1; }
.cp-pair > :nth-child(2) { grid-column: 3; grid-row: 1; }
.cp-pair > :nth-child(3) { grid-column: 2; grid-row: 1; }
.cp-header { display: flex; align-items: center; gap: 10px; padding: 14px 18px; border: 1px solid; border-radius: 10px; min-width: 0; }
.cp-heading { font-size: calc(var(--compare-copy) * var(--type-scale) * var(--compare-fit) * 1.25); line-height: 1.25; overflow-wrap: anywhere; }
.cp-axis-label { align-self: center; text-align: center; color: var(--text-muted); font-size: 16px; }
.cp-mode-badge { flex-shrink: 0; font-size: 14px; padding: 2px 6px; border: 1px solid currentColor; border-radius: 4px; }
.cp-body { flex: 1; min-height: 0; display: grid; grid-auto-rows: minmax(min-content, 1fr); gap: var(--compare-gap); }
.cp-row { min-height: min-content; }
.cp-item { display: flex; align-items: flex-start; gap: 10px; padding: calc(var(--compare-pad) * var(--compare-fit)) 20px; min-width: 0; border: 1px solid; border-left-width: 3px; border-radius: 10px; }
.cp-copy { min-width: 0; width: 100%; }
.cp-item > .cp-icon { margin-top: 5px; }
.cp-icon { flex-shrink: 0; }
.compare-slide .cp-label { font-size: calc(var(--compare-copy) * var(--type-scale) * var(--compare-fit) * 1.12); line-height: 1.3; overflow-wrap: anywhere; }
.compare-slide .cp-desc { margin-top: var(--compare-desc-gap); font-size: calc(var(--compare-copy) * var(--type-scale) * var(--compare-fit)); line-height: var(--compare-leading); opacity: 0.76; overflow-wrap: anywhere; white-space: normal; }
.cp-row-index { display: flex; align-items: center; justify-content: center; color: var(--text-muted); font-size: 12px; font-variant-numeric: tabular-nums; position: relative; }
.cp-row-index::before, .cp-row-index::after { content: ''; position: absolute; height: 1px; width: 7px; background: var(--glass-border); top: 50%; }
.cp-row-index::before { left: 0; }
.cp-row-index::after { right: 0; }
.cp-item--empty { opacity: 0.4; align-items: center; justify-content: center; }
.cp-footer { display: flex; align-items: center; gap: 10px; border: 1px solid; border-radius: 8px; padding: 12px 18px; min-width: 0; }
.compare-slide .cp-conclusion { font-size: max(16px, calc(var(--type-caption) * var(--compare-fit))); line-height: 1.4; overflow-wrap: anywhere; }
.cp-footer--empty { visibility: hidden; }
.compare-footnote { flex-shrink: 0; margin-top: 14px; text-align: center; font-size: max(16px, calc(var(--type-caption) * var(--compare-fit))); line-height: 1.4; }
</style>
