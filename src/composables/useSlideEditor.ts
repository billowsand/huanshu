import { ref, computed, watch, nextTick } from 'vue'
import type { SlideBlueprint, SlideKind, AspectRatio } from '../components/types'
import { ASPECT_DIMENSIONS } from '../components/types'
import { useProjectsStore } from '../stores/projects'
import { useGenerationStore } from '../stores/generation'
import { useEditorSchemaStore } from '../stores/editorSchema'

/** 新建一页时沿用上一页的这些字段（标题、页脚、图片……），让整副牌组风格一致。 */
const SHARED_TEMPLATE_FIELDS: (keyof SlideBlueprint)[] = [
  'section', 'title', 'subtitle', 'badge', 'accent', 'note', 'label', 'label_tone',
  'image', 'images', 'placeholder', 'side_width', 'badges', 'footer', 'aspect_ratio',
]

/** 这三种页不显示章节号，因此它们不参与「新页继承章节」。 */
const NON_CHAPTER_KINDS: SlideKind[] = ['cover', 'overview', 'closing']

/**
 * 编辑器的模式表来自 Rust（`get_editor_schema`，见 `huanshu_core::editor_schema`）：
 * 版式名称、字段、默认内容与表单定义只有一份。`useEditorSchema` 启动时取一次并缓存。
 */
export function useSlideEditor(options: {
  activeSlide: () => number
  setActiveSlide: (v: number) => void
  currentStep: () => number
  projectId: () => number | null
  blueprints: () => SlideBlueprint[]
}) {
  const projectsStore = useProjectsStore()
  const genStore = useGenerationStore()
  const schemaStore = useEditorSchemaStore()

  const jsonText = ref('')
  const jsonError = ref('')
  const liveSlide = ref<SlideBlueprint | null>(null)
  const hasUnsavedChanges = ref(false)
  const editorSaving = ref(false)
  const showKindPicker = ref(false)
  const kindPickerMode = ref<'add' | 'change'>('add')
  const previewArea = ref<HTMLElement | null>(null)
  /** 预览舞台相对画布原尺寸的缩放：由预览区实测尺寸决定。 */
  const previewFit = ref(0.45)
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
    const blueprint = deepClone({ ...schemaStore.kindDefaults[kind], kind }) as SlideBlueprint
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
    const fields = schemaStore.kindFields[slide.kind as SlideKind] ?? Object.keys(slide)
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
      previewFit.value = Math.min(w / dims.w, h / dims.h)
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
      // A new slide joins the chapter of the slide it is inserted after; KIND_DEFAULTS only
      // carries a sample number. Cover/overview/closing are not chapters, so leave it empty.
      const prev = genStore.blueprints[options.activeSlide()] as SlideBlueprint | undefined
      newSlide.section = prev && !NON_CHAPTER_KINDS.includes(prev.kind as SlideKind)
        ? prev.section
        : undefined
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
  const editorStageW = computed(() => Math.round(currentSlideDims.value.w * previewFit.value))
  const editorStageH = computed(() => Math.round(currentSlideDims.value.h * previewFit.value))

  return {
    jsonText,
    jsonError,
    liveSlide,
    hasUnsavedChanges,
    editorSaving,
    showKindPicker,
    kindPickerMode,
    previewArea,
    jsonTextareaRef,
    editorKind,
    editorPreviewSlide,
    editorStageW,
    editorStageH,
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
