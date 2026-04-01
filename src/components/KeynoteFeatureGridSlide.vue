<script setup lang="ts">
import { computed } from 'vue'
import { toneVars, type Tone } from './keynoteTheme'

type CardItem = string | {
  text: string
  icon?: string
  iconTone?: Tone
  accent?: boolean
}

const props = defineProps<{
  section?: string
  title: string
  subtitle?: string
  cols?: number
  cards: Array<{
    title: string
    tone?: Tone
    icon?: string
    subtitle?: string
    body?: string
    number?: string
    items?: CardItem[]
    conclusion?: string
    footerTag?: string
    footer_tag?: string
    footerTone?: Tone
    footer_tone?: Tone
    topBar?: string
    topBarClass?: string
    top_bar_class?: string
    titleTone?: Tone
    centered?: boolean
    risk?: string
  }>
  note?: string
}>()

const effectiveCols = computed(() => props.cols ?? props.cards.length)

// normal ≤3, compact =4, mini ≥5
const density = computed(() => {
  const c = effectiveCols.value
  if (c >= 5) return 'mini'
  if (c === 4) return 'compact'
  return 'normal'
})

// Use inline styles to avoid UnoCSS static-scan limitations with dynamic classes
const headerStyle = computed(() => ({
  normal: { padding: '0.75rem 1rem', fontSize: 'var(--type-grid-header-normal)' },
  compact: { padding: '0.5rem 0.75rem', fontSize: 'var(--type-grid-header-compact)' },
  mini: { padding: '0.5rem', fontSize: 'var(--type-grid-header-mini)' },
}[density.value]))

const bodyStyle = computed(() => ({
  normal: { padding: '1rem', fontSize: 'var(--type-grid-body-normal)' },
  compact: { padding: '0.75rem', fontSize: 'var(--type-grid-body-compact)' },
  mini: { padding: '0.5rem', fontSize: 'var(--type-grid-body-mini)' },
}[density.value]))

const iconStyle = computed(() => ({
  normal: { fontSize: 'var(--type-grid-icon-normal)' },
  compact: { fontSize: 'var(--type-grid-icon-compact)' },
  mini: { fontSize: 'var(--type-grid-icon-mini)' },
}[density.value]))

const numberStyle = computed(() => ({
  normal: { fontSize: 'var(--type-grid-number-normal)' },
  compact: { fontSize: 'var(--type-grid-number-compact)' },
  mini: { fontSize: 'var(--type-grid-number-mini)' },
}[density.value]))

const cardGap = computed(() => ({ normal: '1rem', compact: '0.75rem', mini: '0.5rem' }[density.value]))

function normalizeItem(item: CardItem) {
  return typeof item === 'string' ? { text: item } : item
}

</script>

<template>
  <div v-if="section" class="section-num">{{ section }}</div>

  <div relative z-10 flex flex-col h-full py-6>
    <div class="slide-header">
      <h1 class="slide-title">{{ title }}</h1>
      <div class="slide-title-divider" />
    </div>
    <TagBadge v-if="subtitle" inline class="mt-3">{{ subtitle }}</TagBadge>

    <div flex-1 flex flex-col justify-center>
    <div
      class="grid mt-4"
      :style="{
        gridTemplateColumns: `repeat(${effectiveCols}, minmax(0, 1fr))`,
        gap: cardGap,
      }"
    >
      <div
        v-for="(card, cardIdx) in cards"
        :key="card.title"
        class="feature-card-animated"
        :style="{ animationDelay: `${cardIdx * 80 + 150}ms` }"
      >
        <GlassCard
          overflow
          :text-center="card.centered"
          class="flex flex-col h-full"
        >
          <div
            v-if="card.topBar || card.topBarClass || card.top_bar_class"
            h-1
            :class="card.topBarClass || card.top_bar_class"
          />

          <div
            v-if="card.icon || card.title"
            flex items-center gap-2
            :style="{
              ...headerStyle,
              background: toneVars(card.titleTone || card.tone).bg,
              borderBottom: `1px solid ${toneVars(card.titleTone || card.tone).border}`
            }"
          >
            <div v-if="card.icon" :class="card.icon" :style="{ ...iconStyle, color: toneVars(card.titleTone || card.tone).text }" />
            <span font-bold :style="{ fontSize: headerStyle.fontSize, color: toneVars(card.titleTone || card.tone).text }">{{ card.title }}</span>
          </div>

          <div flex flex-col gap-3 flex-1 :style="bodyStyle">
            <div v-if="card.number" font-mono :style="{ ...numberStyle, color: toneVars(card.tone).solid, opacity: 0.5 }">
              {{ card.number }}
            </div>
            <div v-if="card.subtitle" opacity-60>{{ card.subtitle }}</div>
            <div v-if="card.body" opacity-70 leading-relaxed v-html="card.body" />

            <div v-if="card.items?.length" flex flex-col gap-2>
              <div
                v-for="(item, idx) in card.items"
                :key="idx"
                flex items-start gap-2
                :class="card.centered ? 'justify-center text-center' : ''"
              >
                <div
                  :class="normalizeItem(item).icon || 'i-carbon:checkmark-filled'"
                  mt-0.5
                  flex-shrink-0
                  :style="{ ...iconStyle, color: toneVars(normalizeItem(item).iconTone || card.tone).text }"
                />
                <span
                  :style="normalizeItem(item).accent ? { color: toneVars(card.tone).text, fontWeight: 600 } : {}"
                >
                  {{ normalizeItem(item).text }}
                </span>
              </div>
            </div>

            <div
              v-if="card.risk"
              rounded-full px-3 py-1 text-center
              :style="{
                background: toneVars(card.tone).bg,
                border: `1px solid ${toneVars(card.tone).border}`,
                color: toneVars(card.tone).text
              }"
            >
              {{ card.risk }}
            </div>

            <div
              v-if="card.conclusion"
              mt-1 rounded-lg px-3 py-2 text-center
              :style="{
                background: toneVars(card.tone).bg,
                border: `1px solid ${toneVars(card.tone).border}`
              }"
            >
              <span :style="{ color: toneVars(card.tone).text }" font-bold>
                → {{ card.conclusion }}
              </span>
            </div>

            <div v-if="card.footerTag || card.footer_tag">
              <TagBadge :color="card.footerTone || card.footer_tone || card.tone">{{ card.footerTag || card.footer_tag }}</TagBadge>
            </div>
          </div>
        </GlassCard>
      </div>
    </div>

    <div v-if="note" mt-4 class="glass-card px-5 py-3 flex items-center gap-3">
      <div i-carbon:idea class="slide-icon-xl" text="var(--primary)" />
      <span class="slide-text-note" v-html="note" />
    </div>
    </div>
  </div>
</template>

<style scoped>
.feature-card-animated {
  opacity: 0;
  height: 100%;
  animation: card-scale-in 0.45s cubic-bezier(0.16, 1, 0.3, 1) forwards;
  will-change: transform, opacity;
}

@keyframes card-scale-in {
  from {
    opacity: 0;
    transform: scale(0.92) translateY(10px);
  }
  to {
    opacity: 1;
    transform: scale(1) translateY(0);
  }
}
</style>
