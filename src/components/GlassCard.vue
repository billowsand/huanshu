<script setup lang="ts">
defineProps<{
  padding?: string
  flex?: boolean
  items?: 'start' | 'center' | 'end'
  justify?: 'start' | 'center' | 'end' | 'between'
  gap?: string | number
  overflow?: boolean
  textCenter?: boolean
  h?: string
  noPadding?: boolean
}>()
</script>

<template>
  <div
    class="glass-card"
    :class="[
      noPadding ? '' : (padding || 'px-4 py-3'),
      flex ? 'flex' : '',
      flex && items ? `items-${items}` : '',
      flex && justify ? `justify-${justify}` : '',
      gap ? `gap-${gap}` : '',
      overflow ? 'overflow-hidden' : '',
      textCenter ? 'text-center' : '',
      h || ''
    ]"
    :style="{
      background: 'var(--glass)',
      borderColor: 'var(--glass-border)'
    }"
  >
    <slot />
  </div>
</template>
