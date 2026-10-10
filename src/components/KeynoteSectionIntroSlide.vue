<script setup lang="ts">
import SlideTitle from './SlideTitle.vue'
import { computed } from 'vue'
import { toneVars, type Tone } from './keynoteTheme'

const props = defineProps<{
  section?: string
  badge?: string
  title: string
  subtitle?: string
  cards: Array<{
    title: string
    tone?: Tone
    icon?: string
    body?: string
    items?: string[]
  }>
  note?: string
  aspect_ratio?: 'ratio_16x9' | 'ratio_32x9' | 'ratio_48x9'
}>()

const cardLimit = computed(() => {
  return props.aspect_ratio === 'ratio_48x9' ? 12
    : props.aspect_ratio === 'ratio_32x9' ? 8
    : 6
})

const previewCards = computed(() => props.cards.slice(0, cardLimit.value))

const gridCols = computed(() => {
  if (props.aspect_ratio === 'ratio_48x9') return 'minmax(0, 0.2fr) minmax(0, 2fr)'
  if (props.aspect_ratio === 'ratio_32x9') return 'minmax(0, 0.25fr) minmax(0, 1.5fr)'
  return 'minmax(0, 0.7fr) minmax(0, 1.3fr)'
})

const cardGridCols = computed(() => {
  if (props.aspect_ratio === 'ratio_48x9') return 'repeat(4, minmax(0, 1fr))'
  if (props.aspect_ratio === 'ratio_32x9') return 'repeat(3, minmax(0, 1fr))'
  const count = previewCards.value.length
  if (count <= 3) return 'minmax(0, 1fr)'
  if (count === 5) return 'repeat(6, minmax(0, 1fr))'
  return `repeat(${count === 6 ? 3 : 2}, minmax(0, 1fr))`
})

const standardAspect = computed(() => !props.aspect_ratio || props.aspect_ratio === 'ratio_16x9')
const compactCards = computed(() => standardAspect.value && previewCards.value.length >= 5)

const cardGridGap = computed(() => {
  if (props.aspect_ratio === 'ratio_48x9') return '1.4rem'
  if (props.aspect_ratio === 'ratio_32x9') return '1.2rem'
  return '1rem'
})
</script>

<template>
  <div v-if="section" class="section-num">{{ section }}</div>

  <div class="section-intro" relative z-10 :style="{ gridTemplateColumns: gridCols }">
    <div class="section-intro__lead glass-card">
      <TagBadge v-if="badge" inline color="green" class="section-intro__badge">{{ badge }}</TagBadge>

      <div class="slide-header section-intro__header">
        <SlideTitle class="slide-title section-intro__title" :title="title" />
        <div class="slide-title-divider section-intro__divider" />
      </div>

      <p v-if="subtitle" class="slide-text-note section-intro__subtitle" v-html="subtitle" />

      <div class="section-intro__meta">
        <div class="section-intro__meta-label">本章关注</div>
        <div class="section-intro__meta-value">{{ previewCards.length }} 个子主题</div>
      </div>

      <p v-if="note" class="slide-meta section-intro__note" v-html="note" />
    </div>

    <div class="section-intro__grid"
      :data-count="previewCards.length"
      :class="{ 'section-intro__grid--standard': standardAspect, 'section-intro__grid--compact': compactCards }"
      :style="{ gridTemplateColumns: cardGridCols, gap: cardGridGap }">
      <GlassCard
        v-for="(card, idx) in previewCards"
        :key="card.title"
        class="section-intro__card"
        :data-tone="card.tone || 'amber'"
        no-padding
        :style="{ '--intro-tone': toneVars(card.tone).solid }"
      >
        <div class="section-intro__card-index">{{ `0${idx + 1}`.slice(-2) }}</div>

        <div :class="card.icon || 'i-carbon:arrow-right'" class="section-intro__card-icon" aria-hidden="true" />
        <div class="section-intro__card-title">{{ card.title }}</div>

        <div v-if="card.body" class="slide-text-meta section-intro__card-body" opacity-70 v-html="card.body" />

        <div v-else-if="card.items?.length" class="section-intro__card-items">
          <div v-for="item in card.items.slice(0, 2)" :key="item" class="section-intro__card-item">
            {{ item }}
          </div>
        </div>
      </GlassCard>
    </div>
  </div>
</template>

<style scoped>
.section-intro {
  display: grid;
  grid-template-columns: minmax(0, 0.95fr) minmax(0, 1.25fr);
  gap: 1.25rem;
  height: 100%;
  min-height: 0;
  padding: 0.75rem 0;
  align-items: stretch;
}

