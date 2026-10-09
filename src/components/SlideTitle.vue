<script setup lang="ts">
import { ref, watch } from 'vue'
import { getTitleTokens } from '../composables/useTitleTokens'

const props = withDefaults(defineProps<{
  title: string
  as?: 'h1' | 'h2' | 'span'
}>(), { as: 'h1' })

const tokens = ref<string[]>([])
watch(() => props.title, async (title, _previous, onCleanup) => {
  let cancelled = false
  onCleanup(() => { cancelled = true })
  tokens.value = []
  try {
    const result = await getTitleTokens(title)
    if (!cancelled) tokens.value = result
  } catch (error) {
    console.warn('标题分词失败', error)
  }
}, { immediate: true })
</script>

<template>
  <component :is="as" class="semantic-title">
    <template v-if="tokens.length">
      <template v-for="(token, index) in tokens" :key="index"><span class="semantic-title__word">{{ token }}</span><wbr v-if="index < tokens.length - 1" /></template>
    </template>
    <template v-else>{{ title }}</template>
    <slot />
  </component>
</template>

<style scoped>
.semantic-title {
  min-width: 0;
  max-width: 100%;
  white-space: pre-wrap;
  overflow-wrap: normal;
  word-break: normal;
  text-wrap: balance;
}

.semantic-title__word {
  white-space: pre;
}
</style>
