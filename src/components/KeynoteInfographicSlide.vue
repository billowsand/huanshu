<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch, nextTick } from 'vue'
import SlideReveal from './SlideReveal.vue'

const props = defineProps<{
  section?: string
  title: string
  subtitle?: string
  infographicSyntax: string
  footer?: string
}>()

const containerRef = ref<HTMLDivElement>()
let infographicInstance: any = null

async function initInfographic(syntax: string) {
  if (!containerRef.value || !syntax?.trim()) return

  try {
    const { Infographic } = await import('@antv/infographic')

    // Destroy previous instance if exists
    if (infographicInstance) {
      infographicInstance.destroy()
      infographicInstance = null
    }

    infographicInstance = new Infographic({
      container: containerRef.value,
      width: containerRef.value.clientWidth || 800,
      height: containerRef.value.clientHeight || 450,
    })

    infographicInstance.render(syntax)
  }
  catch (err) {
    console.error('[InfographicSlide] render failed:', err)
    // Fallback: show syntax as text (safe DOM manipulation to prevent XSS)
    if (containerRef.value) {
      const pre = document.createElement('pre')
      pre.textContent = syntax
      pre.style.cssText = 'color:#94a3b8;font-size:0.75rem;padding:16px;white-space:pre-wrap;word-break:break-all;'
      containerRef.value.innerHTML = ''
      containerRef.value.appendChild(pre)
    }
  }
}

onMounted(async () => {
  await nextTick()
  if (props.infographicSyntax) {
    initInfographic(props.infographicSyntax)
  }
})

onUnmounted(() => {
  if (infographicInstance) {
    infographicInstance.destroy()
    infographicInstance = null
  }
})

// Re-render when syntax changes
watch(() => props.infographicSyntax, (newSyntax) => {
  if (newSyntax) {
    initInfographic(newSyntax)
  }
})
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
      <div ref="containerRef" class="infographic-container" />
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

.slide-footer {
  font-size: 0.82rem;
  line-height: 1.6;
  color: rgba(148, 163, 184, 0.72);
}
</style>
