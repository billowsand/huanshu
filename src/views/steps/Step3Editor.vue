// 编辑规则已移植到 huanshu_core::slide_edit（crates/huanshu-core/src/slide_edit.rs），改一边必须改另一边；
// 一致性由 crates/huanshu-core/tests/slide_edit.rs 快照守住。
<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import SlideFrame from '../../components/SlideFrame.vue'
import { useEditorSchemaStore } from '../../stores/editorSchema'
import { useTypstThumbnails } from '../../composables/useTypstThumbnails'
import type { SlideBlueprint, SlideKind } from '../../components/types'
import type { MediaItem } from '../../composables/useMediaLibrary'

// 版式名称、字段、默认内容与表单定义都来自 Rust（`get_editor_schema`）。
const schemaStore = useEditorSchemaStore()
const schemaReady = computed(() => schemaStore.ready)
const schemaError = computed(() => schemaStore.error)
const KIND_META = computed(() => schemaStore.kindMeta)
const ALL_KINDS = computed(() => schemaStore.schema?.kinds.map((entry) => entry.kind) ?? [])
const KIND_FIELDS = computed(() => schemaStore.kindFields)
const SIMPLE_FIELDS = computed(() => schemaStore.simpleFields)
const LIST_SECTIONS = computed(() => schemaStore.listSections)
const OBJECT_SECTIONS = computed(() => schemaStore.objectSections)
const TONE_OPTIONS = computed(() => schemaStore.toneOptions)
const DIRECTION_OPTIONS = computed(() => schemaStore.directionOptions)

const props = defineProps<{
  activeSlide: number
  slideCount: number
  projectId: number | null
  blueprints: SlideBlueprint[]
  mediaItems: MediaItem[]
  jsonText: string
  jsonError: string
  liveSlide: SlideBlueprint | null
  hasUnsavedChanges: boolean
  editorSaving: boolean
  showKindPicker: boolean
  kindPickerMode: 'add' | 'change'
  previewArea: HTMLElement | null
  editorKind: SlideKind
  editorPreviewSlide: SlideBlueprint | null
  editorStageW: number
  editorStageH: number
  jsonTextareaRef: HTMLTextAreaElement | null
  exportingPdf?: boolean
}>()

const emit = defineEmits<{
  'update:activeSlide': [value: number]
  'replace-draft': [slide: SlideBlueprint]
  'json-textarea-ref': [el: HTMLTextAreaElement | null]
  'preview-area-ref': [el: HTMLElement | null]
  'format-json': []
  'apply-changes': []
  'open-kind-picker': [mode: 'add' | 'change']
  'kind-picked': [kind: SlideKind]
  'delete-slide': [idx: number]
  'move-slide': [dir: -1 | 1]
  'insert-media-url': [ref: string]
  'start-presentation': []
  'export-pdf': []
  'go-to-step-1': []
  'textarea-input': [value: string]
  'textarea-keydown': [event: KeyboardEvent]
}>()

function tryParseJson(text: string): Record<string, unknown> | null {
  try { return JSON.parse(text) } catch { return null }
}

function deepClone<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T
}

function parsePath(path: string): Array<string | number> {
  const out: Array<string | number> = []
  const regex = /([^[.\]]+)|\[(\d+)\]/g
  let match: RegExpExecArray | null
  while ((match = regex.exec(path))) {
    if (match[1]) out.push(match[1])
    else out.push(Number(match[2]))
  }
  return out
}

function getAtPath(source: unknown, path: string): unknown {
  return parsePath(path).reduce<unknown>((acc, segment) => {
    if (acc == null) return undefined
    return (acc as Record<string, unknown>)[segment as keyof typeof acc]
  }, source)
}

function setAtPath(target: Record<string, unknown> | unknown[], path: string, value: unknown) {
  const parts = parsePath(path)
  let cursor: Record<string, unknown> | unknown[] = target
  for (let i = 0; i < parts.length - 1; i += 1) {
    const part = parts[i]
    const next = parts[i + 1]
    const current = (cursor as Record<string, unknown>)[part as keyof typeof cursor]
    if (current == null) {
      ;(cursor as Record<string, unknown>)[part as keyof typeof cursor] = typeof next === 'number' ? [] : {}
    }
    cursor = (cursor as Record<string, unknown>)[part as keyof typeof cursor] as Record<string, unknown> | unknown[]
  }
  const last = parts[parts.length - 1]
  ;(cursor as Record<string, unknown>)[last as keyof typeof cursor] = value
}

const parsedJson = computed(() => tryParseJson(props.jsonText))
const currentDraft = computed<SlideBlueprint | null>(() => {
  if (props.liveSlide) return props.liveSlide
  return parsedJson.value as SlideBlueprint | null
})

const gutterRef = ref<HTMLElement | null>(null)
const lineCount = computed(() => props.jsonText.split('\n').length)
const advancedMode = ref(false)
const iconLoadingKey = ref('')
const iconSuggestions = ref<Record<string, string[]>>({})

const errorLine = computed<number | null>(() => {
  if (!props.jsonError) return null
  const msg = props.jsonError
  const lineMatch = msg.match(/line (\d+)/i)
  if (lineMatch) return parseInt(lineMatch[1], 10)
  const posMatch = msg.match(/position (\d+)/i)
  if (posMatch) {
    const pos = parseInt(posMatch[1], 10)
    return props.jsonText.slice(0, pos).split('\n').length
  }
  return null
})

const syncSemanticErrors = computed<string[]>(() => {
  const obj = parsedJson.value
  if (!obj || props.jsonError) return []
  const errors: string[] = []

  const direction = obj.direction
  if (typeof direction === 'string' && !schemaStore.directionOptions.includes(direction)) {
    errors.push(`direction "${direction}" 无效，应为 horizontal 或 vertical`)
  }

  function walkTone(node: unknown, path: string) {
    if (!node || typeof node !== 'object') return
    if (Array.isArray(node)) {
      node.forEach((item, index) => walkTone(item, `${path}[${index}]`))
      return
    }
    for (const [key, value] of Object.entries(node as Record<string, unknown>)) {
      if (key === 'tone' || key === 'footer_tone' || key === 'label_tone') {
        if (typeof value === 'string' && !schemaStore.toneOptions.includes(value)) {
          errors.push(`${path || 'root'}.${key} "${value}" 无效`)
        }
      } else {
        walkTone(value, path ? `${path}.${key}` : key)
      }
    }
  }

  walkTone(obj, '')

  if (obj.kind === 'chart_focus' && obj.chart == null) {
    errors.push('chart_focus 页缺少 chart（图表数据）')
  }
  return errors
})

/** 图表的语义错误来自 Rust（`chart_problems` 命令），随输入防抖请求。 */
const advancedChartErrors = ref<string[]>([])
let advancedChartSeq = 0
let advancedChartTimer: ReturnType<typeof setTimeout> | null = null

