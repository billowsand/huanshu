<script setup lang="ts">
import { computed } from 'vue'

const props = defineProps<{
  section: string
  title: string
  items: Array<{
    number: string
    title: string
    desc: string
  }>
  note?: string
}>()

// Single column for 1–2 items (wider cards, better use of space);
// two columns for 3+ items.
const gridCols = computed(() => props.items.length <= 2 ? 1 : 2)
</script>

<template>
  <div class="section-num">{{ section }}</div>

  <div relative z-10 flex flex-col h-full py-6>
    <div class="slide-header">
      <h1 class="slide-title slide-compact-title">{{ title }}</h1>
      <div class="slide-title-divider" />
    </div>

    <div flex-1 flex flex-col justify-center>
      <CardGrid :cols="gridCols" gap="3">
        <div
          v-for="(item, idx) in items"
          :key="item.number"
          class="overview-item-animated"
          :style="{ animationDelay: `${idx * 80 + 150}ms` }"
        >
          <GlassCard
            padding="px-4 py-3"
            flex
            items-center
            gap="3"
          >
            <div class="slide-icon-2xl" font-mono text="var(--primary)" opacity-50>{{ item.number }}</div>
            <div>
              <div font-bold text="var(--text)">{{ item.title }}</div>
              <div class="slide-text-caption" opacity-50>{{ item.desc }}</div>
            </div>
          </GlassCard>
        </div>
      </CardGrid>

      <div v-if="note" mt-4 class="glass-card px-5 py-3 flex items-center gap-3">
        <div i-carbon:idea class="slide-icon-xl" text="var(--primary)" />
        <span class="slide-text-note" opacity-80 v-html="note" />
      </div>
    </div>
  </div>
</template>

<style scoped>
.overview-item-animated {
  opacity: 0;
  animation: overview-slide-in 0.4s cubic-bezier(0.16, 1, 0.3, 1) forwards;
  will-change: transform, opacity;
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
