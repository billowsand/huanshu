<script setup lang="ts">
import { computed } from 'vue'
import { toneVars, type Tone } from './keynoteTheme'

const props = defineProps<{
  section?: string
  title: string
  subtitle?: string
  direction?: 'horizontal' | 'vertical'
  steps: Array<{
    title: string
    body?: string
    icon?: string
    tone?: Tone
  }>
  footer?: string
}>()

const isVertical = computed(() => props.direction === 'vertical')
const safeStepCount = computed(() => Math.max(props.steps.length, 1))
const trackLeft = computed(() => `${100 / (2 * safeStepCount.value)}%`)
const stepWidth = computed(() => `${100 / safeStepCount.value}%`)
</script>

<template>
  <div v-if="section" class="section-num">{{ section }}</div>

  <div relative z-10 py-6 flex flex-col h-full>
    <div class="slide-header">
      <h1 class="slide-title">{{ title }}</h1>
      <div class="slide-title-divider" />
    </div>
    <TagBadge v-if="subtitle" inline class="mt-2">{{ subtitle }}</TagBadge>

    <!-- ─── Horizontal Flow ─── -->
    <div v-if="!isVertical" flex-1 flex flex-col justify-center>
      <div relative flex justify-around items-start>
        <div
          absolute rounded-full
          :style="{
            top: '2.24rem',
            left: trackLeft,
            right: trackLeft,
            height: '2px',
            background: 'linear-gradient(to right, transparent, var(--glass-border) 20%, var(--glass-border) 80%, transparent)',
            opacity: 0.45,
          }"
        />
        <div
          v-for="(step, idx) in steps"
          :key="step.title"
          flex flex-col items-center
          style="position: relative; z-index: 2; padding: 0 0.5rem"
          :style="{ width: stepWidth }"
        >
          <div
            class="sf-hnode"
            :style="{
              background: toneVars(step.tone || 'blue').bg,
              borderColor: toneVars(step.tone || 'blue').border,
              boxShadow: `0 0 28px ${toneVars(step.tone || 'blue').solid}30, inset 0 1px 0 rgba(255,255,255,0.08)`,
            }"
          >
            <div
              :class="step.icon || 'i-carbon:arrow-right'"
              class="slide-icon-xl"
              :style="{ color: toneVars(step.tone || 'blue').text }"
            />
            <span
              class="sf-num"
              :style="{ background: toneVars(step.tone || 'blue').solid }"
            >{{ idx + 1 }}</span>
          </div>
          <GlassCard
            padding="px-3 py-2"
            class="sf-hcard mt-4"
            textCenter
            :style="{
              background: toneVars(step.tone || 'blue').bg,
              borderColor: toneVars(step.tone || 'blue').border + '70',
            }"
          >
            <div
              font-bold class="slide-text-caption" leading-snug
              :style="{ color: toneVars(step.tone || 'blue').text }"
            >{{ step.title }}</div>
            <div v-if="step.body" class="slide-text-caption" opacity-60 leading-relaxed mt-1>
              {{ step.body }}
            </div>
          </GlassCard>
        </div>
      </div>
    </div>

    <!-- ─── Vertical Flow ─── -->
    <div v-else flex-1 flex flex-col justify-center>
      <div flex flex-col>
        <div
          v-for="(step, idx) in steps"
          :key="step.title"
          flex gap="4"
        >
          <!-- Left: track column (node + line) -->
          <div flex flex-col items-center flex-shrink-0 w-10>
            <div
              class="sf-vnode"
              :style="{
                background: toneVars(step.tone || 'blue').bg,
                borderColor: toneVars(step.tone || 'blue').border,
                boxShadow: `0 0 16px ${toneVars(step.tone || 'blue').solid}28`,
              }"
            >
              <span
                class="slide-text-caption"
                font-bold font-mono
                :style="{ color: toneVars(step.tone || 'blue').text }"
              >{{ idx + 1 }}</span>
            </div>
            <!-- Connecting line to next step -->
            <div
              v-if="idx < steps.length - 1"
              flex-1 mt-1 mb-1
              style="width: 2px; min-height: 14px; border-radius: 2px"
              :style="{
                background: `linear-gradient(to bottom, ${toneVars(step.tone || 'blue').solid}45, ${toneVars(steps[idx + 1].tone || 'blue').solid}18)`,
              }"
            />
          </div>

          <!-- Right: content card -->
          <div flex-1 :class="idx < steps.length - 1 ? 'pb-4' : ''">
            <GlassCard
              padding="px-4 py-3"
              :style="{
                background: toneVars(step.tone || 'blue').bg,
                borderColor: toneVars(step.tone || 'blue').border + '80',
              }"
            >
              <div flex items-center gap-2 mb-1>
                <div
                  v-if="step.icon"
                  :class="step.icon"
                  class="slide-icon-md"
                  flex-shrink-0
                  :style="{ color: toneVars(step.tone || 'blue').text }"
                />
                <div
                  font-bold class="slide-text-meta"
                  :style="{ color: toneVars(step.tone || 'blue').text }"
                >{{ step.title }}</div>
              </div>
              <div v-if="step.body" class="slide-text-caption" opacity-70 leading-relaxed>
                {{ step.body }}
              </div>
            </GlassCard>
          </div>
        </div>
      </div>
    </div>

    <div v-if="footer" mt-4 text-center class="slide-text-caption" opacity-40>
      {{ footer }}
    </div>
  </div>
</template>

<style scoped>
/* Horizontal node */
.sf-hnode {
  position: relative;
  width: 4.5rem;
  height: 4.5rem;
  border-radius: 50%;
  border-width: 1px;
  border-style: solid;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

/* Step number badge (bottom-right of node) */
.sf-num {
  position: absolute;
  bottom: 2px;
  right: 2px;
  width: 1.2rem;
  height: 1.2rem;
  border-radius: 50%;
  font-size: var(--type-caption);
  font-weight: 800;
  font-family: var(--font-mono);
  display: flex;
  align-items: center;
  justify-content: center;
  color: rgba(0, 0, 0, 0.7);
}

.sf-hcard {
  width: 100%;
}

/* Vertical node */
.sf-vnode {
  width: 2.25rem;
  height: 2.25rem;
  border-radius: 50%;
  border-width: 1px;
  border-style: solid;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}
</style>
