import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { SlideBlueprint, AspectRatio } from '../components/types'
export type { SlideBlueprint, AspectRatio } from '../components/types'

export type GenStage =
  | 'idle'
  | 'start'
  | 'init'
  | 'page_plan'
  | 'generating'
  | 'assembling'
  | 'validate'
  | 'done'
  | 'error'

export type PageStage = 'pending' | 'planning' | 'layout' | 'content' | 'normalizing' | 'validating' | 'done' | 'error'

export interface PageStatusEvent {
  slide_index: number
  stage: PageStage
  message?: string
  blueprint?: unknown
}

export interface GenEvent {
  stage: GenStage
  message: string
  progress?: number
  slide_index?: number
  blueprint?: unknown
}

export interface GenerationRun {
  id: number
  project_id?: number | null
  status: string
  current_stage?: string | null
  source_markdown: string
  frontmatter_title?: string | null
  debug_dir: string
  created_at: number
  updated_at: number
  finished_at?: number | null
}

export interface GenerationLogEntry {
  id: number
  run_id: number
  seq: number
  slide_index?: number | null
  stage: string
  kind: string
  title: string
  summary: string
  detail: Record<string, unknown> | null
  important: boolean
  created_at: number
}

const STAGE_LABELS: Record<string, string> = {
  idle: '等待',
  start: '初始化',
  init: '验证连接',
  page_plan: '规划页面',
  generating: '生成中',
  assembling: '组装中',
  validate: '校验中',
  done: '完成',
  error: '出错',
}

