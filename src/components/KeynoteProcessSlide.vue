<script setup lang="ts">
import { toneVars, type Tone } from './keynoteTheme'

defineProps<{
  section?: string
  title: string
  subtitle?: string
  summary?: string
  phases: Array<{
    phase: string
    title: string
    icon?: string
    tone?: Tone
    subtitle?: string
    steps: Array<{
      label: string
      desc?: string
    }>
    highlight?: string
  }>
  footer?: string
}>()
</script>

<template>
  <div v-if="section" class="section-num">{{ section }}</div>

  <div class="process-slide" relative z-10 flex flex-col h-full>
    <div class="slide-header process-header">
      <h1 class="slide-title slide-compact-title">{{ title }}</h1>
      <div class="slide-title-divider" />
    </div>
    <div class="process-topbar">
      <TagBadge v-if="subtitle" inline class="process-badge">{{ subtitle }}</TagBadge>
      <div v-if="summary" class="process-summary glass-card">
        <div class="process-summary__line" />
        <span>{{ summary }}</span>
      </div>
    </div>

    <div
      class="phases-grid process-board"
      grid flex-1
      :style="{ gridTemplateColumns: `repeat(${phases.length}, minmax(0, 1fr))` }"
    >
      <div
        v-for="(phase, phaseIdx) in phases"
        :key="phase.phase"
        class="phase-col phase-col-animated"
        flex flex-col
        :style="{ animationDelay: `${phaseIdx * 80 + 100}ms` }"
      >
        <GlassCard
          padding="px-4 py-3"
          class="phase-head"
          :style="{
            background: toneVars(phase.tone).bg,
            borderColor: toneVars(phase.tone).border,
          }"
        >
          <div class="phase-head__meta">
            <div
              class="phase-chip"
              :style="{
                background: `linear-gradient(180deg, color-mix(in srgb, ${toneVars(phase.tone).solid} 86%, var(--bg)), color-mix(in srgb, ${toneVars(phase.tone).solid} 62%, var(--bg)))`,
                borderColor: toneVars(phase.tone).border,
                color: toneVars(phase.tone).contrast,
              }"
            >{{ phase.phase }}</div>
            <div
              v-if="phase.icon"
              :class="phase.icon"
              class="phase-head__icon"
              :style="{ color: toneVars(phase.tone).text }"
            />
          </div>
          <div
            class="phase-head__title"
            :style="{ color: toneVars(phase.tone).text }"
          >{{ phase.title }}</div>
          <div v-if="phase.subtitle" class="phase-head__subtitle">{{ phase.subtitle }}</div>
        </GlassCard>

        <div class="phase-steps-wrap glass-card" flex-1 flex flex-col>
          <div class="phase-steps-wrap__glow" :style="{ background: toneVars(phase.tone).solid }" />
          <div class="phase-steps-wrap__title">
            <span>关键动作</span>
            <span>{{ phase.steps.length.toString().padStart(2, '0') }}</span>
          </div>
          <div class="phase-steps" flex-1 flex flex-col>
            <div
              v-for="(step, sIdx) in phase.steps"
              :key="step.label"
              class="step-row"
            >
              <div
                class="step-dot"
                flex-shrink-0
                :style="{
                  background: `linear-gradient(180deg, color-mix(in srgb, ${toneVars(phase.tone).solid} 84%, var(--bg)), color-mix(in srgb, ${toneVars(phase.tone).solid} 58%, var(--bg)))`,
                  borderColor: toneVars(phase.tone).border,
                  color: toneVars(phase.tone).contrast,
                }"
              >{{ sIdx + 1 }}</div>
              <div class="step-copy" flex-1>
                <div class="step-label">{{ step.label }}</div>
                <div v-if="step.desc" class="step-desc">
                  {{ step.desc }}
                </div>
              </div>

              <div
                v-if="sIdx < phase.steps.length - 1"
                class="step-conn"
                :style="{ background: toneVars(phase.tone).solid }"
              />
            </div>
          </div>
        </div>

        <GlassCard
          v-if="phase.highlight"
          padding="px-4 py-3"
          class="phase-highlight"
          :style="{
            background: `linear-gradient(180deg, ${toneVars(phase.tone).bg}, var(--glass))`,
            borderColor: toneVars(phase.tone).border,
            borderTopWidth: '2px',
            borderTopColor: toneVars(phase.tone).solid,
          }"
        >
          <div class="phase-highlight__content">
            <div
              i-carbon:checkmark-filled
              class="phase-highlight__icon"
              :style="{ color: toneVars(phase.tone).text }"
            />
            <span class="phase-highlight__text" :style="{ color: toneVars(phase.tone).text }">
              {{ phase.highlight }}
            </span>
          </div>
        </GlassCard>
      </div>
    </div>

    <div v-if="footer" class="process-footer">
      {{ footer }}
    </div>
  </div>
</template>

<style scoped>
.process-slide {
  padding-top: 0.25rem;
  gap: 1rem;
}

.process-header {
  margin-bottom: 0;
}

