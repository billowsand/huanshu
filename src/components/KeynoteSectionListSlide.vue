<script setup lang="ts">
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
    {{ section }}
  </div>

  <div relative z-10 flex flex-col h-full py-6>
    <div class="slide-header">
      <h1 class="slide-title slide-compact-title">{{ title }}</h1>
      <div class="slide-title-divider" />
    </div>
    <TagBadge v-if="subtitle" inline class="mt-3">{{ subtitle }}</TagBadge>

    <div flex-1 flex flex-col justify-center>
    <div flex flex-col gap-4 mt-3>
        <GlassCard
          v-for="item in items"
          :key="item.title"
          padding="px-5 py-4"
          flex
          items-start
          gap="4"
        >
          <StepNumber v-if="item.step" :num="item.step" />
          <div v-else-if="item.icon" :class="item.icon" class="slide-icon-2xl" text="var(--primary)" mt-1 flex-shrink-0 />
          <div>
            <div font-bold mb-1>{{ item.title }}</div>
            <div class="slide-text-meta" opacity-70 v-html="item.body" />
          </div>
        </GlassCard>
    </div>
    </div>
  </div>
</template>
