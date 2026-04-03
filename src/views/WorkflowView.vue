<script setup lang="ts">
import { ref, computed, onMounted, watch, onBeforeUnmount } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { invoke } from '@tauri-apps/api/core'
import { useGenerationStore } from '../stores/generation'
import { useConfigStore } from '../stores/config'
import { useProjectsStore } from '../stores/projects'
import ThemeToggle from '../components/ThemeToggle.vue'
import Step1Prepare from './steps/Step1Prepare.vue'
import Step2Generate from './steps/Step2Generate.vue'
import Step3Editor from './steps/Step3Editor.vue'
import PresentationOverlay from './PresentationOverlay.vue'
import { useMediaLibrary } from '../composables/useMediaLibrary'
import { useSlideEditor } from '../composables/useSlideEditor'
import type { SlideBlueprint } from '../components/types'
import type { Project } from '../stores/projects'

const route = useRoute()
const router = useRouter()
const gen = useGenerationStore()
const config = useConfigStore()
const projects = useProjectsStore()

// ── Root state ────────────────────────────────────────────────────────────────
const currentStep = ref(1)
const projectName = ref('')
const projectId = ref<number | null>(null)
const project = ref<Project | null>(null)
const activeSlide = ref(0)
const loading = ref(false)
const loadError = ref('')
const editingName = ref(false)
const newName = ref('')
const granularity = ref<'auto' | 'h2' | 'h3'>('auto')
const detectedGranularity = ref<'h2' | 'h3'>('h3')
const aspectRatio = ref<'ratio_16x9' | 'ratio_32x9' | 'ratio_48x9'>('ratio_16x9')
const mdFilename = ref('')
const mdContent = ref('')
const selectedSlideIndex = ref<number | null>(null)
const presenting = ref(false)
const draftSaveState = ref<'idle' | 'saving' | 'saved' | 'error'>('idle')
const draftSaveError = ref('')
const step1Bootstrapped = ref(false)

const STEP1_META_KEY = 'auto-slidev:step1-meta'
let step1AutosaveTimer: ReturnType<typeof setTimeout> | null = null

const step1State = ref({
  mdContent: '',
  mdFilename: '',
  projectName: '',
  granularity: 'auto' as 'auto' | 'h2' | 'h3',
  aspectRatio: 'ratio_16x9' as 'ratio_16x9' | 'ratio_32x9' | 'ratio_48x9',
})

// ── Media library ──────────────────────────────────────────────────────────────
async function ensureProjectForMedia() {
  if (projectId.value) return
  const name = projectName.value.trim() || '未命名演示'
  projectId.value = await projects.create(name, mdContent.value || '')
  const p = await projects.open(projectId.value)
  project.value = p
  persistStep1Meta(projectId.value, step1State.value)
  if (!route.params.id || Number(route.params.id) !== projectId.value) {
    router.replace(`/project/${projectId.value}`)
  }
}

const mediaLib = useMediaLibrary({
  projectId: () => projectId.value,
  projectName: () => projectName.value,
  mdContent: () => mdContent.value,
  ensureProjectForMedia,
})

// ── Slide editor ───────────────────────────────────────────────────────────────
const slideEditor = useSlideEditor({
  activeSlide: () => activeSlide.value,
  setActiveSlide: (v) => { activeSlide.value = v },
  currentStep: () => currentStep.value,
  projectId: () => projectId.value,
  mediaMap: () => mediaLib.mediaMap.value,
  blueprints: () => gen.blueprints as unknown as SlideBlueprint[],
})

// ── Lifecycle ─────────────────────────────────────────────────────────────────
onMounted(async () => {
  config.load()
  invoke('ensure_icon_embeddings').catch(() => {})
  const id = route.params.id ? Number(route.params.id) : null
  if (id) {
    loading.value = true
    try {
      const p = await projects.open(id)
      project.value = p
      projectId.value = id
      projectName.value = p.name
      newName.value = p.name
      mdContent.value = p.md_content ?? ''
      step1State.value = {
        mdContent: p.md_content ?? '',
        mdFilename: '',
        projectName: p.name,
        granularity: 'auto',
        aspectRatio: p.aspect_ratio ?? 'ratio_16x9',
      }
      loadStep1Meta(id)
      if (p.blueprints_json) {
        gen.loadFromJson(p.blueprints_json)
      } else {
        gen.reset()
      }
      await gen.loadLatestRun(id)
      await mediaLib.loadProjectMedia()
      currentStep.value = gen.blueprints.length > 0 ? 3 : 1
    } catch (e: unknown) {
      loadError.value = String(e)
    } finally {
      loading.value = false
    }
  } else {
    gen.reset()
    currentStep.value = 1
  }
  step1Bootstrapped.value = true
})

onBeforeUnmount(() => {
  if (step1AutosaveTimer) clearTimeout(step1AutosaveTimer)
})

// Auto-advance after generation completes
watch(() => gen.stage, (stage) => {
  if (stage === 'done' && currentStep.value === 2) {
    setTimeout(() => { currentStep.value = 3 }, 800)
  }
})

// ── Step 1 handlers ───────────────────────────────────────────────────────────
function onStep1UpdateModelValue(val: typeof step1State.value) {
  step1State.value = val
  mdContent.value = val.mdContent
  mdFilename.value = val.mdFilename
  projectName.value = val.projectName
  granularity.value = val.granularity
  aspectRatio.value = val.aspectRatio
}

watch(
  step1State,
  (val) => {
    mdContent.value = val.mdContent
    mdFilename.value = val.mdFilename
    projectName.value = val.projectName
    granularity.value = val.granularity
    aspectRatio.value = val.aspectRatio
  },
  { immediate: true },
)

function step1MetaStorageKey(id: number) {
  return `${STEP1_META_KEY}:${id}`
}

function persistStep1Meta(id: number, val: typeof step1State.value) {
  localStorage.setItem(step1MetaStorageKey(id), JSON.stringify({
    mdFilename: val.mdFilename,
    granularity: val.granularity,
    aspectRatio: val.aspectRatio,
  }))
  if (id) {
    projects.updateAspectRatio(id, val.aspectRatio).catch(() => {})
  }
}

function loadStep1Meta(id: number) {
  const raw = localStorage.getItem(step1MetaStorageKey(id))
  if (!raw) return
  try {
    const parsed = JSON.parse(raw) as Partial<Pick<typeof step1State.value, 'mdFilename' | 'granularity' | 'aspectRatio'>>
    step1State.value = {
      ...step1State.value,
      mdFilename: typeof parsed.mdFilename === 'string' ? parsed.mdFilename : step1State.value.mdFilename,
      granularity: parsed.granularity === 'auto' || parsed.granularity === 'h2' || parsed.granularity === 'h3'
        ? parsed.granularity
        : step1State.value.granularity,
      aspectRatio: parsed.aspectRatio === 'ratio_16x9' || parsed.aspectRatio === 'ratio_32x9' || parsed.aspectRatio === 'ratio_48x9'
        ? parsed.aspectRatio
        : step1State.value.aspectRatio,
    }
  } catch {
    localStorage.removeItem(step1MetaStorageKey(id))
  }
}

function hasMeaningfulStep1Draft() {
  return Boolean(
    step1State.value.mdContent.trim()
    || step1State.value.projectName.trim()
    || step1State.value.mdFilename.trim(),
  )
}

function getDraftProjectName() {
  return step1State.value.projectName.trim()
    || step1State.value.mdFilename.trim().replace(/\.[^.]+$/, '')
    || '未命名演示'
}

async function saveStep1Draft() {
  if (loading.value || !step1Bootstrapped.value) return
  if (!projectId.value && !hasMeaningfulStep1Draft()) {
    draftSaveState.value = 'idle'
    draftSaveError.value = ''
    return
  }

  const draftName = getDraftProjectName()
  draftSaveState.value = 'saving'
  draftSaveError.value = ''

  try {
    if (!projectId.value) {
      const createdId = await projects.create(draftName, step1State.value.mdContent)
      projectId.value = createdId
      project.value = await projects.open(createdId)
      if (!route.params.id || Number(route.params.id) !== createdId) {
        router.replace(`/project/${createdId}`)
      }
    } else {
      await projects.updateContent(projectId.value, draftName, step1State.value.mdContent)
    }

    if (projectId.value) {
      persistStep1Meta(projectId.value, step1State.value)
    }

    projectName.value = draftName
    if (project.value) {
      project.value.name = draftName
      project.value.md_content = step1State.value.mdContent
    }

    draftSaveState.value = 'saved'
  } catch (e: unknown) {
    draftSaveState.value = 'error'
    draftSaveError.value = String(e)
  }
}

watch(
  step1State,
  () => {
    if (step1AutosaveTimer) clearTimeout(step1AutosaveTimer)
    if (!step1Bootstrapped.value) return
    step1AutosaveTimer = setTimeout(() => {
      saveStep1Draft().catch((e) => {
        draftSaveState.value = 'error'
        draftSaveError.value = String(e)
      })
    }, 800)
  },
  { deep: true },
)

