<script setup lang="ts">
import { toneVars, type Tone } from './keynoteTheme'

defineProps<{
  section?: string
  title: string
  subtitle?: string
  mode?: 'side-by-side' | 'pros-cons' | 'versus'
  left: {
    title: string
    tone?: Tone
    icon?: string
    items: Array<{
      label: string
      desc?: string
      highlight?: boolean
    }>
    conclusion?: string
  }
  right: {
    title: string
    tone?: Tone
    icon?: string
    items: Array<{
      label: string
      desc?: string
      highlight?: boolean
    }>
    conclusion?: string
  }
  footer?: string
}>()
</script>

<template>
  <div v-if="section" class="section-num">{{ section }}</div>

  <div relative z-10 py-6 flex flex-col h-full>
    <div class="slide-header">
      <h1 class="slide-title">{{ title }}</h1>
      <div class="slide-title-divider" />
    </div>
    <TagBadge v-if="subtitle" inline class="mt-2">{{ subtitle }}</TagBadge>

    <div class="compare-layout" flex-1 flex gap="4" items-stretch min-h-0 mt-4>

      <!-- ── Left Panel ── -->
      <div class="cp-panel glass-card" flex-1 flex flex-col overflow-hidden>
        <!-- Panel header -->
        <div
          class="cp-header"
          :style="{
            background: toneVars(left.tone || 'blue').bg,
            borderBottomColor: toneVars(left.tone || 'blue').border,
          }"
        >
          <div flex items-center gap-2>
            <div
              v-if="left.icon"
              :class="left.icon"
              class="slide-icon-lg"
              flex-shrink-0
              :style="{ color: toneVars(left.tone || 'blue').text }"
            />
            <!-- pros-cons: show checkmark badge -->
            <div
              v-if="mode === 'pros-cons'"
              class="cp-mode-badge"
              :style="{
                background: toneVars('green').solid + '20',
                borderColor: toneVars('green').border,
                color: toneVars('green').text,
              }"
            >
              <div i-carbon:checkmark-filled class="slide-icon-xs" />
              <span>优势</span>
            </div>
            <span
              font-bold class="slide-text-meta"
              :style="{ color: toneVars(left.tone || 'blue').text }"
            >{{ left.title }}</span>
          </div>
        </div>

        <!-- Panel items -->
        <div class="cp-body" flex-1 flex flex-col gap-2 overflow-y-auto>
          <div
            v-for="(item, idx) in left.items"
            :key="idx"
            class="cp-item"
            :style="item.highlight ? {
              background: toneVars(left.tone || 'blue').bg,
              borderColor: toneVars(left.tone || 'blue').border,
              borderLeftColor: toneVars(left.tone || 'blue').solid,
              borderLeftWidth: '2px',
            } : {}"
          >
            <div flex items-start gap-2>
              <div
                :class="item.highlight ? 'i-carbon:star-filled' : 'i-carbon:checkmark-filled'"
                class="slide-icon-sm"
                flex-shrink-0 mt-0.5
                :style="{ color: toneVars(left.tone || 'blue').text }"
              />
              <div flex-1>
                <div class="slide-text-meta" font-medium leading-snug>{{ item.label }}</div>
                <div v-if="item.desc" class="slide-text-caption" opacity-60 mt-0.5 leading-relaxed>
                  {{ item.desc }}
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- Conclusion footer -->
        <div
          v-if="left.conclusion"
          class="cp-footer"
          :style="{
            borderTopColor: toneVars(left.tone || 'blue').border,
            background: toneVars(left.tone || 'blue').bg,
          }"
        >
          <div i-carbon:idea flex-shrink-0 :style="{ color: toneVars(left.tone || 'blue').text }" />
          <span class="slide-text-caption" :style="{ color: toneVars(left.tone || 'blue').text }">
            {{ left.conclusion }}
          </span>
        </div>
      </div>

      <!-- ── Center Divider ── -->

      <!-- versus mode: VS badge -->
      <div
        v-if="mode === 'versus'"
        class="cp-divider"
        flex flex-col items-center justify-center gap-2 flex-shrink-0
      >
        <div class="cp-divider-line" />
        <div class="glass-card cp-vs-badge">
          <span class="slide-text-meta" font-bold letter-spacing-wider text="var(--primary)">VS</span>
        </div>
        <div class="cp-divider-line" />
      </div>

      <!-- pros-cons mode: balance indicator -->
      <div
        v-else-if="mode === 'pros-cons'"
        class="cp-divider"
        flex flex-col items-center justify-center gap-2 flex-shrink-0
      >
        <div class="cp-divider-line" />
        <div class="glass-card cp-balance-badge" flex flex-col items-center gap-1>
          <div i-carbon:checkmark class="slide-icon-sm" text="var(--success)" />
          <div style="width: 1px; height: 8px; background: var(--glass-border)" />
          <div i-carbon:close class="slide-icon-sm" text="var(--danger)" />
        </div>
        <div class="cp-divider-line" />
      </div>

      <!-- side-by-side mode: simple line -->
      <div
        v-else
        class="cp-simple-divider flex-shrink-0"
      />

      <!-- ── Right Panel ── -->
      <div class="cp-panel glass-card" flex-1 flex flex-col overflow-hidden>
        <!-- Panel header -->
        <div
          class="cp-header"
          :style="{
            background: toneVars(right.tone || 'amber').bg,
            borderBottomColor: toneVars(right.tone || 'amber').border,
          }"
        >
          <div flex items-center gap-2>
            <div
              v-if="right.icon"
              :class="right.icon"
              class="slide-icon-lg"
              flex-shrink-0
              :style="{ color: toneVars(right.tone || 'amber').text }"
            />
            <!-- pros-cons: show cross badge -->
            <div
              v-if="mode === 'pros-cons'"
              class="cp-mode-badge"
              :style="{
                background: toneVars('red').solid + '20',
                borderColor: toneVars('red').border,
                color: toneVars('red').text,
              }"
            >
              <div i-carbon:close-filled class="slide-icon-xs" />
              <span>劣势</span>
            </div>
            <span
              font-bold class="slide-text-meta"
              :style="{ color: toneVars(right.tone || 'amber').text }"
            >{{ right.title }}</span>
          </div>
        </div>

        <!-- Panel items -->
        <div class="cp-body" flex-1 flex flex-col gap-2 overflow-y-auto>
          <div
            v-for="(item, idx) in right.items"
            :key="idx"
            class="cp-item"
            :style="item.highlight ? {
              background: toneVars(right.tone || 'amber').bg,
              borderColor: toneVars(right.tone || 'amber').border,
              borderLeftColor: toneVars(right.tone || 'amber').solid,
              borderLeftWidth: '2px',
            } : {}"
          >
            <div flex items-start gap-2>
              <div
                :class="item.highlight
                  ? 'i-carbon:star-filled'
                  : mode === 'pros-cons'
                    ? 'i-carbon:close'
                    : 'i-carbon:checkmark-filled'"
                class="slide-icon-sm"
                flex-shrink-0 mt-0.5
                :style="{ color: toneVars(right.tone || 'amber').text }"
              />
              <div flex-1>
                <div class="slide-text-meta" font-medium leading-snug>{{ item.label }}</div>
                <div v-if="item.desc" class="slide-text-caption" opacity-60 mt-0.5 leading-relaxed>
                  {{ item.desc }}
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- Conclusion footer -->
        <div
          v-if="right.conclusion"
          class="cp-footer"
          :style="{
            borderTopColor: toneVars(right.tone || 'amber').border,
            background: toneVars(right.tone || 'amber').bg,
          }"
        >
          <div i-carbon:idea flex-shrink-0 :style="{ color: toneVars(right.tone || 'amber').text }" />
          <span class="slide-text-caption" :style="{ color: toneVars(right.tone || 'amber').text }">
            {{ right.conclusion }}
          </span>
        </div>
      </div>
    </div>

    <div v-if="footer" mt-4 text-center class="slide-text-caption" opacity-40>
      {{ footer }}
    </div>
  </div>
