<script setup lang="ts">
import { computed } from 'vue'
import { toneVars, type Tone } from './keynoteTheme'
import InfographicCanvas from './InfographicCanvas.vue'

const props = defineProps<{
  section: string
  title: string
  subtitle?: string
  layersTitle?: string
  layersInfographicSyntax?: string
  leftItems: Array<{
    step?: string
    icon: string
    title: string
    body: string
  }>
  layers: Array<{
    title: string
    meta: string
    tone?: Tone
  }>
  footer?: string
  aspect_ratio?: 'ratio_16x9' | 'ratio_32x9' | 'ratio_48x9'
}>()

const gridCols = computed(() => {
  if (props.aspect_ratio === 'ratio_48x9') return '1fr 4fr'
  if (props.aspect_ratio === 'ratio_32x9') return '1fr 3fr'
  return '1fr 1fr'
})

const gridGap = computed(() => {
  if (props.aspect_ratio === 'ratio_48x9') return '1.5rem'
  if (props.aspect_ratio === 'ratio_32x9') return '1.35rem'
  return '1.25rem'
})

const hasInfographic = computed(() => Boolean(props.layersInfographicSyntax?.trim()))
</script>

<template>
  <div class="section-num">{{ section }}</div>

  <div relative z-10 flex flex-col h-full py-6>
    <div class="slide-header">
      <h1 class="slide-title">{{ title }}</h1>
      <div class="slide-title-divider" />
    </div>
    <TagBadge v-if="subtitle" inline class="mt-3">{{ subtitle }}</TagBadge>

    <div :style="{ display: 'grid', gridTemplateColumns: gridCols, gap: gridGap }" mt-3 class="flex-1 items-stretch min-h-0">
      <div class="split-left-col">
        <GlassCard
          v-for="(item, idx) in leftItems"
          :key="`${item.title}-${idx}`"
          class="split-left-card"
          padding="px-4 py-3"
        >
          <div flex items-start gap="3">
            <div class="split-left-step">
              {{ item.step || String(idx + 1).padStart(2, '0') }}
            </div>
            <div :class="item.icon" class="slide-icon-2xl" text="var(--primary)" mt-1 flex-shrink-0 />
            <div class="min-w-0">
              <div class="split-left-title-row">
                <div font-bold class="slide-text-meta">{{ item.title }}</div>
                <div class="split-left-index">{{ String(idx + 1).padStart(2, '0') }}</div>
              </div>
              <div class="slide-text-caption split-left-body" opacity-78 v-html="item.body" />
            </div>
          </div>
          <div v-if="idx < leftItems.length - 1" class="split-left-connector" />
        </GlassCard>
      </div>

      <div class="glass-card split-right-card p-4 flex flex-col gap-3 min-h-0">
        <div font-bold class="slide-text-meta" text="var(--primary)" mb-1>{{ layersTitle || '架构层次' }}</div>
        <div v-if="hasInfographic" class="split-right-visual">
          <InfographicCanvas
            :syntax="layersInfographicSyntax || ''"
            fallback-class="split-infographic-fallback"
          />
        </div>
        <div v-else flex flex-col gap-2 class="slide-text-caption">
          <template v-for="(layer, idx) in layers" :key="layer.title">
            <div
              rounded-lg px-3 py-2 flex items-center justify-between
              :style="{
                background: toneVars(layer.tone).bg,
                border: `1px solid ${toneVars(layer.tone).border}`
              }"
            >
              <span font-semibold>{{ layer.title }}</span>
              <span opacity-50>{{ layer.meta }}</span>
            </div>
            <div v-if="idx < layers.length - 1" class="slide-icon-xl" text-center opacity-30 :style="{ color: toneVars(layers[idx + 1].tone).text }">↑</div>
          </template>
        </div>
        <div v-if="footer" mt-1 class="slide-text-caption" opacity-50 text-center v-html="footer" />
      </div>
    </div>
  </div>
</template>

<style scoped>
.split-left-col {
  display: grid;
  gap: 0.8rem;
  min-height: 0;
}

.split-left-card {
  position: relative;
  overflow: hidden;
}

.split-left-step {
  min-width: 2.35rem;
  height: 2.35rem;
  border-radius: 999px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 0.72rem;
  font-weight: 700;
  letter-spacing: 0.08em;
  color: var(--primary);
  background: var(--primary-bg);
  border: 1px solid var(--primary-border);
}

.split-left-title-row {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 0.75rem;
  margin-bottom: 0.32rem;
}

.split-left-index {
  font-size: 0.68rem;
  letter-spacing: 0.14em;
  color: var(--text-muted);
}

.split-left-body {
  line-height: 1.6;
}

.split-left-connector {
  margin-top: 0.75rem;
  margin-left: 1.15rem;
  width: 1px;
  height: 0.8rem;
  background: linear-gradient(180deg, var(--primary), var(--glass));
}

.split-right-card {
  overflow: hidden;
}

.split-right-visual {
  flex: 1;
  min-height: 0;
  border-radius: 12px;
  overflow: hidden;
  background: var(--glass);
  border: 1px solid var(--glass-border);
}

:deep(.split-infographic-fallback) {
  color: var(--text-muted);
  font-size: 0.75rem;
  padding: 16px;
}
</style>
