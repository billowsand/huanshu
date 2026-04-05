import { ref, computed, watch, nextTick } from 'vue'
import type { SlideBlueprint, SlideKind, AspectRatio } from '../components/types'
import { ASPECT_DIMENSIONS } from '../components/types'
import { useProjectsStore } from '../stores/projects'
import { useGenerationStore } from '../stores/generation'

type KindMeta = { label: string; icon: string; desc: string }

export const KIND_META: Record<SlideKind, KindMeta> = {
  cover:        { label: '封面',    icon: 'i-carbon:home',         desc: '演示文稿首页，标题 + 副标题 + 标签' },
  closing:      { label: '结束页',  icon: 'i-carbon:collapse-categories', desc: '固定收尾页，感谢聆听 + 批评指正' },
  overview:     { label: '概览',    icon: 'i-carbon:list-bulleted', desc: '章节导航，数字编号列表' },
  section_intro:{ label: '章节导览', icon: 'i-carbon:roadmap',      desc: '章节开场说明 + 子主题预览卡片' },
  feature_grid: { label: '特性网格', icon: 'i-carbon:grid',         desc: '多列卡片展示核心特性' },
  spotlight:    { label: '聚焦',    icon: 'i-carbon:star-filled',   desc: '左侧图片 + 右侧面板列表' },
  split_layers: { label: '分层',    icon: 'i-carbon:split-screen',  desc: '左侧解释卡片 + 右侧分层图/信息图' },
  section_list: { label: '列表',    icon: 'i-carbon:task-view',     desc: '步骤编号 + 内容列表' },
  focus_example:{ label: '示例',    icon: 'i-carbon:zoom-in',       desc: '要点列表 + 右侧类比示例' },
  outcome_grid: { label: '成果网格', icon: 'i-carbon:chart-bar',    desc: '成果 / 结论的卡片网格' },
  center_grid:  { label: '中心网格', icon: 'i-carbon:grid-dots',    desc: '居中大字 + 下方图标网格' },
  timeline:     { label: '时间线',  icon: 'i-carbon:time',          desc: '横向时间轴事件序列' },
  step_flow:    { label: '步骤流',  icon: 'i-carbon:flow',          desc: '横向 / 纵向步骤流程' },
  process:      { label: '流程',    icon: 'i-carbon:process',       desc: '多阶段流程任务分解' },
  compare:      { label: '对比',    icon: 'i-carbon:compare',       desc: '左右双栏对比分析' },
  issue_stack:  { label: '问题堆叠', icon: 'i-carbon:warning-alt',  desc: '纵向堆叠的问题/挑战说明卡片' },
  swot:         { label: 'SWOT',   icon: 'i-carbon:analytics',     desc: '四象限战略分析矩阵' },
  infographic:  { label: '信息图',  icon: 'i-carbon:chart-treemap',  desc: '基于 AntV Infographic 的数据可视化信息图' },
}

export const ALL_KINDS = Object.keys(KIND_META) as SlideKind[]

export const KIND_FIELDS: Record<SlideKind, string[]> = {
  cover:        ['kind', 'title', 'subtitle', 'badges'],
  closing:      ['kind', 'title', 'subtitle', 'note', 'badges'],
  overview:     ['kind', 'section', 'title', 'note', 'overview_items'],
  section_intro:['kind', 'section', 'badge', 'title', 'subtitle', 'note', 'cards'],
  feature_grid: ['kind', 'section', 'title', 'subtitle', 'note', 'cards'],
  spotlight:    ['kind', 'label', 'label_tone', 'title', 'panels', 'images', 'image', 'placeholder', 'side_width'],
  split_layers: ['kind', 'section', 'title', 'subtitle', 'left_items', 'layers', 'layers_infographic_syntax', 'footer'],
  section_list: ['kind', 'section', 'title', 'subtitle', 'list_items'],
  focus_example:['kind', 'section', 'title', 'subtitle', 'points', 'example_title', 'example_body', 'ranking'],
  outcome_grid: ['kind', 'section', 'title', 'subtitle', 'note', 'cards'],
  center_grid:  ['kind', 'badge', 'title', 'accent', 'center_items', 'footer'],
  timeline:     ['kind', 'section', 'title', 'subtitle', 'timeline_events', 'footer'],
  step_flow:    ['kind', 'section', 'title', 'subtitle', 'direction', 'steps', 'footer'],
  process:      ['kind', 'section', 'title', 'subtitle', 'summary', 'phases', 'footer'],
  compare:      ['kind', 'section', 'title', 'subtitle', 'compare_data', 'footer'],
  issue_stack:  ['kind', 'section', 'title', 'subtitle', 'cards', 'footer'],
  swot:         ['kind', 'section', 'title', 'subtitle', 'swot_data'],
  infographic:  ['kind', 'section', 'title', 'subtitle', 'infographic_syntax', 'footer'],
}

