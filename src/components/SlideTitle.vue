<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { getTitleTokens } from '../composables/useTitleTokens'
import { renderRichText } from '../utils/richText'

const props = withDefaults(defineProps<{
  title: string
  as?: 'h1' | 'h2' | 'span'
}>(), { as: 'h1' })

// Math titles skip tokenization: the formula must stay one atomic unit.
const hasMath = computed(() => props.title.includes('$'))

const tokens = ref<string[]>([])
watch(() => props.title, async (title, _previous, onCleanup) => {
  let cancelled = false
  onCleanup(() => { cancelled = true })
  tokens.value = []
  if (hasMath.value) return
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
    <span v-else-if="hasMath" v-html="renderRichText(title)"></span>
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
