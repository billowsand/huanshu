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

  <div relative z-10 flex flex-col h-full py-4>
    <div class="slide-header">
      <SlideTitle class="slide-title" :title="title" />
      <div class="slide-title-divider" />
    </div>
    <TagBadge v-if="subtitle" inline class="slide-subtitle mt-3">{{ subtitle }}</TagBadge>

    <div class="issue-stack" flex-1 min-h-0 flex flex-col gap-4 mt-5>
      <GlassCard
        v-for="(card, idx) in cards"
        :key="`${card.title}-${idx}`"
        overflow
        class="issue-card"
      >
        <div
          class="issue-card__head"
          :style="{
            background: toneVars(card.tone || 'blue').bg,
            borderBottom: `1px solid ${toneVars(card.tone || 'blue').border}`,
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

    <div v-if="footer" mt-4 text-center class="slide-footnote slide-text-caption" opacity-40>
      {{ footer }}
    </div>
  </div>
</template>

<style scoped>
.issue-stack {
  min-height: 0;
}

.issue-card {
  flex: 1 1 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.issue-card__head {
  flex: none;
  padding: 0.6rem 1.2rem 0.55rem;
}

.issue-card__head-inner {
  display: flex;
  align-items: center;
  gap: 0.55rem;
}

.issue-card__eyebrow {
  font-size: calc(var(--type-body) * 1.25);
  font-weight: 600;
  letter-spacing: 0.01em;
  opacity: 0.94;
}

.issue-card__body {
  flex: 1 1 auto;
  min-height: 0;
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 0.65rem;
  padding: 0.8rem 1.25rem 0.85rem;
  color: var(--text);
  overflow: hidden;
}

.issue-card__summary {
  margin: 0;
  font-size: var(--type-body);
  line-height: 1.5;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
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
}
</style>
