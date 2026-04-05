<script setup lang="ts">
import { nextTick, onMounted, onUnmounted, ref, watch } from 'vue'

const props = defineProps<{
  syntax: string
  fallbackClass?: string
}>()

const containerRef = ref<HTMLDivElement>()
let infographicInstance: any = null

async function initInfographic(syntax: string) {
  if (!containerRef.value || !syntax?.trim()) return

  try {
    const { Infographic } = await import('@antv/infographic')

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
    console.error('[InfographicCanvas] render failed:', err)
    if (containerRef.value) {
      const pre = document.createElement('pre')
      pre.textContent = syntax
      pre.className = props.fallbackClass ?? ''
      pre.style.whiteSpace = 'pre-wrap'
      pre.style.wordBreak = 'break-word'
      containerRef.value.innerHTML = ''
      containerRef.value.appendChild(pre)
    }
  }
}

onMounted(async () => {
  await nextTick()
  if (props.syntax) {
    initInfographic(props.syntax)
  }
})

onUnmounted(() => {
  if (infographicInstance) {
    infographicInstance.destroy()
    infographicInstance = null
  }
})

watch(() => props.syntax, (newSyntax) => {
  if (newSyntax) {
    initInfographic(newSyntax)
  }
})
</script>

<template>
  <div ref="containerRef" class="h-full w-full" />
</template>
