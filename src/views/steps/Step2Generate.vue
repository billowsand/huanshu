<script setup lang="ts">
import { computed, watch } from 'vue'
import SlideRenderer from '../../components/SlideRenderer.vue'
import type { SlideBlueprint } from '../../components/types'
import type { GenerationLogEntry } from '../../stores/generation'

type PagePlan = {
  page_id?: string
  section_title?: string
  subsection_title?: string | null
  page_title?: string
  objective?: string
  key_points?: string[]
  takeaway?: string | null
  layout_intent?: string | null
  visual_need?: string | null
  density?: string | null
  page_role?: string | null
}

type LayoutPlan = {
  page_id?: string
  kind?: string
  title?: string
  subtitle?: string | null
  section_label?: string | null
  reason?: string
}

type SlideProgressItem = {
  index: number
  pagePlan: PagePlan | null
  layoutPlan: LayoutPlan | null
  blueprint: SlideBlueprint | null
  contentLog: GenerationLogEntry | null
}

const props = defineProps<{
  stage: string
  stageLabel: string
  running: boolean
  message: string
  slideCount: number
  blueprints: SlideBlueprint[]
  mediaMap: Record<string, string>
  logs: GenerationLogEntry[]
  selectedSlideIndex: number | null
}>()

const emit = defineEmits<{
  'update:selectedSlideIndex': [value: number | null]
  'go-to-step-3': []
  'go-to-step-1': []
  'retry': []
}>()

const LOG_STAGE_LABELS: Record<string, string> = {
  start: '开始',
  init: '初始化',
  clean: '文档清理',
  page_plan: '页面规划',
  layout_plan: '布局规划',
  content: '内容生成',
  normalize: '规范修复',
  validate: '最终校验',
  done: '完成',
  error: '错误',
}

const KIND_LABELS: Record<string, string> = {
  cover: '封面',
  closing: '结束页',
  overview: '目录',
  section_intro: '章节导览',
  feature_grid: '特性格',
  spotlight: '聚光灯',
  split_layers: '分层',
  section_list: '列表',
  focus_example: '焦点例',
  outcome_grid: '成果格',
  center_grid: '中心格',
  timeline: '时间轴',
  step_flow: '步骤流',
  process: '流程',
  compare: '对比',
  swot: 'SWOT',
}

const PIPELINE_NODE_STAGES = [
  { key: 'init', label: '连接验证' },
  { key: 'clean', label: '文档清理' },
  { key: 'page_plan', label: '规划页面' },
  { key: 'layout_plan', label: '布局排版' },
  { key: 'content', label: '内容生成' },
  { key: 'normalize', label: '规范修复' },
  { key: 'validate', label: '最终校验' },
]

const PIPELINE_STAGE_ORDER = ['start', 'init', 'clean', 'page_plan', 'layout_plan', 'content', 'normalize', 'validate', 'done']

function detailText(value: unknown): string {
  if (value === null || value === undefined)
    return ''
  if (typeof value === 'string')
    return value
  return JSON.stringify(value, null, 2)
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object' && !Array.isArray(value)
}

function extractArrayDetail<T>(key: string): T[] {
  for (let i = props.logs.length - 1; i >= 0; i -= 1) {
    const detail = props.logs[i]?.detail
    if (!isRecord(detail))
      continue
    const value = detail[key]
    if (Array.isArray(value))
      return value as T[]
  }
  return []
}

function extractBlueprintFromLog(log: GenerationLogEntry | null): SlideBlueprint | null {
  const detail = log?.detail
  if (!isRecord(detail) || !isRecord(detail.blueprint))
    return null
  return detail.blueprint as unknown as SlideBlueprint
}

function extractPagePlanFromLog(log: GenerationLogEntry | null): PagePlan | null {
  const detail = log?.detail
  if (!isRecord(detail) || !isRecord(detail.page_plan))
    return null
  return detail.page_plan as PagePlan
}

function extractLayoutPlanFromLog(log: GenerationLogEntry | null): LayoutPlan | null {
  const detail = log?.detail
  if (!isRecord(detail) || !isRecord(detail.layout_plan))
    return null
  return detail.layout_plan as LayoutPlan
}

