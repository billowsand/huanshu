<script setup lang="ts">
import SlideTitle from './SlideTitle.vue'
import { toneVars, type Tone } from './keynoteTheme'

defineProps<{
  section?: string
  title: string
  subtitle?: string
  cards: Array<{
    title: string
    tone?: Tone
    icon?: string
    body?: string
    items?: string[]
  }>
  footer?: string
}>()
</script>

<template>
  <div v-if="section" class="section-num">{{ section }}</div>

  <div class="issue-slide">
    <div class="slide-header">
      <SlideTitle class="slide-title" :title="title" />
      <div class="slide-title-divider" />
    </div>
    <TagBadge v-if="subtitle" inline class="slide-subtitle mt-3">{{ subtitle }}</TagBadge>

    <div class="issue-stack" :class="{ 'issue-stack--rows': cards.length >= 4 }">
      <GlassCard
        v-for="(card, idx) in cards"
        :key="`${card.title}-${idx}`"
        no-padding
        class="issue-card"
        :data-tone="card.tone || 'blue'"
      >
        <div
          class="issue-card__head"
          :style="{
            background: toneVars(card.tone || 'blue').bg,
            '--issue-border': toneVars(card.tone || 'blue').border,
          }"
        >
          <div class="issue-card__head-inner">
            <span v-if="card.icon" :class="card.icon" class="slide-icon-sm" :style="{ color: toneVars(card.tone || 'blue').text }" />
            <span class="issue-card__eyebrow" :style="{ color: toneVars(card.tone || 'blue').text }">{{ card.title }}</span>
          </div>
        </div>

        <div class="issue-card__body">
          <p v-if="card.body" class="issue-card__summary">{{ card.body }}</p>

          <div v-if="card.items?.length" class="issue-card__meta">
            <span
              v-for="item in card.items"
              :key="item"
              class="issue-card__pill"
              :style="{
                background: toneVars(card.tone || 'blue').bg,
                border: `1px solid ${toneVars(card.tone || 'blue').border}`,
                color: toneVars(card.tone || 'blue').text,
              }"
            >
              {{ item }}
            </span>
          </div>
        </div>
      </GlassCard>
    </div>

    <div v-if="footer" class="issue-footer slide-footnote slide-text-caption">
      {{ footer }}
    </div>
  </div>
</template>

<style scoped>
.issue-slide {
  position: relative;
  z-index: 10;
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  padding: 1rem 0;
}

.issue-stack {
  flex: 1 1 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
  margin-top: 1.25rem;
}

.issue-card {
  flex: 1 1 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.issue-card__head {
  flex: none;
  padding: 0.4rem 1.2rem;
  border-bottom: 1px solid var(--issue-border);
}

.issue-card__head-inner {
  display: flex;
  align-items: center;
  gap: 0.55rem;
}

.issue-card__head-inner .slide-icon-sm {
  flex-shrink: 0;
}

.issue-card__eyebrow {
  font-size: calc(var(--type-body) * 1.25);
  font-weight: 600;
  letter-spacing: 0.01em;
  opacity: 0.94;
  line-height: 1.3;
  overflow-wrap: anywhere;
}

.issue-card__body {
  flex: 1 1 auto;
  min-width: 0;
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 0.4rem;
  padding: 0.55rem 1.25rem;
  color: var(--text);
}

.issue-card__summary {
  margin: 0;
  font-size: var(--type-body);
  line-height: 1.4;
  overflow-wrap: anywhere;
  flex-shrink: 0;
}

.issue-card__meta {
  flex: none;
  display: flex;
  flex-wrap: wrap;
  gap: 0.45rem;
}

.issue-card__pill {
  display: inline-flex;
  align-items: center;
  min-height: 26px;
  padding: 0.16rem 0.65rem;
  border-radius: 999px;
  font-size: var(--type-caption);
  line-height: 1.2;
  opacity: 0.88;
  max-width: 100%;
  white-space: normal;
  overflow-wrap: anywhere;
}

.issue-stack--rows .issue-card {
  display: grid;
  grid-template-columns: minmax(0, 22fr) minmax(0, 78fr);
}

.issue-stack--rows .issue-card__head {
  display: flex;
  align-items: center;
  padding: 0.8rem 1rem;
  border-bottom: 0;
  border-right: 1px solid var(--issue-border);
}

.issue-stack--rows .issue-card__body {
  gap: 0.4rem;
  padding: 0.65rem 1.1rem;
}

.issue-stack--rows .issue-card__summary {
  line-height: 1.35;
}

.issue-stack--rows .issue-card__meta {
  gap: 0.35rem 0.45rem;
}

.issue-footer {
  flex: none;
  margin-top: 0.85rem;
  text-align: left;
  line-height: 1.45;
  overflow-wrap: anywhere;
}
</style>