watch(
  () => (parsedJson.value?.kind === 'chart_focus' ? parsedJson.value.chart : null),
  (chart) => {
    if (advancedChartTimer !== null) clearTimeout(advancedChartTimer)
    advancedChartSeq += 1
    if (chart == null || chart === undefined) {
      advancedChartErrors.value = []
      return
    }
    const seq = advancedChartSeq
    advancedChartTimer = setTimeout(async () => {
      advancedChartTimer = null
      let problems: string[]
      try {
        problems = await invoke<string[]>('chart_problems', { chart })
      } catch (e) {
        problems = [`校验失败：${e instanceof Error ? e.message : String(e)}`]
      }
      if (seq !== advancedChartSeq) return
      advancedChartErrors.value = problems.map((problem) => `chart：${problem}`)
    }, 150)
  },
  { deep: true },
)

const semanticErrors = computed<string[]>(() => [...syncSemanticErrors.value, ...advancedChartErrors.value])


const simpleFields = computed(() => {
  const allowed = new Set(schemaStore.kindFields[props.editorKind] ?? [])
  return schemaStore.simpleFields.filter((field) => allowed.has(field.key))
})

const listSections = computed(() => schemaStore.listSections[props.editorKind] ?? [])
const objectSections = computed(() => schemaStore.objectSections[props.editorKind] ?? [])
const showBadges = computed(() =>
  (schemaStore.kindFields[props.editorKind] ?? []).includes('badges'),
)
const showImages = computed(() =>
  (schemaStore.kindFields[props.editorKind] ?? []).includes('images'),
)

function syncGutterScroll(event: Event) {
  if (gutterRef.value) {
    gutterRef.value.scrollTop = (event.target as HTMLTextAreaElement).scrollTop
  }
}

function replaceDraft(updated: SlideBlueprint) {
  emit('replace-draft', deepClone(updated))
}

function cloneDraft(): SlideBlueprint | null {
  return currentDraft.value ? deepClone(currentDraft.value) : null
}

function updatePath(path: string, value: unknown) {
  const draft = cloneDraft()
  if (!draft) return
  setAtPath(draft as unknown as Record<string, unknown>, path, value)
  replaceDraft(draft)
}

// ── chart_focus 图表数据：整块 JSON 编辑 ──────────────────────────────────────
// 只有语法正确且通过校验（规则在 Rust 侧，见 `huanshu_core::chart_check`）时才写回牌组，
// 输入中途的半截 JSON 不会把正在预览的图表弄坏。编辑框获得焦点期间不被外部改动覆盖。
// 校验是一次命令调用：输入后 150ms 防抖，期间不写回、也不显示「校验通过」；
// 只有「最新一次输入」的结果才生效，早到的旧请求直接丢弃。
const chartJsonText = ref('')
const chartJsonError = ref('')
const chartJsonFocused = ref(false)
/** 校验进行中：等待期间既不显示通过，也不写回牌组。 */
const chartChecking = ref(false)
let chartCheckSeq = 0
let chartCheckTimer: ReturnType<typeof setTimeout> | null = null
let pendingChart: unknown = null
let pendingSlide = -1

function chartJsonFromDraft() {
  const chart = getFieldValue('chart')
  return chart == null ? '' : JSON.stringify(chart, null, 2)
}

watch(
  () => [props.editorKind, getFieldValue('chart')],
  () => {
    if (chartJsonFocused.value) return
    chartJsonText.value = chartJsonFromDraft()
    chartJsonError.value = ''
  },
  { deep: true, immediate: true },
)

function onChartJsonInput(text: string) {
  chartJsonText.value = text
  let parsed: unknown
  try {
    parsed = JSON.parse(text)
  } catch (e) {
    cancelChartCheck()
    chartJsonError.value = `JSON 语法错误：${e instanceof Error ? e.message : String(e)}`
    return
  }
  scheduleChartCheck(parsed)
}

function cancelChartCheck() {
  if (chartCheckTimer !== null) {
    clearTimeout(chartCheckTimer)
    chartCheckTimer = null
  }
  chartChecking.value = false
  // 作废在途请求：它的结果回来时序号已经不匹配。
  chartCheckSeq += 1
}

function scheduleChartCheck(chart: unknown) {
  if (chartCheckTimer !== null) clearTimeout(chartCheckTimer)
  chartChecking.value = true
  chartJsonError.value = ''
  const seq = ++chartCheckSeq
  pendingChart = chart
  pendingSlide = props.activeSlide
  const slide = pendingSlide
  chartCheckTimer = setTimeout(() => runChartCheck(seq, chart, slide), 150)
}

/** 跳过防抖、立刻校验还在等待的输入（失焦时用：粘贴后马上点别处不能丢掉这次修改）。 */
function flushChartCheck() {
  if (chartCheckTimer === null) return
  clearTimeout(chartCheckTimer)
  runChartCheck(chartCheckSeq, pendingChart, pendingSlide)
}

/** `slide`：输入时所在的页。结果只能写回那一页，换页后迟到的结果直接丢弃。 */
async function runChartCheck(seq: number, chart: unknown, slide: number) {
  chartCheckTimer = null
  let problems: string[]
  try {
    problems = await invoke<string[]>('chart_problems', { chart })
  } catch (e) {
    if (seq !== chartCheckSeq) return
    chartChecking.value = false
    chartJsonError.value = `校验失败：${e instanceof Error ? e.message : String(e)}`
    return
  }
  if (seq !== chartCheckSeq) return
  chartChecking.value = false
  if (slide !== props.activeSlide) return
  if (problems.length) {
    chartJsonError.value = problems.join('；')
    return
  }
  chartJsonError.value = ''
  updatePath('chart', chart)
  // 已失焦：把输入规范化成牌组里的实际内容（内容没变时上面的 watch 不会触发）。
  if (!chartJsonFocused.value) chartJsonText.value = chartJsonFromDraft()
}

function onChartJsonBlur() {
  chartJsonFocused.value = false
  // 还在校验：等结果回来再决定写回与规范化（见 runChartCheck）。
  if (chartChecking.value) {
    flushChartCheck()
    return
  }
  // 离开时若仍有错误，保留输入让用户继续改；否则规范化成牌组里的实际内容
  if (!chartJsonError.value) chartJsonText.value = chartJsonFromDraft()
}

function getFieldValue(path: string): unknown {
  return currentDraft.value ? getAtPath(currentDraft.value, path) : undefined
}

function getStringValue(path: string): string {
  const value = getFieldValue(path)
  return typeof value === 'string' ? value : ''
}

function getBooleanValue(path: string): boolean {
  return Boolean(getFieldValue(path))
}