const pagePlans = computed(() => extractArrayDetail<PagePlan>('page_plans'))
const layoutPlans = computed(() => extractArrayDetail<LayoutPlan>('layout_plans'))
const contentLogBySlideIndex = computed(() => {
  const map = new Map<number, GenerationLogEntry>()
  for (const log of props.logs) {
    if (log.stage !== 'content' || log.kind !== 'slide')
      continue
    if (typeof log.slide_index !== 'number')
      continue
    map.set(log.slide_index, log)
  }
  return map
})
const importantLogCount = computed(() => props.logs.filter(log => log.important).length)
const promptLogCount = computed(() => props.logs.filter(log => ['input', 'prompt_io', 'slide', 'repair'].includes(log.kind)).length)
const recentLogs = computed(() => props.logs.slice(-10).reverse())

const pipelineStages = computed(() => {
  const cur = props.stage
  const curIdx = PIPELINE_STAGE_ORDER.indexOf(cur)
  return PIPELINE_NODE_STAGES.map((s) => {
    const sIdx = PIPELINE_STAGE_ORDER.indexOf(s.key)
    let status: 'done' | 'active' | 'error' | 'pending'
    if (cur === 'error') {
      const lastLog = [...props.logs].reverse().find(l => l.stage !== 'error' && PIPELINE_STAGE_ORDER.indexOf(l.stage) > 0)
      const errIdx = PIPELINE_STAGE_ORDER.indexOf(lastLog?.stage ?? 'init')
      if (sIdx < errIdx)
        status = 'done'
      else if (sIdx === errIdx)
        status = 'error'
      else
        status = 'pending'
    } else {
      if (sIdx < curIdx)
        status = 'done'
      else if (sIdx === curIdx)
        status = 'active'
      else
        status = 'pending'
    }
    return { ...s, status }
  })
})

const slideItems = computed<SlideProgressItem[]>(() => {
  const total = Math.max(
    pagePlans.value.length,
    layoutPlans.value.length,
    contentLogBySlideIndex.value.size,
    props.blueprints.length,
    props.slideCount,
  )

  return Array.from({ length: total }, (_, index) => {
    const contentLog = contentLogBySlideIndex.value.get(index) ?? null
    return {
      index,
      pagePlan: pagePlans.value[index] ?? extractPagePlanFromLog(contentLog),
      layoutPlan: layoutPlans.value[index] ?? extractLayoutPlanFromLog(contentLog),
      blueprint: props.blueprints[index] ?? extractBlueprintFromLog(contentLog),
      contentLog,
    }
  })
})

function preferredSlideIndex(): number | null {
  for (let i = slideItems.value.length - 1; i >= 0; i -= 1) {
    const item = slideItems.value[i]
    if (item.blueprint || item.layoutPlan || item.pagePlan)
      return i
  }
  return slideItems.value.length > 0 ? 0 : null
}

watch(
  () => [slideItems.value.length, props.blueprints.length, props.selectedSlideIndex],
  () => {
    const selected = props.selectedSlideIndex
    if (selected !== null && selected >= 0 && selected < slideItems.value.length)
      return
    emit('update:selectedSlideIndex', preferredSlideIndex())
  },
  { immediate: true },
)

const selectedSlide = computed(() => {
  if (!slideItems.value.length)
    return null
  const idx = props.selectedSlideIndex ?? preferredSlideIndex() ?? 0
  return slideItems.value[idx] ?? null
})

function selectSlide(index: number) {
  emit('update:selectedSlideIndex', index)
}

function kindLabel(kind?: string | null): string {
  if (!kind)
    return '待定'
  return KIND_LABELS[kind] ?? kind
}

function logStageLabel(stage: string): string {
  return LOG_STAGE_LABELS[stage] ?? stage
}

function slideStatus(item: SlideProgressItem): { tone: string, label: string } {
  if (props.stage === 'error')
    return { tone: 'error', label: '生成中断' }
  if (props.stage === 'done' && item.blueprint)
    return { tone: 'done', label: '已完成' }
  if (item.blueprint && ['normalize', 'validate'].includes(props.stage))
    return { tone: 'active', label: props.stage === 'normalize' ? '规范修复中' : '校验中' }
  if (item.blueprint)
    return { tone: 'done', label: '内容已生成' }
  if (item.layoutPlan && PIPELINE_STAGE_ORDER.indexOf(props.stage) >= PIPELINE_STAGE_ORDER.indexOf('content'))
    return { tone: 'active', label: '内容生成中' }
  if (item.layoutPlan)
    return { tone: 'done', label: '布局已确定' }
  if (item.pagePlan && PIPELINE_STAGE_ORDER.indexOf(props.stage) >= PIPELINE_STAGE_ORDER.indexOf('layout_plan'))
    return { tone: 'active', label: '等待布局' }
  if (item.pagePlan)
    return { tone: 'done', label: '规划完成' }
  if (props.stage === 'page_plan')
    return { tone: 'active', label: '规划中' }
  return { tone: 'pending', label: '等待开始' }
}

