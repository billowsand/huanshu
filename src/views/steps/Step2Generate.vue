<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import SlideFrame from '../../components/SlideFrame.vue'
import RetryLayoutDialog from '../../components/RetryLayoutDialog.vue'
import type { SlideBlueprint, AspectRatio, SlideKind } from '../../components/types'
import { ASPECT_DIMENSIONS } from '../../components/types'
import { useTypstThumbnails } from '../../composables/useTypstThumbnails'
import type { GenerationLogEntry } from '../../stores/generation'
import { PAGE_STAGES, pageStepState, resolveProgressStage, summarizeProgress } from '../../utils/generationProgress'
import type { PageProgressStatus, PipelineStage } from '../../utils/generationProgress'

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
  logs: GenerationLogEntry[]
  selectedSlideIndex: number | null
  pageStatuses: Map<number, PageProgressStatus>
  retryingPages: Set<number>
  runId?: number
  projectId?: number | null
}>()

const emit = defineEmits<{
  'update:selectedSlideIndex': [value: number | null]
  'go-to-step-3': []
  'go-to-step-1': []
  'retry': []
  'retry-page': [index: number, kind: SlideKind]
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

const PAGE_STAGE_META = PAGE_STAGES
const progressDetailsOpen = ref(false)
const retryLayoutDialog = ref<InstanceType<typeof RetryLayoutDialog> | null>(null)

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

const slideAspectRatio = computed<AspectRatio>(() => {
  return props.blueprints[0]?.aspect_ratio ?? 'ratio_16x9'
})
const slideDims = computed(() => ASPECT_DIMENSIONS[slideAspectRatio.value])

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

const slideItems = computed<SlideProgressItem[]>(() => {
  // Progress indices refer to content pages, while the assembled deck starts
  // with a cover and overview and ends with a closing slide.
  const contentBlueprints = props.blueprints[0]?.kind === 'cover'
    ? props.blueprints.slice(2, -1) : props.blueprints
  const total = Math.max(
    pagePlans.value.length,
    layoutPlans.value.length,
    ...Array.from(contentLogBySlideIndex.value.keys(), index => index + 1),
    ...Array.from(props.pageStatuses.keys(), index => index + 1),
    contentBlueprints.length,
  )

  return Array.from({ length: total }, (_, index) => {
    const contentLog = contentLogBySlideIndex.value.get(index) ?? null
    return {
      index,
      pagePlan: pagePlans.value[index] ?? extractPagePlanFromLog(contentLog),
      layoutPlan: layoutPlans.value[index] ?? extractLayoutPlanFromLog(contentLog),
      blueprint: contentBlueprints[index] ?? extractBlueprintFromLog(contentLog),
      contentLog,
    }
  })
})

// 生成看板的缩略图走 Typst 光栅化的 PNG：一次要展示很多页。直接用卡片自己的蓝图（含生成中的空位和
// 取自日志的蓝图），缩略图下标就是 `item.index`。必须定义在 `slideItems` 之后：组合函数初始化时
// 就会读取它。位图宽度取卡片 frame 的实测宽度 × dpr，减去边框（按舞台尺寸缩放后铺满 frame）。
const slidesGridRef = ref<HTMLElement | null>(null)
const gridFrameWidth = ref(0)
let gridResizeObserver: ResizeObserver | null = null

watch(slidesGridRef, (el) => {
  gridResizeObserver?.disconnect()
  gridResizeObserver = null
  if (!el) {
    gridFrameWidth.value = 0
    return
  }
  const measure = () => {
    const frame = el.querySelector<HTMLElement>('.spc-preview-frame')
    if (frame) gridFrameWidth.value = frame.clientWidth
  }
  gridResizeObserver = new ResizeObserver(measure)
  gridResizeObserver.observe(el)
  measure()
})

/** 卡片预览位的栅格宽度：还没量到时按 260px（`.slides-grid` 的最小列宽）估一个。 */
const cardPreviewWidth = computed(() => {
  const frame = gridFrameWidth.value || 260
  return Math.max(Math.round((frame - 2) * slideDims.value.w / slideDims.value.h), 1)
})

const thumbnails = useTypstThumbnails({
  slides: () => slideItems.value.map(item => item.blueprint ?? null),
  projectId: () => props.projectId ?? null,
  widthPx: () => Math.ceil(cardPreviewWidth.value * (window.devicePixelRatio || 1)),
})

/** 该页的 Typst 位图；还没有图时返回 null，由加载框占位。 */
function thumbnailFor(index: number) {
  return thumbnails.thumbs.value[index] ?? null
}

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

function moveSelectedSlide(delta: -1 | 1) {
  if (!slideItems.value.length)
    return
  const currentIndex = props.selectedSlideIndex ?? preferredSlideIndex() ?? 0
  const nextIndex = Math.min(Math.max(currentIndex + delta, 0), slideItems.value.length - 1)
  if (nextIndex !== currentIndex)
    selectSlide(nextIndex)
}

function shouldIgnoreKeydown(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement))
    return false
  if (target.isContentEditable)
    return true
  const tagName = target.tagName
  return tagName === 'INPUT' || tagName === 'TEXTAREA' || tagName === 'SELECT'
}

