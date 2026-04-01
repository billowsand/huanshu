<script setup lang="ts">
import { computed } from 'vue'
import { toneVars, type Tone } from './keynoteTheme'
import SlideReveal from './SlideReveal.vue'

const props = defineProps<{
  section?: string
  title: string
  subtitle?: string
  quadrants: Array<{
    key: 'strengths' | 'weaknesses' | 'opportunities' | 'threats' | string
    title: string
    tone?: Tone
    icon?: string
    items: string[]
    summary?: string
  }>
  strategy?: string
}>()

const orderedQuadrants = computed(() => {
  const order = ['strengths', 'weaknesses', 'opportunities', 'threats']
  return [...props.quadrants].sort((a, b) => order.indexOf(a.key) - order.indexOf(b.key))
})

function isRightColumn(cardIdx: number) {
  return cardIdx % 2 === 1
}

function summaryFirst(quadrantKey: string) {
  return quadrantKey === 'strengths' || quadrantKey === 'weaknesses'
}
</script>

<template>
  <div v-if="section" class="section-num">{{ section }}</div>

  <div relative z-10 py-6 flex flex-col gap-4 h-full>
    <div class="slide-header">
      <h1 class="slide-title slide-compact-title">{{ title }}</h1>
      <div class="slide-title-divider" />
    </div>
    <TagBadge v-if="subtitle" inline class="mt-2">{{ subtitle }}</TagBadge>

    <div
      class="grid flex-1 min-h-0"
      :style="{
        gridTemplateColumns: 'repeat(2, minmax(0, 1fr))',
        gridTemplateRows: 'repeat(2, minmax(0, 1fr))',
        gap: '1rem',
      }"
    >
      <GlassCard
        v-for="(quadrant, cardIdx) in orderedQuadrants"
        :key="quadrant.key"
        overflow
        class="swot-card flex flex-col"
        :class="[
          isRightColumn(cardIdx) ? 'swot-card-right' : 'swot-card-left',
          `swot-entrance-${cardIdx}`
        ]"
        no-padding
      >
        <div class="swot-body">
          <div
            class="swot-flag"
            :class="[
              cardIdx === 0 ? 'swot-flag-tr' : '',
              cardIdx === 1 ? 'swot-flag-tl' : '',
              cardIdx === 2 ? 'swot-flag-br' : '',
              cardIdx === 3 ? 'swot-flag-bl' : '',
            ]"
            :style="{
              background: toneVars(quadrant.tone).bg,
              borderColor: toneVars(quadrant.tone).border,
              color: toneVars(quadrant.tone).text,
            }"
          >
            <div
              class="swot-flag-icon"
              :style="{ background: toneVars(quadrant.tone).solid }"
            >
              <div :class="quadrant.icon || 'i-carbon:idea'" class="slide-icon-sm" text-white />
            </div>
            <span class="swot-flag-text">{{ quadrant.title }}</span>
          </div>

          <div
            v-if="quadrant.summary && summaryFirst(quadrant.key)"
            class="swot-summary"
            :class="isRightColumn(cardIdx) ? 'swot-summary-right' : ''"
            :style="{
              background: toneVars(quadrant.tone).bg,
              borderColor: toneVars(quadrant.tone).border,
              color: toneVars(quadrant.tone).text,
            }"
          >
            {{ quadrant.summary }}
          </div>

          <div class="swot-items">
            <div
              v-for="(item, itemIdx) in quadrant.items"
              :key="itemIdx"
              class="swot-item"
              :class="isRightColumn(cardIdx) ? 'swot-item-right' : ''"
              :style="{ borderColor: toneVars(quadrant.tone).border }"
            >
              <div
                class="swot-bullet"
                :style="{ background: toneVars(quadrant.tone).solid }"
              />
              <span leading-relaxed>{{ item }}</span>
            </div>
          </div>

          <div
            v-if="quadrant.summary && !summaryFirst(quadrant.key)"
            class="swot-summary"
            :class="isRightColumn(cardIdx) ? 'swot-summary-right' : ''"
            :style="{
              background: toneVars(quadrant.tone).bg,
              borderColor: toneVars(quadrant.tone).border,
              color: toneVars(quadrant.tone).text,
            }"
          >
            {{ quadrant.summary }}
          </div>
        </div>
      </GlassCard>
    </div>

    <SlideReveal v-if="strategy" :delay="600" class="glass-card px-5 py-3 flex items-center gap-3">
      <div i-carbon:direction-loop-right class="slide-icon-xl" text="var(--primary)" />
      <span class="slide-text-note" leading-relaxed>{{ strategy }}</span>
    </SlideReveal>
  </div>
