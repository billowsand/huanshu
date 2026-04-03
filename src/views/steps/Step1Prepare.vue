<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { readTextFile } from '@tauri-apps/plugin-fs'
import type { MediaItem } from '../../composables/useMediaLibrary'
import MarkdownWorkbench from '../../components/MarkdownWorkbench.vue'

const props = defineProps<{
  modelValue: {
    mdContent: string
    mdFilename: string
    projectName: string
    granularity: 'auto' | 'h2' | 'h3'
    aspectRatio: 'ratio_16x9' | 'ratio_32x9' | 'ratio_48x9'
  }
  detectedGranularity: 'h2' | 'h3'
  canGenerate: boolean
  mediaItems: MediaItem[]
  mediaDropOver: boolean
  draftSaveState: 'idle' | 'saving' | 'saved' | 'error'
  draftSaveError: string
}>()

const emit = defineEmits<{
  'update:modelValue': [value: typeof props.modelValue]
  'update:detectedGranularity': [value: 'h2' | 'h3']
  'startGenerate': []
  'media-drop-over': [value: boolean]
  'open-media-files': []
  'add-media-from-path': [path: string]
  'add-media-from-file': [file: File]
  'remove-media': [id: number]
}>()

type ParsedSection = {
  paragraphs: string[]
  subsections: Array<{ paragraphs: string[] }>
}

type HeadingNumberStyle = 'cn-section' | 'cn-subsection' | 'decimal'

function setStepModel(patch: Partial<typeof props.modelValue>) {
  emit('update:modelValue', { ...props.modelValue, ...patch })
}

const mdContent = computed({
  get: () => props.modelValue.mdContent,
  set: (v) => setStepModel({ mdContent: v }),
})
const projectName = computed({
  get: () => props.modelValue.projectName,
  set: (v) => setStepModel({ projectName: v }),
})
const granularity = computed({
  get: () => props.modelValue.granularity,
  set: (v) => setStepModel({ granularity: v }),
})
const aspectRatio = computed({
  get: () => props.modelValue.aspectRatio,
  set: (v) => setStepModel({ aspectRatio: v }),
})

const GRANULARITY_LABELS: Record<string, string> = {
  h2: '按二级标题',
  h3: '按三级标题',
}

const ASPECT_RATIO_OPTIONS: { value: 'ratio_16x9' | 'ratio_32x9' | 'ratio_48x9', label: string, desc: string }[] = [
  { value: 'ratio_16x9', label: '16:9', desc: '标准宽屏' },
  { value: 'ratio_32x9', label: '32:9', desc: '超宽屏幕' },
  { value: 'ratio_48x9', label: '48:9', desc: '全景屏幕' },
]

const MEDIA_EXTS_IMG = ['png', 'jpg', 'jpeg', 'gif', 'webp', 'svg']
const MEDIA_EXTS_VID = ['mp4', 'webm', 'mov']
const MEDIA_EXTS = [...MEDIA_EXTS_IMG, ...MEDIA_EXTS_VID]

const isDragOver = ref(false)
const optimizing = ref(false)
const optimizeError = ref('')

const draftStatusText = computed(() => {
  if (props.draftSaveState === 'saving') return '草稿保存中...'
  if (props.draftSaveState === 'saved') return '草稿已保存'
  if (props.draftSaveState === 'error') return '草稿保存失败'
  return ''
})

function countExactHeadings(raw: string, level: 2 | 3) {
  return raw
    .split(/\r?\n/)
    .filter((line) => {
      const trimmed = line.trim()
      if (level === 2) return /^##\s+[^#]/.test(trimmed)
      return /^###\s+[^#]/.test(trimmed)
    })
    .length
}