async function onStep1StartGenerate() {
  if (!mdContent.value.trim() || !projectName.value.trim()) return
  if (step1AutosaveTimer) {
    clearTimeout(step1AutosaveTimer)
    step1AutosaveTimer = null
  }
  await saveStep1Draft()
  currentStep.value = 2

  await gen.generate(
    mdContent.value,
    projectName.value,
    granularity.value === 'auto' ? undefined : granularity.value,
    aspectRatio.value,
  )

  if (gen.stage === 'done' && gen.blueprints.length > 0 && projectId.value) {
    await projects.saveBlueprints(projectId.value, gen.blueprints)
  }
}

// ── Step navigation ────────────────────────────────────────────────────────────
const steps = computed(() => [
  {
    num: 1,
    label: '准备素材',
    desc: mdFilename.value
      ? mdFilename.value
      : mdContent.value
        ? `${mdContent.value.length} 字符`
        : '上传或粘贴文稿',
    icon: 'i-carbon:document-import',
    status: currentStep.value > 1 ? 'done'
      : currentStep.value === 1 ? 'active'
      : 'pending',
    clickable: true,
  },
  {
    num: 2,
    label: 'AI 生成',
    desc: gen.stage === 'error' ? '生成出错'
      : gen.running ? gen.stageLabel
      : gen.slideCount > 0 ? `已生成 ${gen.slideCount} 张`
      : '等待开始',
    icon: 'i-carbon:magic-wand',
    status: gen.stage === 'error' ? 'error'
      : currentStep.value > 2 ? 'done'
      : currentStep.value === 2 ? 'active'
      : 'pending',
    clickable: currentStep.value >= 2,
  },
  {
    num: 3,
    label: '编辑完善',
    desc: gen.slideCount > 0 ? `${gen.slideCount} 张幻灯片` : '生成后可编辑',
    icon: 'i-carbon:edit',
    status: currentStep.value > 3 ? 'done'
      : currentStep.value === 3 ? 'active'
      : 'pending',
    clickable: gen.blueprints.length > 0,
  },
])

function goStep(n: number) {
  const s = steps.value[n - 1]
  if (s.clickable) currentStep.value = n
}

const canGenerate = computed(() =>
  mdContent.value.trim().length > 0 &&
  projectName.value.trim().length > 0 &&
  !gen.running
)

// ── Name editing ────────────────────────────────────────────────────────────────
async function saveName() {
  if (!newName.value.trim()) return
  if (projectId.value) {
    await projects.updateContent(projectId.value, newName.value.trim(), mdContent.value)
    if (project.value) project.value.name = newName.value.trim()
  }
  projectName.value = newName.value.trim()
  editingName.value = false
}

// ── Editor keyboard shortcuts ──────────────────────────────────────────────────
function handleEditorKeydown(e: KeyboardEvent) {
  if (!presenting.value && currentStep.value === 3) {
    if ((e.ctrlKey || e.metaKey) && e.key === 's') {
      e.preventDefault()
      slideEditor.applyEditorChanges()
    }
    if (e.key === 'Escape' && slideEditor.showKindPicker.value) {
      slideEditor.showKindPicker.value = false
    }
  }
}

onMounted(() => {
  window.addEventListener('keydown', handleEditorKeydown)
})
</script>

<template>
  <div v-if="loading" class="full-center">
    <span class="spin-icon" />
    <p>加载项目中...</p>
  </div>

  <div v-else-if="loadError" class="full-center">
    <p class="error-text">{{ loadError }}</p>
    <button class="btn" @click="router.push('/')">返回首页</button>
  </div>

  <div v-else class="workflow-root">

    <!-- ── Left sidebar ───────────────────────────────────────────────── -->
    <aside class="sidebar">
      <div class="sidebar-header">
        <button class="back-btn" @click="router.push('/')">
          <span class="i-carbon:arrow-left" />
        </button>
        <div class="project-title-wrap">
          <template v-if="editingName">
            <input
              v-model="newName"
              class="title-input"
              @keydown.enter="saveName"
              @keydown.escape="editingName = false"
              @blur="saveName"
              autofocus
            />
          </template>
          <template v-else>
            <span
              class="project-title"
              :title="projectName || '未命名项目'"
              @click="editingName = true; newName = projectName"
            >
              {{ projectName || '未命名项目' }}
            </span>
          </template>
        </div>
      </div>

      <nav class="steps-nav">
        <button
          v-for="s in steps"
          :key="s.num"
          class="step-item"
          :class="[s.status, { disabled: !s.clickable }]"
          @click="goStep(s.num)"
        >
          <div class="step-indicator">
            <div class="step-circle">
              <span v-if="s.status === 'done'" class="i-carbon:checkmark" />
              <span v-else-if="s.status === 'error'" class="i-carbon:warning" />
              <span v-else-if="s.status === 'active' && s.num === 2 && gen.running" class="step-spinner" />
              <span v-else>{{ s.num }}</span>
            </div>
            <div v-if="s.num < 3" class="step-line" :class="{ done: s.status === 'done' }" />
          </div>
          <div class="step-text">
            <div class="step-label">{{ s.label }}</div>
            <div class="step-desc">{{ s.desc }}</div>
          </div>
        </button>
      </nav>

      <div class="sidebar-footer">
        <ThemeToggle />
        <button class="btn btn-ghost sidebar-settings" @click="router.push('/settings')">
          <span class="i-carbon:settings" /> 设置
        </button>
      </div>
    </aside>

    <!-- ── Main content ───────────────────────────────────────────────── -->
    <main class="main-content">

      <!-- Step 1 -->
      <Step1Prepare
        v-if="currentStep === 1"
        v-model="step1State"
        :detected-granularity="detectedGranularity"
        :can-generate="canGenerate"
        :media-items="mediaLib.mediaItems.value"
        :media-drop-over="mediaLib.mediaDropOver.value"
        :draft-save-state="draftSaveState"
        :draft-save-error="draftSaveError"
        @update:detected-granularity="detectedGranularity = $event"
        @start-generate="onStep1StartGenerate"
        @media-drop-over="mediaLib.mediaDropOver.value = $event"
        @open-media-files="mediaLib.openMediaFiles"
        @add-media-from-path="mediaLib.addMediaFromPath"
        @add-media-from-file="mediaLib.addMediaFromFile"
        @remove-media="mediaLib.removeMedia"
      />

      <!-- Step 2 -->
      <Step2Generate
        v-else-if="currentStep === 2"
        :stage="gen.stage"
        :stage-label="gen.stageLabel"
        :running="gen.running"
        :message="gen.message"
        :slide-count="gen.slideCount"
        :blueprints="(gen.blueprints as unknown as SlideBlueprint[])"
        :media-map="mediaLib.mediaMap.value"
        :logs="gen.logs"
        :selected-slide-index="selectedSlideIndex"
        :page-statuses="gen.pageStatuses"
        @update:selected-slide-index="selectedSlideIndex = $event"
        @go-to-step-3="currentStep = 3"
        @go-to-step-1="currentStep = 1"
        @retry="onStep1StartGenerate"
      />

      <!-- Step 3 -->
      <Step3Editor
        v-else-if="currentStep === 3"
        v-model:active-slide="activeSlide"
        :slide-count="gen.slideCount"
        :blueprints="(gen.blueprints as unknown as SlideBlueprint[])"
        :media-items="mediaLib.mediaItems.value"
        :media-map="mediaLib.mediaMap.value"
        :json-text="slideEditor.jsonText.value"
        :json-error="slideEditor.jsonError.value"
        :live-slide="slideEditor.liveSlide.value"
        :has-unsaved-changes="slideEditor.hasUnsavedChanges.value"
        :editor-saving="slideEditor.editorSaving.value"
        :show-kind-picker="slideEditor.showKindPicker.value"
        :kind-picker-mode="slideEditor.kindPickerMode.value"
        :preview-area="slideEditor.previewArea.value"
        :preview-scale="slideEditor.previewScale.value"
        :editor-kind="slideEditor.editorKind.value"
        :editor-preview-slide="slideEditor.editorPreviewSlide.value"
        :editor-stage-w="slideEditor.editorStageW.value"
        :editor-stage-h="slideEditor.editorStageH.value"
        :current-slide-dims="slideEditor.currentSlideDims.value"
        :json-textarea-ref="slideEditor.jsonTextareaRef.value"
        @replace-draft="slideEditor.replaceEditorDraft"
        @json-textarea-ref="slideEditor.jsonTextareaRef.value = $event"
        @preview-area-ref="slideEditor.previewArea.value = $event"
        @format-json="slideEditor.formatEditorJson"
        @apply-changes="slideEditor.applyEditorChanges"
        @open-kind-picker="slideEditor.openKindPicker"
        @kind-picked="slideEditor.onKindPicked"
        @delete-slide="slideEditor.deleteEditorSlide"
        @move-slide="slideEditor.moveEditorSlide"
        @insert-media-url="slideEditor.insertMediaUrl"
        @start-presentation="presenting = true"
        @go-to-step-1="currentStep = 1"
        @textarea-input="slideEditor.onEditorJsonInput"
        @textarea-keydown="slideEditor.onEditorJsonKeydown"
      />

    </main>

    <!-- Presentation overlay -->
    <PresentationOverlay
      v-if="presenting"
      :blueprints="(gen.blueprints as unknown as SlideBlueprint[])"
      :media-map="mediaLib.mediaMap.value"
      :initial-slide="activeSlide"
      @exit="presenting = false"
    />
  </div>
