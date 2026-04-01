<script setup lang="ts">
/**
 * SlideReveal — 入场动画封装组件
 *
 * 用法：
 *   <SlideReveal :delay="0">
 *     <div>内容</div>
 *   </SlideReveal>
 *
 * 配合 v-for stagger：
 *   <SlideReveal v-for="(item, i) in items" :delay="i * 80">
 */
defineProps<{
  delay?: number   // 延迟毫秒
  duration?: number // 动画时长毫秒，默认 400
  direction?: 'up' | 'fade' | 'scale'
}>()
</script>

<template>
  <div
    class="slide-reveal"
    :style="{
      '--reveal-delay': `${delay ?? 0}ms`,
      '--reveal-duration': `${duration ?? 400}ms`,
    }"
  >
    <slot />
  </div>
</template>

<style scoped>
.slide-reveal {
  opacity: 0;
  animation: reveal-in var(--reveal-duration) cubic-bezier(0.16, 1, 0.3, 1) forwards;
  animation-delay: var(--reveal-delay);
  will-change: transform, opacity;
}

@keyframes reveal-in {
  from {
    opacity: 0;
    transform: translateY(12px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

/* Direction variants */
.slide-reveal[data-direction="fade"] {
  animation-name: reveal-fade;
}
@keyframes reveal-fade {
  from { opacity: 0; }
  to   { opacity: 1; }
}

.slide-reveal[data-direction="scale"] {
  animation-name: reveal-scale;
}
@keyframes reveal-scale {
  from { opacity: 0; transform: scale(0.94); }
  to   { opacity: 1; transform: scale(1); }
}
</style>
