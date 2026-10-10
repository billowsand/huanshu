<script setup lang="ts">
import SlideTitle from './SlideTitle.vue'
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
  <div class="section-num">{{ section }}</div>

  <div relative z-10 flex flex-col h-full py-4>
    <div class="slide-header">
      <SlideTitle class="slide-title" :title="title" />
      <div class="slide-title-divider" />
    </div>
    <TagBadge v-if="subtitle" inline class="slide-subtitle mt-3">{{ subtitle }}</TagBadge>

    <div flex-1 flex flex-col justify-center>
    <div grid grid-cols-2 gap-5 mt-4>
      <div flex flex-col gap-3>
          <GlassCard
            v-for="point in points"
            :key="point.title"
            padding="px-4 py-3"
            flex
            items-start
            gap="3"
          >
            <div :class="point.icon" class="slide-icon-2xl" text="var(--primary)" mt-1 flex-shrink-0 />
            <div>
              <div font-bold class="slide-card-title" text="var(--primary-light)">{{ point.title }}</div>
              <div class="slide-text-caption" opacity-70 mt-1 v-html="point.body" />
            </div>
          </GlassCard>
      </div>

      <div class="glass-card p-5 flex flex-col justify-between">
        <div>
          <div font-bold class="slide-card-title" text="var(--primary)" mb-3>{{ exampleTitle }}</div>
          <div class="slide-text-caption" opacity-70 leading-relaxed mb-4 v-html="exampleBody" />
          <div class="divider-amber" mb-4 />
          <div font-bold class="slide-text-meta" mb-2>正确的关注顺序：</div>
          <div flex flex-col gap-2 class="slide-text-caption">
            <div
              v-for="item in ranking"
              :key="item.index"
              flex items-center gap-2
              :style="item.muted ? { opacity: 0.4 } : {}"
            >
              <div class="slide-number" :style="{ color: item.muted ? 'var(--text-muted)' : 'var(--primary)' }" font-mono>{{ item.index }}</div>
              <span>
                {{ item.label }}
                <span opacity-50>（{{ item.meta }}）</span>
              </span>
            </div>
          </div>
        </div>
      </div>
    </div>
    </div>
  </div>
</template>
