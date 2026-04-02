<script setup lang="ts">
import { computed, nextTick, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import SlideRenderer from '../../components/SlideRenderer.vue'
import { ALL_KINDS, KIND_FIELDS, KIND_META } from '../../composables/useSlideEditor'
import type { SlideBlueprint, SlideKind } from '../../components/types'
import type { MediaItem } from '../../composables/useMediaLibrary'

type FieldType = 'text' | 'textarea' | 'select' | 'boolean'

type EditorField = {
  key: string
  label: string
  type: FieldType
  placeholder?: string
  rows?: number
  options?: string[]
}

type StringListDef = {
  key: string
  label: string
  placeholder?: string
}

type NestedListDef = {
  key: string
  label: string
  createItem: () => Record<string, unknown>
  fields: EditorField[]
}

type ListSectionDef = {
  key: string
  label: string
  emptyText: string
  createItem: () => Record<string, unknown>
  fields: EditorField[]
  stringLists?: StringListDef[]
  nestedLists?: NestedListDef[]
  iconQueryKeys?: string[]
}

type ObjectSectionDef = {
  key: string
  label: string
  fields?: EditorField[]
  stringLists?: StringListDef[]
  listSections?: ListSectionDef[]
}

const TONE_OPTIONS = ['blue', 'emerald', 'green', 'violet', 'amber', 'rose', 'gray', 'red']
const DIRECTION_OPTIONS = ['horizontal', 'vertical']

const SIMPLE_FIELDS: EditorField[] = [
  { key: 'section', label: '章节号', type: 'text', placeholder: '01' },
  { key: 'badge', label: '角标', type: 'text', placeholder: '章节导览' },
  { key: 'label', label: '标签', type: 'text', placeholder: '重点展示' },
  { key: 'label_tone', label: '标签颜色', type: 'select', options: TONE_OPTIONS },
  { key: 'title', label: '标题', type: 'text', placeholder: '输入标题' },
  { key: 'subtitle', label: '副标题', type: 'textarea', rows: 2, placeholder: '输入副标题' },
  { key: 'accent', label: '强调语', type: 'text', placeholder: '一句强调信息' },
  { key: 'note', label: '说明', type: 'textarea', rows: 3, placeholder: '补充说明、背景或提示' },
  { key: 'footer', label: '页脚说明', type: 'textarea', rows: 2, placeholder: '补充一句收束语' },
  { key: 'placeholder', label: '图片占位提示', type: 'text', placeholder: '可填写 media:12 或图片 URL' },
  { key: 'side_width', label: '侧栏宽度', type: 'text', placeholder: '42%' },
  { key: 'direction', label: '布局方向', type: 'select', options: DIRECTION_OPTIONS },
  { key: 'example_title', label: '示例标题', type: 'text', placeholder: '类比例子' },
  { key: 'example_body', label: '示例正文', type: 'textarea', rows: 3, placeholder: '输入示例说明' },
  { key: 'image', label: '主图路径', type: 'text', placeholder: 'media:12 或 https://...' },
]

const LIST_SECTIONS: Partial<Record<SlideKind, ListSectionDef[]>> = {
  overview: [
    {
      key: 'overview_items',
      label: '概览条目',
      emptyText: '还没有概览条目',
      createItem: () => ({ number: '', title: '', desc: '' }),
      fields: [
        { key: 'number', label: '编号', type: 'text', placeholder: '01' },
        { key: 'title', label: '标题', type: 'text', placeholder: '现状判断' },
        { key: 'desc', label: '说明', type: 'textarea', rows: 2, placeholder: '一句话解释本节内容' },
      ],
    },
  ],
  section_intro: [
    {
      key: 'cards',
      label: '导览卡片',
      emptyText: '还没有导览卡片',
      createItem: () => ({ title: '', body: '', tone: 'blue', icon: '', tag: '' }),
      iconQueryKeys: ['title', 'body', 'tag'],
      fields: [
        { key: 'title', label: '标题', type: 'text', placeholder: '目标定义' },
        { key: 'body', label: '说明', type: 'textarea', rows: 3, placeholder: '卡片正文' },
        { key: 'tag', label: '标签', type: 'text', placeholder: 'Why' },
        { key: 'tone', label: '色调', type: 'select', options: TONE_OPTIONS },
        { key: 'icon', label: '图标', type: 'text', placeholder: '由 AI 推荐，通常无需手填' },
      ],
    },
  ],
  feature_grid: [
    {
      key: 'cards',
      label: '特性卡片',
      emptyText: '还没有特性卡片',
      createItem: () => ({ title: '', subtitle: '', body: '', tone: 'blue', icon: '', items: [] }),
      iconQueryKeys: ['title', 'subtitle', 'body'],
      stringLists: [{ key: 'items', label: '要点列表', placeholder: '支持语义搜索' }],
      fields: [
        { key: 'title', label: '标题', type: 'text', placeholder: '统一知识入口' },
        { key: 'subtitle', label: '副标题', type: 'text', placeholder: '降低信息查找成本' },
        { key: 'body', label: '正文', type: 'textarea', rows: 3, placeholder: '卡片正文' },
        { key: 'conclusion', label: '结论', type: 'text', placeholder: '适合标准化程度高的交付场景' },
        { key: 'risk', label: '风险', type: 'text', placeholder: '需提前定义记录粒度' },
        { key: 'footer_tag', label: '底部标签', type: 'text', placeholder: '可复用' },
        { key: 'footer_tone', label: '标签色调', type: 'select', options: TONE_OPTIONS },
        { key: 'tag', label: '卡片标签', type: 'text', placeholder: 'Why' },
        { key: 'tone', label: '主色调', type: 'select', options: TONE_OPTIONS },
        { key: 'icon', label: '图标', type: 'text', placeholder: '由 AI 推荐，通常无需手填' },
      ],
    },
  ],
  spotlight: [
    {
      key: 'panels',
      label: '右侧信息面板',
      emptyText: '还没有信息面板',
      createItem: () => ({ title: '', kind: 'text', body: '', tone: 'blue', icon: '', items: [], steps: [] }),
      iconQueryKeys: ['title', 'body', 'highlight'],
      stringLists: [
        { key: 'items', label: '要点列表', placeholder: '梳理字段' },
        { key: 'steps', label: '结果列表', placeholder: '线索响应时效缩短 60%' },
      ],
      fields: [
        { key: 'title', label: '标题', type: 'text', placeholder: '关键动作' },
        { key: 'kind', label: '类型', type: 'text', placeholder: 'flow / text / steps' },
        { key: 'body', label: '正文', type: 'textarea', rows: 3, placeholder: '面板正文' },
        { key: 'highlight', label: '高亮信息', type: 'text', placeholder: '一句重点结论' },
        { key: 'tone', label: '色调', type: 'select', options: TONE_OPTIONS },
        { key: 'icon', label: '图标', type: 'text', placeholder: '由 AI 推荐，通常无需手填' },
      ],
    },
  ],
  split_layers: [
    {
      key: 'left_items',
      label: '左侧说明项',
      emptyText: '还没有左侧说明项',
      createItem: () => ({ step: '', title: '', body: '', icon: '' }),
      iconQueryKeys: ['title', 'body'],
      fields: [
        { key: 'step', label: '步骤号', type: 'text', placeholder: '01' },
        { key: 'title', label: '标题', type: 'text', placeholder: '业务层' },
        { key: 'body', label: '说明', type: 'textarea', rows: 2, placeholder: '一句说明' },
        { key: 'icon', label: '图标', type: 'text', placeholder: '由 AI 推荐，通常无需手填' },
      ],
    },
    {
      key: 'layers',
      label: '右侧分层',
      emptyText: '还没有右侧分层',
      createItem: () => ({ title: '', meta: '', tone: 'blue' }),
      fields: [
        { key: 'title', label: '层标题', type: 'text', placeholder: '流程编排' },
        { key: 'meta', label: '层说明', type: 'textarea', rows: 2, placeholder: '这一层做什么' },
        { key: 'tone', label: '色调', type: 'select', options: TONE_OPTIONS },
      ],
    },
  ],
  section_list: [
    {
      key: 'list_items',
      label: '列表项',
      emptyText: '还没有列表项',
      createItem: () => ({ step: '', title: '', body: '', icon: '' }),
      iconQueryKeys: ['title', 'body'],
      fields: [
        { key: 'step', label: '编号', type: 'text', placeholder: '01' },
        { key: 'title', label: '标题', type: 'text', placeholder: '明确目标' },
        { key: 'body', label: '说明', type: 'textarea', rows: 2, placeholder: '一句说明' },
        { key: 'icon', label: '图标', type: 'text', placeholder: '由 AI 推荐，通常无需手填' },
      ],
    },
  ],
  focus_example: [
    {
      key: 'points',
      label: '要点',
      emptyText: '还没有要点',
      createItem: () => ({ step: '', title: '', body: '', icon: '' }),
      iconQueryKeys: ['title', 'body'],
      fields: [
        { key: 'step', label: '编号', type: 'text', placeholder: '01' },
        { key: 'title', label: '标题', type: 'text', placeholder: '先给判断标准' },
        { key: 'body', label: '说明', type: 'textarea', rows: 2, placeholder: '一句说明' },
        { key: 'icon', label: '图标', type: 'text', placeholder: '由 AI 推荐，通常无需手填' },
      ],
    },
    {
      key: 'ranking',
      label: '排序/优先级',
      emptyText: '还没有排序项',
      createItem: () => ({ index: '', label: '', meta: '', muted: false }),
      fields: [
        { key: 'index', label: '编号', type: 'text', placeholder: 'A' },
        { key: 'label', label: '标题', type: 'text', placeholder: '优先统一口径' },
        { key: 'meta', label: '说明', type: 'textarea', rows: 2, placeholder: '一句说明' },
        { key: 'muted', label: '弱化显示', type: 'boolean' },
      ],
    },
  ],
  outcome_grid: [
    {
      key: 'cards',
      label: '成果卡片',
      emptyText: '还没有成果卡片',
      createItem: () => ({ title: '', body: '', tone: 'blue', icon: '' }),
      iconQueryKeys: ['title', 'body', 'conclusion'],
      fields: [
        { key: 'title', label: '标题', type: 'text', placeholder: '效率提升' },
        { key: 'body', label: '说明', type: 'textarea', rows: 3, placeholder: '成果说明' },
        { key: 'conclusion', label: '结论', type: 'text', placeholder: '团队可把精力转向深度判断' },
        { key: 'footer_tag', label: '底部标签', type: 'text', placeholder: '可复用' },
        { key: 'footer_tone', label: '标签色调', type: 'select', options: TONE_OPTIONS },
        { key: 'tone', label: '主色调', type: 'select', options: TONE_OPTIONS },
        { key: 'icon', label: '图标', type: 'text', placeholder: '由 AI 推荐，通常无需手填' },
      ],
    },
  ],
  center_grid: [
    {
      key: 'center_items',
      label: '中心能力点',
      emptyText: '还没有能力点',
      createItem: () => ({ title: '', desc: '', tone: 'blue', icon: '' }),
      iconQueryKeys: ['title', 'desc'],
      fields: [
        { key: 'title', label: '标题', type: 'text', placeholder: '目标' },
        { key: 'desc', label: '说明', type: 'text', placeholder: '先定义成效' },
        { key: 'tone', label: '色调', type: 'select', options: TONE_OPTIONS },
        { key: 'icon', label: '图标', type: 'text', placeholder: '由 AI 推荐，通常无需手填' },
      ],
    },
  ],
  timeline: [
    {
      key: 'timeline_events',
      label: '时间线事件',
      emptyText: '还没有时间线事件',
      createItem: () => ({ date: '', title: '', body: '', tone: 'blue', icon: '' }),
      iconQueryKeys: ['date', 'title', 'body'],
      fields: [
        { key: 'date', label: '时间', type: 'text', placeholder: '第 1 周' },
        { key: 'title', label: '标题', type: 'text', placeholder: '需求对齐' },
        { key: 'body', label: '说明', type: 'textarea', rows: 2, placeholder: '这一阶段做什么' },
        { key: 'tone', label: '色调', type: 'select', options: TONE_OPTIONS },
        { key: 'icon', label: '图标', type: 'text', placeholder: '由 AI 推荐，通常无需手填' },
      ],
    },
  ],
  step_flow: [
    {
      key: 'steps',
      label: '流程步骤',
      emptyText: '还没有流程步骤',
      createItem: () => ({ title: '', body: '', tone: 'blue', icon: '' }),
      iconQueryKeys: ['title', 'body'],
      fields: [
        { key: 'title', label: '标题', type: 'text', placeholder: '识别问题' },
        { key: 'body', label: '说明', type: 'textarea', rows: 2, placeholder: '步骤说明' },
        { key: 'tone', label: '色调', type: 'select', options: TONE_OPTIONS },
        { key: 'icon', label: '图标', type: 'text', placeholder: '由 AI 推荐，通常无需手填' },
      ],
    },
  ],
  process: [
    {
      key: 'phases',
      label: '流程阶段',
      emptyText: '还没有流程阶段',
      createItem: () => ({ phase: '', title: '', tone: 'blue', icon: '', highlight: '', steps: [] }),
      iconQueryKeys: ['phase', 'title', 'highlight'],
      nestedLists: [
        {
          key: 'steps',
          label: '阶段步骤',
          createItem: () => ({ label: '', desc: '' }),
          fields: [
            { key: 'label', label: '步骤标题', type: 'text', placeholder: '明确负责人' },
            { key: 'desc', label: '步骤说明', type: 'textarea', rows: 2, placeholder: '说明这一步做什么' },
          ],
        },
      ],
      fields: [
        { key: 'phase', label: '阶段名', type: 'text', placeholder: 'Phase 1' },
        { key: 'title', label: '标题', type: 'text', placeholder: '准备阶段' },
        { key: 'highlight', label: '高亮提示', type: 'text', placeholder: '确保启动前信息齐备' },
        { key: 'tone', label: '色调', type: 'select', options: TONE_OPTIONS },
        { key: 'icon', label: '图标', type: 'text', placeholder: '由 AI 推荐，通常无需手填' },
      ],
    },
  ],
  issue_stack: [
    {
      key: 'cards',
      label: '问题卡片',
      emptyText: '还没有问题卡片',
      createItem: () => ({ title: '', body: '', tone: 'blue', icon: '', items: [] }),
      iconQueryKeys: ['title', 'body'],
      stringLists: [{ key: 'items', label: '补充标签', placeholder: '难调试' }],
      fields: [
        { key: 'title', label: '标题', type: 'text', placeholder: '黑盒中的黑盒' },
        { key: 'body', label: '说明', type: 'textarea', rows: 3, placeholder: '问题说明' },
        { key: 'tone', label: '色调', type: 'select', options: TONE_OPTIONS },
        { key: 'icon', label: '图标', type: 'text', placeholder: '由 AI 推荐，通常无需手填' },
      ],
    },
  ],
}

const OBJECT_SECTIONS: Partial<Record<SlideKind, ObjectSectionDef[]>> = {
  compare: [
    {
      key: 'compare_data',
      label: '对比设置',
      fields: [{ key: 'mode', label: '布局模式', type: 'text', placeholder: 'side-by-side' }],
    },
    {
      key: 'compare_data.left',
      label: '左侧方案',
      fields: [
        { key: 'title', label: '标题', type: 'text', placeholder: '方案 A：快速上线' },
        { key: 'tone', label: '色调', type: 'select', options: TONE_OPTIONS },
        { key: 'icon', label: '图标', type: 'text', placeholder: '由 AI 推荐，通常无需手填' },
        { key: 'conclusion', label: '结论', type: 'text', placeholder: '适合先跑通闭环' },
      ],
      listSections: [
        {
          key: 'items',
          label: '条目',
          emptyText: '还没有左侧条目',
          createItem: () => ({ label: '', desc: '', highlight: false }),
          fields: [
            { key: 'label', label: '标题', type: 'text', placeholder: '优势' },
            { key: 'desc', label: '说明', type: 'textarea', rows: 2, placeholder: '一句说明' },
            { key: 'highlight', label: '高亮', type: 'boolean' },
          ],
        },
      ],
    },
    {
      key: 'compare_data.right',
      label: '右侧方案',
      fields: [
        { key: 'title', label: '标题', type: 'text', placeholder: '方案 B：体系化建设' },
        { key: 'tone', label: '色调', type: 'select', options: TONE_OPTIONS },
        { key: 'icon', label: '图标', type: 'text', placeholder: '由 AI 推荐，通常无需手填' },
        { key: 'conclusion', label: '结论', type: 'text', placeholder: '适合中长期投入' },
      ],
      listSections: [
        {
          key: 'items',
          label: '条目',
          emptyText: '还没有右侧条目',
          createItem: () => ({ label: '', desc: '', highlight: false }),
          fields: [
            { key: 'label', label: '标题', type: 'text', placeholder: '优势' },
            { key: 'desc', label: '说明', type: 'textarea', rows: 2, placeholder: '一句说明' },
            { key: 'highlight', label: '高亮', type: 'boolean' },
          ],
        },
      ],
    },
  ],
  swot: [
    {
      key: 'swot_data',
      label: 'SWOT 设置',
      fields: [
        { key: 'strategy', label: '战略建议', type: 'textarea', rows: 3, placeholder: '用于总结整体建议' },
      ],
      listSections: [
        {
          key: 'quadrants',
          label: '四象限',
          emptyText: '还没有 SWOT 象限',
          createItem: () => ({ key: '', title: '', tone: 'blue', icon: '', items: [], summary: '' }),
          iconQueryKeys: ['key', 'title', 'summary'],
          stringLists: [{ key: 'items', label: '要点', placeholder: '已有行业案例积累' }],
          fields: [
            { key: 'key', label: '象限标识', type: 'text', placeholder: 'S' },
            { key: 'title', label: '标题', type: 'text', placeholder: '优势 Strengths' },
            { key: 'summary', label: '总结', type: 'text', placeholder: '内部基础较好' },
            { key: 'tone', label: '色调', type: 'select', options: TONE_OPTIONS },
            { key: 'icon', label: '图标', type: 'text', placeholder: '由 AI 推荐，通常无需手填' },
          ],
        },
      ],
    },
  ],
}

const props = defineProps<{
  activeSlide: number
  slideCount: number
  blueprints: SlideBlueprint[]
  mediaItems: MediaItem[]
  mediaMap: Record<string, string>
  jsonText: string
  jsonError: string
  liveSlide: SlideBlueprint | null
  hasUnsavedChanges: boolean
  editorSaving: boolean
  showKindPicker: boolean
  kindPickerMode: 'add' | 'change'
  previewArea: HTMLElement | null
  previewScale: number
  editorKind: SlideKind
  editorPreviewSlide: SlideBlueprint | null
  editorStageW: number
  editorStageH: number
  jsonTextareaRef: HTMLTextAreaElement | null
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

const semanticErrors = computed<string[]>(() => {
  const obj = parsedJson.value
  if (!obj || props.jsonError) return []
  const errors: string[] = []

  const direction = obj.direction
  if (typeof direction === 'string' && !DIRECTION_OPTIONS.includes(direction)) {
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
        if (typeof value === 'string' && !TONE_OPTIONS.includes(value)) {
          errors.push(`${path || 'root'}.${key} "${value}" 无效`)
        }
      } else {
        walkTone(value, path ? `${path}.${key}` : key)
      }
    }
  }

  walkTone(obj, '')
  return errors
})