</template>

<style scoped>
.swot-card {
  min-height: 0;
  position: relative;
  opacity: 0;
}

/* SWOT entrance animations: cards fly in from their respective corners */
.swot-entrance-0 { animation: swot-tr 0.5s cubic-bezier(0.16, 1, 0.3, 1) 0.1s forwards; }
.swot-entrance-1 { animation: swot-tl 0.5s cubic-bezier(0.16, 1, 0.3, 1) 0.2s forwards; }
.swot-entrance-2 { animation: swot-br 0.5s cubic-bezier(0.16, 1, 0.3, 1) 0.3s forwards; }
.swot-entrance-3 { animation: swot-bl 0.5s cubic-bezier(0.16, 1, 0.3, 1) 0.4s forwards; }

@keyframes swot-tr {
  from { opacity: 0; transform: translate(40px, -40px); }
  to   { opacity: 1; transform: translate(0, 0); }
}
@keyframes swot-tl {
  from { opacity: 0; transform: translate(-40px, -40px); }
  to   { opacity: 1; transform: translate(0, 0); }
}
@keyframes swot-br {
  from { opacity: 0; transform: translate(40px, 40px); }
  to   { opacity: 1; transform: translate(0, 0); }
}
@keyframes swot-bl {
  from { opacity: 0; transform: translate(-40px, 40px); }
  to   { opacity: 1; transform: translate(0, 0); }
}

.swot-body {
  padding: 1rem 1rem 0.95rem;
  display: flex;
  flex-direction: column;
  gap: 0.78rem;
  min-height: 0;
  height: 100%;
}

.swot-flag {
  position: absolute;
  z-index: 2;
  display: inline-flex;
  align-items: center;
  gap: 0.55rem;
  width: fit-content;
  max-width: calc(100% - 1.2rem);
  border: 1px solid;
  padding: 0.34rem 0.92rem 0.34rem 0.28rem;
  backdrop-filter: blur(10px);
  box-shadow: 0 10px 24px rgba(15, 23, 42, 0.18);
}

.swot-flag-icon {
  width: 1.7rem;
  height: 1.7rem;
  border-radius: 999px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.swot-flag-text {
  font-size: var(--type-body);
  font-weight: 700;
  line-height: 1.05;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.swot-items {
  display: flex;
  flex-direction: column;
  gap: 0.65rem;
  flex: 1;
}

.swot-item {
  display: flex;
  align-items: flex-start;
  gap: 0.65rem;
  padding-top: 0.08rem;
}

.swot-item-right {
  flex-direction: row-reverse;
  text-align: right;
}

.swot-bullet {
  width: 0.55rem;
  height: 0.55rem;
  border-radius: 999px;
  margin-top: 0.42rem;
  flex-shrink: 0;
}

.swot-summary {
  border: 1px solid;
  border-radius: 0.9rem;
  padding: 0.65rem 0.8rem;
  font-size: var(--type-caption);
  line-height: 1.45;
}

.swot-summary-right {
  text-align: right;
}

.swot-card-left .swot-items,
.swot-card-left .swot-summary {
  text-align: left;
}

.swot-card-right .swot-items,
.swot-card-right .swot-summary {
  text-align: right;
}

.swot-flag-tr {
  right: 0.28rem;
  bottom: 0.18rem;
  border-radius: 1rem 1rem 0.28rem 1rem;
}

.swot-flag-tl {
  left: 0.28rem;
  bottom: 0.18rem;
  border-radius: 1rem 1rem 1rem 0.28rem;
}

.swot-flag-br {
  right: 0.28rem;
  top: 0.18rem;
  border-radius: 1rem 0.28rem 1rem 1rem;
}

.swot-flag-bl {
  left: 0.28rem;
  top: 0.18rem;
  border-radius: 0.28rem 1rem 1rem 1rem;
}

</style>
