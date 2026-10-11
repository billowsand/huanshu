<script setup lang="ts">
import SlideTitle from './SlideTitle.vue'
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
const firstBodyRow = computed(() => {
  const fields = ['number', 'subtitle', 'body', 'items', 'risk', 'conclusion', 'footerTag'] as const
  return fields.find(field => props.cards.some(card => field === 'items'
    ? card.items?.length
    : field === 'footerTag' ? card.footerTag || card.footer_tag : card[field]))
})

// normal ≤3, compact =4, mini ≥5
const density = computed(() => {
  const c = effectiveCols.value
  if (c >= 5) return 'mini'
  if (c === 4) return 'compact'
  return 'normal'
})

// 内容量自适应：列数分档之外，再按「字段数 + 文本量」收敛。
// 高度压力 = 字段数×固定间距 + 文本量（决定换行行数）；列越少卡片越宽，预算越高。
const contentScale = computed(() => {
  const strip = (value?: string) => (value ?? '').replace(/<[^>]*>/g, '')
  const fieldCount = (card: Record<string, unknown>) => [
    card.number, card.subtitle, card.body, (card.items as unknown[])?.length,
    card.risk, card.conclusion, card.footerTag || card.footer_tag,
  ].filter(Boolean).length
  const pressure = Math.max(0, ...(props.cards ?? []).map(card =>
    fieldCount(card as unknown as Record<string, unknown>) * 30
    + strip(card.body).length
    + strip(card.conclusion).length
    + strip(card.risk).length
    + (card.items ?? []).reduce((sum, item) => sum + strip(typeof item === 'string' ? item : item.text).length, 0),
  ))
  const budget = { normal: 420, compact: 190, mini: 140 }[density.value]
  if (pressure > budget * 1.1) return 0.78
  if (pressure > budget) return 0.92
  return 1
})

const isDenseContent = computed(() => contentScale.value < 1)

// 局部重算字号变量：自定义属性的 var() 替换发生在「声明该属性所在的元素」，
// 因此在后代元素上覆盖 --type-scale 不会改变已继承下来的 --type-grid-*，
// 必须在这一层把整组字号重新声明出来。
const contentVars = computed(() => {
  const scale = contentScale.value
  if (scale === 1) return {}
  const size = (base: string) => `calc(${base} * var(--type-scale) * ${scale})`
  return {
    '--type-grid-header-normal': size('1.25rem'),
    '--type-grid-header-compact': size('1.171875rem'),
    '--type-grid-header-mini': size('1.09375rem'),
    '--type-grid-body-normal': size('1rem'),
    '--type-grid-body-compact': size('0.9375rem'),
    '--type-grid-body-mini': size('0.875rem'),
    '--type-grid-icon-normal': size('1.25rem'),
    '--type-grid-icon-compact': size('1.125rem'),
    '--type-grid-icon-mini': size('1rem'),
    '--type-grid-number-normal': size('1.5rem'),
    '--type-grid-number-compact': size('1.25rem'),
    '--type-grid-number-mini': size('1.125rem'),
  }
})

// padding 是字面值，不随变量收敛，需要单独按同一系数缩放
const scalePadding = (padding: string) => {
  const scale = contentScale.value
  if (scale === 1) return padding
  return padding
    .split(' ')
    .map(token => (token.endsWith('rem') ? `calc(${token} * ${scale})` : token))
    .join(' ')
}

// Use inline styles to avoid UnoCSS static-scan limitations with dynamic classes
const headerStyle = computed(() => {
  const base = {
    normal: { padding: '0.75rem 1rem', fontSize: 'var(--type-grid-header-normal)' },
    compact: { padding: '0.5rem 0.75rem', fontSize: 'var(--type-grid-header-compact)' },
    mini: { padding: '0.5rem', fontSize: 'var(--type-grid-header-mini)' },
  }[density.value]
  return { padding: scalePadding(base.padding), fontSize: base.fontSize }
})

const bodyStyle = computed(() => {
  const base = {
    normal: { padding: '1rem', fontSize: 'var(--type-grid-body-normal)' },
    compact: { padding: '0.75rem', fontSize: 'var(--type-grid-body-compact)' },
    mini: { padding: '0.5rem', fontSize: 'var(--type-grid-body-mini)' },
  }[density.value]
  return { padding: scalePadding(base.padding), fontSize: base.fontSize }
})

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

const cardGap = computed(() => {
  const base = { normal: 1, compact: 0.75, mini: 0.5 }[density.value]
  return `${(base * contentScale.value).toFixed(3)}rem`
})

