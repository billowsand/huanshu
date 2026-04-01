<script setup lang="ts">
import { toneVars, type Tone } from './keynoteTheme'

defineProps<{
  section: string
  title: string
  subtitle?: string
  layersTitle?: string
  leftItems: Array<{
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
}>()
</script>

<template>
  <div class="section-num">{{ section }}</div>

  <div relative z-10 flex flex-col h-full py-6>
    <div class="slide-header">
      <h1 class="slide-title">{{ title }}</h1>
      <div class="slide-title-divider" />
    </div>
    <TagBadge v-if="subtitle" inline class="mt-3">{{ subtitle }}</TagBadge>

    <div grid grid-cols-2 gap-5 mt-3 class="flex-1 items-stretch">
      <div flex flex-col gap-3>
          <GlassCard
            v-for="item in leftItems"
            :key="item.title"
            class="flex-1"
            padding="px-4 py-3"
            flex
            items-start
            gap="3"
          >
            <div :class="item.icon" class="slide-icon-2xl" text="var(--primary)" mt-1 flex-shrink-0 />
            <div>
              <div font-bold class="slide-text-meta" mb-1>{{ item.title }}</div>
              <div class="slide-text-caption" opacity-70 v-html="item.body" />
            </div>
          </GlassCard>
      </div>

      <div class="glass-card p-4 flex flex-col gap-3">
        <div font-bold class="slide-text-meta" text="var(--primary)" mb-1>{{ layersTitle || '架构层次' }}</div>
        <div flex flex-col gap-2 class="slide-text-caption">
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