function parseSections(raw: string): ParsedSection[] {
  const sections: ParsedSection[] = []
  let currentSection: ParsedSection | null = null
  let currentSubsection: { paragraphs: string[] } | null = null
  let paragraphBuffer: string[] = []

  const flushParagraph = () => {
    if (!paragraphBuffer.length) return
    const chunk = paragraphBuffer.join('\n').trim()
    paragraphBuffer = []
    if (!chunk) return
    if (currentSubsection) currentSubsection.paragraphs.push(chunk)
    else if (currentSection) currentSection.paragraphs.push(chunk)
  }

  const flushSubsection = () => {
    if (!currentSection || !currentSubsection) return
    currentSection.subsections.push(currentSubsection)
    currentSubsection = null
  }

  const flushSection = () => {
    if (!currentSection) return
    sections.push(currentSection)
    currentSection = null
  }

  for (const rawLine of raw.split(/\r?\n/)) {
    const line = rawLine.trim()
    if (line === '---') {
      flushParagraph()
      continue
    }
    if (line.startsWith('# ') && !line.startsWith('## ')) {
      flushParagraph()
      flushSubsection()
      flushSection()
      continue
    }
    if (/^##\s+[^#]/.test(line)) {
      flushParagraph()
      flushSubsection()
      flushSection()
      currentSection = { paragraphs: [], subsections: [] }
      continue
    }
    if (/^###\s+[^#]/.test(line)) {
      flushParagraph()
      flushSubsection()
      currentSubsection = { paragraphs: [] }
      continue
    }
    if (!line) {
      flushParagraph()
      continue
    }
    paragraphBuffer.push(line)
  }

  flushParagraph()
  flushSubsection()
  flushSection()
  return sections
}

const h2Count = computed(() => countExactHeadings(mdContent.value, 2))
const h3Count = computed(() => countExactHeadings(mdContent.value, 3))
const detectedGranularityLocal = computed<'h2' | 'h3'>(() => (h2Count.value > h3Count.value ? 'h2' : 'h3'))
const parsedSections = computed(() => parseSections(mdContent.value))

function estimateSlides(mode: 'auto' | 'h2' | 'h3') {
  const sections = parsedSections.value
  if (!sections.length) return 0
  const resolved = mode === 'auto' ? detectedGranularityLocal.value : mode
  if (resolved === 'h2') return sections.length
  return sections.reduce((sum, section) => sum + 1 + section.subsections.length, 0)
}

const estimatedSlideCount = computed(() => estimateSlides(granularity.value))

watch(
  detectedGranularityLocal,
  (value) => emit('update:detectedGranularity', value),
  { immediate: true },
)

async function openFile() {
  try {
    const path = await open({ filters: [{ name: 'Markdown', extensions: ['md', 'txt'] }] })
    if (!path || typeof path !== 'string') return
    await loadFilePath(path)
  } catch (e) {
    console.error(e)
  }
}

async function loadFilePath(path: string) {
  const text = await readTextFile(path)
  const filename = path.split(/[/\\]/).pop() ?? path
  const h1 = text.match(/^#\s+(.+)$/m)
  setStepModel({
    mdContent: text,
    mdFilename: filename,
    projectName: h1 ? h1[1].trim() : (props.modelValue.projectName || filename.replace(/\.[^.]+$/, '')),
  })
}

function onDragOver(e: DragEvent) {
  e.preventDefault()
  isDragOver.value = true
}

function onDragLeave() {
  isDragOver.value = false
}

async function onDrop(e: DragEvent) {
  e.preventDefault()
  isDragOver.value = false
  const file = e.dataTransfer?.files[0]
  if (!file) return
  const text = await file.text()
  const h1 = text.match(/^#\s+(.+)$/m)
  setStepModel({
    mdContent: text,
    mdFilename: file.name,
    projectName: h1 ? h1[1].trim() : (props.modelValue.projectName || file.name.replace(/\.[^.]+$/, '')),
  })
}

function onMediaDragOver(e: DragEvent) {
  e.preventDefault()
  emit('media-drop-over', true)
}

function onMediaDragLeave() {
  emit('media-drop-over', false)
}

function onMediaDrop(e: DragEvent) {
  e.preventDefault()
  emit('media-drop-over', false)
  for (const file of Array.from(e.dataTransfer?.files ?? [])) {
    const ext = file.name.split('.').pop()?.toLowerCase() ?? ''
    if (MEDIA_EXTS.includes(ext)) emit('add-media-from-file', file)
  }
}

function stripHeadingNumberPrefix(title: string) {
  return title
    .trim()
    .replace(/^(?:第[一二三四五六七八九十百千万零〇两\d]+\s*[章节部分篇讲]|[一二三四五六七八九十百千万零〇两]+[、.．:：]|（[一二三四五六七八九十百千万零〇两\d]+）|\([一二三四五六七八九十百千万零〇两\d]+\)|\d+[、.．)\]:：-]|[A-Za-z][、.．)\]:：-])\s*/u, '')
}