function getArrayValue(path: string): unknown[] {
  const value = getFieldValue(path)
  return Array.isArray(value) ? value : []
}

/** `template` 来自 schema 的 `new_item`：Rust 已经算好了「新增一项」的内容。 */
function addItem(path: string, template: Record<string, unknown>) {
  const draft = cloneDraft()
  if (!draft) return
  const items = getArrayValue(path)
  setAtPath(draft as unknown as Record<string, unknown>, path, [...items, deepClone(template)])
  replaceDraft(draft)
}

function removeItem(path: string, index: number) {
  const draft = cloneDraft()
  if (!draft) return
  const items = getArrayValue(path).filter((_, currentIndex) => currentIndex !== index)
  setAtPath(draft as unknown as Record<string, unknown>, path, items)
  replaceDraft(draft)
}

function moveItem(path: string, index: number, dir: -1 | 1) {
  const draft = cloneDraft()
  if (!draft) return
  const items = [...getArrayValue(path)]
  const nextIndex = index + dir
  if (nextIndex < 0 || nextIndex >= items.length) return
  ;[items[index], items[nextIndex]] = [items[nextIndex], items[index]]
  setAtPath(draft as unknown as Record<string, unknown>, path, items)
  replaceDraft(draft)
}

function addStringItem(path: string) {
  const draft = cloneDraft()
  if (!draft) return
  const items = [...getArrayValue(path), '']
  setAtPath(draft as unknown as Record<string, unknown>, path, items)
  replaceDraft(draft)
}

function updateStringItem(path: string, index: number, value: string) {
  const draft = cloneDraft()
  if (!draft) return
  const items = [...getArrayValue(path)]
  items[index] = value
  setAtPath(draft as unknown as Record<string, unknown>, path, items)
  replaceDraft(draft)
}

function removeStringItem(path: string, index: number) {
  const draft = cloneDraft()
  if (!draft) return
  const items = getArrayValue(path).filter((_, currentIndex) => currentIndex !== index)
  setAtPath(draft as unknown as Record<string, unknown>, path, items)
  replaceDraft(draft)
}

function mediaCandidates() {
  return props.mediaItems.filter((item) => item.type === 'image')
}

function setPrimaryImage(ref: string) {
  updatePath('image', ref)
}

function appendGalleryImage(ref: string) {
  const draft = cloneDraft()
  if (!draft) return
  const images = getArrayValue('images').map((item) => String(item))
  if (!images.includes(ref)) images.push(ref)
  setAtPath(draft as unknown as Record<string, unknown>, 'images', images)
  replaceDraft(draft)
}

function buildIconQuery(basePath: string, keys: string[]) {
  return keys
    .map((key) => getStringValue(`${basePath}.${key}`).trim())
    .filter(Boolean)
    .join(' ')
}

async function recommendIcon(basePath: string, keys: string[]) {
  const query = buildIconQuery(basePath, keys)
  if (!query) return
  iconLoadingKey.value = basePath
  try {
    const candidates = await invoke<string[]>('recommend_icons_for_query', { query, limit: 6 })
    iconSuggestions.value = { ...iconSuggestions.value, [basePath]: candidates }
    if (candidates[0]) updatePath(`${basePath}.icon`, candidates[0])
  } catch {
    iconSuggestions.value = { ...iconSuggestions.value, [basePath]: [] }
  } finally {
    iconLoadingKey.value = ''
  }
}

function applySuggestedIcon(basePath: string, icon: string) {
  updatePath(`${basePath}.icon`, icon)
}

function setActiveSlide(index: number) {
  if (index < 0 || index >= props.slideCount || index === props.activeSlide)
    return
  emit('update:activeSlide', index)
}

// ── Typst 预览位图 ────────────────────────────────────────────────────────────
// 编辑器的预览走 Typst 光栅化的 PNG（与缩略图、演示主画面同一条链路）。
// **只发当前这一页**：版式只依赖本页数据，单页编译最快（溢出标记的下标是 0，取图时换算回
// `activeSlide`）。打字时每敲一个字内容就变，缓存帮不上忙，所以用 `serial`（同一时刻只有一个
// 请求在途）把连续输入合并成一次尾部请求，再用 `progressive` 让每个响应只要比画面新就先落下：
// 预览最多落后一个请求的时间，停手后一定落在最新内容上。

const typstPreview = useTypstThumbnails({
  slides: () => (props.editorPreviewSlide ? [props.editorPreviewSlide] : []),
  projectId: () => props.projectId ?? null,
  widthPx: () => Math.ceil(Math.max(props.editorStageW, 1) * (window.devicePixelRatio || 1)),
  // 换页就是换目标：上一页的在途响应作废，图清空，界面显示加载框。
  resetKey: () => props.activeSlide,
  serial: true,
  progressive: true,
  debounceMs: 120,
})

/** 这一页的 Typst 位图；还没有图时返回 null，由加载框占位。 */
const typstPreviewImage = computed(() => {
  // 只发这一页，所以它在数组里的下标恒为 0（`activeSlide` 是牌组页序，不是这里的键）。
  return typstPreview.thumbs.value[0] ?? null
})


/** 「渲染中」标记：`pending` 持续不足 150ms 就不显示，避免打字时闪个不停。 */
const renderBusy = ref(false)
let renderBusyTimer: ReturnType<typeof setTimeout> | null = null
watch(() => typstPreview.pending.value, (pending) => {
  if (renderBusyTimer) {
    clearTimeout(renderBusyTimer)
    renderBusyTimer = null
  }
  if (!pending) {
    renderBusy.value = false
    return
  }
  renderBusyTimer = setTimeout(() => {
    renderBusyTimer = null
    renderBusy.value = true
  }, 150)
})

/** 内容缩到最小字号仍放不下、超出的部分被裁切。 */
const typstPreviewClipped = computed(() =>
  Boolean(typstPreview.overflow.value[0]),
)

function shouldIgnoreEditorKeydown(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement))
    return false
  if (target.isContentEditable)
    return true
  const tagName = target.tagName
  return tagName === 'INPUT' || tagName === 'TEXTAREA' || tagName === 'SELECT'
}

function handleEditorArrowKeydown(event: KeyboardEvent) {
  if (props.showKindPicker)
    return
  if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey)
    return
  if (shouldIgnoreEditorKeydown(event.target))
    return

  if (event.key === 'ArrowUp') {
    event.preventDefault()
    setActiveSlide(props.activeSlide - 1)
  } else if (event.key === 'ArrowDown') {
    event.preventDefault()
    setActiveSlide(props.activeSlide + 1)
  }
}

function handleTextareaKeydown(event: KeyboardEvent) {
  if ((event.ctrlKey || event.metaKey) && event.key === 's') {
    event.preventDefault()
    emit('apply-changes')
    return
  }
  emit('textarea-keydown', event)
}

