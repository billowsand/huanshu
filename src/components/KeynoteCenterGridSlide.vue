<script setup lang="ts">
import { toneVars, type Tone } from './keynoteTheme'

defineProps<{
  badge?: string
  title: string
  accent?: string
  cols?: number
  items: Array<{
    title: string
    desc: string
    icon: string
    tone?: Tone
  }>
  footer?: string
}>()
</script>

<template>
  <div relative z-10 flex flex-col items-center justify-center gap-5 text-center h-full py-6>
    <TagBadge v-if="badge">{{ badge }}</TagBadge>

    <div class="slide-header center-grid-header">
      <h2 class="slide-title center-grid-title">
        {{ title }}<br v-if="accent" />
        <span v-if="accent" text="var(--primary)">{{ accent }}</span>
      </h2>
      <div class="slide-title-divider center-grid-divider" />
    </div>

    <CardGrid :cols="cols || 3" gap="4" w-full max-w-2xl>
      <GlassCard
        v-for="item in items"
        :key="item.title"
        padding="px-4 py-4"
        flex
        flex-col
        items-center
        gap="2"
        text-center
      >
        <div :class="item.icon" class="slide-icon-2xl" :style="{ color: toneVars(item.tone).solid }" />
        <div class="slide-body-tight center-grid-item-title">{{ item.title }}</div>
        <div class="slide-meta" opacity-60>{{ item.desc }}</div>
      </GlassCard>
    </CardGrid>

    <div v-if="footer" class="slide-meta center-grid-footer" mt-2 font-mono>{{ footer }}</div>
  </div>
</template>

<style scoped>
.center-grid-header {
  width: 100%;
}

.center-grid-title {
  text-align: center;
}

.center-grid-divider {
  margin: 0.75rem auto 0;
  width: 16rem;
}

.center-grid-item-title {
  font-weight: 700;
}

.center-grid-footer {
  opacity: 0.42;
}
</style>