function stageStepState(item: SlideProgressItem, key: 'page_plan' | 'layout_plan' | 'content'): 'done' | 'active' | 'pending' {
  if (key === 'page_plan') {
    if (item.pagePlan)
      return 'done'
    return props.stage === 'page_plan' ? 'active' : 'pending'
  }
  if (key === 'layout_plan') {
    if (item.layoutPlan)
      return 'done'
    if (item.pagePlan && ['layout_plan', 'content', 'normalize', 'validate', 'done'].includes(props.stage))
      return 'active'
    return 'pending'
  }
  if (item.blueprint)
    return 'done'
  if (item.layoutPlan && ['content', 'normalize', 'validate'].includes(props.stage))
    return 'active'
  return 'pending'
}

function selectedDebugText(key: string): string {
  const detail = selectedSlide.value?.contentLog?.detail
  if (!isRecord(detail))
    return ''
  return detailText(detail[key])
}
</script>

<template>
  <div class="step-panel step-generate">
    <div class="gen-header">
      <div class="gen-hd-left">
        <div class="gen-status-orb" :class="`gen-orb--${props.stage}`" />
        <div>
          <div class="gen-hd-title">
            <template v-if="props.running">{{ props.stageLabel }}<span class="gen-hd-ellipsis">...</span></template>
            <template v-else-if="props.stage === 'done'">生成完成 · {{ slideItems.length }} 张幻灯片</template>
            <template v-else-if="props.stage === 'error'">生成遇到问题</template>
            <template v-else>准备开始</template>
          </div>
          <div v-if="props.message" class="gen-hd-msg">{{ props.message }}</div>
        </div>
      </div>
      <div class="gen-hd-stats">
        <div class="gen-stat-chip">
          <span class="gen-stat-n">{{ slideItems.length }}</span>
          <span class="gen-stat-lbl">幻灯片</span>
        </div>
        <div class="gen-stat-chip">
          <span class="gen-stat-n">{{ importantLogCount }}</span>
          <span class="gen-stat-lbl">关键事件</span>
        </div>
        <div class="gen-stat-chip">
          <span class="gen-stat-n gen-stat-n--accent">{{ promptLogCount }}</span>
          <span class="gen-stat-lbl">LLM 调用</span>
        </div>
      </div>
    </div>

    <div class="gen-stage-track">
      <template v-for="(stg, i) in pipelineStages" :key="stg.key">
        <div class="gen-stage-node" :class="stg.status">
          <div class="gsn-dot">
            <span v-if="stg.status === 'done'" class="i-carbon:checkmark gsn-icon" />
            <span v-else-if="stg.status === 'error'" class="i-carbon:close gsn-icon" />
            <span v-else-if="stg.status === 'active'" class="gsn-spin" />
          </div>
          <span class="gsn-label">{{ stg.label }}</span>
        </div>
        <div v-if="i < pipelineStages.length - 1" class="gsn-connector" :class="{ done: stg.status === 'done' }" />
      </template>
    </div>

    <div class="slides-workbench">
      <div class="slides-grid-panel">
        <div class="gen-panel-head">
          <span class="i-carbon:catalog" />
          幻灯片生成看板
          <div v-if="props.running" class="gen-live-dot" />
          <span class="gen-panel-count">{{ slideItems.length }}</span>
        </div>

        <div v-if="slideItems.length === 0" class="slides-empty">
            <span class="i-carbon:document-blank slides-empty-icon" />
          <p>生成开始后，这里会直接显示每一张幻灯片的生成进度。</p>
        </div>

        <div v-else class="slides-grid">
          <button
            v-for="item in slideItems"
            :key="item.index"
            type="button"
            class="slide-progress-card"
            :class="{
              'is-active': props.selectedSlideIndex === item.index,
              [`status--${slideStatus(item).tone}`]: true,
            }"
            @click="selectSlide(item.index)"
          >
            <div class="spc-top">
              <div class="spc-head">
                <span class="spc-num">#{{ item.index + 1 }}</span>
                <span class="spc-title">{{ item.pagePlan?.page_title || item.layoutPlan?.title || item.blueprint?.title || `第 ${item.index + 1} 张` }}</span>
              </div>
              <span class="spc-status" :class="`spc-status--${slideStatus(item).tone}`">{{ slideStatus(item).label }}</span>
            </div>

            <div class="spc-steps">
              <span class="spc-step" :class="`is-${stageStepState(item, 'page_plan')}`">规划</span>
              <span class="spc-step" :class="`is-${stageStepState(item, 'layout_plan')}`">布局</span>
              <span class="spc-step" :class="`is-${stageStepState(item, 'content')}`">内容</span>
            </div>

            <div class="spc-summary">
              <div class="spc-section">
                <div class="spc-label">规划要点</div>
                <ul v-if="item.pagePlan?.key_points?.length" class="spc-points">
                  <li v-for="point in item.pagePlan.key_points.slice(0, 3)" :key="point">{{ point }}</li>
                </ul>
                <p v-else class="spc-placeholder">等待页面规划结果</p>
              </div>

              <div class="spc-section">
                <div class="spc-label">布局方案</div>
                <template v-if="item.layoutPlan">
                  <div class="spc-layout-kind">{{ kindLabel(item.layoutPlan.kind) }}</div>
                  <p class="spc-layout-reason">{{ item.layoutPlan.reason || '布局已确定，等待内容生成。' }}</p>
                </template>
                <p v-else class="spc-placeholder">等待布局排版结果</p>
              </div>
            </div>

            <div class="spc-preview">
              <div v-if="item.blueprint" class="spc-preview-frame">
                <div class="spc-preview-canvas">
                  <SlideRenderer :slide="item.blueprint" :slide-index="item.index" :media-map="props.mediaMap" />
                </div>
              </div>
              <div v-else class="spc-preview-placeholder">
                <span class="i-carbon:task" />
                <p>{{ item.layoutPlan ? '内容生成后将在这里出现预览' : '当前还没有可渲染内容' }}</p>
              </div>
            </div>
          </button>
        </div>
      </div>

      <aside class="slide-detail-panel">
        <div class="gen-panel-head">
          <span class="i-carbon:view" />
          幻灯片详情
        </div>

        <template v-if="selectedSlide">
          <div class="sdp-head">
            <div class="sdp-kicker">第 {{ selectedSlide.index + 1 }} 张</div>
            <div class="sdp-title">{{ selectedSlide.pagePlan?.page_title || selectedSlide.layoutPlan?.title || selectedSlide.blueprint?.title || '待生成幻灯片' }}</div>
            <div class="sdp-meta">
              <span class="sdp-chip">{{ slideStatus(selectedSlide).label }}</span>
              <span v-if="selectedSlide.layoutPlan?.kind" class="sdp-chip sdp-chip--accent">{{ kindLabel(selectedSlide.layoutPlan.kind) }}</span>
              <span v-if="selectedSlide.pagePlan?.section_title" class="sdp-chip">{{ selectedSlide.pagePlan.section_title }}</span>
            </div>
          </div>

          <div class="sdp-preview-shell">
            <div v-if="selectedSlide.blueprint" class="sdp-preview-stage">
              <div class="sdp-preview-canvas">
                <SlideRenderer :slide="selectedSlide.blueprint" :slide-index="selectedSlide.index" :media-map="props.mediaMap" />
              </div>
            </div>
            <div v-else class="sdp-preview-empty">
              <span class="i-carbon:image-search" />
              <p>当前页尚未生成最终内容。</p>
            </div>
          </div>

          <div class="sdp-body">
            <section class="sdp-section">
              <div class="sdp-section-title">规划页面</div>
              <p v-if="selectedSlide.pagePlan?.objective" class="sdp-text">{{ selectedSlide.pagePlan.objective }}</p>
              <ul v-if="selectedSlide.pagePlan?.key_points?.length" class="sdp-list">
                <li v-for="point in selectedSlide.pagePlan.key_points" :key="point">{{ point }}</li>
              </ul>
              <p v-if="selectedSlide.pagePlan?.takeaway" class="sdp-note">结论：{{ selectedSlide.pagePlan.takeaway }}</p>
              <p v-if="!selectedSlide.pagePlan" class="sdp-empty-hint">这一页的规划结果还没出来。</p>
            </section>

            <section class="sdp-section">
              <div class="sdp-section-title">布局排版</div>
              <p v-if="selectedSlide.layoutPlan" class="sdp-text">
                <strong>{{ kindLabel(selectedSlide.layoutPlan.kind) }}</strong>
                <span v-if="selectedSlide.layoutPlan.reason"> · {{ selectedSlide.layoutPlan.reason }}</span>
              </p>
              <p v-if="selectedSlide.layoutPlan?.subtitle" class="sdp-note">{{ selectedSlide.layoutPlan.subtitle }}</p>
              <p v-if="!selectedSlide.layoutPlan" class="sdp-empty-hint">这一页还没有布局方案。</p>
            </section>

            <section class="sdp-section">
              <div class="sdp-section-title">内容结果</div>
              <p v-if="selectedSlide.blueprint" class="sdp-text">
                标题：{{ selectedSlide.blueprint.title }}
                <span v-if="selectedSlide.blueprint.subtitle"> · {{ selectedSlide.blueprint.subtitle }}</span>
              </p>
              <p v-if="selectedSlide.blueprint?.note" class="sdp-note">{{ selectedSlide.blueprint.note }}</p>
              <p v-if="!selectedSlide.blueprint" class="sdp-empty-hint">内容尚未生成完成。</p>
            </section>

            <details v-if="selectedSlide.contentLog" class="sdp-debug">
              <summary>查看当前页调试信息</summary>
              <div v-if="selectedDebugText('system_prompt')" class="sdp-debug-block">
                <div class="sdp-debug-label">System Prompt</div>
                <pre>{{ selectedDebugText('system_prompt') }}</pre>
              </div>
              <div v-if="selectedDebugText('user_prompt')" class="sdp-debug-block">
                <div class="sdp-debug-label">User Prompt</div>
                <pre>{{ selectedDebugText('user_prompt') }}</pre>
              </div>
              <div v-if="selectedDebugText('raw_output')" class="sdp-debug-block">
                <div class="sdp-debug-label">模型原始输出</div>
                <pre>{{ selectedDebugText('raw_output') }}</pre>
              </div>
              <div v-if="selectedDebugText('parsed_output')" class="sdp-debug-block">
                <div class="sdp-debug-label">解析结果</div>
                <pre>{{ selectedDebugText('parsed_output') }}</pre>
              </div>
            </details>

            <details class="sdp-debug">
              <summary>查看最近日志</summary>
              <div class="sdp-log-list">
                <div v-for="log in recentLogs" :key="log.id" class="sdp-log-item">
                  <div class="sdp-log-meta">{{ logStageLabel(log.stage) }} · #{{ log.seq }}</div>
                  <div class="sdp-log-title">{{ log.title || log.summary }}</div>
                </div>
              </div>
            </details>
          </div>
        </template>

        <div v-else class="sdp-empty">
          <span class="i-carbon:cursor-1" />
          <p>生成开始后，选择任意一张幻灯片查看详情。</p>
        </div>
      </aside>
    </div>

    <div v-if="!props.running" class="step-actions">
      <button v-if="props.stage === 'done'" class="btn btn-primary" @click="$emit('go-to-step-3')">
        <span class="i-carbon:edit" /> 前往编辑
      </button>
      <button v-if="props.stage === 'error'" class="btn" @click="$emit('go-to-step-1')">
        <span class="i-carbon:arrow-left" /> 返回修改
      </button>
      <button v-if="props.stage === 'error'" class="btn btn-primary" @click="$emit('retry')">
        <span class="i-carbon:renew" /> 重试
      </button>
    </div>
  </div>