</template>

<style scoped>
/* Panel card */
.cp-panel {
  border-radius: 16px;
  border-width: 1px;
  border-style: solid;
  min-width: 0;
}

/* Panel header */
.cp-header {
  padding: 0.625rem 1rem;
  border-radius: 16px 16px 0 0;
  border-bottom-width: 1px;
  border-bottom-style: solid;
  flex-shrink: 0;
}

/* Mode badge (pros/cons label) */
.cp-mode-badge {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  padding: 1px 8px 1px 5px;
  border-radius: 999px;
  border-width: 1px;
  border-style: solid;
  font-size: var(--type-caption);
  font-weight: 700;
  letter-spacing: 0.03em;
}

/* Panel body (scrollable items) */
.cp-body {
  padding: 0.75rem;
  overflow-y: auto;
}

/* Item row */
.cp-item {
  padding: 0.5rem 0.625rem;
  border-radius: 8px;
  border-width: 1px;
  border-style: solid;
  border-color: transparent;
  transition: background 0.15s;
}

/* Conclusion footer */
.cp-footer {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.5rem 0.875rem;
  border-top-width: 1px;
  border-top-style: solid;
  border-radius: 0 0 16px 16px;
  flex-shrink: 0;
  font-size: var(--type-meta);
}

/* Versus / balance center divider */
.cp-divider {
  width: 3rem;
}

.cp-divider-line {
  width: 2px;
  flex: 1;
  border-radius: 2px;
  background: linear-gradient(to bottom, transparent, var(--glass-border), transparent);
}

.cp-vs-badge,
.cp-balance-badge {
  border-radius: 999px;
  padding: 0.375rem 0.625rem;
}

/* Side-by-side thin separator */
.cp-simple-divider {
  width: 2px;
  align-self: stretch;
  border-radius: 2px;
  background: linear-gradient(to bottom, transparent, var(--glass-border) 20%, var(--glass-border) 80%, transparent);
  opacity: 0.6;
}
</style>
