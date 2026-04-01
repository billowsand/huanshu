<script setup lang="ts">
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

</script>

<template>
  <div relative z-10 h-full flex flex-col py-6>
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
      <h2 class="slide-title">{{ title }}</h2>
    </div>

    <div flex gap-5 flex-1 min-h-0>
      <div flex-1 rounded-xl overflow-hidden border="1 solid white/8" bg="white/3" flex items-center justify-center>
        <ImageCarousel v-if="images?.length" :images="images" />
        <img v-else-if="props.image" :src="props.image" object-contain max-h-full max-w-full rounded-lg />
        <div v-else class="glass-card w-full h-full flex items-center justify-center">
          <div flex flex-col items-center gap-3 opacity-40>
            <div i-carbon:image class="slide-icon-hero" />
            <div class="slide-meta" text-mono>{{ placeholder || '图片待补充' }}</div>
          </div>
        </div>
      </div>

      <div :class="sideWidth || 'w-72'" flex flex-col gap-3 flex-shrink-0>
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
            <span class="slide-body-tight spotlight-panel-title">{{ panel.title }}</span>
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
</style>
