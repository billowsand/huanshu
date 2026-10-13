<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { getTitleTokens } from '../composables/useTitleTokens'
import { renderRichText } from '../utils/richText'
import { titleTailOffset } from '../utils/titleTail'

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

// Head / tail word groups for the editorial two-tone headline (see titleTail.ts).
// A token that straddles the cut is split in two so the tail always starts
// exactly after the punctuation, even when segmentation is unavailable.
const groups = computed(() => {
  const words = tokens.value.length ? tokens.value : [props.title]
  const cut = titleTailOffset(props.title)
  if (cut === null) return { head: words, tail: [] as string[] }
  const head: string[] = []
  const tail: string[] = []
  let offset = 0
  for (const word of words) {
    const end = offset + word.length
    if (end <= cut) head.push(word)
    else if (offset >= cut) tail.push(word)
    else {
      head.push(word.slice(0, cut - offset))
      tail.push(word.slice(cut - offset))
    }
    offset = end
  }
  return { head, tail }
})
</script>

<template>
  <component :is="as" class="semantic-title">
    <span v-if="hasMath" v-html="renderRichText(title)"></span>
    <template v-else>
      <template v-if="tokens.length">
        <template v-for="(token, index) in groups.head" :key="`h${index}`"><span class="semantic-title__word">{{ token }}</span><wbr v-if="index < groups.head.length - 1 || groups.tail.length" /></template>
        <span v-if="groups.tail.length" class="semantic-title__tail"><template v-for="(token, index) in groups.tail" :key="`t${index}`"><span class="semantic-title__word">{{ token }}</span><wbr v-if="index < groups.tail.length - 1" /></template></span>
      </template>
      <!-- Unsegmented (loading, or outside Tauri): plain text keeps normal wrapping. -->
      <template v-else>{{ groups.head.join('') }}<span v-if="groups.tail.length" class="semantic-title__tail">{{ groups.tail.join('') }}</span></template>
    </template>
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
