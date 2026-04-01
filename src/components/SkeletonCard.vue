<script setup lang="ts">
/**
 * SkeletonCard — 骨架屏占位组件
 *
 * 用法：
 *   <SkeletonCard :rows="3" height="80px" />
 */
defineProps<{
  rows?: number    // 骨架行数
  height?: string  // 单行高度
  width?: string   // 可选：固定宽度
}>()
</script>

<template>
  <div class="skeleton-card">
    <div
      v-for="i in (rows ?? 3)"
      :key="i"
      class="skeleton-line"
      :style="{
        height: height ?? '14px',
        width: width ?? (i === (rows ?? 3) ? '60%' : '100%'),
        animationDelay: `${i * 100}ms`,
      }"
    />
  </div>
</template>

<style scoped>
.skeleton-card {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 1rem;
}

.skeleton-line {
  background: var(--studio-surface);
  border-radius: 4px;
  overflow: hidden;
  position: relative;
}

.skeleton-line::after {
  content: '';
  position: absolute;
  inset: 0;
  background: linear-gradient(
    90deg,
    transparent 0%,
    rgba(255,255,255,0.05) 50%,
    transparent 100%
  );
  animation: skeleton-shimmer 1.6s ease-in-out infinite;
  animation-delay: inherit;
}

@keyframes skeleton-shimmer {
  0%   { transform: translateX(-100%); }
  100% { transform: translateX(200%); }
}
</style>
