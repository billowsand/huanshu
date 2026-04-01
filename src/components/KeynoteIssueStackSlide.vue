<script setup lang="ts">
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

  <div relative z-10 flex flex-col h-full py-6>
    <div class="slide-header">
      <h1 class="slide-title">{{ title }}</h1>
      <div class="slide-title-divider" />
    </div>
    <TagBadge v-if="subtitle" inline class="mt-3">{{ subtitle }}</TagBadge>

    <div class="issue-stack" flex-1 flex flex-col gap-5 justify-center mt-5>
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

    <div v-if="footer" mt-4 text-center class="slide-text-caption" opacity-40>
      {{ footer }}
    </div>
  </div>
</template>

<style scoped>
.issue-card {
  min-height: 132px;
}

.issue-card__head {
  padding: 0.8rem 1.2rem 0.7rem;
}

.issue-card__head-inner {
  display: flex;
  align-items: center;
  gap: 0.55rem;
}

.issue-card__eyebrow {
  font-size: 1rem;
  font-weight: 600;
  letter-spacing: 0.01em;
  opacity: 0.94;
}

.issue-card__body {
  display: flex;
  flex-direction: column;
  gap: 1rem;
  padding: 1.2rem 1.35rem 1.15rem;
  color: var(--text);
}

.issue-card__summary {
  margin: 0;
  font-size: 1.05rem;
  line-height: 1.55;
}

.issue-card__meta {
  display: flex;
  flex-wrap: wrap;
  gap: 0.55rem;
}

.issue-card__pill {
  display: inline-flex;
  align-items: center;
  min-height: 28px;
  padding: 0.2rem 0.72rem;
  border-radius: 999px;
  font-size: 0.82rem;
  line-height: 1.2;
  opacity: 0.88;
}
</style>
