<script setup lang="ts">
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
    : 4
})

const previewCards = computed(() => props.cards.slice(0, cardLimit.value))

const gridCols = computed(() => {
  if (props.aspect_ratio === 'ratio_48x9') return 'minmax(0, 0.2fr) minmax(0, 2fr)'
  if (props.aspect_ratio === 'ratio_32x9') return 'minmax(0, 0.25fr) minmax(0, 1.5fr)'
  return 'minmax(0, 0.95fr) minmax(0, 1.25fr)'
})

const cardGridCols = computed(() => {
  if (props.aspect_ratio === 'ratio_48x9') return 'repeat(4, minmax(0, 1fr))'
  if (props.aspect_ratio === 'ratio_32x9') return 'repeat(3, minmax(0, 1fr))'
  return 'repeat(2, minmax(0, 1fr))'
})

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
      <TagBadge v-if="badge" inline color="green">{{ badge }}</TagBadge>

      <div class="slide-header section-intro__header">
        <h1 class="slide-title section-intro__title">{{ title }}</h1>
        <div class="slide-title-divider section-intro__divider" />
      </div>

      <p v-if="subtitle" class="slide-text-note section-intro__subtitle" v-html="subtitle" />

      <div class="section-intro__meta">
        <div class="section-intro__meta-label">本章关注</div>
        <div class="section-intro__meta-value">{{ previewCards.length }} 个子主题</div>
      </div>

      <p v-if="note" class="slide-meta section-intro__note" v-html="note" />
    </div>

    <div class="section-intro__grid" :style="{ gridTemplateColumns: cardGridCols, gap: cardGridGap }">
      <GlassCard
        v-for="(card, idx) in previewCards"
        :key="card.title"
        class="section-intro__card"
        padding="px-5 py-5"
      >
        <div class="section-intro__card-index">{{ `0${idx + 1}`.slice(-2) }}</div>

        <div class="section-intro__card-head">
          <div
            :class="card.icon || 'i-carbon:arrow-right'"
            class="slide-icon-xl"
            :style="{ color: toneVars(card.tone).solid }"
          />
          <div class="section-intro__card-title">{{ card.title }}</div>
        </div>

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
  min-height: 100%;
  padding: 1.25rem 0;
  align-items: stretch;
}

.section-intro__lead {
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
  max-width: 11ch;
  text-align: left;
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
  font-size: 0.72rem;
  letter-spacing: 0.22em;
  text-transform: uppercase;
  color: rgba(255, 255, 255, 0.42);
}

.section-intro__meta-value {
  margin-top: 0.45rem;
  font-size: 1.25rem;
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
  align-content: center;
}

.section-intro__card {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: 1rem;
  min-height: 11.5rem;
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
  font-size: 0.78rem;
  font-family: var(--font-mono);
  color: rgba(255, 255, 255, 0.38);
}

.section-intro__card-head {
  display: flex;
  flex-direction: column;
  gap: 0.85rem;
}

.section-intro__card-title {
  font-size: 1.15rem;
  font-weight: 700;
  line-height: 1.35;
}

.section-intro__card-body {
  line-height: 1.7;
}

.section-intro__card-items {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.section-intro__card-item {
  font-size: 0.92rem;
  line-height: 1.6;
  color: rgba(255, 255, 255, 0.72);
}
</style>
