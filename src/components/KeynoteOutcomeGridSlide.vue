<script setup lang="ts">
import SlideTitle from './SlideTitle.vue'
import { computed } from 'vue'
import { toneVars, type Tone } from './keynoteTheme'

const props = defineProps<{
  section: string
  title: string
  subtitle?: string
  cols?: number
  cards: Array<{
    title: string
    body: string
    icon: string
    tone?: Tone
    tag: string
    topBarClass?: string
    top_bar_class?: string
  }>
  note?: string
}>()

const effectiveCols = computed(() => props.cols ?? props.cards.length)

// normal ≤3, compact =4, mini ≥5
const density = computed(() => {
  const c = effectiveCols.value
  if (c >= 5) return 'mini'
  if (c === 4) return 'compact'
  return 'normal'
})

// Use inline styles to avoid UnoCSS static-scan limitations with dynamic classes
const iconStyle = computed(() => ({
  normal: { fontSize: 'var(--type-outcome-icon-normal)' },
  compact: { fontSize: 'var(--type-outcome-icon-compact)' },
  mini: { fontSize: 'var(--type-outcome-icon-mini)' },
}[density.value]))

const bodyStyle = computed(() => ({
  normal: { padding: '1.25rem 1rem', fontSize: 'var(--type-outcome-body-normal)' },
  compact: { padding: '1rem 0.75rem', fontSize: 'var(--type-outcome-body-compact)' },
  mini: { padding: '0.75rem 0.5rem', fontSize: 'var(--type-outcome-body-mini)' },
}[density.value]))

const cardGap = computed(() => ({ normal: '1rem', compact: '0.75rem', mini: '0.5rem' }[density.value]))
</script>

<template>
  <div class="section-num">{{ section }}</div>

  <div relative z-10 flex flex-col h-full py-4>
    <div class="slide-header">
      <SlideTitle class="slide-title" :title="title" />
      <div class="slide-title-divider" />
    </div>
    <TagBadge v-if="subtitle" inline class="slide-subtitle mt-3">{{ subtitle }}</TagBadge>

    <div flex-1 flex flex-col justify-center>
    <div
      class="outcome-grid mt-4"
      :style="{
        gridTemplateColumns: `repeat(${effectiveCols}, minmax(0, 1fr))`,
        gap: cardGap,
      }"
    >
      <GlassCard
        v-for="card in cards"
        :key="card.title"
        overflow
        text-center
        class="outcome-card"
      >
        <div class="outcome-top-bar" h-1 :class="card.topBarClass || card.top_bar_class" />
        <div class="outcome-content" :style="bodyStyle">
          <div class="outcome-icon" :class="card.icon" :style="{ ...iconStyle, color: toneVars(card.tone).text }" />
          <div class="outcome-title slide-card-title" font-bold :style="{ fontSize: `calc(${bodyStyle.fontSize} * 1.25)` }">{{ card.title }}</div>
          <div class="outcome-copy" opacity-60 leading-relaxed :style="{ fontSize: bodyStyle.fontSize }" v-html="card.body" />
          <TagBadge :color="card.tone" inline class="outcome-tag">{{ card.tag }}</TagBadge>
        </div>
      </GlassCard>
    </div>

    <div v-if="note" mt-4 class="glass-card px-5 py-3 flex items-center gap-3">
      <div i-carbon:collaborate class="slide-icon-xl" text="var(--primary)" />
      <span class="slide-text-note" v-html="note" />
    </div>
    </div>
  </div>
</template>

<style scoped>
.outcome-grid { display: grid; }

.outcome-card {
  display: grid;
  grid-template-rows: subgrid;
  grid-row: span 5;
  row-gap: 0;
  min-width: 0;
}

.outcome-top-bar { grid-row: 1; }

.outcome-content {
  display: grid;
  grid-template-rows: subgrid;
  grid-row: 2 / span 4;
  row-gap: 0.75rem;
  justify-items: center;
}

.outcome-content > * {
  min-width: 0;
  align-self: start;
  overflow-wrap: anywhere;
}

.outcome-icon { grid-row: 1; }
.outcome-title { grid-row: 2; }
.outcome-copy { grid-row: 3; }
.outcome-tag { grid-row: 4; }
</style>
