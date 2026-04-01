<script setup lang="ts">
defineProps<{
  num: string | number
  color?: 'amber' | 'blue' | 'green'
  size?: string
  filled?: boolean
}>()

const colorMap: Record<string, { bg: string; border: string; text: string }> = {
  amber: { bg: 'var(--warning-bg)', border: 'var(--warning-border)', text: 'var(--warning-light)' },
  blue: { bg: 'var(--info-bg)', border: 'var(--info-border)', text: 'var(--info-light)' },
  green: { bg: 'var(--success-bg)', border: 'var(--success-border)', text: 'var(--success-light)' },
}
</script>

<template>
  <div
    rounded-full
    :class="size || 'w-10 h-10'"
    flex
    items-center
    justify-center
    font-mono
    font-bold
    :style="[
      (size ? '' : 'font-size: 1.125rem;'),
      {
        background: color ? colorMap[color]?.bg : 'var(--primary-bg)',
        border: `1px solid ${color ? colorMap[color]?.border : 'var(--primary-border)'}`,
        color: color ? colorMap[color]?.text : 'var(--primary-light)'
      }
    ]"
  >
    {{ num }}
  </div>
</template>
