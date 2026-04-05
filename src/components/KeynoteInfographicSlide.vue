<script setup lang="ts">
import SlideReveal from './SlideReveal.vue'
import InfographicCanvas from './InfographicCanvas.vue'

const props = defineProps<{
  section?: string
  title: string
  subtitle?: string
  infographicSyntax: string
  footer?: string
}>()
</script>

<template>
  <div relative z-10 flex flex-col h-full>
    <!-- Header -->
    <SlideReveal :delay="80" flex items-center gap-3 mb-4>
      <span v-if="section" class="section-tag">{{ section }}</span>
      <span class="infographic-kicker">INFOGRAPHIC</span>
    </SlideReveal>

    <SlideReveal :delay="140" mb-3>
      <h2 class="slide-title">{{ title }}</h2>
      <div v-if="subtitle" class="slide-subtitle">{{ subtitle }}</div>
    </SlideReveal>

    <!-- Infographic container -->
    <SlideReveal :delay="220" flex-1 min-h-0>
      <div class="infographic-container">
        <InfographicCanvas
          :syntax="infographicSyntax"
          fallback-class="infographic-fallback"
        />
      </div>
    </SlideReveal>

    <!-- Footer -->
    <SlideReveal v-if="footer" :delay="320" mt-3>
      <div class="slide-footer">{{ footer }}</div>
    </SlideReveal>
  </div>
</template>

<style scoped>
.section-tag {
  font-size: 0.68rem;
  font-weight: 600;
  letter-spacing: 0.12em;
  text-transform: uppercase;
  color: var(--warning);
  background: var(--warning-bg);
  border: 1px solid var(--warning-border);
  border-radius: 6px;
  padding: 2px 10px;
}

.infographic-kicker {
  font-size: 0.68rem;
  letter-spacing: 0.32em;
  text-transform: uppercase;
  color: rgba(148, 163, 184, 0.6);
}

.slide-title {
  font-size: 1.42rem;
  font-weight: 700;
  color: var(--text);
  letter-spacing: -0.02em;
  line-height: 1.3;
}

.slide-subtitle {
  margin-top: 0.4rem;
  font-size: 0.92rem;
  line-height: 1.6;
  color: rgba(226, 232, 240, 0.72);
}

.infographic-container {
  width: 100%;
  height: 100%;
  border-radius: 12px;
  overflow: hidden;
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid rgba(148, 163, 184, 0.12);
}

:deep(.infographic-fallback) {
  color: #94a3b8;
  font-size: 0.75rem;
  padding: 16px;
}

.slide-footer {
  font-size: 0.82rem;
  line-height: 1.6;
  color: rgba(148, 163, 184, 0.72);
}
</style>
