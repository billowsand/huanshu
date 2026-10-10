<script setup lang="ts">
import { computed } from 'vue'
import SlideTitle from './SlideTitle.vue'
import { toneVars, type Tone } from './keynoteTheme'

const props = defineProps<{
  label: string
  labelTone?: Tone
  title: string
  images?: string[]
  image?: string
  placeholder?: string
  sideWidth?: string
  panels: Array<{
    kind?: 'default' | 'flow' | 'matrix'
    title: string
    icon?: string
    tone?: Tone
    body?: string
    items?: string[]
    steps?: string[]
    highlight?: string
  }>
}>()

// 侧栏内容量自适应：条目数与文本量共同决定高度压力，压力越大收得越紧。
// 与 feature_grid 同理——在后代元素上覆盖 --type-scale 无法影响已经
// 继承下来的 --type-*，所以必须在这一层重新声明字号。
const sideScale = computed(() => {
  const panels = props.panels ?? []
  const items = panels.reduce((sum, panel) => sum + (panel.items?.length ?? 0) + (panel.steps?.length ?? 0), 0)
  const chars = panels.reduce((sum, panel) =>
    sum + (panel.body?.length ?? 0) + (panel.items ?? []).join('').length + (panel.steps ?? []).join('').length, 0)
  const pressure = items * 26 + chars
  if (pressure >= 460) return 0.78
  if (pressure >= 360) return 0.86
  if (pressure >= 270) return 0.92
  return 1
})

const sideDense = computed(() => sideScale.value < 1)

const sideVars = computed(() => {
  const scale = sideScale.value
  if (scale === 1) return {}
  const size = (base: string) => `calc(${base} * var(--type-scale) * ${scale})`
  return {
    '--type-body': size('1.125rem'),
    '--type-card-title': size('1.25rem'),
    '--type-body-tight': size('1.0625rem'),
    '--type-meta': size('1rem'),
    '--type-caption': size('0.9375rem'),
    '--type-icon-lg': size('1.25rem'),
    '--type-icon-sm': size('0.875rem'),
  }
})

</script>