</template>

<style>
/* ── Root layout ─────────────────────────────────────────────────────────── */
.workflow-root {
  display: flex;
  height: 100%;
  width: 100%;
  overflow: hidden;
  background: var(--studio-bg);
}

.full-center {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100%;
  gap: 1rem;
  color: var(--studio-muted);
  font-size: 0.875rem;
}

.error-text { color: var(--studio-error); }

.spin-icon {
  width: 24px; height: 24px;
  border: 2px solid var(--studio-border);
  border-top-color: var(--studio-primary);
  border-radius: 50%;
  animation: spin 0.7s linear infinite;
}

/* ── Sidebar ─────────────────────────────────────────────────────────────── */
.sidebar {
  width: 220px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  background: var(--studio-panel);
  border-right: 1px solid var(--studio-border);
  overflow: hidden;
}

.sidebar-header {
  padding: 0.875rem 1rem 0.75rem;
  border-bottom: 1px solid var(--studio-border);
  display: flex;
  align-items: center;
  gap: 0.6rem;
}

.back-btn {
  flex-shrink: 0;
  width: 28px; height: 28px;
  border: 1px solid var(--studio-border);
  border-radius: 6px;
  background: transparent;
  color: var(--studio-muted);
  cursor: pointer;
  display: flex; align-items: center; justify-content: center;
  font-size: 0.875rem;
  transition: all 0.15s;
}
.back-btn:hover { color: var(--studio-text); border-color: var(--studio-border-hover); }

.project-title-wrap { flex: 1; min-width: 0; }

.project-title {
  font-size: 0.8125rem;
  font-weight: 600;
  color: var(--studio-text);
  display: block;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  cursor: pointer;
  border-radius: 3px;
  padding: 0.1rem 0.2rem;
  transition: background 0.15s;
}
.project-title:hover { background: var(--studio-surface); }

.title-input {
  font-size: 0.8125rem;
  font-weight: 600;
  padding: 0.2rem 0.4rem;
  height: 26px;
}

/* Step navigation */
.steps-nav {
  flex: 1;
  padding: 1rem 0.75rem;
  display: flex;
  flex-direction: column;
  gap: 0;
}

.step-item {
  display: flex;
  align-items: flex-start;
  gap: 0.75rem;
  padding: 0 0.25rem;
  background: transparent;
  border: none;
  cursor: pointer;
  text-align: left;
  width: 100%;
  transition: opacity 0.15s;
}

.step-item.disabled { cursor: not-allowed; opacity: 0.45; }
.step-item:not(.disabled):hover .step-label { color: var(--studio-text); }

.step-indicator {
  display: flex;
  flex-direction: column;
  align-items: center;
  flex-shrink: 0;
}

.step-circle {
  width: 32px; height: 32px;
  border-radius: 50%;
  border: 2px solid var(--studio-border);
  background: var(--studio-surface);
  display: flex; align-items: center; justify-content: center;
  font-size: 0.75rem;
  font-weight: 700;
  color: var(--studio-muted);
  transition: all 0.25s cubic-bezier(0.34, 1.56, 0.64, 1);
  position: relative;
  z-index: 1;
}

.step-item.active .step-circle {
  border-color: var(--studio-primary);
  background: var(--studio-primary-bg);
  color: var(--studio-primary);
  box-shadow: 0 0 0 4px var(--studio-primary-bg), 0 0 16px var(--studio-primary-bg);
  transform: scale(1.1);
}

.step-item.done .step-circle {
  border-color: var(--studio-success);
  background: var(--studio-success-bg);
  color: var(--studio-success);
}

.step-item.error .step-circle {
  border-color: var(--studio-error);
  background: var(--studio-error-bg);
  color: var(--studio-error);
}

.step-spinner {
  width: 14px; height: 14px;
  border: 2px solid var(--studio-primary-bg);
  border-top-color: var(--studio-primary);
  border-radius: 50%;
  animation: spin 0.7s linear infinite;
}

.step-line {
  width: 2px;
  height: 32px;
  background: var(--studio-border);
  margin: 2px 0;
  transition: background 0.4s ease;
  border-radius: 1px;
  position: relative;
  overflow: hidden;
}

.step-line::after {
  content: '';
  position: absolute;
  inset: 0;
  background: var(--studio-success);
  transform: scaleY(0);
  transform-origin: top;
  transition: transform 0.4s cubic-bezier(0.16, 1, 0.3, 1);
}

.step-line.done::after {
  transform: scaleY(1);
}

.step-line.done {
  background: transparent;
}

.step-text {
  padding-top: 4px;
  padding-bottom: 30px;
}

.step-label {
  font-size: 0.8125rem;
  font-weight: 500;
  color: var(--studio-muted);
  line-height: 1.3;
  transition: color 0.15s;
}
.step-item.active .step-label { color: var(--studio-text); font-weight: 600; }
.step-item.done .step-label { color: var(--studio-text); }

.step-desc {
  font-size: 0.7rem;
  color: var(--studio-muted);
  margin-top: 0.15rem;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 130px;
}

.sidebar-footer {
  display: flex;
  flex-direction: column;
}

.sidebar-settings {
  width: 100%;
  font-size: 0.8rem;
  justify-content: center;
  color: var(--studio-muted);
  border-top: none;
  padding: 0.5rem 0.75rem;
}

/* ── Main content ────────────────────────────────────────────────────────── */
.main-content {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  min-width: 0;
}