.process-topbar {
  display: flex;
  align-items: center;
  gap: 0.875rem;
}

.process-badge {
  flex-shrink: 0;
}

.process-summary {
  position: relative;
  display: flex;
  align-items: center;
  gap: 0.75rem;
  min-height: 2.9rem;
  padding: 0.75rem 1rem;
  overflow: hidden;
  color: var(--text-muted);
  font-size: var(--type-body-tight);
  line-height: 1.45;
}

.process-summary__line {
  width: 42px;
  height: 2px;
  flex-shrink: 0;
  border-radius: 999px;
  background: linear-gradient(90deg, var(--primary), transparent);
}

.process-board {
  position: relative;
  gap: 1rem;
  min-height: 0;
  padding-top: 0.25rem;
}

.phase-col {
  min-width: 0;
  gap: 0.75rem;
}

.phase-col-animated {
  opacity: 0;
  animation: phase-col-in 0.5s cubic-bezier(0.16, 1, 0.3, 1) forwards;
  will-change: transform, opacity;
}

@keyframes phase-col-in {
  from {
    opacity: 0;
    transform: translateY(20px) scale(0.97);
  }
  to {
    opacity: 1;
    transform: translateY(0) scale(1);
  }
}

.phase-head {
  position: relative;
  min-height: 106px;
  overflow: hidden;
}

.phase-head::after {
  content: '';
  position: absolute;
  inset: auto -16px -22px auto;
  width: 120px;
  height: 120px;
  border-radius: 50%;
  background: radial-gradient(circle, rgba(255,255,255,0.16), transparent 64%);
  opacity: 0.45;
}

.phase-head__meta {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
}

.phase-chip {
  display: inline-flex;
  align-items: center;
  border-radius: 999px;
  padding: 0.32rem 0.7rem;
  border: 1px solid transparent;
  font-size: var(--type-meta);
  font-weight: 800;
  letter-spacing: 0.01em;
  box-shadow:
    inset 0 1px 0 rgba(255,255,255,0.16),
    0 10px 20px rgba(0,0,0,0.18);
  text-shadow: 0 1px 2px rgba(0,0,0,0.28);
}

.phase-head__icon {
  font-size: var(--type-body);
  opacity: 0.85;
}

.phase-head__title {
  margin-top: 0.7rem;
  font-size: var(--type-body-strong);
  font-weight: 700;
  line-height: 1.15;
}

.phase-head__subtitle {
  margin-top: 0.4rem;
  max-width: 80%;
  color: var(--text-muted);
  font-size: var(--type-caption);
  line-height: 1.45;
}

.phase-steps-wrap {
  position: relative;
  min-height: 0;
  padding: 0.95rem 0.95rem 0.85rem;
  overflow: hidden;
}

.phase-steps-wrap__glow {
  position: absolute;
  inset: 14px auto auto -10px;
  width: 56px;
  height: 56px;
  border-radius: 50%;
  opacity: 0.14;
  filter: blur(18px);
}

.phase-steps-wrap__title {
  position: relative;
  z-index: 1;
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 0.65rem;
  color: var(--text-muted);
  font-size: var(--type-caption);
  font-weight: 700;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.phase-steps {
  position: relative;
  z-index: 1;
  gap: 0.1rem;
}

.step-row {
  position: relative;
  display: flex;
  align-items: flex-start;
  gap: 0.7rem;
  padding: 0.5rem 0;
}

.step-dot {
  width: 1.5rem;
  height: 1.5rem;
  border-radius: 999px;
  border: 1px solid transparent;
  font-size: var(--type-caption);
  font-weight: 800;
  font-family: var(--font-mono);
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  margin-top: 0.05rem;
  box-shadow:
    inset 0 1px 0 rgba(255,255,255,0.16),
    0 8px 18px rgba(0,0,0,0.16);
  text-shadow: 0 1px 2px rgba(0,0,0,0.26);
}

.step-copy {
  min-width: 0;
}

.step-label {
  color: var(--text);
  font-size: var(--type-body-tight);
  font-weight: 600;
  line-height: 1.35;
}

.step-desc {
  margin-top: 0.22rem;
  color: var(--text-muted);
  font-size: var(--type-caption);
  line-height: 1.5;
}

.step-conn {
  position: absolute;
  left: 0.72rem;
  top: 1.92rem;
  width: 2px;
  height: calc(100% - 1.5rem);
  border-radius: 1px;
  opacity: 0.26;
}

.phase-highlight {
  min-height: 58px;
}

.phase-highlight__content {
  display: flex;
  align-items: center;
  gap: 0.65rem;
}

.phase-highlight__icon {
  font-size: var(--type-body);
  flex-shrink: 0;
}

.phase-highlight__text {
  font-size: var(--type-body-tight);
  font-weight: 600;
  line-height: 1.4;
}

.process-footer {
  padding-top: 0.15rem;
  text-align: center;
  color: var(--text-muted);
  font-size: var(--type-body-tight);
  opacity: 0.78;
}

@media (max-width: 1280px) {
  .phase-head__subtitle {
    max-width: 100%;
  }
}
</style>
