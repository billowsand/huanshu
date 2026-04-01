<script setup lang="ts">
const props = defineProps<{
  cols?: number
  gap?: string | number
}>()

function toGap(value?: string | number, fallback = 4) {
  const raw = value ?? fallback
  const num = typeof raw === 'number' ? raw : Number(raw)
  return Number.isFinite(num) ? `${num * 0.25}rem` : String(raw)
}
</script>

<template>
  <div
    class="grid"
    :style="{
      gridTemplateColumns: `repeat(${props.cols || 2}, minmax(0, 1fr))`,
      gap: toGap(props.gap, 4),
    }"
  >
    <slot />
  </div>
</template>