<template>
  <div relative z-10 h-full flex flex-col py-4>
    <div class="slide-header spotlight-header" flex items-center gap-3 mb-4>
      <div
        class="tool-badge"
        :style="{
          background: toneVars(labelTone).bg,
          borderColor: toneVars(labelTone).border,
          color: toneVars(labelTone).text
        }"
      >
        {{ label }}
      </div>
      <SlideTitle as="h2" class="slide-title" :title="title" />
    </div>

    <div flex gap-5 flex-1 min-h-0>
      <div class="spotlight-media" flex-1 rounded-xl overflow-hidden border="1 solid white/8" bg="white/3" flex items-center justify-center>
        <ImageCarousel v-if="images?.length" :images="images" />
        <img v-else-if="props.image" :src="props.image" object-contain max-h-full max-w-full rounded-lg />
        <div v-else class="glass-card w-full h-full flex items-center justify-center">
          <div flex flex-col items-center gap-3 opacity-40>
            <div i-carbon:image class="slide-icon-hero" />
            <div class="slide-meta" text-mono>{{ placeholder || '图片待补充' }}</div>
          </div>
        </div>
      </div>

      <div
        class="spotlight-side"
        :class="[sideWidth || 'w-72', { 'spotlight-side--dense': sideDense }]"
        :style="sideVars"
        flex flex-col gap-3 flex-shrink-0
      >
        <div
          v-for="panel in panels"
          :key="panel.title"
          class="glass-card px-4 py-3"
          :style="panel.highlight ? {
            background: toneVars(panel.tone).bg,
            border: `1px solid ${toneVars(panel.tone).border}`
          } : {}"
        >
          <div flex items-center gap-2 mb-2>
            <div v-if="panel.icon" :class="panel.icon" class="slide-icon-lg" :style="{ color: toneVars(panel.tone).text }" />
            <span class="slide-card-title spotlight-panel-title">{{ panel.title }}</span>
          </div>
          <div v-if="panel.body" class="slide-meta" opacity-70 v-html="panel.body" />
          <div
            v-if="panel.kind !== 'flow' && panel.kind !== 'matrix' && panel.items?.length"
            class="slide-meta"
            flex flex-col gap-1 opacity-70
          >
            <div v-for="item in panel.items" :key="item" flex items-center gap-1>
              <div i-carbon:checkmark text="var(--primary)" />
              <span>{{ item }}</span>
            </div>
          </div>
          <div v-if="panel.kind === 'flow' && panel.items?.length" class="slide-meta" flex flex-col gap-2>
            <template v-for="(item, idx) in panel.items" :key="item">
              <div
                class="glass-card px-3 py-2 flex items-center gap-2"
                :style="idx === panel.items.length - 1
                  ? {
                    background: 'var(--warning-bg)',
                    borderLeft: '2px solid var(--warning)',
                  }
                  : {
                    borderLeft: `2px solid ${toneVars(panel.tone).solid}`,
                  }"
              >
                <div
                  :class="idx === panel.items.length - 1 ? 'i-carbon:rotate-clockwise' : panel.icon || 'i-carbon:chevron-right'"
                  flex-shrink-0
                  :style="{ color: idx === panel.items.length - 1 ? 'var(--primary)' : toneVars(panel.tone).text }"
                />
                <span :style="idx === panel.items.length - 1 ? { color: 'var(--primary-light)', fontWeight: 600 } : {}">
                  {{ item }}
                </span>
              </div>
              <div
                v-if="idx < panel.items.length - 1"
                class="slide-caption"
                text-center opacity-30 font-mono
              >
                ↓
              </div>
            </template>
          </div>
          <div v-if="panel.kind === 'matrix' && panel.items?.length" class="slide-meta" grid grid-cols-2 gap-2>
            <div
              v-for="(item, idx) in panel.items"
              :key="item"
              class="glass-card px-2 py-2 flex items-center gap-1"
              :style="idx === panel.items.length - 1
                ? {
                  background: 'var(--warning-bg)',
                  borderTop: '2px solid var(--warning)',
                }
                : idx < 4
                  ? {
                    borderTop: `2px solid ${toneVars(panel.tone).solid}`,
                  }
                  : {
                    borderTop: '2px solid var(--warning)',
                  }"
            >
              <div
                :class="
                  idx === 0 ? 'i-carbon:map'
                  : idx === 1 ? 'i-carbon:application'
                  : idx === 2 ? 'i-carbon:run-mirror'
                  : idx === 3 ? 'i-carbon:task-view'
                  : idx === 4 ? 'i-carbon:lightning'
                  : idx === 5 ? 'i-carbon:network-4'
                  : idx === 6 ? 'i-carbon:compare'
                  : 'i-carbon:checkmark-outline'
                "
                class="slide-icon-sm"
                flex-shrink-0
                :style="{ color: idx <= 3 ? toneVars(panel.tone).text : idx === panel.items.length - 1 ? 'var(--primary-light)' : 'var(--primary)' }"
              />
              <span :style="idx === panel.items.length - 1 ? { color: 'var(--primary-light)' } : {}">
                {{ item }}
              </span>
            </div>
          </div>
          <div v-if="panel.steps?.length" class="slide-meta" flex flex-col gap-2>
            <div v-for="(step, idx) in panel.steps" :key="step" flex items-center gap-2>
              <div
                rounded-full w-5 h-5 flex items-center justify-center slide-text-caption font-bold flex-shrink-0
                :style="{
                  background: toneVars(panel.tone).solid,
                  border: `1px solid ${toneVars(panel.tone).border}`,
                  color: toneVars(panel.tone).text
                }"
              >
                {{ idx + 1 }}
              </div>
              <span opacity-70>{{ step }}</span>
            </div>
          </div>
          <div v-if="panel.highlight" class="slide-meta" mt-1 :style="{ color: toneVars(panel.tone).text, fontWeight: 700 }">
            {{ panel.highlight }}
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.spotlight-header {
  align-items: center;
}

.spotlight-panel-title {
  font-weight: 700;
}

/* 内容超预算时一并收紧侧栏间距与面板内边距（覆盖 UnoCSS 的 px-4 py-3） */
.spotlight-side--dense {
  gap: 0.5rem;
}

.spotlight-side--dense :deep(.glass-card) {
  padding: 0.5rem 0.62rem;
}
</style>