export const KIND_DEFAULTS: Record<SlideKind, Partial<SlideBlueprint>> = {
  cover: {
    title: 'AI 战略规划报告',
    subtitle: '从业务机会到落地路径的完整说明',
    badges: ['2026 Q2', '内部汇报', '30 分钟'],
  },
  closing: {
    title: '感谢聆听',
    subtitle: '欢迎继续交流方案细节与排期',
    note: '如需会后材料，可补充联系人、二维码或下一步安排。',
    badges: ['Q&A', 'Follow-up'],
  },
  overview: {
    section: '01',
    title: '内容概览',
    note: '建议保持 3-5 个模块，便于听众快速建立全局认知。',
    overview_items: [
      { number: '01', title: '现状判断', desc: '当前业务痛点、市场变化与内部约束。' },
      { number: '02', title: '方案设计', desc: '核心策略、能力架构与关键动作。' },
      { number: '03', title: '落地计划', desc: '阶段目标、资源需求与里程碑安排。' },
    ],
  },
  section_intro: {
    section: '02',
    badge: '章节导览',
    title: '解决方案总览',
    subtitle: '先看结构，再展开每个关键模块。',
    note: '卡片标题建议短，正文控制在一行到两行。',
    cards: [
      { title: '目标定义', icon: 'i-carbon:target', tone: 'blue', body: '明确业务目标、成效口径与验收标准。', tag: 'Why' },
      { title: '能力建设', icon: 'i-carbon:skill-level-advanced', tone: 'emerald', body: '搭建数据、模型、流程三层能力。', tag: 'What' },
      { title: '推进机制', icon: 'i-carbon:workflow-automation', tone: 'amber', body: '建立跨团队协同与持续优化闭环。', tag: 'How' },
    ],
  },
  feature_grid: {
    section: '03',
    title: '核心特性',
    subtitle: '适合展示 3-6 个能力点，每个卡片一条主信息。',
    note: '可补充 `items`、`conclusion`、`risk` 等字段增强信息层次。',
    cards: [
      {
        title: '统一知识入口',
        subtitle: '降低信息查找成本',
        icon: 'i-carbon:data-base',
        tone: 'blue',
        body: '把分散文档、案例、规范集中到一个检索入口。',
        items: ['支持语义搜索', '权限可控', '可接业务系统'],
      },
      {
        title: '智能内容生成',
        subtitle: '缩短方案产出时间',
        icon: 'i-carbon:machine-learning-model',
        tone: 'violet',
        body: '基于模板与上下文快速生成初稿，再由人工精修。',
        conclusion: '适合标准化程度高的交付场景',
      },
      {
        title: '过程可追踪',
        subtitle: '方便复盘与审计',
        icon: 'i-carbon:task-complete',
        tone: 'emerald',
        body: '保留版本、输入来源和关键决策记录。',
        risk: '需提前定义记录粒度',
      },
    ],
  },
  spotlight: {
    label: '重点展示',
    label_tone: 'blue',
    title: '重点案例拆解',
    image: '',
    images: [],
    placeholder: '可填写 media:12、文件路径或图片 URL',
    side_width: '42%',
    panels: [
      {
        title: '背景',
        kind: 'text',
        icon: 'i-carbon:document',
        tone: 'blue',
        body: '客户希望在 6 周内完成销售线索分级与自动跟进方案验证。',
      },
      {
        title: '关键动作',
        kind: 'flow',
        icon: 'i-carbon:flow-stream',
        tone: 'emerald',
        items: ['梳理字段', '搭建评分模型', '接入 CRM', '验证转化'],
        highlight: '从人工分配改为自动优先级推荐',
      },
      {
        title: '交付结果',
        kind: 'steps',
        icon: 'i-carbon:chart-line',
        tone: 'amber',
        steps: ['线索响应时效缩短 60%', '销售跟进更聚焦', '复盘数据可量化'],
      },
    ],
  },
  split_layers: {
    section: '04',
    title: '分层结构',
    subtitle: '适合表达"左侧说明 + 右侧架构层次"的页面。',
    left_items: [
      { step: '01', title: '业务层', body: '围绕用户场景定义服务目标。', icon: 'i-carbon:user-service' },
      { step: '02', title: '能力层', body: '沉淀通用流程、工具与规则。', icon: 'i-carbon:layers' },
      { step: '03', title: '数据层', body: '提供指标、知识和底座支撑。', icon: 'i-carbon:data-base' },
    ],
    layers: [
      { title: '前台应用', meta: '面向销售、运营、客服的工作台', tone: 'blue' },
      { title: '流程编排', meta: '任务流、审批流、智能触发器', tone: 'emerald' },
      { title: '模型与规则', meta: '分类、评分、召回、推荐', tone: 'violet' },
      { title: '数据底座', meta: '主数据、知识库、行为日志', tone: 'amber' },
    ],
    layers_infographic_syntax: 'infographic hierarchy-tree-horizontal\\ndata\\n  root\\n    label 智能运营架构\\n    children\\n      - label 前台应用\\n        children\\n          - label 销售工作台\\n          - label 运营看板\\n      - label 流程编排\\n        children\\n          - label 触发规则\\n          - label 审批流\\n      - label 模型与规则\\n        children\\n          - label 评分\\n          - label 推荐\\n      - label 数据底座\\n        children\\n          - label 主数据\\n          - label 知识库',
    footer: '建议层数控制在 3-5 层，避免右侧过密。',
  },
  section_list: {
    section: '05',
    title: '执行清单',
    subtitle: '适合罗列步骤、要点、任务项。',
    list_items: [
      { step: '01', title: '明确目标', body: '先确认这页要回答的问题，避免只堆信息。', icon: 'i-carbon:target' },
      { step: '02', title: '列关键点', body: '每条聚焦一个结论，正文尽量不超过两行。', icon: 'i-carbon:idea' },
      { step: '03', title: '补充证据', body: '必要时加入数据、案例或素材引用。', icon: 'i-carbon:chart-line-data' },
    ],
  },
  focus_example: {
    section: '06',
    title: '方法说明',
    subtitle: '左边讲原则，右边给例子，便于理解抽象概念。',
    points: [
      { step: '01', title: '先给判断标准', body: '说明什么情况下该采用这套方法。', icon: 'i-carbon:rule' },
      { step: '02', title: '再讲落地动作', body: '把动作拆成可执行的步骤。', icon: 'i-carbon:play-outline-filled' },
      { step: '03', title: '最后展示效果', body: '给出产出、收益或变化。', icon: 'i-carbon:result' },
    ],
    example_title: '类比例子',
    example_body: '可以把这套机制理解成"先搭导航，再接数据，再让流程自动跑起来"。',
    ranking: [
      { index: 'A', label: '优先统一口径', meta: '先对齐概念和指标，再谈自动化。' },
      { index: 'B', label: '优先打通流程', meta: '确保数据和执行动作能串起来。' },
      { index: 'C', label: '优先做局部试点', meta: '用小范围结果验证价值。', muted: true },
    ],
  },
  outcome_grid: {
    section: '07',
    title: '阶段成果',
    subtitle: '适合总结结果、结论、价值输出。',
    note: '可以把 `footer_tag`、`conclusion` 当作每个成果卡片的收束语。',
    cards: [
      {
        title: '效率提升',
        icon: 'i-carbon:flash',
        tone: 'amber',
        body: '提案准备时间从 2 天缩短到 4 小时。',
        conclusion: '团队可把精力转向深度判断',
      },
      {
        title: '质量更稳',
        icon: 'i-carbon:checkmark-outline',
        tone: 'emerald',
        body: '输出结构更统一，漏项明显减少。',
        footer_tag: '可复用',
      },
      {
        title: '协作顺畅',
        icon: 'i-carbon:group',
        tone: 'blue',
        body: '销售、方案、交付之间共享同一套上下文。',
        footer_tone: 'blue',
      },
    ],
  },
  center_grid: {
    badge: '核心观点',
    title: '战略落地不是单点优化，而是能力系统建设',
    accent: '统一口径、流程串联、数据闭环',
    center_items: [
      { title: '目标', desc: '先定义成效', icon: 'i-carbon:target', tone: 'blue' },
      { title: '机制', desc: '再固化流程', icon: 'i-carbon:workflow-automation', tone: 'emerald' },
      { title: '数据', desc: '持续反馈优化', icon: 'i-carbon:data-vis-4', tone: 'violet' },
      { title: '组织', desc: '明确协作责任', icon: 'i-carbon:user-multiple', tone: 'amber' },
    ],
    footer: '中间标题适合放一句最重要的结论。',
  },
  timeline: {
    section: '08',
    title: '推进时间线',
    subtitle: '用时间轴表达阶段节奏与里程碑。',
    timeline_events: [
      { date: '第 1 周', title: '需求对齐', body: '完成目标确认、范围边界和数据盘点。', tone: 'blue', icon: 'i-carbon:flag' },
      { date: '第 2-3 周', title: '方案搭建', body: '输出流程设计、页面原型和字段映射。', tone: 'violet', icon: 'i-carbon:build-tool' },
      { date: '第 4-5 周', title: '试点运行', body: '接入真实业务，跟踪异常和效果。', tone: 'emerald', icon: 'i-carbon:play-filled' },
      { date: '第 6 周', title: '复盘优化', body: '沉淀 SOP、指标面板和后续迭代项。', tone: 'amber', icon: 'i-carbon:renew' },
    ],
    footer: '时间点可写日期，也可写"第 N 周 / 阶段名"。',
  },
  step_flow: {
    section: '09',
    title: '实施步骤',
    subtitle: '横向适合展示流程，纵向适合展示说明更长的步骤。',
    direction: 'horizontal',
    steps: [
      { title: '识别问题', body: '确认最值得优先解决的业务卡点。', icon: 'i-carbon:search', tone: 'blue' },
      { title: '设计机制', body: '定义流程、规则和人机协作方式。', icon: 'i-carbon:flow', tone: 'violet' },
      { title: '接入系统', body: '把能力挂到真实业务链路中。', icon: 'i-carbon:application-web', tone: 'emerald' },
      { title: '持续优化', body: '基于数据反馈迭代策略。', icon: 'i-carbon:chart-line', tone: 'amber' },
    ],
    footer: '如需竖排，可把 direction 改为 vertical。',
  },
  process: {
    section: '10',
    title: '执行流程',
    subtitle: '适合多阶段、多步骤的任务分解。',
    phases: [
      {
        phase: 'Phase 1',
        title: '准备阶段',
        icon: 'i-carbon:document-preliminary',
        tone: 'blue',
        steps: [
          { label: '明确负责人', desc: '确定项目 owner 与参与角色。' },
          { label: '梳理输入物', desc: '准备文档、数据、历史案例。' },
        ],
        highlight: '确保启动前信息齐备',
      },
      {
        phase: 'Phase 2',
        title: '实施阶段',
        icon: 'i-carbon:play-outline',
        tone: 'emerald',
        steps: [
          { label: '搭建流程', desc: '把方案转成可运行链路。' },
          { label: '小范围试点', desc: '先验证关键环节效果。' },
        ],
      },
      {
        phase: 'Phase 3',
        title: '优化阶段',
        icon: 'i-carbon:analytics',
        tone: 'amber',
        steps: [
          { label: '复盘问题', desc: '识别异常与低效节点。' },
          { label: '沉淀标准', desc: '固化 SOP 与复用模板。' },
        ],
      },
    ],
    footer: '每个阶段建议 2-4 个步骤，避免页面过满。',
  },
  compare: {
    section: '11',
    title: '方案对比',
    subtitle: '左右两栏适合比较不同路线、工具或策略。',
    compare_data: {
      mode: 'side-by-side',
      left: {
        title: '方案 A：快速上线',
        tone: 'blue',
        icon: 'i-carbon:flash',
        items: [
          { label: '优势', desc: '投入小、启动快、便于试错。', highlight: true },
          { label: '代价', desc: '后期扩展性一般，规则维护成本较高。' },
          { label: '适用场景', desc: '需求还不稳定，先验证业务价值。' },
        ],
        conclusion: '适合先跑通闭环',
      },
      right: {
        title: '方案 B：体系化建设',
        tone: 'emerald',
        icon: 'i-carbon:enterprise',
        items: [
          { label: '优势', desc: '长期复用性强，适合多团队推广。', highlight: true },
          { label: '代价', desc: '前期协调与建设成本更高。' },
          { label: '适用场景', desc: '业务成熟，准备做规模化复制。' },
        ],
        conclusion: '适合中长期投入',
      },
    },
    footer: '需要上下对比时，可调整 mode 字段。',
  },
  issue_stack: {
    section: '12',
    title: '为什么这件事难做',
    subtitle: '核心障碍、根因与可能的突破口',
    cards: [
      {
        title: '黑盒中的黑盒',
        icon: 'i-carbon:ibm-watson-machine-learning',
        tone: 'blue',
        body: '底层能力强依赖框架与通信栈本身，定位链路深、调试窗口窄，管理成本高。',
        items: ['难调试', '难追踪', '难治理', '难控制'],
      },
      {
        title: '恢复链路不完整',
        icon: 'i-carbon:warning-other',
        tone: 'green',
        body: '即使上层已有编排系统，真正的自愈、自恢复和自动缓解仍然难以稳定落地。',
        items: ['难自愈', '难恢复', '难缓解'],
      },
      {
        title: '可观测性缺块',
        icon: 'i-carbon:network-4',
        tone: 'amber',
        body: '驱动、硬件、GPU 与网络故障往往跨层出现，根因信息分散，日志与事件不完整。',
        items: ['根因难判定', '事件难采集', '观测不足'],
      },
    ],
    footer: '适合 2-4 个纵向问题块，每块一句主判断 + 若干补充标签。',
  },
  swot: {
    section: '12',
    title: 'SWOT 分析',
    subtitle: '用于总结内外部条件并给出战略建议。',
    swot_data: {
      quadrants: [
        { key: 'S', title: '优势 Strengths', tone: 'emerald', icon: 'i-carbon:thumbs-up', items: ['已有行业案例积累', '团队对业务流程熟悉'], summary: '内部基础较好' },
        { key: 'W', title: '劣势 Weaknesses', tone: 'amber', icon: 'i-carbon:warning', items: ['数据口径不统一', '系统集成链路较长'], summary: '落地复杂度偏高' },
        { key: 'O', title: '机会 Opportunities', tone: 'blue', icon: 'i-carbon:growth', items: ['客户对自动化需求提升', '管理层支持试点预算'], summary: '外部窗口期明确' },
        { key: 'T', title: '威胁 Threats', tone: 'rose', icon: 'i-carbon:storm-tracker', items: ['竞品推进速度快', '合规要求可能收紧'], summary: '需要控制推进节奏' },
      ],
      strategy: '建议采用"试点先行 + 能力沉淀"策略，先快速证明价值，再逐步扩展。',
    },
  },
  infographic: {
    section: '13',
    title: '关键数据一览',
    subtitle: '用可视化方式呈现核心信息。',
    infographic_syntax: 'infographic list-grid-3-col\ndata\n  lists\n    - label 核心指标\n      desc 关键数据描述\n    - label 增长趋势\n      desc 趋势描述\n    - label 关键发现\n      desc 发现描述',
    footer: '信息图模板可替换为其他内置模板。',
  },
}

