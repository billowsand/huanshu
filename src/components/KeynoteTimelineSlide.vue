<script setup lang="ts">
import { toneVars, type Tone } from './keynoteTheme'

defineProps<{
  section?: string
  title: string
  subtitle?: string
  events: Array<{
    date: string
    title: string
    body: string
    tone?: Tone
    icon?: string
  }>
  footer?: string
}>()
</script>

<template>
  <div v-if="section" class="section-num">{{ section }}</div>

  <div relative z-10 py-6 flex flex-col h-full>
    <div class="slide-header">
      <h1 class="slide-title slide-compact-title">{{ title }}</h1>
      <div class="slide-title-divider" />
    </div>
    <TagBadge v-if="subtitle" inline class="mt-2">{{ subtitle }}</TagBadge>

    <div class="tl-wrap" flex-1 flex flex-col justify-center>
      <div class="tl-axis" relative>

        <!-- Axis line -->
        <div
          class="tl-axis-line"
          absolute left-0 right-0
          style="top: 50%; transform: translateY(-50%)"
        />

        <!-- Events row -->
        <div class="tl-events" w-full flex>
            <div
              v-for="(evt, idx) in events"
              :key="evt.date"
              class="tl-col tl-item-animated"
              flex flex-col items-center
              :style="{
                width: `${100 / events.length}%`,
                animationDelay: `${idx * 100 + 150}ms`
              }"
            >

              <!-- ── Even: card above axis ── -->
              <template v-if="idx % 2 === 0">
                <!-- Card above -->
                <div class="tl-above" flex-1 flex flex-col justify-end pb-3 w-full px-2>
                  <GlassCard
                    padding="px-3 py-2"
                    class="tl-card"
                    :style="{
                      borderTopWidth: '2px',
                      borderTopColor: toneVars(evt.tone).solid,
                      borderColor: toneVars(evt.tone).border,
                      background: toneVars(evt.tone).bg,
                    }"
                  >
                    <div
                      v-if="evt.icon"
                      :class="evt.icon"
                      class="slide-icon-md"
                      mb-1
                      :style="{ color: toneVars(evt.tone).text }"
                    />
                    <div
                      font-bold class="slide-text-caption" leading-snug mb-1
                      :style="{ color: toneVars(evt.tone).text }"
                    >{{ evt.title }}</div>
                    <div class="slide-text-caption" opacity-60 leading-snug>{{ evt.body }}</div>
                  </GlassCard>
                </div>
                <!-- Stem: card → node -->
                <div
                  class="tl-stem"
                  :style="{ background: toneVars(evt.tone).solid + '70' }"
                />
                <!-- Node ring -->
                <div
                  class="tl-node"
                  :style="{
                    background: toneVars(evt.tone).solid,
                    boxShadow: `0 0 12px ${toneVars(evt.tone).solid}90, 0 0 24px ${toneVars(evt.tone).solid}40`,
                    borderColor: toneVars(evt.tone).border,
                  }"
                />
                <!-- Stem: node → date -->
                <div
                  class="tl-stem"
                  :style="{ background: toneVars(evt.tone).solid + '30' }"
                />
                <!-- Date label -->
                <div class="tl-date pt-1">{{ evt.date }}</div>
                <!-- Bottom spacer -->
                <div flex-1 />
              </template>

              <!-- ── Odd: card below axis ── -->
              <template v-else>
                <!-- Top spacer -->
                <div flex-1 />
                <!-- Date label -->
                <div class="tl-date pb-1">{{ evt.date }}</div>
                <!-- Stem: date → node -->
                <div
                  class="tl-stem"
                  :style="{ background: toneVars(evt.tone).solid + '30' }"
                />
                <!-- Node ring -->
                <div
                  class="tl-node"
                  :style="{
                    background: toneVars(evt.tone).solid,
                    boxShadow: `0 0 12px ${toneVars(evt.tone).solid}90, 0 0 24px ${toneVars(evt.tone).solid}40`,
                    borderColor: toneVars(evt.tone).border,
                  }"
                />
                <!-- Stem: node → card -->
                <div
                  class="tl-stem"
                  :style="{ background: toneVars(evt.tone).solid + '70' }"
                />
                <!-- Card below -->
                <div class="tl-below" flex-1 flex flex-col justify-start pt-3 w-full px-2>
                  <GlassCard
                    padding="px-3 py-2"
                    class="tl-card"
                    :style="{
                      borderBottomWidth: '2px',
                      borderBottomColor: toneVars(evt.tone).solid,
                      borderColor: toneVars(evt.tone).border,
                      background: toneVars(evt.tone).bg,
                    }"
                  >
                    <div
                      v-if="evt.icon"
                      :class="evt.icon"
                      class="slide-icon-md"
                      mb-1
                      :style="{ color: toneVars(evt.tone).text }"
                    />
                    <div
                      font-bold class="slide-text-caption" leading-snug mb-1
                      :style="{ color: toneVars(evt.tone).text }"
                    >{{ evt.title }}</div>
                    <div class="slide-text-caption" opacity-60 leading-snug>{{ evt.body }}</div>
                  </GlassCard>
                </div>
              </template>

            </div>
        </div>
      </div>
    </div>

    <div v-if="footer" mt-3 class="slide-text-caption" opacity-40 text-center v-html="footer" />
  </div>
</template>

<style scoped>
.tl-axis {
  min-height: 320px;
}

.tl-axis-line {
  height: 2px;
  border-radius: 2px;
  background: linear-gradient(
    to right,
    transparent,
    var(--glass-border) 15%,
    var(--glass-border) 85%,
    transparent
  );
  opacity: 0.5;
  transform-origin: left center;
  animation: axis-draw 0.7s cubic-bezier(0.4, 0, 0.2, 1) forwards;
}

@keyframes axis-draw {
  from { transform: scaleX(0); opacity: 0; }
  to   { transform: scaleX(1); opacity: 0.5; }
}

.tl-events {
  min-height: 320px;
  align-items: stretch;
  width: 100%;
}

.tl-col {
  position: relative;
  min-height: 320px;
}

/* Node dot with outer ring */
.tl-node {
  width: 1rem;
  height: 1rem;
  border-radius: 50%;
  border-width: 2px;
  border-style: solid;
  position: relative;
  z-index: 2;
  flex-shrink: 0;
}

/* Connector stems */
.tl-stem {
  width: 2px;
  height: 1.25rem;
  flex-shrink: 0;
  border-radius: 2px;
}

/* Date label */
.tl-date {
  font-size: var(--type-caption);
  font-family: var(--font-mono);
  opacity: 0.5;
  text-align: center;
  white-space: nowrap;
  letter-spacing: 0.04em;
}

/* Card constraints */
.tl-card {
  border-radius: 10px;
  border-width: 1px;
  border-style: solid;
}

/* Timeline item entrance */
.tl-item-animated {
  opacity: 0;
  animation: tl-pop 0.5s cubic-bezier(0.16, 1, 0.3, 1) forwards;
}

@keyframes tl-pop {
  from {
    opacity: 0;
    transform: scale(0.85) translateY(8px);
  }
  to {
    opacity: 1;
    transform: scale(1) translateY(0);
  }
}
</style>