</template>

<style scoped>
.slides-workbench {
  display: grid;
  grid-template-columns: minmax(0, 1.5fr) minmax(340px, 0.9fr);
  gap: 1rem;
  min-height: 0;
  flex: 1;
}

.slides-grid-panel,
.slide-detail-panel {
  min-height: 0;
  border: 1px solid var(--studio-border);
  border-radius: 16px;
  background:
    linear-gradient(180deg, rgba(255,255,255,0.03), rgba(255,255,255,0.01)),
    var(--studio-surface);
  overflow: hidden;
}

.slides-grid-panel {
  display: flex;
  flex-direction: column;
}

.slide-detail-panel {
  display: flex;
  flex-direction: column;
}

.slides-grid {
  padding: 1rem;
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
  gap: 1rem;
  overflow: auto;
}

.slides-empty,
.sdp-empty {
  min-height: 320px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  color: var(--studio-muted);
  gap: 0.75rem;
}

.slides-empty-icon,
.sdp-empty span {
  font-size: 2rem;
  opacity: 0.22;
}

.slide-progress-card {
  text-align: left;
  border: 1px solid var(--studio-border);
  border-radius: 16px;
  background:
    radial-gradient(circle at top right, rgba(99, 102, 241, 0.08), transparent 38%),
    var(--studio-panel);
  padding: 0.9rem;
  color: inherit;
  display: flex;
  flex-direction: column;
  gap: 0.85rem;
  cursor: pointer;
  transition: border-color 0.18s ease, transform 0.18s ease, background 0.18s ease;
}