function normalizeItem(item: CardItem) {
  return typeof item === 'string' ? { text: item } : item
}

</script>

<template>
  <div v-if="section" class="section-num">{{ section }}</div>

  <div relative z-10 flex flex-col h-full py-4>
    <div class="slide-header">
      <SlideTitle class="slide-title" :title="title" />
      <div class="slide-title-divider" />
    </div>
    <TagBadge v-if="subtitle" inline class="slide-subtitle mt-3">{{ subtitle }}</TagBadge>

    <div flex-1 flex flex-col justify-center>
    <div
      class="feature-grid mt-4"
      :class="{ 'feature-grid--dense': isDenseContent }"
      :style="[
        {
          gridTemplateColumns: `repeat(${effectiveCols}, minmax(0, 1fr))`,
          gap: cardGap,
        },
        contentVars,
      ]"
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
          class="feature-card"
          :data-tone="card.tone || 'amber'"
        >
          <div
            v-if="card.topBar || card.topBarClass || card.top_bar_class"
            class="feature-top-bar"
            h-1
            :class="card.topBarClass || card.top_bar_class"
          />

          <div
            v-if="card.icon || card.title"
            class="feature-header"
            :data-tone="card.titleTone || card.tone || 'amber'"
            flex items-center gap-2
            :style="{
              ...headerStyle,
              background: toneVars(card.titleTone || card.tone).bg,
              borderBottom: `1px solid ${toneVars(card.titleTone || card.tone).border}`
            }"
          >
            <div v-if="card.icon" :class="card.icon" :style="{ ...iconStyle, color: toneVars(card.titleTone || card.tone).text }" />
            <span class="slide-font-title" font-bold :style="{ fontSize: headerStyle.fontSize, color: toneVars(card.titleTone || card.tone).text }">{{ card.title }}</span>
          </div>

          <div class="feature-body" :data-first-row="firstBodyRow" :style="bodyStyle">
            <div v-if="card.number" class="slide-number feature-number" :style="{ ...numberStyle, color: toneVars(card.tone).solid, opacity: 0.5 }">
              {{ card.number }}
            </div>
            <div v-if="card.subtitle" class="feature-subtitle" opacity-60>{{ card.subtitle }}</div>
            <div v-if="card.body" class="feature-copy" opacity-70 leading-relaxed v-html="card.body" />

            <div v-if="card.items?.length" class="feature-items" flex flex-col gap-2>
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
              class="feature-risk"
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
              class="feature-conclusion"
              rounded-lg px-3 py-2 text-center
              :style="{
                background: toneVars(card.tone).bg,
                border: `1px solid ${toneVars(card.tone).border}`
              }"
            >
              <span :style="{ color: toneVars(card.tone).text }" font-bold>
                → {{ card.conclusion }}
              </span>
            </div>

            <div v-if="card.footerTag || card.footer_tag" class="feature-footer">
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
/* Each visual row shares its tallest content across the cards in that row.
   Explicit tracks keep optional fields from shifting the following sections. */
.feature-grid {
  display: grid;
}

.feature-card-animated,
.feature-card {
  display: grid;
  grid-template-rows: subgrid;
  grid-row: span 9;
  row-gap: 0;
  min-width: 0;
}

.feature-top-bar { grid-row: 1; }
.feature-header { grid-row: 2; }

.feature-body {
  display: grid;
  grid-template-rows: subgrid;
  grid-row: 3 / span 7;
  row-gap: 0;
  min-width: 0;
}

.feature-body > * {
  min-width: 0;
  align-self: start;
  overflow-wrap: anywhere;
}

.feature-body > * { margin-top: 0.75rem; }

/* 内容超预算时同步收紧字段间距，与 contentVars 的字号收敛配合 */
.feature-grid--dense .feature-body > * { margin-top: 0.32rem; }
.feature-body[data-first-row="number"] > .feature-number,
.feature-body[data-first-row="subtitle"] > .feature-subtitle,
.feature-body[data-first-row="body"] > .feature-copy,
.feature-body[data-first-row="items"] > .feature-items,
.feature-body[data-first-row="risk"] > .feature-risk,
.feature-body[data-first-row="conclusion"] > .feature-conclusion,
.feature-body[data-first-row="footerTag"] > .feature-footer { margin-top: 0; }
.feature-number { grid-row: 1; }
.feature-subtitle { grid-row: 2; }
.feature-copy { grid-row: 3; }
.feature-items { grid-row: 4; }
.feature-risk { grid-row: 5; }
.feature-conclusion { grid-row: 6; align-self: stretch; }
.feature-footer { grid-row: 7; }

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
