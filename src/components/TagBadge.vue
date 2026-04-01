<script setup lang="ts">
defineProps<{
  color?: 'amber' | 'blue' | 'green' | 'purple' | 'red' | 'teal' | 'indigo'
  inline?: boolean
}>()

const colorMap: Record<string, { bg: string; border: string; text: string }> = {
  amber: { bg: 'var(--warning-bg)', border: 'var(--warning-border)', text: 'var(--warning-light)' },
  blue: { bg: 'var(--info-bg)', border: 'var(--info-border)', text: 'var(--info-light)' },
  green: { bg: 'var(--success-bg)', border: 'var(--success-border)', text: 'var(--success-light)' },
  purple: { bg: 'var(--info-bg)', border: 'var(--info-border)', text: 'var(--info-light)' },
  red: { bg: 'var(--danger-bg)', border: 'var(--danger-border)', text: 'var(--danger-light)' },
  teal: { bg: 'var(--success-bg)', border: 'var(--success-border)', text: 'var(--success-light)' },
  indigo: { bg: 'var(--info-bg)', border: 'var(--info-border)', text: 'var(--info-light)' },
}
</script>

<template>
  <div
    class="tag-pill"
    :class="inline ? 'inline-block' : ''"
    :style="{
      background: color ? colorMap[color]?.bg : 'var(--primary-bg)',
      border: `1px solid ${color ? colorMap[color]?.border : 'var(--primary-border)'}`,
      color: color ? colorMap[color]?.text : 'var(--primary-light)'
    }"
  >
    <slot />
  </div>
</template>
