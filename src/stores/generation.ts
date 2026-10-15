// 规则已移植到 huanshu_core::generation_progress（crates/huanshu-core/src/generation_progress.rs），改一边必须改另一边；一致性由 crates/huanshu-core/tests/generation_progress.rs 快照守住。
import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { SlideBlueprint, AspectRatio, SlideKind } from '../components/types'
import type { PageProgressStatus } from '../utils/generationProgress'
import { countReadyBlueprints } from '../utils/generationProgress'
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
  waiting?: boolean
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

/** Result of `retry_generation_page`: the page is saved even while other pages still fail. */
export interface RetryPageResult {
  slide: SlideBlueprint
  /** Whole deck passes validation; the run is now done. */
  deck_ready: boolean
  /** What still blocks the deck when it is not ready. */
  remaining: string
  /** Content pages that still fail validation. */
  invalid_pages: number[]
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
  const pageStatuses = ref<Map<number, PageProgressStatus>>(new Map())
  const retryingPages = ref<Set<number>>(new Set())
  const queuedRetries = new Map<number, SlideKind | undefined>()
  let generationVersion = 0

  const stageLabel = computed(() => STAGE_LABELS[stage.value] ?? stage.value)
  const slideCount = computed(() => countReadyBlueprints(blueprints.value))

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
      const previous = pageStatuses.value.get(ev.slide_index)
      pageStatuses.value.set(ev.slide_index, {
        stage: ev.stage,
        message: ev.message ?? undefined,
        waiting: ev.waiting ?? false,
        previousStage: ev.stage === 'error' ? (previous?.stage === 'error' ? previous.previousStage : previous?.stage) : undefined,
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

  /**
   * The deck a run produced, asked for by the project it was started for.
   *
   * The user may have opened another project while the run was going, so the open deck
   * (`get_blueprints`) is only right while the run's project is not in the database yet
   * (`/new` before it is saved, i.e. `activeProjectId` is null). Not keyed on the run id:
   * that comes from the `gen:log` event, whose arrival is unordered against `invoke`'s.
   */
  async function loadRunDeck(projectId: number | null): Promise<SlideBlueprint[]> {
    if (projectId === null) return invoke<SlideBlueprint[]>('get_blueprints')
    return invoke<SlideBlueprint[]>('get_generation_deck', { projectId })
  }

  async function generate(mdContent: string, frontmatterTitle?: string, granularity?: string, aspectRatio?: AspectRatio, projectId?: number | null) {
    if (running.value || retryingPages.value.size > 0) return
    generationVersion++
    running.value = true
    lastError.value = ''
    blueprints.value = []
    completedSlides.value = new Set()
    logs.value = []
    currentRun.value = null
    activeProjectId.value = projectId ?? null
    // The project this run belongs to, fixed for the whole run.
    const runProjectId = activeProjectId.value
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
      // After generation, load the deck the run assembled in its own project.
      const final = await loadRunDeck(runProjectId)
      blueprints.value = final
      completedSlides.value = new Set(final.map((_, index) => index))
      // Keep errors visible even when the final deck contains fallback slides.
      pageStatuses.value = new Map(Array.from(pageStatuses.value, ([index, status]) =>
        [index, status.stage === 'error' ? status : { stage: 'done' }]))
      stage.value = 'done'
      progress.value = 1
      message.value = `生成完成，共 ${count} 张幻灯片`
    } catch (e: unknown) {
      lastError.value = String(e)
      stage.value = 'error'
      message.value = String(e)
      // Final validation can fail after assembly. Recover that deck for page retries.
      try {
        const assembled = await loadRunDeck(runProjectId)
        if (assembled[0]?.kind === 'cover') {
          blueprints.value = assembled
          completedSlides.value = new Set(assembled.map((_, index) => index))
        }
      } catch {
        // Preserve the original generation error and any previews already received.
      }
    } finally {
      running.value = false
      stopListening()
      for (const [index, layoutKind] of queuedRetries) {
        queuedRetries.delete(index)
        await executePageRetry(index, layoutKind)
      }
    }
  }

  async function executePageRetry(index: number, layoutKind?: SlideKind) {
    const version = generationVersion
    const previous = pageStatuses.value.get(index)
    pageStatuses.value.set(index, { stage: 'content', message: '正在重试该页' })
    try {
      if (!currentRun.value) throw new Error('缺少生成记录，请重新生成')
      const result = await invoke<RetryPageResult>('retry_generation_page', {
        runId: currentRun.value.id, pageIndex: index, layoutKind,
      })
      if (version !== generationVersion) return
      blueprints.value[index + 2] = result.slide
      completedSlides.value.add(index + 2)
      pageStatuses.value.set(index, { stage: 'done', message: '重试完成' })
      // The page is saved either way; the run only completes once the whole deck validates.
      if (result.deck_ready) {
        stage.value = 'done'
        progress.value = 1
        lastError.value = ''
        const now = Math.floor(Date.now() / 1000)
        if (currentRun.value) {
          currentRun.value = { ...currentRun.value, status: 'done', current_stage: 'done', updated_at: now, finished_at: now }
        }
        message.value = '该页重试完成，可继续编辑'
      } else {
        for (const page of result.invalid_pages) {
          if (pageStatuses.value.get(page)?.stage !== 'error') {
            pageStatuses.value.set(page, { stage: 'error', previousStage: 'content', message: '未通过最终校验' })
          }
        }
        lastError.value = result.remaining
        message.value = `该页已更新；${result.remaining}`
      }
    } catch (error) {
      if (version !== generationVersion) return
      pageStatuses.value.set(index, {
        stage: 'error', previousStage: previous?.previousStage ?? 'content', message: String(error),
      })
    } finally {
      if (version === generationVersion) retryingPages.value.delete(index)
    }
  }

  async function retryPage(index: number, layoutKind?: SlideKind) {
    if (retryingPages.value.has(index) || (!running.value && retryingPages.value.size > 0)
      || pageStatuses.value.get(index)?.stage !== 'error') return
    retryingPages.value.add(index)
    if (running.value) {
      queuedRetries.set(index, layoutKind)
      return
    }
    await executePageRetry(index, layoutKind)
  }

  async function repairSlide(index: number, feedback: string) {
    const repaired = await invoke<SlideBlueprint>('repair_slide', { index, feedback })
    blueprints.value[index] = repaired
    return repaired
  }

  function reset() {
    generationVersion++
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
    queuedRetries.clear()
    retryingPages.value = new Set()
  }

  function loadFromJson(json: string) {
    try {
      const parsed = JSON.parse(json) as SlideBlueprint[]
      generationVersion++
      queuedRetries.clear()
      retryingPages.value = new Set()
      blueprints.value = parsed
      completedSlides.value = new Set(parsed.map((_, index) => index))
      const contentPages = parsed[0]?.kind === 'cover' ? parsed.slice(2, -1) : parsed
      pageStatuses.value = new Map(contentPages.map((_, index) => [index, { stage: 'done' }]))
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
    // Recover failed/fixed content-page statuses after reopening the project.
    for (const log of logs.value) {
      if (typeof log.slide_index !== 'number') continue
      if (log.kind === 'page_error') {
        pageStatuses.value.set(log.slide_index, { stage: 'error', previousStage: 'content', message: log.summary })
      } else if (log.kind === 'retry') {
        pageStatuses.value.set(log.slide_index, log.detail?.retry_succeeded
          ? { stage: 'done', message: '重试完成' }
          : { stage: 'error', previousStage: 'content', message: log.summary })
      }
    }
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
    activeProjectId, pageStatuses, retryingPages,
    generate, retryPage, repairSlide, reset, loadFromJson, loadLatestRun,
  }
})