const SHARED_TEMPLATE_FIELDS: (keyof SlideBlueprint)[] = [
  'section', 'title', 'subtitle', 'badge', 'accent', 'note', 'label', 'label_tone',
  'image', 'images', 'placeholder', 'side_width', 'badges', 'footer', 'aspect_ratio',
]

export function useSlideEditor(options: {
  activeSlide: () => number
  setActiveSlide: (v: number) => void
  currentStep: () => number
  projectId: () => number | null
  mediaMap: () => Record<string, string>
  blueprints: () => SlideBlueprint[]
}) {
  const projectsStore = useProjectsStore()
  const genStore = useGenerationStore()

  const jsonText = ref('')
  const jsonError = ref('')
  const liveSlide = ref<SlideBlueprint | null>(null)
  const hasUnsavedChanges = ref(false)
  const editorSaving = ref(false)
  const showKindPicker = ref(false)
  const kindPickerMode = ref<'add' | 'change'>('add')
  const previewArea = ref<HTMLElement | null>(null)
  const previewScale = ref(0.45)
  const jsonTextareaRef = ref<HTMLTextAreaElement | null>(null)
  let editorRo: ResizeObserver | null = null

  function deepClone<T>(value: T): T {
    return JSON.parse(JSON.stringify(value)) as T
  }

  function hasMeaningfulValue(value: unknown) {
    if (value === undefined || value === null) return false
    if (typeof value === 'string') return value.trim().length > 0
    if (Array.isArray(value)) return value.length > 0
    if (typeof value === 'object') return Object.keys(value as Record<string, unknown>).length > 0
    return true
  }

  function createKindBlueprint(kind: SlideKind, seed?: SlideBlueprint | null): SlideBlueprint {
    const blueprint = deepClone({ ...KIND_DEFAULTS[kind], kind }) as SlideBlueprint
    if (!seed) return blueprint
    for (const field of SHARED_TEMPLATE_FIELDS) {
      const value = seed[field]
      if (hasMeaningfulValue(value)) {
        ;(blueprint as unknown as Record<string, unknown>)[field] = deepClone(value)
      }
    }
    blueprint.kind = kind
    return blueprint
  }

  function filterSlideJson(slide: SlideBlueprint): Record<string, unknown> {
    const fields = KIND_FIELDS[slide.kind as SlideKind] ?? Object.keys(slide)
    const out: Record<string, unknown> = {}
    for (const f of fields) {
      const v = (slide as unknown as Record<string, unknown>)[f]
      if (v !== undefined) out[f] = v
    }
    return out
  }

  function initEditorSlide(slide: SlideBlueprint) {
    const cloned = deepClone(slide)
    jsonText.value = JSON.stringify(filterSlideJson(cloned), null, 2)
    liveSlide.value = cloned
    jsonError.value = ''
    hasUnsavedChanges.value = false
  }

  function replaceEditorDraft(slide: SlideBlueprint, markUnsaved = true) {
    const cloned = deepClone(slide)
    liveSlide.value = cloned
    jsonText.value = JSON.stringify(filterSlideJson(cloned), null, 2)
    jsonError.value = ''
    hasUnsavedChanges.value = markUnsaved
  }

  function updateEditorScale() {
    if (!previewArea.value) return
    const w = previewArea.value.clientWidth - 32
    const h = previewArea.value.clientHeight - 32
    if (w > 0 && h > 0) {
      const bp = editorPreviewSlide.value
      const ratio = bp?.aspect_ratio ?? 'ratio_16x9'
      const dims = ASPECT_DIMENSIONS[ratio]
      previewScale.value = Math.min(w / dims.w, h / dims.h)
    }
  }

  watch(previewArea, (el) => {
    editorRo?.disconnect()
    if (!el) return
    editorRo = new ResizeObserver(updateEditorScale)
    editorRo.observe(el)
    updateEditorScale()
  })

  watch(() => options.activeSlide(), (idx) => {
    if (options.currentStep() !== 3) return
    const s = options.blueprints()[idx]
    if (s) initEditorSlide(s as unknown as SlideBlueprint)
  })

  watch(() => options.currentStep(), async (step) => {
    if (step === 3 && options.blueprints()[options.activeSlide()]) {
      await nextTick()
      initEditorSlide(options.blueprints()[options.activeSlide()] as unknown as SlideBlueprint)
      updateEditorScale()
    }
  })

  function onEditorJsonInput(value: string) {
    jsonText.value = value
    try {
      liveSlide.value = JSON.parse(jsonText.value) as SlideBlueprint
      jsonError.value = ''
      hasUnsavedChanges.value = true
    } catch (e: unknown) {
      jsonError.value = (e as Error).message
    }
  }

  function onEditorJsonKeydown(e: KeyboardEvent) {
    if (e.key !== 'Tab') return
    e.preventDefault()
    const el = e.target as HTMLTextAreaElement
    const s = el.selectionStart
    const en = el.selectionEnd
    jsonText.value = jsonText.value.slice(0, s) + '  ' + jsonText.value.slice(en)
    nextTick(() => { el.selectionStart = el.selectionEnd = s + 2 })
  }

  function formatEditorJson() {
    try {
      const parsed = JSON.parse(jsonText.value) as SlideBlueprint
      replaceEditorDraft(parsed, true)
    } catch {
      /* keep */
    }
  }

  async function applyEditorChanges() {
    if (jsonError.value || !liveSlide.value || !options.projectId()) return
    editorSaving.value = true
    try {
      const parsed = JSON.parse(jsonText.value) as SlideBlueprint
      genStore.blueprints[options.activeSlide()] = parsed as unknown as SlideBlueprint
      await projectsStore.saveBlueprints(options.projectId()!, genStore.blueprints as unknown as SlideBlueprint[])
      liveSlide.value = parsed
      hasUnsavedChanges.value = false
    } catch (e) {
      jsonError.value = String(e)
    } finally {
      editorSaving.value = false
    }
  }

  function openKindPicker(mode: 'add' | 'change') {
    kindPickerMode.value = mode
    showKindPicker.value = true
  }

  function onKindPicked(kind: SlideKind) {
    if (kindPickerMode.value === 'change' && liveSlide.value) {
      const updated = createKindBlueprint(kind, liveSlide.value)
      liveSlide.value = updated as SlideBlueprint
      jsonText.value = JSON.stringify(filterSlideJson(updated as SlideBlueprint), null, 2)
      jsonError.value = ''
      hasUnsavedChanges.value = true
    } else {
      const newSlide = createKindBlueprint(kind)
      const at = options.activeSlide() + 1
      genStore.blueprints.splice(at, 0, newSlide as unknown as SlideBlueprint)
      options.setActiveSlide(at)
    }
    showKindPicker.value = false
  }

  async function deleteEditorSlide(idx: number) {
    if (genStore.blueprints.length <= 1) return
    genStore.blueprints.splice(idx, 1)
    options.setActiveSlide(Math.min(idx, genStore.blueprints.length - 1))
    if (options.projectId()) await projectsStore.saveBlueprints(options.projectId()!, genStore.blueprints as unknown as SlideBlueprint[])
  }

  async function moveEditorSlide(dir: -1 | 1) {
    const i = options.activeSlide()
    const j = i + dir
    if (j < 0 || j >= genStore.blueprints.length) return
    ;[genStore.blueprints[i], genStore.blueprints[j]] = [genStore.blueprints[j], genStore.blueprints[i]]
    options.setActiveSlide(j)
    if (options.projectId()) await projectsStore.saveBlueprints(options.projectId()!, genStore.blueprints as unknown as SlideBlueprint[])
  }

  function insertMediaUrl(ref: string) {
    const el = jsonTextareaRef.value
    if (!el) { navigator.clipboard.writeText(`"${ref}"`).catch(() => {}); return }
    const s = el.selectionStart
    const en = el.selectionEnd
    const quoted = `"${ref}"`
    jsonText.value = jsonText.value.slice(0, s) + quoted + jsonText.value.slice(en)
    nextTick(() => {
      el.selectionStart = el.selectionEnd = s + quoted.length
      el.focus()
      onEditorJsonInput(jsonText.value)
    })
  }

  const editorKind = computed(() =>
    (liveSlide.value?.kind ?? options.blueprints()[options.activeSlide()]?.kind ?? 'section_list') as SlideKind
  )
  const editorPreviewSlide = computed(() =>
    liveSlide.value ?? (options.blueprints()[options.activeSlide()] as unknown as SlideBlueprint ?? null)
  )
  const currentSlideDims = computed(() => {
    const bp = editorPreviewSlide.value
    const ratio = bp?.aspect_ratio ?? 'ratio_16x9'
    return ASPECT_DIMENSIONS[ratio]
  })
  const editorStageW = computed(() => Math.round(currentSlideDims.value.w * previewScale.value))
  const editorStageH = computed(() => Math.round(currentSlideDims.value.h * previewScale.value))

  return {
    jsonText,
    jsonError,
    liveSlide,
    hasUnsavedChanges,
    editorSaving,
    showKindPicker,
    kindPickerMode,
    previewArea,
    previewScale,
    jsonTextareaRef,
    editorKind,
    editorPreviewSlide,
    editorStageW,
    editorStageH,
    currentSlideDims,
    replaceEditorDraft,
    onEditorJsonInput,
    onEditorJsonKeydown,
    formatEditorJson,
    applyEditorChanges,
    openKindPicker,
    onKindPicked,
    deleteEditorSlide,
    moveEditorSlide,
    insertMediaUrl,
    initEditorSlide,
  }
}
