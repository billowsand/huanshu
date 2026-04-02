<script setup lang="ts">
import { computed } from 'vue'
import type { GenStage } from '../stores/generation'

const props = defineProps<{
  stage: GenStage
  message: string
  progress: number
  slideCount: number
  running: boolean
  error: string
}>()

const stages: { key: GenStage; label: string; icon: string }[] = [
  { key: 'init',       label: '初始化',     icon: 'i-carbon:settings' },
  { key: 'page_plan',  label: '规划页面',   icon: 'i-carbon:list-boxes' },
  { key: 'generating', label: '并发生成',   icon: 'i-carbon:magic-wand' },
  { key: 'assembling', label: '组装',       icon: 'i-carbon:assembly' },
  { key: 'done',       label: '完成',       icon: 'i-carbon:checkmark-filled' },
]

const stageOrder = stages.map(s => s.key)

const currentIdx = computed(() => {
  const i = stageOrder.indexOf(props.stage)
  return i === -1 ? (props.stage === 'done' ? stageOrder.length : -1) : i
})

function stageStatus(key: GenStage) {
  const idx = stageOrder.indexOf(key)
  if (props.stage === 'error') return idx <= currentIdx.value ? 'error' : 'pending'
  if (idx < currentIdx.value) return 'done'
  if (idx === currentIdx.value) return 'active'
  return 'pending'
}
</script>

<template>
  <div class="pipeline">
    <!-- Stage pills -->
    <div class="stages">
      <div
        v-for="s in stages"
        :key="s.key"
        class="stage-item"
        :class="stageStatus(s.key)"
      >
        <div class="stage-icon">
          <span :class="s.icon" />
          <div v-if="stageStatus(s.key) === 'active'" class="pulse-ring" />
        </div>
        <span class="stage-label">{{ s.label }}</span>
      </div>
    </div>

    <!-- Progress bar -->
    <div class="progress-track">
      <div class="progress-fill" :style="{ width: (progress * 100) + '%' }" :class="{ error: stage === 'error' }" />
    </div>

    <!-- Status message -->
    <div class="status-msg" :class="{ error: stage === 'error' }">
      <span v-if="running" class="spinner-inline" />
      <span v-else-if="stage === 'done'" class="i-carbon:checkmark-filled" style="color:var(--studio-success)" />
      <span v-else-if="stage === 'error'" class="i-carbon:warning-filled" style="color:var(--studio-error)" />
      <span class="msg-text">{{ message || (running ? '处理中...' : '') }}</span>
      <span v-if="stage === 'generating' && slideCount > 0" class="slide-badge">{{ slideCount }} 张</span>
    </div>
  </div>
</template>

<style scoped>
.pipeline {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  padding: 0.75rem;
}

.stages {
  display: flex;
  align-items: center;
  gap: 0.25rem;
}

.stage-item {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.2rem;
  flex: 1;
  position: relative;
}

.stage-item:not(:last-child)::after {
  content: '';
  position: absolute;
  top: 14px;
  left: calc(50% + 12px);
  right: calc(-50% + 12px);
  height: 1px;
  background: var(--studio-border);
  z-index: 0;
}

.stage-item.done:not(:last-child)::after { background: var(--studio-success); }

.stage-icon {
  width: 28px;
  height: 28px;
  border-radius: 50%;
  background: var(--studio-surface);
  border: 1.5px solid var(--studio-border);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 0.875rem;
  position: relative;
  z-index: 1;
  transition: all 0.2s ease;
}

.stage-item.done .stage-icon {
  border-color: var(--studio-success);
  color: var(--studio-success);
  background: var(--studio-success-bg);
}

.stage-item.active .stage-icon {
  border-color: var(--studio-primary);
  color: var(--studio-primary);
  background: var(--studio-primary-bg);
}

.stage-item.error .stage-icon {
  border-color: var(--studio-error);
  color: var(--studio-error);
}

.stage-item.pending .stage-icon {
  opacity: 0.35;
}

.pulse-ring {
  position: absolute;
  inset: -4px;
  border-radius: 50%;
  border: 2px solid var(--studio-primary);
  opacity: 0.5;
  animation: pulse-ring 1.5s ease-out infinite;
}

@keyframes pulse-ring {
  0% { transform: scale(0.85); opacity: 0.7; }
  100% { transform: scale(1.4); opacity: 0; }
}

.stage-label {
  font-size: 0.6rem;
  color: var(--studio-muted);
  white-space: nowrap;
}

.stage-item.done .stage-label,
.stage-item.active .stage-label {
  color: var(--studio-text);
}

.progress-track {
  height: 3px;
  background: var(--studio-surface);
  border-radius: 2px;
  overflow: hidden;
}

.progress-fill {
  height: 100%;
  background: linear-gradient(90deg, var(--studio-primary), #fcd34d);
  border-radius: 2px;
  transition: width 0.4s ease;
}

.progress-fill.error { background: var(--studio-error); }

.status-msg {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  font-size: 0.75rem;
  color: var(--studio-muted);
  min-height: 1.2rem;
}

.status-msg.error { color: var(--studio-error); }

.spinner-inline {
  width: 12px;
  height: 12px;
  border: 1.5px solid var(--studio-border);
  border-top-color: var(--studio-primary);
  border-radius: 50%;
  animation: spin 0.7s linear infinite;
  flex-shrink: 0;
}

@keyframes spin { to { transform: rotate(360deg); } }

.msg-text { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

.slide-badge {
  background: var(--studio-primary-bg);
  border: 1px solid var(--studio-primary-border);
  color: var(--studio-primary);
  border-radius: 4px;
  padding: 0 0.4rem;
  font-size: 0.65rem;
  white-space: nowrap;
}
</style>