function handleStepKeydown(event: KeyboardEvent) {
  if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey)
    return
  if (shouldIgnoreKeydown(event.target))
    return

  if (event.key === 'ArrowUp') {
    event.preventDefault()
    moveSelectedSlide(-1)
  } else if (event.key === 'ArrowDown') {
    event.preventDefault()
    moveSelectedSlide(1)
  }
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
  if (props.retryingPages.has(item.index))
    return { tone: 'active', label: props.running ? '等待重试' : '重试中' }
  if (props.stage === 'error') {
    // Check if this specific page had an error
    const ps = props.pageStatuses.get(item.index)
    if (ps?.stage === 'error')
      return { tone: 'error', label: '生成失败' }
    if (item.blueprint)
      return { tone: 'done', label: '已完成' }
    return { tone: 'error', label: '生成中断' }
  }

  const ps = props.pageStatuses.get(item.index)
  const stage = resolveProgressStage(item, ps)
  if (stage === 'done') return { tone: 'done', label: '已完成' }
  if (stage === 'error') return { tone: 'error', label: '生成失败' }
  if (!props.running && stage !== 'pending') return { tone: 'pending', label: '已中断' }
  if (ps?.waiting || stage === 'pending') return { tone: 'pending', label: ps?.message || '等待开始' }
  const label = PAGE_STAGES.find(s => s.key === stage)?.label
  return { tone: 'active', label: `${label ?? '生成'}中` }
}

function stageStepState(item: SlideProgressItem, key: PipelineStage) {
  return pageStepState(item, props.pageStatuses.get(item.index), key, props.running || props.retryingPages.size > 0)
}

const progressSummary = computed(() => summarizeProgress(slideItems.value, props.pageStatuses, props.running || props.retryingPages.size > 0))

function selectedDebugText(key: string): string {
  const detail = selectedSlide.value?.contentLog?.detail
  if (!isRecord(detail))
    return ''
  return detailText(detail[key])
}

const detailStageRef = ref<HTMLElement | null>(null)
const detailStageWidth = ref(0)
let detailResizeObserver: ResizeObserver | null = null

watch(detailStageRef, (el) => {
  if (detailResizeObserver) {
    detailResizeObserver.disconnect()
    detailResizeObserver = null
  }
  if (el) {
    detailResizeObserver = new ResizeObserver(entries => {
      for (const entry of entries) {
        detailStageWidth.value = entry.contentRect.width
      }
    })
    detailResizeObserver.observe(el)
  } else {
    detailStageWidth.value = 0
  }
})

/** 详情大图要覆盖的 CSS 宽度，取详情舞台的实测宽度（还没量到时按 16:9 的画布原宽估）。 */
function detailStageSize() {
  return { width: detailStageWidth.value || slideDims.value.w }
}