.step-panel {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

/* ── Step header ─────────────────────────────────────────────────────────── */
.step-header {
  padding: 1.5rem 2rem 1rem;
  flex-shrink: 0;
  border-bottom: 1px solid var(--studio-border);
}

.step-title {
  font-size: 1.25rem;
  font-weight: 700;
  margin: 0 0 0.25rem;
  color: var(--studio-text);
}

.step-subtitle {
  font-size: 0.8125rem;
  color: var(--studio-muted);
  margin: 0;
}

/* ── Step 1: Prepare ─────────────────────────────────────────────────────── */

/* Top bar: title · project name · actions */
.prepare-topbar {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: 0.875rem;
  padding: 0.625rem 1.25rem;
  border-bottom: 1px solid var(--studio-border);
  background: var(--studio-panel);
  flex-wrap: wrap;
}

.prepare-topbar-left {
  display: flex;
  align-items: center;
  gap: 0.875rem;
  flex: 1;
  min-width: 0;
}

.prepare-page-label {
  font-size: 0.8rem;
  font-weight: 700;
  color: var(--studio-muted);
  text-transform: uppercase;
  letter-spacing: 0.07em;
  white-space: nowrap;
  flex-shrink: 0;
}

.prepare-name-wrap {
  flex: 1;
  min-width: 0;
  max-width: 420px;
}

.project-name-input {
  width: 100%;
  box-sizing: border-box;
  font-size: 0.9375rem;
  font-weight: 600;
  padding: 0.4rem 0.75rem;
  background: var(--studio-surface);
  border: 1px solid var(--studio-border);
  border-radius: 6px;
  color: var(--studio-text);
  font-family: var(--font-sans);
  transition: border-color 0.15s, box-shadow 0.15s;
}
.project-name-input::placeholder { color: var(--studio-muted); font-weight: 400; }
.project-name-input:focus {
  outline: none;
  border-color: var(--studio-primary-border);
  box-shadow: 0 0 0 3px var(--studio-primary-bg);
}

.prepare-topbar-right {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  flex-shrink: 0;
  flex-wrap: wrap;
}

/* Heading tools bar */
.prepare-tools {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: 0.375rem;
  padding: 0.45rem 1.25rem;
  border-bottom: 1px solid var(--studio-border);
  flex-wrap: wrap;
  background: var(--studio-panel);
}

.tools-label {
  font-size: 0.65rem;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.07em;
  color: var(--studio-muted);
  margin-right: 0.25rem;
  flex-shrink: 0;
}

.btn-tool {
  font-size: 0.75rem;
  padding: 0.25rem 0.625rem;
}

.prepare-inline-error {
  font-size: 0.75rem;
  line-height: 1.5;
  color: var(--studio-danger, #d16b6b);
  margin-left: 0.25rem;
}

/* Main workbench grid */
.prepare-body {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: minmax(0, 1fr) 288px;
  overflow: hidden;
}

/* Editor column */
.prepare-editor-col {
  display: flex;
  flex-direction: column;
  min-height: 0;
  overflow: hidden;
  position: relative;
  border-right: 1px solid var(--studio-border);
  transition: background 0.18s;
}

.prepare-editor-col--dragover {
  background: var(--studio-primary-bg);
  outline: 2px dashed var(--studio-primary);
  outline-offset: -3px;
}

.prepare-drop-overlay {
  position: absolute;
  inset: 0;
  z-index: 10;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 0.625rem;
  font-size: 0.9rem;
  color: var(--studio-primary);
  background: var(--studio-primary-bg);
  pointer-events: none;
}
.prepare-drop-overlay .i-carbon\:document-import { font-size: 2.25rem; }

/* MarkdownWorkbench fills editor col via flex */
.prepare-editor {
  flex: 1;
  min-height: 0;
  overflow: hidden;
}

/* Right sidebar */
.prepare-sidebar {
  display: flex;
  flex-direction: column;
  overflow-y: auto;
  background: var(--studio-panel);
}

.sidebar-card {
  flex-shrink: 0;
  padding: 1rem;
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
  border-bottom: 1px solid var(--studio-border);
}

.sidebar-card-hd {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
}

.sidebar-card-hd-actions {
  display: flex;
  align-items: center;
  gap: 0.375rem;
}

.sidebar-note {
  font-size: 0.7rem;
  color: var(--studio-muted);
  line-height: 1.5;
}

/* Shared utility classes */
.field-label {
  font-size: 0.7rem;
  font-weight: 600;
  color: var(--studio-muted);
  text-transform: uppercase;
  letter-spacing: 0.06em;
  margin: 0;
}

.filename-tag {
  display: flex;
  align-items: center;
  gap: 0.25rem;
  font-size: 0.7rem;
  color: var(--studio-primary);
  background: var(--studio-primary-bg);
  border: 1px solid var(--studio-primary-border);
  border-radius: 4px;
  padding: 0.1rem 0.5rem;
  max-width: 160px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.char-pill {
  font-size: 0.7rem;
  color: var(--studio-muted);
  background: var(--studio-surface);
  border: 1px solid var(--studio-border);
  border-radius: 10px;
  padding: 0.1rem 0.5rem;
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.accent-pill {
  color: var(--studio-primary);
  background: var(--studio-primary-bg);
  border-color: var(--studio-primary-border);
}

.upload-btn {
  font-size: 0.8rem;
  padding: 0.3rem 0.75rem;
}

/* Granularity chooser */
.granularity-stack {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.gran-choice {
  width: 100%;
  padding: 0.7rem 0.8rem;
  border-radius: 8px;
  border: 1px solid var(--studio-border);
  background: var(--studio-surface);
  color: var(--studio-text);
  cursor: pointer;
  transition: background 0.15s, border-color 0.15s, transform 0.12s;
  text-align: left;
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
}

.gran-choice:hover {
  background: var(--studio-hover);
  transform: translateY(-1px);
}

.gran-choice.active {
  background: var(--studio-primary-bg);
  border-color: var(--studio-primary-border);
}

.gran-choice-title {
  font-size: 0.8125rem;
  font-weight: 600;
}

.gran-choice-meta {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  flex-wrap: wrap;
}

/* Aspect ratio chooser */
.aspect-ratio-stack {
  display: flex;
  flex-direction: row;
  gap: 0.5rem;
}

.aspect-ratio-choice {
  flex: 1;
  padding: 0.6rem 0.5rem;
  border-radius: 8px;
  border: 1px solid var(--studio-border);
  background: var(--studio-surface);
  color: var(--studio-text);
  cursor: pointer;
  transition: background 0.15s, border-color 0.15s, transform 0.12s;
  text-align: center;
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
}

.aspect-ratio-choice:hover {
  background: var(--studio-hover);
  transform: translateY(-1px);
}

.aspect-ratio-choice.active {
  background: var(--studio-primary-bg);
  border-color: var(--studio-primary-border);
}

.aspect-ratio-label {
  font-size: 0.875rem;
  font-weight: 600;
}

.aspect-ratio-desc {
  font-size: 0.65rem;
  color: var(--studio-muted);
}

/* Media library */
.media-card { min-height: 0; }

.media-drop-zone {
  min-height: 120px;
  border: 1.5px dashed var(--studio-border);
  border-radius: 8px;
  padding: 0.5rem;
  display: flex;
  align-items: flex-start;
  transition: all 0.18s;
  overflow-y: auto;
}

.media-drop-zone.dragover {
  border-color: var(--studio-primary);
  background: var(--studio-primary-bg);
}

.media-empty {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 0.4rem;
  color: var(--studio-muted);
  font-size: 0.75rem;
  padding: 0.75rem;
  text-align: center;
}

.media-empty-icon { font-size: 1.75rem; }

.media-grid {
  display: flex;
  flex-wrap: wrap;
  gap: 0.5rem;
}

.media-thumb-wrap {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.2rem;
  width: 68px;
}

.media-thumb {
  position: relative;
  width: 68px;
  height: 51px;
  border-radius: 6px;
  overflow: hidden;
  border: 1px solid var(--studio-border);
  background: var(--studio-surface);
  flex-shrink: 0;
}

.media-thumb-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.media-thumb-video {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--studio-muted);
  font-size: 1.5rem;
}

.media-del-btn {
  position: absolute;
  top: 2px;
  right: 2px;
  width: 18px;
  height: 18px;
  border-radius: 50%;
  background: rgba(0,0,0,0.55);
  color: #fff;
  border: none;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 0.65rem;
  opacity: 0;
  transition: opacity 0.15s;
  padding: 0;
}

.media-thumb:hover .media-del-btn { opacity: 1; }

.media-name {
  font-size: 0.6rem;
  color: var(--studio-muted);
  max-width: 68px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  text-align: center;
}

/* Footer action bar */
.step-actions {
  padding: 0.875rem 1.5rem;
  border-top: 1px solid var(--studio-border);
  display: flex;
  align-items: center;
  gap: 0.75rem;
  flex-shrink: 0;
  background: var(--studio-panel);
}

.generate-btn {
  padding: 0.575rem 1.5rem;
  font-size: 0.9375rem;
}

.hint-text {
  font-size: 0.8rem;
  color: var(--studio-muted);
}

@media (max-width: 1100px) {
  .prepare-body {
    grid-template-columns: minmax(0, 1fr);
  }

  .prepare-sidebar {
    border-top: 1px solid var(--studio-border);
    max-height: 340px;
  }
}

/* ── Step 2: Generate ────────────────────────────────────────────────────── */
.step-generate { gap: 0; }

/* ── Header bar ── */
.gen-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.875rem 1.5rem;
  border-bottom: 1px solid var(--studio-border);
  background: var(--studio-panel);
  flex-shrink: 0;
  gap: 1rem;
}

.gen-hd-left {
  display: flex;
  align-items: center;
  gap: 0.875rem;
  min-width: 0;
}

.gen-status-orb {
  width: 12px; height: 12px;
  border-radius: 50%;
  flex-shrink: 0;
  background: var(--studio-muted);
  transition: all 0.4s;
}

.gen-orb--init, .gen-orb--clean, .gen-orb--page_plan,
.gen-orb--layout_plan, .gen-orb--content, .gen-orb--normalize,
.gen-orb--validate, .gen-orb--start {
  background: var(--studio-primary);
  box-shadow: 0 0 0 3px var(--studio-primary-bg), 0 0 14px var(--studio-primary);
  animation: orb-pulse 1.8s ease-in-out infinite;
}
.gen-orb--done {
  background: var(--studio-success);
  box-shadow: 0 0 0 3px var(--studio-success-bg);
}
.gen-orb--error {
  background: var(--studio-error);
  box-shadow: 0 0 0 3px var(--studio-error-bg);
}
@keyframes orb-pulse {
  0%, 100% { opacity: 1; transform: scale(1); }
  50% { opacity: 0.65; transform: scale(1.3); }
}

.gen-hd-title {
  font-size: 0.9375rem;
  font-weight: 700;
  color: var(--studio-text);
  line-height: 1.3;
  display: flex;
  align-items: baseline;
  gap: 0.15em;
}

.gen-hd-ellipsis { animation: blink-text 1s step-end infinite; color: var(--studio-muted); }
@keyframes blink-text { 50% { opacity: 0; } }

.gen-hd-msg {
  margin-top: 0.15rem;
  font-size: 0.74rem;
  color: var(--studio-muted);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 420px;
}

.gen-hd-stats { display: flex; gap: 0.45rem; flex-shrink: 0; }

.gen-stat-chip {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.1rem;
  padding: 0.4rem 0.7rem;
  border: 1px solid var(--studio-border);
  border-radius: 8px;
  background: var(--studio-surface);
  min-width: 54px;
}

.gen-stat-n {
  font-size: 1.125rem;
  font-weight: 800;
  color: var(--studio-text);
  font-variant-numeric: tabular-nums;
  line-height: 1;
}
.gen-stat-n--warn   { color: #f59e0b; }
.gen-stat-n--accent { color: var(--studio-primary); }

.gen-stat-lbl {
  font-size: 0.56rem;
  color: var(--studio-muted);
  text-transform: uppercase;
  letter-spacing: 0.05em;
  white-space: nowrap;
}

/* ── Pipeline stage track ── */
.gen-stage-track {
  display: flex;
  align-items: flex-start;
  padding: 0.875rem 2rem 2.1rem;
  background: var(--studio-panel-2);
  border-bottom: 1px solid var(--studio-border);
  flex-shrink: 0;
  gap: 0;
}

.gen-stage-node {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.3rem;
  flex-shrink: 0;
}

.gsn-dot {
  width: 26px; height: 26px;
  border-radius: 50%;
  border: 2px solid var(--studio-border);
  background: var(--studio-bg);
  display: flex; align-items: center; justify-content: center;
  font-size: 0.65rem;
  flex-shrink: 0;
  transition: all 0.3s cubic-bezier(0.34, 1.56, 0.64, 1);
}

.gen-stage-node.done .gsn-dot {
  border-color: var(--studio-success);
  background: var(--studio-success-bg);
  color: var(--studio-success);
}
.gen-stage-node.active .gsn-dot {
  border-color: var(--studio-primary);
  background: var(--studio-primary-bg);
  color: var(--studio-primary);
  box-shadow: 0 0 0 4px var(--studio-primary-bg);
  transform: scale(1.12);
}
.gen-stage-node.error .gsn-dot {
  border-color: var(--studio-error);
  background: var(--studio-error-bg);
  color: var(--studio-error);
}

.gsn-icon { font-size: 0.68rem; }

.gsn-spin {
  width: 10px; height: 10px;
  border: 2px solid var(--studio-primary-bg);
  border-top-color: var(--studio-primary);
  border-radius: 50%;
  animation: spin 0.7s linear infinite;
}

.gsn-label {
  font-size: 0.58rem;
  color: var(--studio-muted);
  white-space: nowrap;
  font-weight: 500;
  transition: color 0.2s;
}
.gen-stage-node.active .gsn-label { color: var(--studio-primary); font-weight: 700; }
.gen-stage-node.done   .gsn-label { color: var(--studio-text); }
.gen-stage-node.error  .gsn-label { color: var(--studio-error); }

.gsn-connector {
  flex-shrink: 0;
  width: 44px;
  height: 2px;
  background: var(--studio-border);
  margin-top: 12px;
  border-radius: 1px;
  transition: background 0.5s ease;
}
.gsn-connector.done { background: var(--studio-success); }

/* ── Main body ── */
.gen-body {
  flex: 1;
  display: grid;
  grid-template-columns: 310px 1fr;
  overflow: hidden;
  min-height: 0;
}

/* ── Log panel ── */
.gen-log-panel {
  display: flex;
  flex-direction: column;
  overflow: hidden;
  border-right: 1px solid var(--studio-border);
}

.gen-panel-head {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  padding: 0.45rem 0.75rem;
  background: var(--studio-panel-2);
  border-bottom: 1px solid var(--studio-border);
  font-size: 0.63rem;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.07em;
  color: var(--studio-muted);
  flex-shrink: 0;
}

.gen-panel-count {
  margin-left: auto;
  font-size: 0.6rem;
  background: var(--studio-surface);
  border: 1px solid var(--studio-border);
  border-radius: 999px;
  padding: 0 0.4rem;
  color: var(--studio-muted);
  font-variant-numeric: tabular-nums;
}

.gen-live-dot {
  width: 6px; height: 6px;
  border-radius: 50%;
  background: var(--studio-error);
  animation: blink 1.2s ease-in-out infinite;
  flex-shrink: 0;
}

.gen-log-scroll {
  flex: 1;
  overflow-y: auto;
  padding: 0.35rem;
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
}

.gen-log-empty {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 0.5rem;
  color: var(--studio-muted);
  font-size: 0.78rem;
  padding: 2rem;
  text-align: center;
}

.gen-empty-icon { font-size: 2rem; opacity: 0.18; }

/* ── Log row ── */
.gen-log-row {
  display: flex;
  align-items: stretch;
  width: 100%;
  border: 1px solid var(--studio-border);
  border-radius: 7px;
  background: var(--studio-surface);
  cursor: pointer;
  text-align: left;
  font-family: inherit;
  color: var(--studio-text);
  overflow: hidden;
  transition: border-color 0.12s, background 0.12s;
  animation: slide-in 0.18s ease;
}
.gen-log-row:hover { border-color: var(--studio-border-hover); background: var(--studio-panel); }
.gen-log-row.glr--active {
  border-color: var(--kc, var(--studio-primary));
  background: var(--studio-panel);
}
.gen-log-row.glr--important {
  background: color-mix(in srgb, #f59e0b 5%, var(--studio-surface));
}

.glr-stripe {
  width: 3px;
  flex-shrink: 0;
  background: var(--kc, var(--studio-border));
  opacity: 0.7;
}
.gen-log-row.glr--active .glr-stripe { opacity: 1; }

.glr-inner {
  flex: 1;
  min-width: 0;
  padding: 0.42rem 0.55rem;
}

.glr-meta {
  display: flex;
  align-items: center;
  gap: 0.3rem;
  margin-bottom: 0.28rem;
}

.glr-stage-tag {
  font-size: 0.56rem;
  font-weight: 700;
  color: var(--studio-muted);
  text-transform: uppercase;
  letter-spacing: 0.05em;
  background: var(--studio-bg);
  border: 1px solid var(--studio-border);
  border-radius: 3px;
  padding: 0.02rem 0.28rem;
  white-space: nowrap;
}

.glr-kind-tag {
  font-size: 0.6rem;
  font-weight: 600;
  white-space: nowrap;
}

.glr-seq {
  font-size: 0.58rem;
  color: var(--studio-muted);
  margin-left: auto;
  font-variant-numeric: tabular-nums;
  font-family: var(--font-mono);
}

.glr-title {
  font-size: 0.74rem;
  font-weight: 500;
  line-height: 1.35;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.glr-star {
  flex-shrink: 0;
  align-self: center;
  padding: 0 0.4rem 0 0;
  font-size: 0.6rem;
  color: #f59e0b;
  line-height: 1;
}

/* ── Detail pane ── */
.gen-detail-panel {
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: var(--studio-panel);
}

.gdp-head {
  padding: 0.875rem 1.1rem 0.75rem;
  border-bottom: 1px solid var(--studio-border);
  background: var(--studio-panel-2);
  flex-shrink: 0;
}

.gdp-meta {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  margin-bottom: 0.5rem;
}

.gdp-kind-orb {
  width: 8px; height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}

.gdp-stage-tag {
  font-size: 0.58rem;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  color: var(--studio-muted);
  background: var(--studio-bg);
  border: 1px solid var(--studio-border);
  border-radius: 4px;
  padding: 0.04rem 0.32rem;
}

.gdp-kind-lbl {
  font-size: 0.65rem;
  font-weight: 600;
}

.gdp-flag {
  font-size: 0.58rem;
  background: #fef3c7;
  color: #92400e;
  border: 1px solid #fcd34d;
  border-radius: 4px;
  padding: 0.04rem 0.35rem;
  font-weight: 700;
}

.gdp-seq {
  font-size: 0.6rem;
  color: var(--studio-muted);
  font-family: var(--font-mono);
  margin-left: auto;
}

.gdp-title {
  font-size: 1rem;
  font-weight: 700;
  color: var(--studio-text);
  line-height: 1.4;
}

.gdp-summary {
  margin-top: 0.4rem;
  font-size: 0.77rem;
  color: var(--studio-muted);
  line-height: 1.65;
  white-space: pre-wrap;
  word-break: break-word;
}

.gdp-body {
  flex: 1;
  overflow-y: auto;
  padding: 0.875rem;
  display: flex;
  flex-direction: column;
  gap: 0.65rem;
}

.gdp-empty {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 0.5rem;
  color: var(--studio-muted);
  font-size: 0.82rem;
  text-align: center;
  padding: 2.5rem;
}

.gdp-empty-hint {
  font-size: 0.72rem;
  opacity: 0.6;
  margin-top: 0.15rem;
}

/* ── Detail entry sections ── */
.gde-section {
  border-radius: 8px;
  overflow: hidden;
  border: 1px solid var(--studio-border);
  background: var(--studio-surface);
}

.gde--prompt {
  border-color: rgba(99, 102, 241, 0.32);
  background: color-mix(in srgb, #6366f1 3.5%, var(--studio-surface));
}
.gde--output {
  border-color: rgba(245, 158, 11, 0.32);
  background: color-mix(in srgb, #f59e0b 3.5%, var(--studio-surface));
}
.gde--result {
  border-color: rgba(16, 185, 129, 0.28);
  background: color-mix(in srgb, #10b981 3.5%, var(--studio-surface));
}
.gde--input {
  border-color: rgba(59, 130, 246, 0.28);
  background: color-mix(in srgb, #3b82f6 3.5%, var(--studio-surface));
}
.gde--error {
  border-color: rgba(239, 68, 68, 0.38);
  background: color-mix(in srgb, #ef4444 5%, var(--studio-surface));
}
.gde--config { opacity: 0.75; }

.gde-label {
  display: flex;
  align-items: center;
  gap: 0.35rem;
  padding: 0.45rem 0.65rem;
  font-size: 0.68rem;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--studio-muted);
  background: var(--studio-bg);
  border-bottom: 1px solid var(--studio-border);
}

.gde-label-icon { font-size: 0.8rem; }

.gde-pre {
  margin: 0;
  padding: 0.6rem 0.65rem;
  font-family: var(--font-mono);
  font-size: 0.7rem;
  line-height: 1.65;
  color: var(--studio-text);
  white-space: pre-wrap;
  word-break: break-word;
  overflow-x: auto;
  max-height: 280px;
  overflow-y: auto;
}

/* ── Step 3: Editor ──────────────────────────────────────────────────────── */
.edit-three-col {
  flex: 1;
  display: grid;
  grid-template-columns: 220px 1fr 390px;
  overflow: hidden;
  min-height: 0;
}

.edit-list-col, .edit-preview-col, .edit-json-col {
  display: flex;
  flex-direction: column;
  overflow: hidden;
  border-right: 1px solid var(--studio-border);
}
.edit-json-col { border-right: none; }

.edit-col-head {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  padding: 0.5rem 0.75rem;
  background: var(--studio-panel-2);
  border-bottom: 1px solid var(--studio-border);
  font-size: 0.68rem;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.07em;
  color: var(--studio-muted);
  flex-shrink: 0;
}

.edit-badge {
  background: var(--studio-surface);
  border: 1px solid var(--studio-border);
  border-radius: 999px;
  padding: 0 0.4rem;
  font-size: 0.6rem;
  font-variant-numeric: tabular-nums;
}

.edit-icon-btn {
  width: 26px; height: 26px;
  padding: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 0.8rem;
}

.ml-auto { margin-left: auto; }

/* Slide list */
.edit-list-scroll {
  flex: 1;
  overflow-y: auto;
  padding: 0.35rem;
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
}

.edit-slide-row {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.4rem 0.5rem;
  border-radius: 7px;
  border: 1px solid transparent;
  cursor: pointer;
  transition: background 0.12s, border-color 0.12s;
}
.edit-slide-row:hover { background: var(--studio-hover); }
.edit-slide-row--active {
  background: var(--studio-primary-bg);
  border-color: var(--studio-primary);
}

.edit-row-num {
  font-size: 0.6rem;
  font-weight: 700;
  color: var(--studio-muted);
  font-variant-numeric: tabular-nums;
  min-width: 16px;
}

.edit-row-body {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 0.1rem;
}

.edit-row-title {
  font-size: 0.72rem;
  font-weight: 600;
  color: var(--studio-text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.edit-row-kind {
  font-size: 0.58rem;
  color: var(--studio-muted);
}

.edit-row-del {
  width: 20px; height: 20px;
  border: none;
  background: transparent;
  color: var(--studio-muted);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 0.75rem;
  border-radius: 4px;
  opacity: 0;
  transition: opacity 0.15s, background 0.15s;
  padding: 0;
}
.edit-slide-row:hover .edit-row-del { opacity: 1; }
.edit-row-del:hover { background: var(--studio-error-bg); color: var(--studio-error); }
.edit-row-del:disabled { opacity: 0 !important; cursor: not-allowed; }

.edit-list-footer {
  padding: 0.5rem;
  border-top: 1px solid var(--studio-border);
  flex-shrink: 0;
}

.edit-add-btn {
  width: 100%;
  font-size: 0.72rem;
  justify-content: center;
}

/* Preview */
.edit-preview-col {}

.edit-preview-nav {
  display: flex;
  align-items: center;
  gap: 0.25rem;
}

.edit-preview-pos {
  font-size: 0.65rem;
  font-variant-numeric: tabular-nums;
  min-width: 36px;
  text-align: center;
}

.edit-present-btn {
  font-size: 0.72rem;
  padding: 0.3rem 0.65rem;
}

.edit-preview-stage-wrap {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 1rem;
  overflow: hidden;
  min-height: 0;
}

.edit-preview-stage {
  box-shadow: 0 4px 24px rgba(0,0,0,0.25);
  border-radius: 4px;
  overflow: hidden;
  background: #fff;
  flex-shrink: 0;
}

.edit-preview-canvas {
  pointer-events: none;
}

/* JSON editor */
.edit-json-col {}

.edit-json-inner {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow-y: auto;
  overflow-x: hidden;
  padding: 0.75rem;
  gap: 0.6rem;
  min-height: 0;
}

.edit-head-pos {
  font-size: 0.65rem;
  font-variant-numeric: tabular-nums;
}

.edit-json-acts {
  display: flex;
  align-items: center;
  gap: 0.2rem;
}

.edit-del-btn { color: var(--studio-error); }

.ej-field-group {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
}

.ej-field-group--grow {
  flex: 0 0 auto;
  min-height: 0;
}

.ej-save-btn {
  position: sticky;
  bottom: 0;
  z-index: 1;
}

.ej-label {
  font-size: 0.68rem;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  color: var(--studio-muted);
}

.ej-label-row {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.ej-hint {
  font-size: 0.6rem;
  color: var(--studio-muted);
  margin-left: auto;
}

.ej-fmt-btn {
  font-size: 0.65rem;
  padding: 0.2rem 0.5rem;
}

.ej-kind-row {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.4rem 0.6rem;
  background: var(--studio-surface);
  border: 1px solid var(--studio-border);
  border-radius: 6px;
}

.ej-kind-icon { font-size: 1rem; color: var(--studio-primary); }

.ej-kind-name {
  font-size: 0.78rem;
  font-weight: 600;
  color: var(--studio-text);
}

.ej-kind-key {
  font-size: 0.62rem;
  font-family: var(--font-mono);
  color: var(--studio-muted);
}

.ej-change-btn {
  margin-left: auto;
  font-size: 0.65rem;
  padding: 0.2rem 0.5rem;
}

/* ── Quick fields ── */
.ej-quick-fields {
  display: flex;
  align-items: flex-end;
  gap: 0;
  background: var(--studio-surface);
  border: 1px solid var(--studio-border);
  border-radius: 6px;
  overflow: hidden;
  flex-shrink: 0;
}

.ej-qf-item {
  display: flex;
  flex-direction: column;
  gap: 0.18rem;
  padding: 0.35rem 0.55rem;
  min-width: 0;
}

.ej-qf-item--grow { flex: 1; }

.ej-qf-item--section { flex: 0 0 62px; }

.ej-qf-sep {
  width: 1px;
  align-self: stretch;
  background: var(--studio-border);
  flex-shrink: 0;
}

.ej-qf-label {
  font-size: 0.58rem;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  color: var(--studio-muted);
  user-select: none;
}

.ej-qf-input {
  font-size: 0.75rem !important;
  padding: 0.2rem 0 !important;
  height: auto !important;
  background: transparent !important;
  border: none !important;
  border-radius: 0 !important;
  box-shadow: none !important;
  color: var(--studio-text) !important;
  width: 100%;
}

.ej-qf-input:focus {
  outline: none !important;
  box-shadow: none !important;
  border: none !important;
}

.ej-qf-textarea {
  resize: vertical;
  min-height: 4.5rem;
  padding-top: 0.35rem !important;
  border: 1px solid var(--studio-border) !important;
  border-radius: 6px !important;
  background: var(--studio-panel) !important;
  padding-left: 0.55rem !important;
  padding-right: 0.55rem !important;
}

.ej-structured-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 0.55rem;
}

.ej-struct-field {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
  min-width: 0;
  padding: 0.5rem 0.6rem;
  border: 1px solid var(--studio-border);
  border-radius: 6px;
  background: var(--studio-surface);
}

.ej-struct-field--wide {
  grid-column: 1 / -1;
}

.ej-list-stack {
  display: flex;
  flex-direction: column;
  gap: 0.45rem;
}

.ej-card-block,
.ej-nested-card,
.ej-icon-box {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  padding: 0.6rem;
  border: 1px solid var(--studio-border);
  border-radius: 8px;
  background: var(--studio-panel-2);
}

.ej-nested-card {
  background: var(--studio-surface);
}

.ej-card-head {
  display: flex;
  align-items: center;
  gap: 0.4rem;
}

.ej-card-title {
  font-size: 0.72rem;
  font-weight: 700;
  color: var(--studio-text);
}

.ej-inline-list-item {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  padding: 0.35rem 0.55rem;
  border: 1px solid var(--studio-border);
  border-radius: 6px;
  background: var(--studio-surface);
}

.ej-bool-row {
  display: flex;
  align-items: center;
  gap: 0.45rem;
  min-height: 2.25rem;
  font-size: 0.72rem;
  color: var(--studio-text);
}

.ej-icon-suggestions {
  display: flex;
  flex-wrap: wrap;
  gap: 0.4rem;
}

.ej-icon-chip {
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
  padding: 0.25rem 0.5rem;
  border-radius: 999px;
  border: 1px solid var(--studio-border);
  background: var(--studio-surface);
  color: var(--studio-text);
  font-size: 0.65rem;
}

.ej-icon-chip-preview {
  font-size: 0.9rem;
  color: var(--studio-primary);
}

.ej-media-actions {
  display: flex;
  gap: 0.3rem;
  flex-wrap: wrap;
}

/* ── JSON editor wrap with gutter ── */
.ej-wrap {
  flex: 1;
  display: flex;
  flex-direction: column;
  border: 1px solid var(--studio-border);
  border-radius: 6px;
  overflow: hidden;
  background: var(--studio-panel);
  min-height: 0;
  transition: border-color 0.15s;
}

.ej-wrap--error { border-color: var(--studio-error); }

.ej-editor-row {
  flex: 1;
  display: flex;
  min-height: 0;
  overflow: hidden;
}

/* Line number gutter */
.ej-gutter {
  width: 34px;
  flex-shrink: 0;
  overflow: hidden;
  background: var(--studio-panel-2);
  border-right: 1px solid var(--studio-border);
  display: flex;
  flex-direction: column;
  padding-top: 0.6rem;
  padding-bottom: 0.6rem;
  user-select: none;
}

.ej-line-num {
  display: block;
  font-family: var(--font-mono);
  font-size: 0.62rem;
  line-height: 1.65;
  text-align: right;
  padding-right: 0.45rem;
  color: var(--studio-muted);
  opacity: 0.45;
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
  transition: opacity 0.1s, color 0.1s;
}

.ej-line-num--error {
  color: var(--studio-error) !important;
  opacity: 1 !important;
  background: var(--studio-error-bg);
}

.ej-textarea {
  flex: 1;
  min-height: 0;
  font-family: var(--font-mono);
  font-size: 0.72rem;
  line-height: 1.65;
  resize: none;
  padding: 0.6rem 0.75rem;
  background: transparent;
  border: none;
  color: var(--studio-text);
  width: 100%;
  box-sizing: border-box;
}

.ej-textarea:focus { outline: none; }

/* Parse error bar */
.ej-error {
  display: flex;
  align-items: flex-start;
  gap: 0.35rem;
  padding: 0.35rem 0.6rem;
  font-size: 0.65rem;
  color: var(--studio-error);
  background: var(--studio-error-bg);
  border-top: 1px solid var(--studio-error-border);
  flex-shrink: 0;
}

.ej-error-icon { font-size: 0.72rem; margin-top: 0.05rem; flex-shrink: 0; }

.ej-error-text {
  flex: 1;
  line-height: 1.5;
  word-break: break-all;
}

/* Semantic warnings */
.ej-semantic-errors {
  display: flex;
  flex-direction: column;
  gap: 0.18rem;
  padding: 0.3rem 0.6rem;
  background: var(--studio-panel-2);
  border-top: 1px solid var(--studio-border);
  flex-shrink: 0;
}

.ej-semantic-item {
  display: flex;
  align-items: center;
  gap: 0.3rem;
  font-size: 0.62rem;
  color: var(--studio-text);
  opacity: 0.72;
  line-height: 1.4;
}

.ej-semantic-icon {
  font-size: 0.68rem;
  color: var(--studio-primary);
  flex-shrink: 0;
}

/* ── Field chips section ── */
.ej-chips-section {
  display: flex;
  flex-direction: column;
  gap: 0.3rem;
  flex-shrink: 0;
  background: var(--studio-panel-2);
  border: 1px solid var(--studio-border);
  border-radius: 6px;
  padding: 0.45rem 0.6rem;
}

.ej-chips-row {
  display: flex;
  align-items: center;
  gap: 0.3rem;
  flex-wrap: wrap;
}

.ej-chips-label {
  font-size: 0.58rem;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--studio-muted);
  font-family: var(--font-mono);
  min-width: 42px;
  flex-shrink: 0;
}

@media (max-width: 1280px) {
  .ej-structured-grid {
    grid-template-columns: minmax(0, 1fr);
  }
}

.ej-tone-chip {
  display: inline-flex;
  align-items: center;
  gap: 0.22rem;
  padding: 0.13rem 0.42rem;
  border-radius: 999px;
  font-size: 0.6rem;
  font-weight: 600;
  font-family: var(--font-mono);
  border: 1px solid var(--studio-border);
  background: var(--studio-surface);
  color: var(--studio-text);
  cursor: pointer;
  transition: border-color 0.12s, background 0.12s, transform 0.12s;
  line-height: 1;
}

.ej-tone-chip:hover {
  border-color: var(--studio-border-hover);
  background: var(--studio-panel);
  transform: translateY(-1px);
}

.ej-chip-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  flex-shrink: 0;
}

.ej-dir-chip {
  display: inline-flex;
  align-items: center;
  gap: 0.25rem;
  padding: 0.13rem 0.5rem;
  border-radius: 5px;
  font-size: 0.62rem;
  font-family: var(--font-mono);
  border: 1px solid var(--studio-border);
  background: var(--studio-surface);
  color: var(--studio-text);
  cursor: pointer;
  transition: all 0.12s;
  line-height: 1;
}

.ej-dir-chip:hover {
  border-color: var(--studio-primary-border);
  background: var(--studio-primary-bg);
  color: var(--studio-primary);
}

/* ── Media library ── */
.ej-media-group { flex-shrink: 0; }

.ej-media-strip {
  display: flex;
  flex-wrap: wrap;
  gap: 0.35rem;
  padding: 0.35rem;
  background: var(--studio-surface);
  border: 1px solid var(--studio-border);
  border-radius: 6px;
  max-height: 120px;
  overflow-y: auto;
}

.ej-media-item {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.15rem;
  cursor: pointer;
  border-radius: 5px;
  padding: 0.2rem;
  transition: background 0.15s;
  width: 56px;
}

.ej-media-item:hover { background: var(--studio-primary-bg); }

.ej-media-thumb {
  width: 52px;
  height: 36px;
  object-fit: cover;
  border-radius: 4px;
  border: 1px solid var(--studio-border);
}

.ej-media-video-thumb {
  width: 52px;
  height: 36px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--studio-panel);
  border: 1px solid var(--studio-border);
  border-radius: 4px;
  color: var(--studio-muted);
  font-size: 1.1rem;
}

.ej-media-label {
  font-size: 0.55rem;
  color: var(--studio-muted);
  max-width: 52px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ej-save-btn {
  font-size: 0.78rem;
  padding: 0.5rem 1rem;
  flex-shrink: 0;
}

/* ── Kind picker modal ───────────────────────────────────────────────────── */
.kp-backdrop {
  position: fixed;
  inset: 0;
  background: rgba(0,0,0,0.5);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
  backdrop-filter: blur(2px);
}

.kp-box {
  background: var(--studio-panel);
  border: 1px solid var(--studio-border);
  border-radius: 12px;
  width: 680px;
  max-height: 80vh;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  box-shadow: 0 20px 60px rgba(0,0,0,0.4);
}

.kp-head {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.875rem 1.1rem;
  border-bottom: 1px solid var(--studio-border);
  font-size: 0.875rem;
  font-weight: 700;
  color: var(--studio-text);
  flex-shrink: 0;
}

.kp-head-icon { font-size: 1.1rem; color: var(--studio-primary); }

.kp-close { margin-left: auto; }

.kp-sub {
  margin: 0;
  padding: 0.6rem 1.1rem;
  font-size: 0.72rem;
  color: var(--studio-muted);
  border-bottom: 1px solid var(--studio-border);
  flex-shrink: 0;
}

.kp-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 0.5rem;
  padding: 0.875rem;
  overflow-y: auto;
}

.kp-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.35rem;
  padding: 0.875rem 0.5rem;
  border: 1px solid var(--studio-border);
  border-radius: 8px;
  background: var(--studio-surface);
  cursor: pointer;
  text-align: center;
  font-family: inherit;
  color: var(--studio-text);
  transition: all 0.15s;
}
.kp-card:hover { border-color: var(--studio-primary); background: var(--studio-primary-bg); }
.kp-card--current { border-color: var(--studio-primary); background: var(--studio-primary-bg); }

.kp-card-icon { font-size: 1.5rem; color: var(--studio-primary); }

.kp-card-label {
  font-size: 0.78rem;
  font-weight: 700;
  color: var(--studio-text);
}

.kp-card-key {
  font-size: 0.58rem;
  font-family: var(--font-mono);
  color: var(--studio-muted);
}

.kp-card-desc {
  font-size: 0.62rem;
  color: var(--studio-muted);
  line-height: 1.4;
}

/* ── Presentation overlay ───────────────────────────────────────────────── */
.presentation-overlay {
  position: fixed;
  inset: 0;
  width: 100vw;
  height: 100vh;
  min-height: 100vh;
  background:
    radial-gradient(circle at top, color-mix(in srgb, var(--studio-primary) 12%, transparent), transparent 34%),
    linear-gradient(180deg, color-mix(in srgb, var(--studio-panel) 74%, transparent), transparent 22%),
    var(--studio-bg);
  z-index: 2000;
  display: flex;
  flex-direction: column;
  color: var(--studio-text);
  box-sizing: border-box;
}

.presentation-overlay:fullscreen {
  position: fixed;
  inset: 0;
  width: 100%;
  height: 100%;
  min-height: 100%;
  max-width: none;
  max-height: none;
  background:
    radial-gradient(circle at top, color-mix(in srgb, var(--studio-primary) 12%, transparent), transparent 34%),
    linear-gradient(180deg, color-mix(in srgb, var(--studio-panel) 74%, transparent), transparent 22%),
    var(--studio-bg);
}

.presentation-overlay:fullscreen::backdrop {
  background: var(--studio-bg);
}

.shortcut-hint {
  position: absolute;
  top: 1rem;
  left: 50%;
  transform: translateX(-50%);
  display: flex;
  gap: 1rem;
  background: color-mix(in srgb, var(--studio-panel) 92%, transparent);
  border: 1px solid var(--studio-border);
  box-shadow: 0 18px 40px color-mix(in srgb, var(--studio-bg) 32%, transparent);
  border-radius: 10px;
  padding: 0.6rem 1.25rem;
  z-index: 10;
  pointer-events: none;
}

.shortcut-item {
  display: flex;
  align-items: center;
  gap: 0.35rem;
  font-size: 0.72rem;
  color: var(--studio-muted);
}

kbd {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 22px;
  height: 22px;
  padding: 0 0.35rem;
  background: var(--studio-surface);
  border: 1px solid var(--studio-border);
  border-radius: 4px;
  font-family: var(--font-mono);
  font-size: 0.65rem;
  color: var(--studio-text);
}

.shortcut-fade-enter-active,
.shortcut-fade-leave-active {
  transition: opacity 0.3s;
}
.shortcut-fade-enter-from,
.shortcut-fade-leave-to {
  opacity: 0;
}

.pres-nav {
  position: absolute;
  top: 50%;
  transform: translateY(-50%);
  width: 52px; height: 52px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 1.5rem;
  color: var(--studio-muted);
  background: color-mix(in srgb, var(--studio-panel) 84%, transparent);
  border: 1px solid var(--studio-border);
  border-radius: 999px;
  box-shadow: 0 10px 24px color-mix(in srgb, var(--studio-bg) 26%, transparent);
  backdrop-filter: blur(14px);
  cursor: pointer;
  z-index: 5;
  transition: color 0.15s, border-color 0.15s, background 0.15s, transform 0.15s;
}
.pres-nav:hover {
  color: var(--studio-text);
  border-color: var(--studio-border-hover);
  background: color-mix(in srgb, var(--studio-panel-2) 88%, transparent);
}
.pres-prev { left: 1rem; }
.pres-next { right: 1rem; }

.pres-stage {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  overflow: hidden;
  min-height: 0;
}

.pres-overview {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 2rem 2.25rem 1.5rem;
  box-sizing: border-box;
}

.pres-overview-head {
  margin: 0 auto 1.5rem;
  width: min(1480px, 100%);
}

.pres-overview-title {
  font-size: 1.2rem;
  font-weight: 700;
  color: var(--studio-text);
}

.pres-overview-subtitle {
  margin-top: 0.3rem;
  font-size: 0.82rem;
  color: var(--studio-muted);
}

.pres-overview-grid {
  width: min(1480px, 100%);
  margin: 0 auto;
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
  gap: 1rem;
}

.pres-overview-card {
  display: flex;
  flex-direction: column;
  gap: 0.7rem;
  padding: 0.7rem;
  border-radius: 14px;
  background: color-mix(in srgb, var(--studio-panel) 84%, transparent);
  border: 1px solid var(--studio-border);
  cursor: pointer;
  transition: transform 0.18s ease, border-color 0.18s ease, background 0.18s ease, box-shadow 0.18s ease;
}

.pres-overview-card:hover {
  transform: translateY(-2px);
  border-color: var(--studio-border-hover);
  background: color-mix(in srgb, var(--studio-panel-2) 88%, transparent);
}

.pres-overview-card.active {
  border-color: var(--studio-primary);
  background: color-mix(in srgb, var(--studio-primary) 12%, rgba(255,255,255,0.04));
  box-shadow: 0 0 0 1px color-mix(in srgb, var(--studio-primary) 55%, transparent);
}

.pres-overview-thumb {
  position: relative;
  width: min(100%, 256px);
  height: 144px;
  margin: 0 auto;
  overflow: hidden;
  border-radius: 10px;
  background: var(--studio-bg);
  border: 1px solid color-mix(in srgb, var(--studio-border-hover) 72%, transparent);
}

.pres-overview-slide {
  width: 1280px;
  height: 720px;
  transform: scale(0.2);
  transform-origin: top left;
  pointer-events: none;
}

.pres-overview-meta {
  display: flex;
  align-items: center;
  gap: 0.6rem;
  min-width: 0;
}

.pres-overview-index {
  flex-shrink: 0;
  width: 1.7rem;
  height: 1.7rem;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: 999px;
  background: var(--studio-surface);
  font-size: 0.74rem;
  font-weight: 700;
  color: var(--studio-text);
}

.pres-overview-name {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 0.82rem;
  font-weight: 600;
  color: var(--studio-text);
}

.pres-slide-wrapper {
  position: relative;
  border-radius: 20px;
  overflow: hidden;
  box-shadow:
    0 0 0 1px color-mix(in srgb, var(--studio-border-hover) 85%, transparent),
    0 26px 80px color-mix(in srgb, var(--studio-bg) 42%, transparent);
}

.pres-slide {
  pointer-events: none;
}

.pres-controls {
  display: flex;
  align-items: center;
  gap: 1rem;
  padding: 0.75rem 1.25rem;
  background:
    linear-gradient(180deg, color-mix(in srgb, var(--studio-panel-2) 94%, transparent), color-mix(in srgb, var(--studio-panel) 96%, transparent));
  border-top: 1px solid var(--studio-border);
  box-shadow: 0 -14px 34px color-mix(in srgb, var(--studio-bg) 18%, transparent);
  flex-shrink: 0;
}

.pres-filmstrip {
  display: flex;
  gap: 0.35rem;
  overflow-x: auto;
  flex: 1;
  padding-bottom: 2px;
}

.filmstrip-item {
  flex-shrink: 0;
  width: 36px; height: 28px;
  border-radius: 4px;
  border: 1.5px solid var(--studio-border);
  background: var(--studio-surface);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: border-color 0.15s, background 0.15s;
}
.filmstrip-item:hover { border-color: var(--studio-border-hover); background: var(--studio-panel-2); }
.filmstrip-item.active { border-color: var(--studio-primary); background: var(--studio-primary-bg); }

.filmstrip-num {
  font-size: 0.6rem;
  font-weight: 700;
  font-variant-numeric: tabular-nums;
  color: var(--studio-muted);
}
.filmstrip-item.active .filmstrip-num { color: var(--studio-primary); }

.pres-controls-right {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  flex-shrink: 0;
}

.pres-center-info {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.2rem;
}

.pres-counter {
  font-size: 0.72rem;
  font-weight: 700;
  font-variant-numeric: tabular-nums;
  color: var(--studio-text);
}

.pres-progress-wrap {
  width: 80px;
}

.pres-progress {
  height: 3px;
  background: var(--studio-border);
  border-radius: 2px;
  overflow: hidden;
}

.pres-progress-fill {
  height: 100%;
  background: var(--studio-primary);
  transition: width 0.2s ease;
}

.pres-theme-toggle {
  margin: 0 0.25rem;
}

.pres-ctrl-btn {
  width: 32px; height: 32px;
  padding: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 0.875rem;
  color: var(--studio-muted);
  background: var(--studio-surface);
  border: 1px solid var(--studio-border);
}
.pres-ctrl-btn:hover {
  color: var(--studio-text);
  border-color: var(--studio-border-hover);
  background: var(--studio-panel-2);
}

@keyframes spin {
  to { transform: rotate(360deg); }
}
@keyframes blink {
  0%, 100% { opacity: 1; }
  50% { opacity: 0; }
}
@keyframes slide-in {
  from { opacity: 0; transform: translateY(4px); }
  to   { opacity: 1; transform: translateY(0); }
}
</style>
