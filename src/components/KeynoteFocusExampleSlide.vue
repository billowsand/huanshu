<script setup lang="ts">
import SlideTitle from './SlideTitle.vue'
import { renderRichText } from '../utils/richText'
defineProps<{
  section: string
  title: string
  subtitle?: string
  points: Array<{
    icon: string
    title: string
    body: string
  }>
  exampleTitle: string
  exampleBody: string
  ranking: Array<{
    index: string
    label: string
    meta: string
    muted?: boolean
  }>
}>()
</script>

<template>
  <div class="section-num"><span v-html="renderRichText(section)"></span></div>

  <div relative z-10 flex flex-col h-full py-4>
    <div class="slide-header">
      <SlideTitle class="slide-title" :title="title" />
      <div class="slide-title-divider" />
    </div>
    <TagBadge v-if="subtitle" inline class="slide-subtitle mt-3"><span v-html="renderRichText(subtitle)"></span></TagBadge>

    <div flex-1 flex flex-col justify-center>
    <div grid grid-cols-2 gap-5 mt-4>
      <div flex flex-col gap-3>
          <GlassCard
            v-for="point in points"
            :key="point.title"
            class="focus-point"
            padding="px-4 py-3"
            flex
            items-start
            gap="3"
          >
            <div :class="point.icon" class="slide-icon-2xl" text="var(--primary)" mt-1 flex-shrink-0 />
            <div>
              <div font-bold class="slide-card-title" text="var(--primary-light)"><span v-html="renderRichText(point.title)"></span></div>
              <div class="slide-text-caption slide-font-body" opacity-70 mt-1 v-html="renderRichText(point.body)" />
            </div>
          </GlassCard>
      </div>

      <div class="glass-card focus-example p-5 flex flex-col justify-between">
        <div>
          <div font-bold class="slide-card-title" text="var(--primary)" mb-3><span v-html="renderRichText(exampleTitle)"></span></div>
          <div class="slide-text-caption slide-font-body" opacity-70 leading-relaxed mb-4 v-html="renderRichText(exampleBody)" />
          <div class="divider-amber" mb-4 />
          <div font-bold class="slide-text-meta" mb-2>正确的关注顺序：</div>
          <div flex flex-col gap-2 class="slide-text-caption slide-font-body">
            <div
              v-for="item in ranking"
              :key="item.index"
              class="focus-ranking-row"
              flex items-center gap-2
              :style="item.muted ? { opacity: 0.4 } : {}"
            >
              <div class="slide-number" :style="{ color: item.muted ? 'var(--text-muted)' : 'var(--primary)' }"><span v-html="renderRichText(item.index)"></span></div>
              <span>
                <span v-html="renderRichText(item.label)"></span>
                <span opacity-50>（<span v-html="renderRichText(item.meta)"></span>）</span>
              </span>
            </div>
          </div>
        </div>
      </div>
    </div>
    </div>
  </div>
</template>