export const useGenerationStore = defineStore('generation', () => {
  const stage = ref<GenStage>('idle')
  const message = ref('')
  const progress = ref(0)
  const blueprints = ref<SlideBlueprint[]>([])
  const completedSlides = ref<Set<number>>(new Set())
  const running = ref(false)
  const lastError = ref('')
  const logs = ref<GenerationLogEntry[]>([])
  const currentRun = ref<GenerationRun | null>(null)
  const activeProjectId = ref<number | null>(null)
  const pageStatuses = ref<Map<number, { stage: PageStage, message?: string }>>(new Map())

  const stageLabel = computed(() => STAGE_LABELS[stage.value] ?? stage.value)
  const slideCount = computed(() => blueprints.value.length)

  let unlistenProgress: (() => void) | null = null
  let unlistenSlide: (() => void) | null = null
  let unlistenLog: (() => void) | null = null
  let unlistenError: (() => void) | null = null
  let unlistenPageStatus: (() => void) | null = null

  async function startListening() {
    if (unlistenProgress || unlistenSlide || unlistenLog || unlistenError || unlistenPageStatus)
      return

    unlistenProgress = await listen<GenEvent>('gen:progress', (e) => {
      const ev = e.payload
      stage.value = ev.stage as GenStage
      message.value = ev.message
      if (currentRun.value) {
        currentRun.value = {
          ...currentRun.value,
          status: ev.stage === 'done' ? 'done' : 'running',
          current_stage: ev.stage,
          updated_at: Math.floor(Date.now() / 1000),
        }
      }
      if (ev.progress !== undefined && ev.progress !== null) {
        progress.value = ev.progress
      }
    })

    unlistenLog = await listen<GenerationLogEntry>('gen:log', (e) => {
      const ev = e.payload
      logs.value.push(ev)
      if (!currentRun.value || currentRun.value.id !== ev.run_id) {
        currentRun.value = {
          id: ev.run_id,
          project_id: activeProjectId.value,
          status: 'running',
          current_stage: ev.stage,
          source_markdown: '',
          frontmatter_title: null,
          debug_dir: '',
          created_at: ev.created_at,
          updated_at: ev.created_at,
          finished_at: null,
        }
      } else {
        currentRun.value = {
          ...currentRun.value,
          status: 'running',
          current_stage: ev.stage,
          updated_at: ev.created_at,
        }
      }
    })

    /** Ensure the blueprints array is large enough for index-based insertion */
    function ensureBlueprintSlot(idx: number) {
      while (blueprints.value.length <= idx) {
        blueprints.value.push(null as unknown as SlideBlueprint)
      }
    }

    unlistenSlide = await listen<GenEvent>('gen:slide_ready', (e) => {
      const ev = e.payload
      if (ev.slide_index !== undefined && ev.blueprint) {
        const idx = ev.slide_index
        ensureBlueprintSlot(idx)
        blueprints.value[idx] = ev.blueprint as SlideBlueprint
        completedSlides.value.add(idx)
      }
    })

    unlistenPageStatus = await listen<PageStatusEvent>('gen:page_status', (e) => {
      const ev = e.payload
      pageStatuses.value.set(ev.slide_index, {
        stage: ev.stage,
        message: ev.message ?? undefined,
      })
      // Also update blueprints if a blueprint is provided
      if (ev.blueprint) {
        const idx = ev.slide_index
        ensureBlueprintSlot(idx)
        blueprints.value[idx] = ev.blueprint as SlideBlueprint
        completedSlides.value.add(idx)
      }
    })

    unlistenError = await listen<GenEvent>('gen:error', (e) => {
      stage.value = 'error'
      message.value = e.payload.message
      lastError.value = e.payload.message
      running.value = false
      if (currentRun.value) {
        currentRun.value = {
          ...currentRun.value,
          status: 'error',
          current_stage: 'error',
          updated_at: Math.floor(Date.now() / 1000),
        }
      }
    })
  }

  function stopListening() {
    unlistenProgress?.()
    unlistenSlide?.()
    unlistenLog?.()
    unlistenError?.()
    unlistenPageStatus?.()
    unlistenProgress = null
    unlistenSlide = null
    unlistenLog = null
    unlistenError = null
    unlistenPageStatus = null
  }

  async function generate(mdContent: string, frontmatterTitle?: string, granularity?: string, aspectRatio?: AspectRatio, projectId?: number | null) {
    running.value = true
    lastError.value = ''
    blueprints.value = []
    completedSlides.value = new Set()
    logs.value = []
    currentRun.value = null
    activeProjectId.value = projectId ?? null
    pageStatuses.value = new Map()
    stage.value = 'start'
    message.value = ''
    progress.value = 0

    await startListening()

    try {
      const count = await invoke<number>('generate_slides', {
        mdContent,
        frontmatterTitle: frontmatterTitle ?? null,
        granularity: granularity ?? null,
        aspectRatio: aspectRatio ?? null,
      })
      // After generation, load final blueprints
      const final = await invoke<SlideBlueprint[]>('get_blueprints')
      blueprints.value = final
      stage.value = 'done'
      progress.value = 1
      message.value = `生成完成，共 ${count} 张幻灯片`
    } catch (e: unknown) {
      lastError.value = String(e)
      stage.value = 'error'
      message.value = String(e)
    } finally {
      running.value = false
      stopListening()
    }
  }

  async function repairSlide(index: number, feedback: string) {
    const repaired = await invoke<SlideBlueprint>('repair_slide', { index, feedback })
    blueprints.value[index] = repaired
    return repaired
  }

  function reset() {
    stopListening()
    stage.value = 'idle'
    message.value = ''
    progress.value = 0
    blueprints.value = []
    completedSlides.value = new Set()
    running.value = false
    lastError.value = ''
    logs.value = []
    currentRun.value = null
    activeProjectId.value = null
    pageStatuses.value = new Map()
  }

  function loadFromJson(json: string) {
    try {
      const parsed = JSON.parse(json) as SlideBlueprint[]
      blueprints.value = parsed
      stage.value = 'done'
      progress.value = 1
      message.value = `已加载 ${parsed.length} 张幻灯片`
    } catch {
      // ignore parse errors
    }
  }

  async function loadLatestRun(projectId: number) {
    activeProjectId.value = projectId
    const run = await invoke<GenerationRun | null>('get_latest_generation_run', { projectId })
    currentRun.value = run
    if (!run) {
      logs.value = []
      return null
    }
    logs.value = await invoke<GenerationLogEntry[]>('get_generation_logs', { runId: run.id })
    if (run.status === 'running') {
      running.value = true
      stage.value = (run.current_stage as GenStage) ?? 'start'
      message.value = message.value || '正在恢复生成进度...'
      await startListening()
    } else if (!running.value) {
      running.value = false
      if (run.status === 'error') {
        stage.value = 'error'
      } else if (run.status === 'done' && blueprints.value.length > 0) {
        stage.value = 'done'
      }
    }
    return run
  }

  return {
    stage, message, progress, blueprints, completedSlides,
    running, lastError, logs, currentRun, stageLabel, slideCount,
    activeProjectId, pageStatuses,
    generate, repairSlide, reset, loadFromJson, loadLatestRun,
  }
})