function toChineseNumber(value: number) {
  const digits = ['零', '一', '二', '三', '四', '五', '六', '七', '八', '九']
  if (value <= 10) {
    if (value === 10) return '十'
    return digits[value]
  }
  if (value < 20) return `十${digits[value % 10]}`
  if (value < 100) {
    const tens = Math.floor(value / 10)
    const ones = value % 10
    return `${digits[tens]}十${ones ? digits[ones] : ''}`
  }
  return String(value)
}

function addHeadingNumbers(level: 2 | 3 | 4, style: HeadingNumberStyle) {
  const lines = mdContent.value.split(/\r?\n/)
  let h2Index = 0
  let h3Index = 0
  let h4Index = 0
  const nextContent = lines.map((line) => {
    const match = line.match(/^(\s*)(#{1,6})\s+(.+)$/)
    if (!match) return line
    const indent = match[1] ?? ''
    const hashes = match[2] ?? ''
    const rawTitle = stripHeadingNumberPrefix(match[3] ?? '')
    const currentLevel = hashes.length

    if (currentLevel === 2) {
      h2Index += 1
      h3Index = 0
      h4Index = 0
    } else if (currentLevel === 3) {
      h3Index += 1
      h4Index = 0
    } else if (currentLevel === 4) {
      h4Index += 1
    }

    if (currentLevel !== level) return line

    const index = level === 2 ? h2Index : level === 3 ? h3Index : h4Index
    if (index <= 0) return `${indent}${hashes} ${rawTitle}`.trimEnd()

    const marker = style === 'cn-section'
      ? `${toChineseNumber(index)}、`
      : style === 'cn-subsection'
        ? `（${toChineseNumber(index)}）`
        : `${index}.`
    return `${indent}${hashes} ${marker} ${rawTitle}`.trimEnd()
  }).join('\n')
  setStepModel({ mdContent: nextContent })
}

function removeHeadingNumbers() {
  const nextContent = mdContent.value
    .split(/\r?\n/)
    .map((line) => {
      const match = line.match(/^(\s*#{1,6}\s+)(.+)$/)
      if (!match) return line
      return `${match[1]}${stripHeadingNumberPrefix(match[2])}`.trimEnd()
    })
    .join('\n')
  setStepModel({ mdContent: nextContent })
}

function autoDetectHeadings() {
  const nextContent = mdContent.value
    .split(/\r?\n/)
    .map((line) => {
      const trimmed = line.trim()
      if (!trimmed || trimmed.startsWith('#'))
        return line

      const h2Match = trimmed.match(/^[一二三四五六七八九十百千万零〇两]+[、.．:：]\s*(.+)$/u)
      if (h2Match)
        return `## ${h2Match[1].trim()}`

      const h3Match = trimmed.match(/^(?:（[一二三四五六七八九十百千万零〇两\d]+）|\([一二三四五六七八九十百千万零〇两\d]+\))\s*(.+)$/u)
      if (h3Match)
        return `### ${h3Match[1].trim()}`

      return line
    })
    .join('\n')
  setStepModel({ mdContent: nextContent })
}

function clearHeadingBold() {
  const nextContent = mdContent.value
    .split(/\r?\n/)
    .map((line) => {
      const match = line.match(/^(\s*#{1,6}\s+)(.+)$/)
      if (!match)
        return line
      return `${match[1]}${match[2].replace(/\*\*(.+?)\*\*/g, '$1')}`.trimEnd()
    })
    .join('\n')
  setStepModel({ mdContent: nextContent })
}

async function optimizeMarkdown() {
  if (!mdContent.value.trim() || optimizing.value) return
  optimizing.value = true
  optimizeError.value = ''
  try {
    const optimized = await invoke<string>('optimize_markdown_headings', {
      rawContent: mdContent.value,
      titleHint: projectName.value.trim() || null,
    })
    const nextContent = optimized.trim()
    const h1 = optimized.match(/^#\s+(.+)$/m)
    setStepModel({
      mdContent: nextContent,
      projectName: h1 ? h1[1].trim() : props.modelValue.projectName,
    })
  } catch (e) {
    optimizeError.value = String(e)
  } finally {
    optimizing.value = false
  }
}
</script>

<template>
  <div class="step-panel step-prepare">

    <!-- ── Top bar: title · project name · file actions ── -->
    <div class="prepare-topbar">
      <div class="prepare-topbar-left">
        <span class="prepare-page-label">准备素材</span>
        <div class="prepare-name-wrap">
          <input
            v-model="projectName"
            class="project-name-input"
            placeholder="演示名称（从 H1 自动提取或手动输入）..."
          />
        </div>
      </div>
      <div class="prepare-topbar-right">
        <span v-if="modelValue.mdFilename" class="filename-tag">
          <span class="i-carbon:document" />
          {{ modelValue.mdFilename }}
        </span>
        <span v-if="draftStatusText" class="char-pill">{{ draftStatusText }}</span>
        <span class="char-pill">{{ mdContent.length }} 字</span>
        <button
          class="btn btn-ghost upload-btn"
          :disabled="optimizing || !mdContent.trim()"
          @click="optimizeMarkdown"
        >
          <span class="i-carbon:magic-wand" />
          {{ optimizing ? '优化中...' : 'AI 优化' }}
        </button>
        <button class="btn btn-ghost upload-btn" :disabled="optimizing" @click="openFile">
          <span class="i-carbon:upload" />
          上传文件
        </button>
      </div>
    </div>

    <!-- ── Heading number tools ── -->
    <div class="prepare-tools">
      <span class="tools-label">格式工具</span>
      <button
        class="btn btn-ghost btn-tool"
        :disabled="optimizing || !mdContent.trim()"
        @click="autoDetectHeadings"
      >
        自动标题
      </button>
      <button
        class="btn btn-ghost btn-tool"
        :disabled="optimizing || !mdContent.trim()"
        @click="clearHeadingBold"
      >
        清除标题加粗
      </button>
      <button
        class="btn btn-ghost btn-tool"
        :disabled="optimizing || !mdContent.trim()"
        @click="removeHeadingNumbers"
      >
        删除序号
      </button>
      <button
        class="btn btn-ghost btn-tool"
        :disabled="optimizing || !mdContent.trim()"
        @click="addHeadingNumbers(2, 'cn-section')"
      >
        `##` 一、二、
      </button>
      <button
        class="btn btn-ghost btn-tool"
        :disabled="optimizing || !mdContent.trim()"
        @click="addHeadingNumbers(3, 'cn-subsection')"
      >
        `###` （一）（二）
      </button>
      <button
        class="btn btn-ghost btn-tool"
        :disabled="optimizing || !mdContent.trim()"
        @click="addHeadingNumbers(4, 'decimal')"
      >
        `####` 1. 2.
      </button>
      <span v-if="draftSaveState === 'error' && draftSaveError" class="prepare-inline-error">{{ draftSaveError }}</span>
      <span v-if="optimizeError" class="prepare-inline-error">{{ optimizeError }}</span>
    </div>

    <!-- ── Main workbench: editor + sidebar ── -->
    <div class="prepare-body">

      <!-- Editor column with drag-drop -->
      <div
        class="prepare-editor-col"
        :class="{ 'prepare-editor-col--dragover': isDragOver }"
        @dragover="onDragOver"
        @dragleave="onDragLeave"
        @drop="onDrop"
      >
        <div v-if="isDragOver" class="prepare-drop-overlay">
          <span class="i-carbon:document-import" />
          <span>释放以加载 Markdown 文件</span>
        </div>
        <MarkdownWorkbench
          v-model="mdContent"
          class="prepare-editor"
          :granularity="granularity"
          :detected-granularity="detectedGranularityLocal"
          placeholder="粘贴 Markdown 内容，或拖拽 .md 文件到此处...

# 演示标题

## 章节一
内容描述...

## 章节二
内容描述..."
        />
      </div>

      <!-- Right sidebar -->
      <aside class="prepare-sidebar">

        <!-- Granularity card -->
        <div class="sidebar-card">
          <div class="sidebar-card-hd">
            <span class="field-label">分页粒度</span>
            <span class="char-pill accent-pill">预计 {{ estimatedSlideCount }} 页</span>
          </div>
          <div class="granularity-stack">
            <button
              v-for="opt in (['auto', 'h2', 'h3'] as const)"
              :key="opt"
              class="gran-choice"
              :class="{ active: granularity === opt }"
              @click="granularity = opt"
            >
              <div class="gran-choice-title">
                {{ opt === 'auto' ? '自动推荐' : GRANULARITY_LABELS[opt] }}
              </div>
              <div class="gran-choice-meta">
                <span class="char-pill">
                  {{
                    opt === 'auto'
                      ? GRANULARITY_LABELS[detectedGranularityLocal]
                      : `${opt === 'h2' ? h2Count : h3Count} 个标题`
                  }}
                </span>
                <span class="char-pill">{{ estimateSlides(opt) }} 页</span>
              </div>
            </button>
          </div>
          <div class="sidebar-note">
            二级标题 {{ h2Count }} 个，三级标题 {{ h3Count }} 个
          </div>
        </div>

        <!-- Aspect Ratio card -->
        <div class="sidebar-card">
          <div class="sidebar-card-hd">
            <span class="field-label">幻灯片比例</span>
          </div>
          <div class="aspect-ratio-stack">
            <button
              v-for="opt in ASPECT_RATIO_OPTIONS"
              :key="opt.value"
              class="aspect-ratio-choice"
              :class="{ active: aspectRatio === opt.value }"
              @click="aspectRatio = opt.value"
            >
              <div class="aspect-ratio-label">{{ opt.label }}</div>
              <div class="aspect-ratio-desc">{{ opt.desc }}</div>
            </button>
          </div>
          <div class="sidebar-note">
            32:9 / 48:9 在 16:9 屏幕上播放时会横向压缩
          </div>
        </div>

        <!-- Media card -->
        <div class="sidebar-card media-card">
          <div class="sidebar-card-hd">
            <span class="field-label">素材库</span>
            <div class="sidebar-card-hd-actions">
              <span class="char-pill">{{ mediaItems.length }} 个文件</span>
              <button class="btn btn-ghost btn-tool" @click="$emit('open-media-files')">
                <span class="i-carbon:add-alt" /> 添加素材
              </button>
            </div>
          </div>
          <div
            class="media-drop-zone"
            :class="{ dragover: mediaDropOver }"
            @dragover="onMediaDragOver"
            @dragleave="onMediaDragLeave"
            @drop="onMediaDrop"
          >
            <div v-if="mediaItems.length === 0" class="media-empty">
              <span class="i-carbon:image-copy media-empty-icon" />
              <span>拖拽图片 / 视频到此处</span>
            </div>
            <div v-else class="media-grid">
              <div v-for="m in mediaItems" :key="m.id" class="media-thumb-wrap">
                <div class="media-thumb">
                  <img v-if="m.type === 'image'" :src="m.url" class="media-thumb-img" />
                  <div v-else class="media-thumb-video">
                    <span class="i-carbon:video-filled" />
                  </div>
                  <button class="media-del-btn" @click="$emit('remove-media', m.id)" title="移除">
                    <span class="i-carbon:close" />
                  </button>
                </div>
                <span class="media-name" :title="m.name">{{ m.name }}</span>
              </div>
            </div>
          </div>
          <div class="sidebar-note">素材生成时自动参与图文匹配。</div>
        </div>

      </aside>
    </div>

    <!-- ── Footer: generate action ── -->
    <div class="step-actions">
      <button
        class="btn btn-primary generate-btn"
        :disabled="!canGenerate || optimizing"
        @click="$emit('startGenerate')"
      >
        <span class="i-carbon:magic-wand" />
        开始 AI 生成
      </button>
      <span v-if="!projectName.trim()" class="hint-text">请填写演示名称</span>
      <span v-else-if="!mdContent.trim()" class="hint-text">请上传或粘贴文稿内容</span>
      <span v-else class="hint-text">当前预计生成 {{ estimatedSlideCount }} 张幻灯片</span>
    </div>

  </div>
</template>