const simpleFields = computed(() => {
  const allowed = new Set(KIND_FIELDS[props.editorKind] ?? [])
  return SIMPLE_FIELDS.filter((field) => allowed.has(field.key))
})

const listSections = computed(() => LIST_SECTIONS[props.editorKind] ?? [])
const objectSections = computed(() => OBJECT_SECTIONS[props.editorKind] ?? [])
const showBadges = computed(() => (KIND_FIELDS[props.editorKind] ?? []).includes('badges'))
const showImages = computed(() => (KIND_FIELDS[props.editorKind] ?? []).includes('images'))

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

function addItem(path: string, factory: () => Record<string, unknown>) {
  const draft = cloneDraft()
  if (!draft) return
  const items = getArrayValue(path)
  setAtPath(draft as unknown as Record<string, unknown>, path, [...items, factory()])
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

function handleTextareaKeydown(event: KeyboardEvent) {
  if ((event.ctrlKey || event.metaKey) && event.key === 's') {
    event.preventDefault()
    emit('apply-changes')
    return
  }
  emit('textarea-keydown', event)
}
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
          <div class="edit-preview-nav ml-auto">
            <button class="btn btn-ghost edit-icon-btn" :disabled="activeSlide === 0" @click="$emit('update:activeSlide', activeSlide - 1)">
              <span class="i-carbon:arrow-left" />
            </button>
            <span class="edit-preview-pos">{{ activeSlide + 1 }} / {{ slideCount }}</span>
            <button class="btn btn-ghost edit-icon-btn" :disabled="activeSlide >= slideCount - 1" @click="$emit('update:activeSlide', activeSlide + 1)">
              <span class="i-carbon:arrow-right" />
            </button>
          </div>
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
            <div
              class="edit-preview-canvas"
              :style="{ transform: `scale(${previewScale})`, transformOrigin: 'top left', width: '1280px', height: '720px' }"
            >
              <SlideRenderer :slide="(editorPreviewSlide as any)" :slide-index="activeSlide" :media-map="mediaMap" />
            </div>
          </div>
        </div>
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

        <div class="edit-json-inner">
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
              <button class="btn btn-ghost edit-add-btn" @click="addItem(section.key, section.createItem)">
                <span class="i-carbon:add" /> 添加
              </button>
            </div>
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
                  <button class="btn btn-ghost edit-add-btn" @click="addItem(`${section.key}[${index}].${nested.key}`, nested.createItem)">
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
                <button class="btn btn-ghost edit-add-btn" @click="addItem(`${section.key}.${subList.key}`, subList.createItem)">
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
            <span :class="KIND_META[k].icon" class="kp-card-icon" />
            <span class="kp-card-label">{{ KIND_META[k].label }}</span>
            <span class="kp-card-key">{{ k }}</span>
            <span class="kp-card-desc">{{ KIND_META[k].desc }}</span>
          </button>
        </div>
      </div>
    </div>
  </Teleport>
</template>
