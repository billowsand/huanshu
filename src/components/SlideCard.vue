<script setup lang="ts">
import { ref, computed } from 'vue'
import { useGenerationStore } from '../stores/generation'
import type { SlideBlueprint } from '../stores/generation'

const props = defineProps<{
  slide: SlideBlueprint
  index: number
  active: boolean
}>()

const emit = defineEmits<{ select: [index: number] }>()

const store = useGenerationStore()
const showDebug = ref(false)
const feedback = ref('')
const repairing = ref(false)

const kindLabel: Record<string, string> = {
  cover: '封面', closing: '结束页', overview: '目录', section_intro: '章节导览', feature_grid: '特性格', spotlight: '聚光灯',
  split_layers: '分层', section_list: '列表', focus_example: '焦点例', outcome_grid: '成果格',
  center_grid: '中心格', timeline: '时间轴', step_flow: '步骤流', process: '流程',
  compare: '对比', swot: 'SWOT',
}

const kindColor: Record<string, string> = {
  cover: '#f59e0b', closing: '#f43f5e', overview: '#8b5cf6', section_intro: '#0f766e', feature_grid: '#3b82f6',
  spotlight: '#06b6d4', split_layers: '#10b981', section_list: '#f97316',
  focus_example: '#ec4899', outcome_grid: '#84cc16', center_grid: '#6366f1',
  timeline: '#14b8a6', step_flow: '#f59e0b', process: '#8b5cf6',
  compare: '#ef4444', swot: '#22d3ee',
}

const color = computed(() => kindColor[props.slide.kind] ?? '#888')
const label = computed(() => kindLabel[props.slide.kind] ?? props.slide.kind)

async function repair() {
  if (!feedback.value.trim()) return
  repairing.value = true
  try {
    await store.repairSlide(props.index, feedback.value)
    feedback.value = ''
    showDebug.value = false
  } finally {
    repairing.value = false
  }
}
</script>

<template>
  <div
    class="slide-card"
    :class="{ active }"
    @click="emit('select', index)"
  >
    <!-- Card header -->
    <div class="card-head">
      <span class="slide-num">{{ index + 1 }}</span>
      <span class="kind-badge" :style="{ background: color + '22', color, borderColor: color + '44' }">
        {{ label }}
      </span>
      <span class="slide-title">{{ slide.title }}</span>
      <button
        class="debug-btn"
        :class="{ active: showDebug }"
        title="调试"
        @click.stop="showDebug = !showDebug"
      >
        <span class="i-carbon:debug" />
      </button>
    </div>

    <!-- Debug panel -->
    <div v-if="showDebug" class="debug-panel" @click.stop>
      <pre class="json-view">{{ JSON.stringify(slide, null, 2) }}</pre>
      <div class="repair-row">
        <input
          v-model="feedback"
          placeholder="描述修改意见，例如：标题太长，请精简到10字以内"
          @keydown.enter.prevent="repair"
        />
        <button class="btn btn-primary" :disabled="repairing || !feedback.trim()" @click="repair">
          <span v-if="repairing" class="i-carbon:renew" style="animation:spin 0.7s linear infinite" />
          <span v-else class="i-carbon:magic-wand" />
          修复
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.slide-card {
  border: 1px solid var(--studio-border);
  border-radius: 6px;
  overflow: hidden;
  cursor: pointer;
  transition: all 0.15s ease;
  background: var(--studio-surface);
}

.slide-card:hover { border-color: rgba(255,255,255,0.15); }
.slide-card.active { border-color: var(--studio-primary-border); background: var(--studio-primary-bg); }

.card-head {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  padding: 0.4rem 0.5rem;
}

.slide-num {
  font-size: 0.65rem;
  color: var(--studio-muted);
  font-variant-numeric: tabular-nums;
  min-width: 1.5rem;
}

.kind-badge {
  font-size: 0.6rem;
  padding: 0 0.35rem;
  border-radius: 3px;
  border: 1px solid;
  white-space: nowrap;
}

.slide-title {
  flex: 1;
  font-size: 0.75rem;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.debug-btn {
  width: 22px;
  height: 22px;
  border: 1px solid transparent;
  border-radius: 4px;
  background: transparent;
  color: var(--studio-muted);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.15s;
  padding: 0;
  font-size: 0.8rem;
}
.debug-btn:hover, .debug-btn.active { color: var(--studio-primary); border-color: var(--studio-primary-border); background: var(--studio-primary-bg); }

.debug-panel {
  border-top: 1px solid var(--studio-border);
  background: var(--studio-bg);
}

.json-view {
  font-size: 0.65rem;
  padding: 0.5rem;
  max-height: 200px;
  overflow: auto;
  color: var(--studio-muted);
  white-space: pre-wrap;
  word-break: break-all;
  font-family: var(--font-mono);
}

.repair-row {
  display: flex;
  gap: 0.4rem;
  padding: 0.4rem 0.5rem;
  border-top: 1px solid var(--studio-border);
}

.repair-row input { font-size: 0.75rem; }
.repair-row .btn { white-space: nowrap; font-size: 0.75rem; padding: 0.35rem 0.6rem; }

@keyframes spin { to { transform: rotate(360deg); } }
</style>