// 右侧详情大图同样改成 Typst 位图：牌组与看板相同（`slideItems` 的蓝图，含生成中的空位），
// 只请求选中这一页，缓存上限 4 张。
const detailThumbnails = useTypstThumbnails({
  slides: () => slideItems.value.map(item => item.blueprint ?? null),
  projectId: () => props.projectId ?? null,
  widthPx: () => Math.ceil(detailStageSize().width * (window.devicePixelRatio || 1)),
  wanted: () => (props.selectedSlideIndex === null ? [] : [props.selectedSlideIndex]),
  maxCached: 4,
})

/** 选中页的 Typst 位图；还没有图时返回 null，由加载框占位。 */
function detailThumbnail() {
  const index = props.selectedSlideIndex
  if (index === null)
    return null
  return detailThumbnails.thumbs.value[index] ?? null
}

onMounted(() => {
  window.addEventListener('keydown', handleStepKeydown)
})
onBeforeUnmount(() => {
  window.removeEventListener('keydown', handleStepKeydown)
  if (detailResizeObserver) {
    detailResizeObserver.disconnect()
    detailResizeObserver = null
  }
  if (gridResizeObserver) {
    gridResizeObserver.disconnect()
    gridResizeObserver = null
  }
})
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
          <span class="gen-stat-lbl">生成记录</span>
        </div>
      </div>
    </div>

    <section class="gen-progress-overview" aria-label="幻灯片生成进度">
      <div class="gpo-summary">
        <div class="gpo-main">
          <span class="gpo-title">整体进度</span>
          <span class="gpo-meta">已完成 {{ progressSummary.done }}/{{ progressSummary.total }}</span>
          <span v-if="progressSummary.active" class="gpo-meta">进行中 {{ progressSummary.active }}</span>
          <span v-if="progressSummary.error" class="gpo-meta gpo-error">失败 {{ progressSummary.error }}</span>
          <span v-if="progressSummary.interrupted" class="gpo-meta">已中断 {{ progressSummary.interrupted }}</span>
        </div>
        <div class="gpo-actions">
          <span class="gpo-percent">{{ progressSummary.percent }}%</span>
          <button type="button" class="gpo-toggle" :aria-expanded="progressDetailsOpen" aria-controls="generation-progress-details" @click="progressDetailsOpen = !progressDetailsOpen">
            {{ progressDetailsOpen ? '收起详情' : '查看详情' }}
            <span class="i-carbon:chevron-down" :class="{ 'is-open': progressDetailsOpen }" />
          </button>
        </div>
      </div>
      <div class="gpo-bar" role="progressbar" aria-label="已完成幻灯片比例" :aria-valuenow="progressSummary.percent" :aria-valuetext="`已完成 ${progressSummary.done}/${progressSummary.total}`" :aria-valuemin="0" :aria-valuemax="100">
        <span class="gpo-bar-fill" :style="{ width: progressSummary.doneWidth }" />
      </div>
      <ol class="gpo-stages" aria-label="生成阶段">
        <li v-for="stage in progressSummary.stageCards" :key="stage.key" class="gpo-stage" :class="`is-${stage.state}`">
          <span class="gpo-stage-icon" aria-hidden="true"><span v-if="stage.state === 'done'" class="i-carbon:checkmark" /><span v-else-if="stage.state === 'active'" class="gpo-stage-dot" /></span>
          <div class="gpo-stage-body">
            <div class="gpo-stage-heading"><span>{{ stage.label }}</span><span v-if="stage.active" class="gpo-stage-badge">进行中</span><span v-else-if="stage.state === 'done'" class="sr-only">已完成</span></div>
            <span v-if="stage.completed || stage.active" class="gpo-stage-meta">{{ stage.completed }}/{{ progressSummary.total }}</span>
          </div>
        </li>
      </ol>
      <div v-if="progressDetailsOpen" id="generation-progress-details" class="gpo-details">
        <p>完成数仅统计已通过该阶段的页面；整体进度按全部阶段完成的页面计算。</p>
        <p>共 {{ progressSummary.total }} 页 · 已完成 {{ progressSummary.done }} · 进行中 {{ progressSummary.active }} · 等待 {{ progressSummary.pending }} · 失败 {{ progressSummary.error }} · 已中断 {{ progressSummary.interrupted }}</p>
        <div class="gpo-detail-table" role="table" aria-label="各阶段进度">
          <div class="gpo-detail-row" role="row"><span role="columnheader">阶段</span><span role="columnheader">已完成</span><span role="columnheader">进行中</span><span role="columnheader">完成比例</span></div>
          <div v-for="stage in progressSummary.stageCards" :key="stage.key" class="gpo-detail-row" role="row"><span role="cell">{{ stage.label }}</span><span role="cell">{{ stage.completed }}/{{ progressSummary.total }}</span><span role="cell">{{ stage.active }}</span><span role="cell">{{ stage.percent }}%</span></div>
        </div>
      </div>
    </section>

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

        <div v-else ref="slidesGridRef" class="slides-grid">
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
              <span class="spc-step" :class="`is-${stageStepState(item, 'planning')}`">规划</span>
              <span class="spc-step" :class="`is-${stageStepState(item, 'layout')}`">布局</span>
              <span class="spc-step" :class="`is-${stageStepState(item, 'content')}`">内容</span>
              <span class="spc-step" :class="`is-${stageStepState(item, 'normalizing')}`">规范</span>
              <span class="spc-step" :class="`is-${stageStepState(item, 'validating')}`">校验</span>
            </div>

            <div class="spc-summary">
              <span v-if="item.layoutPlan" class="spc-layout-tag">{{ kindLabel(item.blueprint?.kind ?? item.layoutPlan.kind) }}</span>
              <span v-else class="spc-layout-tag spc-layout-tag--pending">等待布局排版结果</span>
            </div>

            <div class="spc-preview">
              <div
                v-if="item.blueprint"
                class="spc-preview-frame"
                :style="{ aspectRatio: `${slideDims.w} / ${slideDims.h}` }"
              >
                <SlideFrame
                  :src="thumbnailFor(item.index)?.url ?? null"
                  :pending="thumbnails.pending.value"
                  :error="thumbnails.failed.value"
                  alt=""
                  @retry="thumbnails.refresh()"
                />
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
              <span v-if="selectedSlide.layoutPlan?.kind" class="sdp-chip sdp-chip--accent">{{ kindLabel(selectedSlide.blueprint?.kind ?? selectedSlide.layoutPlan.kind) }}</span>
              <span v-if="selectedSlide.pagePlan?.section_title" class="sdp-chip">{{ selectedSlide.pagePlan.section_title }}</span>
            </div>
          </div>

          <div class="sdp-preview-shell">
            <div
              v-if="selectedSlide.blueprint"
              ref="detailStageRef"
              class="sdp-preview-stage"
              :style="{ aspectRatio: `${slideDims.w} / ${slideDims.h}` }"
            >
              <!--
                Typst 位图直接铺满舞台（舞台已按页面比例定高），不放进带 `transform: scale()` 的
                画布里：那里的 CSS 尺寸会再被缩放一次，图只剩左上角一小块。
              -->
              <SlideFrame
                :src="detailThumbnail()?.url ?? null"
                :pending="detailThumbnails.pending.value"
                :error="detailThumbnails.failed.value"
                alt=""
                @retry="detailThumbnails.refresh()"
              />
            </div>
            <div v-else class="sdp-preview-empty">
              <span class="i-carbon:image-search" />
              <p>当前页尚未生成最终内容。</p>
            </div>
          </div>

          <div class="sdp-body">
            <section v-if="props.pageStatuses.get(selectedSlide.index)?.stage === 'error' || props.retryingPages.has(selectedSlide.index)" class="sdp-section">
              <div class="sdp-section-title">生成失败，可单独重试</div>
              <p class="sdp-text" role="status">{{ props.retryingPages.has(selectedSlide.index) ? (props.running ? '已排队，其他页面生成结束后重试。' : '正在重新生成这一页…') : props.pageStatuses.get(selectedSlide.index)?.message }}</p>
              <button type="button" class="btn btn-primary" :disabled="props.retryingPages.has(selectedSlide.index) || (!props.running && props.retryingPages.size > 0)" @click="retryLayoutDialog?.open(props.runId, selectedSlide.index)">
                <span class="i-carbon:renew" /> {{ props.retryingPages.has(selectedSlide.index) ? '重试处理中' : '重试这一页' }}
              </button>
              <p v-if="selectedSlide.blueprint" class="sdp-note">当前保留简化内容，可继续编辑，也可重试生成。</p>
            </section>
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
                <strong>{{ kindLabel(selectedSlide.blueprint?.kind ?? selectedSlide.layoutPlan.kind) }}</strong>
                <span v-if="selectedSlide.blueprint && selectedSlide.blueprint.kind !== selectedSlide.layoutPlan.kind"> · 已调整布局</span>
                <span v-else-if="selectedSlide.layoutPlan.reason"> · {{ selectedSlide.layoutPlan.reason }}</span>
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
      <button v-if="props.stage === 'done'" class="btn btn-primary" :disabled="props.retryingPages.size > 0" @click="$emit('go-to-step-3')">
        <span class="i-carbon:edit" /> 前往编辑
      </button>
      <button v-if="props.stage === 'error'" class="btn" @click="$emit('go-to-step-1')">
        <span class="i-carbon:arrow-left" /> 返回修改
      </button>
      <button v-if="props.stage === 'error'" class="btn btn-primary" :disabled="props.retryingPages.size > 0" @click="$emit('retry')">
        <span class="i-carbon:renew" /> 重新生成全部
      </button>
    </div>
  </div>
    <RetryLayoutDialog ref="retryLayoutDialog" @confirm="(index, kind) => emit('retry-page', index, kind)" />