.slide-progress-card:hover {
  transform: translateY(-2px);
  border-color: var(--studio-border-hover);
}

.slide-progress-card.is-active {
  border-color: var(--studio-primary-border);
  box-shadow: 0 0 0 1px rgba(59, 130, 246, 0.18);
}

.slide-progress-card.status--done {
  background:
    radial-gradient(circle at top right, rgba(16, 185, 129, 0.1), transparent 38%),
    var(--studio-panel);
}

.slide-progress-card.status--active {
  background:
    radial-gradient(circle at top right, rgba(59, 130, 246, 0.12), transparent 38%),
    var(--studio-panel);
}

.slide-progress-card.status--error {
  background:
    radial-gradient(circle at top right, rgba(239, 68, 68, 0.12), transparent 38%),
    var(--studio-panel);
}

.spc-top,
.spc-head {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.spc-top {
  justify-content: space-between;
}

.spc-num {
  font-size: 0.76rem;
  color: var(--studio-muted);
  font-variant-numeric: tabular-nums;
}

.spc-title {
  font-size: 0.95rem;
  font-weight: 600;
  line-height: 1.35;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.spc-status,
.sdp-chip {
  display: inline-flex;
  align-items: center;
  border: 1px solid var(--studio-border);
  border-radius: 999px;
  padding: 0.22rem 0.55rem;
  font-size: 0.72rem;
  white-space: nowrap;
}

.spc-status--done,
.sdp-chip--accent {
  border-color: rgba(16, 185, 129, 0.28);
  color: #34d399;
  background: rgba(16, 185, 129, 0.1);
}

.spc-status--active {
  border-color: rgba(59, 130, 246, 0.28);
  color: #60a5fa;
  background: rgba(59, 130, 246, 0.1);
}

.spc-status--pending {
  color: var(--studio-muted);
}

.spc-status--error {
  border-color: rgba(239, 68, 68, 0.3);
  color: #f87171;
  background: rgba(239, 68, 68, 0.1);
}

.spc-steps {
  display: flex;
  gap: 0.5rem;
  flex-wrap: wrap;
}

.spc-step {
  border-radius: 999px;
  padding: 0.18rem 0.5rem;
  font-size: 0.72rem;
  border: 1px solid var(--studio-border);
  color: var(--studio-muted);
}

.spc-step.is-done {
  color: #a7f3d0;
  border-color: rgba(16, 185, 129, 0.24);
  background: rgba(16, 185, 129, 0.08);
}

.spc-step.is-active {
  color: #93c5fd;
  border-color: rgba(59, 130, 246, 0.24);
  background: rgba(59, 130, 246, 0.08);
}

.spc-summary,
.sdp-body {
  display: flex;
  flex-direction: column;
  gap: 0.9rem;
}

.spc-section,
.sdp-section {
  border: 1px solid var(--studio-border);
  border-radius: 12px;
  background: rgba(255,255,255,0.02);
  padding: 0.75rem;
}

.spc-label,
.sdp-section-title,
.sdp-debug-label {
  font-size: 0.76rem;
  font-weight: 600;
  color: var(--studio-muted);
  margin-bottom: 0.45rem;
}

.spc-points,
.sdp-list {
  margin: 0;
  padding-left: 1rem;
  display: grid;
  gap: 0.35rem;
  font-size: 0.8rem;
  line-height: 1.45;
}

.spc-layout-kind {
  font-size: 0.86rem;
  font-weight: 600;
  margin-bottom: 0.3rem;
}

.spc-layout-reason,
.spc-placeholder,
.sdp-text,
.sdp-note,
.sdp-empty-hint {
  margin: 0;
  font-size: 0.8rem;
  line-height: 1.5;
  color: var(--studio-muted);
}

.spc-preview {
  border-radius: 12px;
  overflow: hidden;
  border: 1px solid var(--studio-border);
  background: #0b1020;
  min-height: 140px;
}

.spc-preview-frame {
  height: 100%;
  aspect-ratio: 16 / 9;
  overflow: hidden;
}

.spc-preview-canvas {
  width: 1280px;
  height: 720px;
  transform: scale(0.2);
  transform-origin: top left;
}

.spc-preview-placeholder,
.sdp-preview-empty {
  min-height: 140px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 0.5rem;
  color: #94a3b8;
  padding: 1rem;
  text-align: center;
}

.spc-preview-placeholder span,
.sdp-preview-empty span {
  font-size: 1.4rem;
  opacity: 0.5;
}

.slide-detail-panel {
  overflow: auto;
}

.sdp-head,
.sdp-body {
  padding: 1rem;
}

.sdp-head {
  border-bottom: 1px solid var(--studio-border);
}

.sdp-kicker {
  font-size: 0.75rem;
  color: var(--studio-muted);
  text-transform: uppercase;
  letter-spacing: 0.08em;
}

.sdp-title {
  font-size: 1.1rem;
  font-weight: 700;
  margin-top: 0.3rem;
}

.sdp-meta {
  margin-top: 0.75rem;
  display: flex;
  gap: 0.5rem;
  flex-wrap: wrap;
}

.sdp-preview-shell {
  padding: 1rem;
  border-bottom: 1px solid var(--studio-border);
}

.sdp-preview-stage {
  width: 100%;
  aspect-ratio: 16 / 9;
  overflow: hidden;
  border-radius: 14px;
  border: 1px solid var(--studio-border);
  background: #0b1020;
}

.sdp-preview-canvas {
  width: 1280px;
  height: 720px;
  transform: scale(0.315);
  transform-origin: top left;
}

.sdp-note {
  margin-top: 0.5rem;
}

.sdp-debug {
  border: 1px solid var(--studio-border);
  border-radius: 12px;
  background: rgba(255,255,255,0.02);
  padding: 0.75rem;
}

.sdp-debug summary {
  cursor: pointer;
  font-size: 0.82rem;
  font-weight: 600;
}

.sdp-debug-block + .sdp-debug-block {
  margin-top: 0.75rem;
}

.sdp-debug pre {
  margin: 0;
  white-space: pre-wrap;
  word-break: break-word;
  font-size: 0.72rem;
  line-height: 1.55;
  color: #cbd5e1;
  background: rgba(2, 6, 23, 0.58);
  border: 1px solid rgba(148, 163, 184, 0.16);
  border-radius: 10px;
  padding: 0.75rem;
  max-height: 240px;
  overflow: auto;
}

.sdp-log-list {
  display: grid;
  gap: 0.65rem;
  margin-top: 0.75rem;
}

.sdp-log-item {
  padding: 0.65rem 0.75rem;
  border-radius: 10px;
  border: 1px solid var(--studio-border);
  background: rgba(255,255,255,0.02);
}

.sdp-log-meta {
  font-size: 0.72rem;
  color: var(--studio-muted);
  margin-bottom: 0.2rem;
}

.sdp-log-title {
  font-size: 0.8rem;
  line-height: 1.45;
}

@media (max-width: 1200px) {
  .slides-workbench {
    grid-template-columns: 1fr;
  }

  .slide-detail-panel {
    max-height: none;
  }
}

@media (max-width: 720px) {
  .slides-grid {
    grid-template-columns: 1fr;
    padding: 0.75rem;
  }

  .spc-top {
    align-items: flex-start;
    flex-direction: column;
  }
}
</style>