.section-intro__lead {
  min-width: 0;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  padding: 1.75rem;
  background:
    linear-gradient(160deg, color-mix(in srgb, var(--success) 10%, transparent), transparent 55%),
    linear-gradient(180deg, rgba(255, 255, 255, 0.02), rgba(255, 255, 255, 0.01));
}

.section-intro__header {
  margin-top: 0.85rem;
}

.section-intro__title {
  max-width: 100%;
  text-align: left;
  overflow-wrap: anywhere;
}

.section-intro__title :deep(.semantic-title__word) {
  white-space: pre-wrap;
}

.section-intro__divider {
  width: 10rem;
  margin-top: 0.85rem;
}

.section-intro__subtitle {
  max-width: 24rem;
  margin-top: 1rem;
  line-height: 1.7;
}

.section-intro__meta {
  margin-top: 1.25rem;
  padding-top: 1rem;
  border-top: 1px solid rgba(255, 255, 255, 0.08);
}

.section-intro__meta-label {
  font-size: var(--type-caption);
  letter-spacing: 0.22em;
  text-transform: uppercase;
  color: rgba(255, 255, 255, 0.42);
}

.section-intro__meta-value {
  margin-top: 0.45rem;
  font-size: var(--type-body-strong);
  font-weight: 700;
  color: var(--success-light);
}

.section-intro__note {
  margin-top: 1.5rem;
  padding-left: 0.85rem;
  border-left: 3px solid rgba(52, 211, 153, 0.35);
}

.section-intro__grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 1rem;
  grid-auto-rows: minmax(0, 1fr);
  min-height: 0;
}

.section-intro__card {
  position: relative;
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  grid-template-rows: 2.75rem auto minmax(0, 1fr);
  align-content: start;
  gap: 1rem 0.75rem;
  min-width: 0;
  min-height: 0;
  padding: 1.5rem;
  overflow: hidden;
}

.section-intro__card::after {
  content: '';
  position: absolute;
  right: -2rem;
  bottom: -2rem;
  width: 7rem;
  height: 7rem;
  border-radius: 999px;
  background: radial-gradient(circle, rgba(255, 255, 255, 0.07), transparent 70%);
  pointer-events: none;
}

.section-intro__card-index {
  font-size: var(--type-meta);
  font-family: var(--font-mono);
  color: var(--intro-tone);
  align-self: start;
}

.section-intro__card-icon {
  width: 2.75rem;
  height: 2.75rem;
  color: var(--intro-tone);
  justify-self: end;
}

.section-intro__card-title {
  grid-column: 1 / -1;
  font-size: var(--type-body-strong);
  font-weight: 700;
  line-height: 1.35;
}

.section-intro__card-body {
  grid-column: 1 / -1;
  line-height: 1.7;
}

.section-intro__card-items {
  grid-column: 1 / -1;
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.section-intro__grid--standard[data-count="5"] > .section-intro__card {
  grid-column: span 2;
}
.section-intro__grid--standard[data-count="5"] > .section-intro__card:nth-child(-n + 2) {
  grid-column: span 3;
}
.section-intro__grid--compact .section-intro__card {
  padding: 1.1rem;
  gap: 0.75rem;
  grid-template-rows: 2.4rem auto minmax(0, 1fr);
}
.section-intro__grid--compact .section-intro__card-icon {
  width: 2.4rem;
  height: 2.4rem;
}
.section-intro__grid--compact .section-intro__card-title {
  font-size: var(--type-body);
}
.section-intro__grid--compact :is(.section-intro__card-body, .section-intro__card-item) {
  font-size: var(--type-caption);
  line-height: 1.5;
}
.section-intro__grid--standard[data-count="3"] .section-intro__card {
  grid-template-columns: 2.25rem minmax(0, 1fr) 2.75rem;
  grid-template-rows: auto minmax(0, 1fr);
  gap: 0.5rem 0.75rem;
}
.section-intro__grid--standard[data-count="3"] .section-intro__card-title {
  grid-area: 1 / 2;
}
.section-intro__grid--standard[data-count="3"] .section-intro__card-icon {
  grid-area: 1 / 3 / 3 / 4;
}
.section-intro__grid--standard[data-count="3"] :is(.section-intro__card-body, .section-intro__card-items) {
  grid-area: 2 / 2;
}

.section-intro__card-item {
  font-size: var(--type-body-tight);
  line-height: 1.6;
  color: rgba(255, 255, 255, 0.72);
}
</style>
