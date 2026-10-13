<script setup lang="ts">
import SlideTitle from './SlideTitle.vue'
import { renderRichText } from '../utils/richText'
defineProps<{
  section?: string
  sectionLabelSize?: string
  title: string
  subtitle?: string
  items: Array<{
    step?: string
    icon?: string
    title: string
    body: string
  }>
}>()
</script>

<template>
  <div v-if="section" class="section-num" :style="sectionLabelSize ? { fontSize: sectionLabelSize } : {}">
    <span v-html="renderRichText(section)"></span>
  </div>

  <div relative z-10 flex flex-col h-full py-4>
    <div class="slide-header">
      <SlideTitle class="slide-title slide-compact-title" :title="title" />
      <div class="slide-title-divider" />
    </div>
    <TagBadge v-if="subtitle" inline class="slide-subtitle mt-3"><span v-html="renderRichText(subtitle)"></span></TagBadge>

    <div flex-1 flex flex-col justify-center>
    <div flex flex-col gap-4 mt-3>
        <GlassCard
          v-for="item in items"
          :key="item.title"
          class="section-list-card"
          padding="px-5 py-4"
          flex
          items-start
          gap="4"
        >
          <StepNumber v-if="item.step" :num="item.step" />
          <div v-else-if="item.icon" :class="item.icon" class="slide-icon-2xl" text="var(--primary)" mt-1 flex-shrink-0 />
          <div>
            <div class="slide-card-title" mb-1><span v-html="renderRichText(item.title)"></span></div>
            <div class="slide-text-meta" opacity-70 v-html="renderRichText(item.body)" />
          </div>
        </GlassCard>
    </div>
    </div>
  </div>
</template>