</template>

<style scoped>
.gen-progress-overview { border: 1px solid var(--studio-border); border-radius: 14px; background: var(--studio-surface); padding: 0.9rem 1.1rem; margin-bottom: 1rem; flex-shrink: 0; }
.gpo-summary, .gpo-main, .gpo-actions { display: flex; align-items: center; gap: 1rem; }
.gpo-summary { justify-content: space-between; }
.gpo-main { min-width: 0; flex-wrap: wrap; gap: 0.45rem 1rem; }
.gpo-title { font-size: 0.88rem; font-weight: 700; }
.gpo-meta, .gpo-toggle { font-size: 0.78rem; color: var(--studio-muted); }
.gpo-error { color: var(--studio-error); }
.gpo-actions { flex-shrink: 0; }
.gpo-percent { font-size: 1.3rem; font-weight: 700; font-variant-numeric: tabular-nums; }
.gpo-toggle { display: flex; align-items: center; gap: 0.4rem; background: transparent; border: 0; border-left: 1px solid var(--studio-border); padding: 0.3rem 0 0.3rem 1rem; cursor: pointer; }
.gpo-toggle:hover { color: var(--studio-text); }
.gpo-toggle:focus-visible { outline: 2px solid var(--studio-primary); outline-offset: 4px; border-radius: 3px; }
.gpo-toggle .is-open { transform: rotate(180deg); }
.gpo-bar { margin-top: 0.6rem; height: 4px; border-radius: 999px; overflow: hidden; background: var(--studio-border); }
.gpo-bar-fill { display: block; height: 100%; background: var(--studio-primary); transition: width 0.2s ease; }
.gpo-stages { list-style: none; margin: 0.85rem 0 0; padding: 0; display: flex; gap: 1rem; }
.gpo-stage { display: flex; align-items: center; gap: 0.6rem; flex: 1; min-width: 0; color: var(--studio-muted); }
.gpo-stage:not(:last-child)::after { content: ''; flex: 1; min-width: 12px; height: 1px; margin-left: 0.25rem; background: var(--studio-border); }
.gpo-stage-icon { width: 24px; height: 24px; display: flex; align-items: center; justify-content: center; border: 1.5px solid var(--studio-border-hover); border-radius: 50%; flex-shrink: 0; }
.gpo-stage.is-done .gpo-stage-icon { background: var(--studio-success); color: var(--studio-surface); border-color: var(--studio-success); }
.gpo-stage.is-active .gpo-stage-icon { border-color: #3b82f6; color: #3b82f6; }
.gpo-stage-dot { width: 12px; height: 12px; border-radius: 50%; background: currentColor; }
.gpo-stage.is-done, .gpo-stage.is-active { color: var(--studio-text); }
.gpo-stage-body { flex-shrink: 0; }
.gpo-stage-heading { display: flex; align-items: center; gap: 0.4rem; font-size: 0.8rem; white-space: nowrap; }
.gpo-stage-badge { font-size: 0.65rem; color: #3b82f6; background: rgba(59,130,246,0.1); border-radius: 5px; padding: 0.12rem 0.3rem; }
.gpo-stage-meta { display: block; margin-top: 0.16rem; font-size: 0.72rem; color: var(--studio-muted); font-variant-numeric: tabular-nums; }
.gpo-details { margin-top: 0.85rem; border-top: 1px solid var(--studio-border); padding-top: 0.6rem; color: var(--studio-muted); font-size: 0.75rem; }
.gpo-details p { margin: 0.3rem 0 0.6rem; }
.gpo-detail-row { display: grid; grid-template-columns: 1.5fr repeat(3, 1fr); gap: 0.5rem; padding: 0.4rem 0; font-variant-numeric: tabular-nums; }
.gpo-detail-row:first-child { font-weight: 600; color: var(--studio-text); }
.sr-only { position: absolute; width: 1px; height: 1px; padding: 0; overflow: hidden; clip: rect(0, 0, 0, 0); white-space: nowrap; }
@media (max-width: 1050px) { .gpo-stages { overflow-x: auto; padding-bottom: 0.25rem; } .gpo-stage { flex: 0 0 auto; } .gpo-stage:not(:last-child)::after { width: 20px; } }

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
  align-items: start;
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
  align-items: flex-start;
  gap: 0.75rem;
}

.spc-num {
  font-size: 0.76rem;
  color: var(--studio-muted);
  font-variant-numeric: tabular-nums;
  flex-shrink: 0;
}

.spc-head {
  min-width: 0;
  flex: 1;
}

.spc-title {
  font-size: 0.95rem;
  font-weight: 600;
  line-height: 1.35;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  min-width: 0;
  flex: 1;
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

.spc-status {
  flex-shrink: 0;
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

.spc-summary {
  flex: 0 0 auto;
  flex-direction: row;
  align-items: center;
  gap: 0.5rem;
}

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

.spc-layout-tag {
  display: inline-flex;
  align-items: center;
  max-width: 100%;
  min-width: 0;
  border-radius: 999px;
  border: 1px solid rgba(59, 130, 246, 0.24);
  background: rgba(59, 130, 246, 0.1);
  color: #93c5fd;
  padding: 0.24rem 0.65rem;
  font-size: 0.75rem;
  line-height: 1.2;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.spc-layout-tag--pending {
  border-color: var(--studio-border);
  background: rgba(255,255,255,0.03);
  color: var(--studio-muted);
}

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
  margin-top: 0.2rem;
}

.spc-preview-frame {
  width: 100%;
  overflow: hidden;
  border-radius: 12px;
  border: 1px solid var(--studio-border);
  background: var(--studio-bg);
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
  border-radius: 12px;
  border: 1px solid var(--studio-border);
  background: var(--studio-bg);
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
  overflow: hidden;
  border-radius: 14px;
  border: 1px solid var(--studio-border);
  background: var(--studio-bg);
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
  .gpo-summary {
    align-items: flex-start;
    flex-direction: column;
  }

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