onMounted(() => {
  window.addEventListener('keydown', handleEditorArrowKeydown)
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', handleEditorArrowKeydown)
  if (renderBusyTimer) {
    clearTimeout(renderBusyTimer)
    renderBusyTimer = null
  }
})
</script>

<template>
  <div class="step-panel step-edit">
    <div class="edit-three-col">
      <aside class="edit-list-col">
        <div class="edit-col-head">
          <span class="i-carbon:layers" />
          幻灯片
          <span class="edit-badge">{{ slideCount }}</span>
          <button class="btn btn-ghost edit-icon-btn ml-auto" title="重新生成" @click="$emit('go-to-step-1')">
            <span class="i-carbon:reset" />
          </button>
        </div>
        <div class="edit-list-scroll">
          <div
            v-for="(slide, idx) in blueprints"
            :key="idx"
            class="edit-slide-row"
            :class="{ 'edit-slide-row--active': idx === activeSlide }"
            role="button"
            tabindex="0"
            :aria-pressed="idx === activeSlide"
            @click="$emit('update:activeSlide', idx)"
            @keydown.enter.prevent="$emit('update:activeSlide', idx)"
            @keydown.space.prevent="$emit('update:activeSlide', idx)"
          >
            <span class="edit-row-num">{{ idx + 1 }}</span>
            <div class="edit-row-body">
              <span class="edit-row-title">{{ slide.title }}</span>
              <span class="edit-row-kind">{{ KIND_META[slide.kind as SlideKind]?.label ?? slide.kind }}</span>
            </div>
            <button class="edit-row-del" :disabled="blueprints.length <= 1" title="删除" @click.stop="$emit('delete-slide', idx)">
              <span class="i-carbon:trash-can" />
            </button>
          </div>
        </div>
        <div class="edit-list-footer">
          <button class="btn btn-ghost edit-add-btn" @click="$emit('open-kind-picker', 'add')">
            <span class="i-carbon:add" /> 新建幻灯片
          </button>
        </div>
      </aside>

      <div class="edit-preview-col" :ref="(el) => $emit('preview-area-ref', el as HTMLElement)">
        <div class="edit-col-head">
          <span class="i-carbon:view" />
          实时预览
          <span v-if="hasUnsavedChanges" class="edit-unsaved">● 未保存</span>
          <span v-if="renderBusy" class="edit-rendering" role="status">渲染中…</span>
          <div class="edit-preview-nav ml-auto">
            <button class="btn btn-ghost edit-icon-btn" :disabled="activeSlide === 0" @click="$emit('update:activeSlide', activeSlide - 1)">
              <span class="i-carbon:arrow-left" />
            </button>
            <span class="edit-preview-pos">{{ activeSlide + 1 }} / {{ slideCount }}</span>
            <button class="btn btn-ghost edit-icon-btn" :disabled="activeSlide >= slideCount - 1" @click="$emit('update:activeSlide', activeSlide + 1)">
              <span class="i-carbon:arrow-right" />
            </button>
          </div>
          <button
            class="btn btn-ghost edit-present-btn"
            :disabled="blueprints.length === 0 || exportingPdf"
            title="导出为 PDF 文档"
            @click="$emit('export-pdf')"
          >
            <span :class="exportingPdf ? 'i-carbon:renew' : 'i-carbon:document-pdf'" /> 导出 PDF
          </button>
          <button class="btn btn-primary edit-present-btn" :disabled="blueprints.length === 0" @click="$emit('start-presentation')">
            <span class="i-carbon:presentation-file" /> 演示
          </button>
        </div>
        <div class="edit-preview-stage-wrap">
          <div
            v-if="editorPreviewSlide"
            class="edit-preview-stage"
            :style="{ width: editorStageW + 'px', height: editorStageH + 'px' }"
          >
            <!--
              Typst 位图直接按舞台尺寸显示（不能放进带 `transform: scale()` 的画布，
              缩放过的位图会发糊）。还没有图时显示加载框，预览不放空白。
            -->
            <SlideFrame
              :src="typstPreviewImage?.url ?? null"
              :pending="typstPreview.pending.value"
              :error="typstPreview.failed.value"
              alt=""
              @retry="typstPreview.refresh()"
            />
          </div>
        </div>
        <p v-if="typstPreviewClipped" class="edit-preview-clip" role="status">
          <span class="i-carbon:warning-alt" />
          内容过多，已缩到最小字号仍放不下，超出部分被裁切
        </p>
      </div>

      <div class="edit-json-col">
        <div class="edit-col-head">
          <span class="i-carbon:edit" />
          编辑&nbsp;<span class="edit-head-pos">第 {{ activeSlide + 1 }} 张</span>
          <div class="edit-json-acts ml-auto">
            <button class="btn btn-ghost edit-icon-btn" title="上移" :disabled="activeSlide === 0" @click="$emit('move-slide', -1)">
              <span class="i-carbon:arrow-up" />
            </button>
            <button class="btn btn-ghost edit-icon-btn" title="下移" :disabled="activeSlide >= slideCount - 1" @click="$emit('move-slide', 1)">
              <span class="i-carbon:arrow-down" />
            </button>
            <button class="btn btn-ghost edit-icon-btn edit-del-btn" title="删除" :disabled="blueprints.length <= 1" @click="$emit('delete-slide', activeSlide)">
              <span class="i-carbon:trash-can" />
            </button>
          </div>
        </div>

        <!-- 版式表来自 Rust：还没取到时不渲染表单，避免显示一个空的编辑器。 -->
        <div v-if="!schemaReady" class="edit-json-inner">
          <p v-if="schemaError" class="ej-error">
            <span class="i-carbon:warning-filled ej-error-icon" />
            <span class="ej-error-text">读取版式信息失败：{{ schemaError }}</span>
          </p>
          <p v-else class="ej-hint" role="status">正在读取版式信息…</p>
        </div>

        <div v-else class="edit-json-inner">
          <div class="ej-field-group">
            <label class="ej-label">幻灯片类型</label>
            <div class="ej-kind-row">
              <span :class="KIND_META[editorKind]?.icon" class="ej-kind-icon" />
              <span class="ej-kind-name">{{ KIND_META[editorKind]?.label }}</span>
              <span class="ej-kind-key">{{ editorKind }}</span>
              <button class="btn btn-ghost ej-change-btn" @click="$emit('open-kind-picker', 'change')">
                更换 <span class="i-carbon:chevron-down" />
              </button>
            </div>
          </div>

          <div class="ej-field-group">
            <div class="ej-label-row">
              <label class="ej-label">编辑模式</label>
              <button class="btn btn-ghost ej-fmt-btn" @click="advancedMode = !advancedMode">
                <span :class="advancedMode ? 'i-carbon:chevron-up' : 'i-carbon:chevron-down'" />
                {{ advancedMode ? '收起高级 JSON' : '打开高级 JSON' }}
              </button>
            </div>
            <div class="ej-hint">
              默认只编辑文字、结构、图片和图标推荐。JSON 保留给高级调整和排错。
            </div>
          </div>

          <div v-if="currentDraft" class="ej-field-group">
            <div class="ej-label-row">
              <label class="ej-label">基础信息</label>
              <span class="ej-hint">优先编辑用户能理解的内容字段</span>
            </div>
            <div class="ej-structured-grid">
              <template v-for="field in simpleFields" :key="field.key">
                <div class="ej-struct-field" :class="{ 'ej-struct-field--wide': field.type === 'textarea' }">
                  <label class="ej-qf-label">{{ field.label }}</label>
                  <textarea
                    v-if="field.type === 'textarea'"
                    class="ej-qf-input ej-qf-textarea"
                    :rows="field.rows || 3"
                    :value="getStringValue(field.key)"
                    :placeholder="field.placeholder"
                    @input="updatePath(field.key, ($event.target as HTMLTextAreaElement).value)"
                  />
                  <select
                    v-else-if="field.type === 'select'"
                    class="ej-qf-input"
                    :value="getStringValue(field.key)"
                    @change="updatePath(field.key, ($event.target as HTMLSelectElement).value)"
                  >
                    <option value="">请选择</option>
                    <option v-for="option in field.options" :key="option" :value="option">{{ option }}</option>
                  </select>
                  <input
                    v-else
                    type="text"
                    class="ej-qf-input"
                    :value="getStringValue(field.key)"
                    :placeholder="field.placeholder"
                    @input="updatePath(field.key, ($event.target as HTMLInputElement).value)"
                  />
                </div>
              </template>
            </div>
          </div>

          <div v-if="showBadges" class="ej-field-group">
            <div class="ej-label-row">
              <label class="ej-label">标签组</label>
              <span class="ej-hint">用于封面、结束页等标签型内容</span>
            </div>
            <div class="ej-list-stack">
              <div v-for="(badge, badgeIndex) in getArrayValue('badges')" :key="`badge-${badgeIndex}`" class="ej-inline-list-item">
                <input
                  type="text"
                  class="ej-qf-input"
                  :value="String(badge ?? '')"
                  placeholder="标签文案"
                  @input="updateStringItem('badges', badgeIndex, ($event.target as HTMLInputElement).value)"
                />
                <button class="btn btn-ghost edit-icon-btn" @click="removeStringItem('badges', badgeIndex)">
                  <span class="i-carbon:trash-can" />
                </button>
              </div>
              <button class="btn btn-ghost edit-add-btn" @click="addStringItem('badges')">
                <span class="i-carbon:add" /> 添加标签
              </button>
            </div>
          </div>

          <div v-if="showImages || getStringValue('image')" class="ej-field-group ej-media-group">
            <div class="ej-label-row">
              <label class="ej-label">图片素材</label>
              <span class="ej-hint">主图直接设置，图集可追加多张</span>
            </div>
            <div class="ej-structured-grid">
              <div class="ej-struct-field ej-struct-field--wide">
                <label class="ej-qf-label">主图</label>
                <input
                  type="text"
                  class="ej-qf-input"
                  :value="getStringValue('image')"
                  placeholder="media:12 或 https://..."
                  @input="updatePath('image', ($event.target as HTMLInputElement).value)"
                />
              </div>
            </div>
            <div v-if="showImages" class="ej-list-stack">
              <div v-for="(image, imageIndex) in getArrayValue('images')" :key="`image-${imageIndex}`" class="ej-inline-list-item">
                <input
                  type="text"
                  class="ej-qf-input"
                  :value="String(image ?? '')"
                  placeholder="图集中的一张图片"
                  @input="updateStringItem('images', imageIndex, ($event.target as HTMLInputElement).value)"
                />
                <button class="btn btn-ghost edit-icon-btn" @click="removeStringItem('images', imageIndex)">
                  <span class="i-carbon:trash-can" />
                </button>
              </div>
              <button class="btn btn-ghost edit-add-btn" @click="addStringItem('images')">
                <span class="i-carbon:add" /> 添加图集图片
              </button>
            </div>
            <div v-if="mediaCandidates().length > 0" class="ej-media-strip">
              <div
                v-for="m in mediaCandidates()"
                :key="m.id"
                class="ej-media-item"
                :title="m.name"
              >
                <img v-if="m.type === 'image'" :src="m.url" class="ej-media-thumb" />
                <span class="ej-media-label">{{ m.name }}</span>
                <div class="ej-media-actions">
                  <button class="btn btn-ghost edit-add-btn" @click="setPrimaryImage(m.ref)">设为主图</button>
                  <button v-if="showImages" class="btn btn-ghost edit-add-btn" @click="appendGalleryImage(m.ref)">加入图集</button>
                </div>
              </div>
            </div>
          </div>

          <div v-for="section in listSections" :key="section.key" class="ej-field-group">
            <div class="ej-label-row">
              <label class="ej-label">{{ section.label }}</label>
              <button class="btn btn-ghost edit-add-btn" @click="addItem(section.key, section.new_item)">
                <span class="i-carbon:add" /> 添加
              </button>
            </div>
            <p v-if="editorKind === 'issue_stack' && section.key === 'cards'" class="ej-hint" role="status">
              {{ getArrayValue(section.key).length > 4
                ? '问题卡片超过 4 张，建议拆成两页，保证说明和关键词完整显示。'
                : '2–3 张使用上下结构，4 张使用左右结构。每张保留一句说明和 2–4 个短关键词；说明较长时建议拆页。' }}
            </p>
            <div v-if="getArrayValue(section.key).length === 0" class="ej-hint">{{ section.emptyText }}</div>
            <div v-for="(item, index) in getArrayValue(section.key)" :key="`${section.key}-${index}`" class="ej-card-block">
              <div class="ej-card-head">
                <span class="ej-card-title">{{ section.label }} {{ index + 1 }}</span>
                <div class="edit-json-acts ml-auto">
                  <button class="btn btn-ghost edit-icon-btn" :disabled="index === 0" @click="moveItem(section.key, index, -1)">
                    <span class="i-carbon:arrow-up" />
                  </button>
                  <button class="btn btn-ghost edit-icon-btn" :disabled="index >= getArrayValue(section.key).length - 1" @click="moveItem(section.key, index, 1)">
                    <span class="i-carbon:arrow-down" />
                  </button>
                  <button class="btn btn-ghost edit-icon-btn" @click="removeItem(section.key, index)">
                    <span class="i-carbon:trash-can" />
                  </button>
                </div>
              </div>

              <div class="ej-structured-grid">
                <template v-for="field in section.fields" :key="field.key">
                  <div class="ej-struct-field" :class="{ 'ej-struct-field--wide': field.type === 'textarea' }">
                    <label class="ej-qf-label">{{ field.label }}</label>
                    <textarea
                      v-if="field.type === 'textarea'"
                      class="ej-qf-input ej-qf-textarea"
                      :rows="field.rows || 3"
                      :value="getStringValue(`${section.key}[${index}].${field.key}`)"
                      :placeholder="field.placeholder"
                      @input="updatePath(`${section.key}[${index}].${field.key}`, ($event.target as HTMLTextAreaElement).value)"
                    />
                    <select
                      v-else-if="field.type === 'select'"
                      class="ej-qf-input"
                      :value="getStringValue(`${section.key}[${index}].${field.key}`)"
                      @change="updatePath(`${section.key}[${index}].${field.key}`, ($event.target as HTMLSelectElement).value)"
                    >
                      <option value="">请选择</option>
                      <option v-for="option in field.options" :key="option" :value="option">{{ option }}</option>
                    </select>
                    <label v-else-if="field.type === 'boolean'" class="ej-bool-row">
                      <input
                        type="checkbox"
                        :checked="getBooleanValue(`${section.key}[${index}].${field.key}`)"
                        @change="updatePath(`${section.key}[${index}].${field.key}`, ($event.target as HTMLInputElement).checked)"
                      />
                      <span>{{ field.label }}</span>
                    </label>
                    <input
                      v-else
                      type="text"
                      class="ej-qf-input"
                      :value="getStringValue(`${section.key}[${index}].${field.key}`)"
                      :placeholder="field.placeholder"
                      @input="updatePath(`${section.key}[${index}].${field.key}`, ($event.target as HTMLInputElement).value)"
                    />
                  </div>
                </template>
              </div>

              <div v-if="section.iconQueryKeys" class="ej-icon-box">
                <div class="ej-label-row">
                  <span class="ej-qf-label">图标助手</span>
                  <button
                    class="btn btn-ghost edit-add-btn"
                    :disabled="iconLoadingKey === `${section.key}[${index}]`"
                    @click="recommendIcon(`${section.key}[${index}]`, section.iconQueryKeys)"
                  >
                    <span :class="iconLoadingKey === `${section.key}[${index}]` ? 'i-carbon:renew spin' : 'i-carbon:magic-wand'" />
                    {{ iconLoadingKey === `${section.key}[${index}]` ? '推荐中…' : 'AI 推荐图标' }}
                  </button>
                </div>
                <div v-if="iconSuggestions[`${section.key}[${index}]`]?.length" class="ej-icon-suggestions">
                  <button
                    v-for="icon in iconSuggestions[`${section.key}[${index}]`]"
                    :key="icon"
                    class="ej-icon-chip"
                    @click="applySuggestedIcon(`${section.key}[${index}]`, icon)"
                  >
                    <span :class="icon" class="ej-icon-chip-preview" />
                    {{ icon.replace('i-carbon:', '') }}
                  </button>
                </div>
              </div>

              <div v-for="stringList in section.stringLists ?? []" :key="`${section.key}-${index}-${stringList.key}`" class="ej-list-stack">
                <div class="ej-label-row">
                  <label class="ej-qf-label">{{ stringList.label }}</label>
                  <button class="btn btn-ghost edit-add-btn" @click="addStringItem(`${section.key}[${index}].${stringList.key}`)">
                    <span class="i-carbon:add" /> 添加
                  </button>
                </div>
                <div
                  v-for="(value, valueIndex) in getArrayValue(`${section.key}[${index}].${stringList.key}`)"
                  :key="`${section.key}-${index}-${stringList.key}-${valueIndex}`"
                  class="ej-inline-list-item"
                >
                  <input
                    type="text"
                    class="ej-qf-input"
                    :value="String(value ?? '')"
                    :placeholder="stringList.placeholder"
                    @input="updateStringItem(`${section.key}[${index}].${stringList.key}`, valueIndex, ($event.target as HTMLInputElement).value)"
                  />
                  <button class="btn btn-ghost edit-icon-btn" @click="removeStringItem(`${section.key}[${index}].${stringList.key}`, valueIndex)">
                    <span class="i-carbon:trash-can" />
                  </button>
                </div>
              </div>

              <div v-for="nested in section.nestedLists ?? []" :key="`${section.key}-${index}-${nested.key}`" class="ej-list-stack">
                <div class="ej-label-row">
                  <label class="ej-qf-label">{{ nested.label }}</label>
                  <button class="btn btn-ghost edit-add-btn" @click="addItem(`${section.key}[${index}].${nested.key}`, nested.new_item)">
                    <span class="i-carbon:add" /> 添加
                  </button>
                </div>
                <div
                  v-for="(nestedItem, nestedIndex) in getArrayValue(`${section.key}[${index}].${nested.key}`)"
                  :key="`${section.key}-${index}-${nested.key}-${nestedIndex}`"
                  class="ej-nested-card"
                >
                  <div class="ej-card-head">
                    <span class="ej-card-title">{{ nested.label }} {{ nestedIndex + 1 }}</span>
                    <button class="btn btn-ghost edit-icon-btn" @click="removeItem(`${section.key}[${index}].${nested.key}`, nestedIndex)">
                      <span class="i-carbon:trash-can" />
                    </button>
                  </div>
                  <div class="ej-structured-grid">
                    <template v-for="field in nested.fields" :key="field.key">
                      <div class="ej-struct-field" :class="{ 'ej-struct-field--wide': field.type === 'textarea' }">
                        <label class="ej-qf-label">{{ field.label }}</label>
                        <textarea
                          v-if="field.type === 'textarea'"
                          class="ej-qf-input ej-qf-textarea"
                          :rows="field.rows || 3"
                          :value="getStringValue(`${section.key}[${index}].${nested.key}[${nestedIndex}].${field.key}`)"
                          :placeholder="field.placeholder"
                          @input="updatePath(`${section.key}[${index}].${nested.key}[${nestedIndex}].${field.key}`, ($event.target as HTMLTextAreaElement).value)"
                        />
                        <input
                          v-else
                          type="text"
                          class="ej-qf-input"
                          :value="getStringValue(`${section.key}[${index}].${nested.key}[${nestedIndex}].${field.key}`)"
                          :placeholder="field.placeholder"
                          @input="updatePath(`${section.key}[${index}].${nested.key}[${nestedIndex}].${field.key}`, ($event.target as HTMLInputElement).value)"
                        />
                      </div>
                    </template>
                  </div>
                </div>
              </div>
            </div>
          </div>

          <div v-for="section in objectSections" :key="section.key" class="ej-field-group">
            <div class="ej-label-row">
              <label class="ej-label">{{ section.label }}</label>
            </div>
            <div v-if="section.fields?.length" class="ej-structured-grid">
              <template v-for="field in section.fields" :key="field.key">
                <div class="ej-struct-field" :class="{ 'ej-struct-field--wide': field.type === 'textarea' }">
                  <label v-if="field.type !== 'boolean'" class="ej-qf-label">{{ field.label }}</label>
                  <textarea
                    v-if="field.type === 'textarea'"
                    class="ej-qf-input ej-qf-textarea"
                    :rows="field.rows || 3"
                    :value="getStringValue(`${section.key}.${field.key}`)"
                    :placeholder="field.placeholder"
                    @input="updatePath(`${section.key}.${field.key}`, ($event.target as HTMLTextAreaElement).value)"
                  />
                  <select
                    v-else-if="field.type === 'select'"
                    class="ej-qf-input"
                    :value="getStringValue(`${section.key}.${field.key}`)"
                    @change="updatePath(`${section.key}.${field.key}`, ($event.target as HTMLSelectElement).value)"
                  >
                    <option value="">请选择</option>
                    <option v-for="option in field.options" :key="option" :value="option">{{ option }}</option>
                  </select>
                  <label v-else-if="field.type === 'boolean'" class="ej-bool-row">
                    <input
                      type="checkbox"
                      :checked="getBooleanValue(`${section.key}.${field.key}`)"
                      @change="updatePath(`${section.key}.${field.key}`, ($event.target as HTMLInputElement).checked)"
                    />
                    <span>{{ field.label }}</span>
                  </label>
                  <input
                    v-else
                    type="text"
                    class="ej-qf-input"
                    :value="getStringValue(`${section.key}.${field.key}`)"
                    :placeholder="field.placeholder"
                    @input="updatePath(`${section.key}.${field.key}`, ($event.target as HTMLInputElement).value)"
                  />
                </div>
              </template>
            </div>

            <div v-for="subList in section.listSections ?? []" :key="`${section.key}-${subList.key}`" class="ej-list-stack">
              <div class="ej-label-row">
                <label class="ej-qf-label">{{ subList.label }}</label>
                <button class="btn btn-ghost edit-add-btn" @click="addItem(`${section.key}.${subList.key}`, subList.new_item)">
                  <span class="i-carbon:add" /> 添加
                </button>
              </div>
              <div v-if="getArrayValue(`${section.key}.${subList.key}`).length === 0" class="ej-hint">{{ subList.emptyText }}</div>
              <div
                v-for="(item, index) in getArrayValue(`${section.key}.${subList.key}`)"
                :key="`${section.key}-${subList.key}-${index}`"
                class="ej-card-block"
              >
                <div class="ej-card-head">
                  <span class="ej-card-title">{{ subList.label }} {{ index + 1 }}</span>
                  <div class="edit-json-acts ml-auto">
                    <button class="btn btn-ghost edit-icon-btn" :disabled="index === 0" @click="moveItem(`${section.key}.${subList.key}`, index, -1)">
                      <span class="i-carbon:arrow-up" />
                    </button>
                    <button class="btn btn-ghost edit-icon-btn" :disabled="index >= getArrayValue(`${section.key}.${subList.key}`).length - 1" @click="moveItem(`${section.key}.${subList.key}`, index, 1)">
                      <span class="i-carbon:arrow-down" />
                    </button>
                    <button class="btn btn-ghost edit-icon-btn" @click="removeItem(`${section.key}.${subList.key}`, index)">
                      <span class="i-carbon:trash-can" />
                    </button>
                  </div>
                </div>
                <div class="ej-structured-grid">
                  <template v-for="field in subList.fields" :key="field.key">
                    <div class="ej-struct-field" :class="{ 'ej-struct-field--wide': field.type === 'textarea' }">
                      <label v-if="field.type !== 'boolean'" class="ej-qf-label">{{ field.label }}</label>
                      <textarea
                        v-if="field.type === 'textarea'"
                        class="ej-qf-input ej-qf-textarea"
                        :rows="field.rows || 3"
                        :value="getStringValue(`${section.key}.${subList.key}[${index}].${field.key}`)"
                        :placeholder="field.placeholder"
                        @input="updatePath(`${section.key}.${subList.key}[${index}].${field.key}`, ($event.target as HTMLTextAreaElement).value)"
                      />
                      <select
                        v-else-if="field.type === 'select'"
                        class="ej-qf-input"
                        :value="getStringValue(`${section.key}.${subList.key}[${index}].${field.key}`)"
                        @change="updatePath(`${section.key}.${subList.key}[${index}].${field.key}`, ($event.target as HTMLSelectElement).value)"
                      >
                        <option value="">请选择</option>
                        <option v-for="option in field.options" :key="option" :value="option">{{ option }}</option>
                      </select>
                      <label v-else-if="field.type === 'boolean'" class="ej-bool-row">
                        <input
                          type="checkbox"
                          :checked="getBooleanValue(`${section.key}.${subList.key}[${index}].${field.key}`)"
                          @change="updatePath(`${section.key}.${subList.key}[${index}].${field.key}`, ($event.target as HTMLInputElement).checked)"
                        />
                        <span>{{ field.label }}</span>
                      </label>
                      <input
                        v-else
                        type="text"
                        class="ej-qf-input"
                        :value="getStringValue(`${section.key}.${subList.key}[${index}].${field.key}`)"
                        :placeholder="field.placeholder"
                        @input="updatePath(`${section.key}.${subList.key}[${index}].${field.key}`, ($event.target as HTMLInputElement).value)"
                      />
                    </div>
                  </template>
                </div>

                <div v-if="subList.iconQueryKeys" class="ej-icon-box">
                  <div class="ej-label-row">
                    <span class="ej-qf-label">图标助手</span>
                    <button
                      class="btn btn-ghost edit-add-btn"
                      :disabled="iconLoadingKey === `${section.key}.${subList.key}[${index}]`"
                      @click="recommendIcon(`${section.key}.${subList.key}[${index}]`, subList.iconQueryKeys)"
                    >
                      <span :class="iconLoadingKey === `${section.key}.${subList.key}[${index}]` ? 'i-carbon:renew spin' : 'i-carbon:magic-wand'" />
                      {{ iconLoadingKey === `${section.key}.${subList.key}[${index}]` ? '推荐中…' : 'AI 推荐图标' }}
                    </button>
                  </div>
                  <div v-if="iconSuggestions[`${section.key}.${subList.key}[${index}]`]?.length" class="ej-icon-suggestions">
                    <button
                      v-for="icon in iconSuggestions[`${section.key}.${subList.key}[${index}]`]"
                      :key="icon"
                      class="ej-icon-chip"
                      @click="applySuggestedIcon(`${section.key}.${subList.key}[${index}]`, icon)"
                    >
                      <span :class="icon" class="ej-icon-chip-preview" />
                      {{ icon.replace('i-carbon:', '') }}
                    </button>
                  </div>
                </div>

                <div v-for="stringList in subList.stringLists ?? []" :key="`${section.key}-${subList.key}-${index}-${stringList.key}`" class="ej-list-stack">
                  <div class="ej-label-row">
                    <label class="ej-qf-label">{{ stringList.label }}</label>
                    <button class="btn btn-ghost edit-add-btn" @click="addStringItem(`${section.key}.${subList.key}[${index}].${stringList.key}`)">
                      <span class="i-carbon:add" /> 添加
                    </button>
                  </div>
                  <div
                    v-for="(value, valueIndex) in getArrayValue(`${section.key}.${subList.key}[${index}].${stringList.key}`)"
                    :key="`${section.key}-${subList.key}-${index}-${stringList.key}-${valueIndex}`"
                    class="ej-inline-list-item"
                  >
                    <input
                      type="text"
                      class="ej-qf-input"
                      :value="String(value ?? '')"
                      :placeholder="stringList.placeholder"
                      @input="updateStringItem(`${section.key}.${subList.key}[${index}].${stringList.key}`, valueIndex, ($event.target as HTMLInputElement).value)"
                    />
                    <button class="btn btn-ghost edit-icon-btn" @click="removeStringItem(`${section.key}.${subList.key}[${index}].${stringList.key}`, valueIndex)">
                      <span class="i-carbon:trash-can" />
                    </button>
                  </div>
                </div>
              </div>
            </div>
          </div>

          <div v-if="editorKind === 'chart_focus'" class="ej-field-group">
            <div class="ej-label-row">
              <label class="ej-label">图表数据（JSON）</label>
              <span class="ej-hint">
                <template v-if="chartChecking">正在校验…</template>
                <template v-else>type / data / encoding / series，校验通过才会应用到预览</template>
              </span>
            </div>
            <div class="ej-wrap" :class="{ 'ej-wrap--error': !!chartJsonError }">
              <textarea
                class="ej-qf-input ej-qf-textarea ej-chart-json"
                rows="12"
                spellcheck="false"
                :value="chartJsonText"
                placeholder='{ "type": "bar", "title": "…", "data": [], "encoding": { "x": "…", "y": "…" } }'
                @focus="chartJsonFocused = true"
                @blur="onChartJsonBlur"
                @input="onChartJsonInput(($event.target as HTMLTextAreaElement).value)"
              />
            </div>
            <div v-if="chartJsonError" class="ej-error">
              <span class="i-carbon:warning-filled ej-error-icon" />
              <span class="ej-error-text">{{ chartJsonError }}</span>
            </div>
          </div>

          <div v-if="advancedMode" class="ej-field-group ej-field-group--grow">
            <div class="ej-label-row">
              <label class="ej-label">高级 Blueprint JSON</label>
              <span class="ej-hint">保留给高级用户和特殊字段</span>
              <button class="btn btn-ghost ej-fmt-btn" @click="$emit('format-json')">
                <span class="i-carbon:clean" /> 格式化
              </button>
            </div>
            <div class="ej-wrap" :class="{ 'ej-wrap--error': !!jsonError }">
              <div class="ej-editor-row">
                <div class="ej-gutter" ref="gutterRef" aria-hidden="true">
                  <span
                    v-for="n in lineCount"
                    :key="n"
                    class="ej-line-num"
                    :class="{ 'ej-line-num--error': errorLine === n }"
                  >{{ n }}</span>
                </div>
                <textarea
                  :ref="(el) => $emit('json-textarea-ref', el as HTMLTextAreaElement)"
                  class="ej-textarea"
                  :value="jsonText"
                  spellcheck="false"
                  autocomplete="off"
                  @input="$emit('textarea-input', ($event.target as HTMLTextAreaElement).value)"
                  @keydown="handleTextareaKeydown"
                  @scroll="syncGutterScroll"
                />
              </div>
              <div v-if="jsonError" class="ej-error">
                <span class="i-carbon:warning-filled ej-error-icon" />
                <span class="ej-error-text">
                  <template v-if="errorLine !== null">第 {{ errorLine }} 行：</template>{{ jsonError }}
                </span>
              </div>
              <div v-if="semanticErrors.length > 0 && !jsonError" class="ej-semantic-errors">
                <div v-for="err in semanticErrors" :key="err" class="ej-semantic-item">
                  <span class="i-carbon:warning ej-semantic-icon" />{{ err }}
                </div>
              </div>
            </div>
          </div>

          <button
            class="btn btn-primary ej-save-btn"
            :disabled="!!jsonError || !hasUnsavedChanges || editorSaving"
            @click="$emit('apply-changes')"
          >
            <span v-if="editorSaving" class="i-carbon:renew spin" />
            <span v-else-if="hasUnsavedChanges" class="i-carbon:save" />
            <span v-else class="i-carbon:checkmark" />
            {{ editorSaving ? '保存中…' : hasUnsavedChanges ? '应用更改 (Ctrl+S)' : '已保存' }}
          </button>
        </div>
      </div>
    </div>
  </div>

  <Teleport to="body">
    <div v-if="showKindPicker" class="kp-backdrop" @click.self="$emit('open-kind-picker', kindPickerMode)">
      <div class="kp-box">
        <div class="kp-head">
          <span :class="kindPickerMode === 'add' ? 'i-carbon:add-alt' : 'i-carbon:arrows-horizontal'" class="kp-head-icon" />
          {{ kindPickerMode === 'add' ? '新建幻灯片 — 选择类型' : '更换幻灯片类型' }}
          <button class="btn btn-ghost edit-icon-btn kp-close" @click="$emit('open-kind-picker', kindPickerMode)">
            <span class="i-carbon:close" />
          </button>
        </div>
        <p class="kp-sub">
          <template v-if="kindPickerMode === 'add'">新幻灯片将插入到第 {{ activeSlide + 1 }} 张之后</template>
          <template v-else>选择新类型后将自动保留可迁移的公共字段</template>
        </p>
        <div class="kp-grid">
          <button
            v-for="k in ALL_KINDS" :key="k"
            class="kp-card"
            :class="{ 'kp-card--current': kindPickerMode === 'change' && k === editorKind }"
            @click="$emit('kind-picked', k)"
          >
            <span :class="KIND_META[k]?.icon" class="kp-card-icon" />
            <span class="kp-card-label">{{ KIND_META[k]?.label }}</span>
            <span class="kp-card-key">{{ k }}</span>
            <span class="kp-card-desc">{{ KIND_META[k]?.desc }}</span>
          </button>
        </div>
      </div>
    </div>
  </Teleport>
</template>
