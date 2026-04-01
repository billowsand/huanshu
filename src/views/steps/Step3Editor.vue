<script setup lang="ts">
import { computed, ref, nextTick } from 'vue'
import SlideRenderer from '../../components/SlideRenderer.vue'
import { KIND_META, ALL_KINDS, KIND_FIELDS } from '../../composables/useSlideEditor'
import type { SlideBlueprint, SlideKind } from '../../components/types'
import type { MediaItem } from '../../composables/useMediaLibrary'

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

// ── Quick fields ─────────────────────────────────────────────────────────────

function tryParseJson(text: string): Record<string, unknown> | null {
  try { return JSON.parse(text) } catch { return null }
}

const parsedJson = computed(() => tryParseJson(props.jsonText))

const quickTitle    = computed(() => (parsedJson.value?.title    as string) ?? '')
const quickSubtitle = computed(() => (parsedJson.value?.subtitle as string) ?? '')
const quickSection  = computed(() => (parsedJson.value?.section  as string) ?? '')

const kindFields  = computed(() => KIND_FIELDS[props.editorKind] ?? [])
const hasSubtitle = computed(() => kindFields.value.includes('subtitle'))
const hasSection  = computed(() => kindFields.value.includes('section'))

function handleQuickField(field: string, value: string) {
  const obj = parsedJson.value
  if (!obj) return
  const updated: Record<string, unknown> = { ...obj }
  if (value.trim() === '') {
    delete updated[field]
  } else {
    updated[field] = value
  }
  const newText = JSON.stringify(updated, null, 2)
  if (props.jsonTextareaRef) props.jsonTextareaRef.value = newText
  emit('textarea-input', newText)
}

// ── Line number gutter ───────────────────────────────────────────────────────

const gutterRef = ref<HTMLElement | null>(null)
const lineCount  = computed(() => props.jsonText.split('\n').length)

function syncGutterScroll(event: Event) {
  if (gutterRef.value) {
    gutterRef.value.scrollTop = (event.target as HTMLTextAreaElement).scrollTop
  }
}

// ── Error line detection ─────────────────────────────────────────────────────

const errorLine = computed<number | null>(() => {
  if (!props.jsonError) return null
  const msg = props.jsonError
  const lineMatch = msg.match(/line (\d+)/i)
  if (lineMatch) return parseInt(lineMatch[1])
  const posMatch = msg.match(/position (\d+)/i)
  if (posMatch) {
    const pos = parseInt(posMatch[1])
    return props.jsonText.slice(0, pos).split('\n').length
  }
  return null
})

// ── Semantic validation ──────────────────────────────────────────────────────

const VALID_TONES      = new Set(['blue', 'emerald', 'green', 'violet', 'amber', 'rose', 'gray', 'red'])
const VALID_DIRECTIONS = new Set(['horizontal', 'vertical'])

const semanticErrors = computed<string[]>(() => {
  const obj = parsedJson.value
  if (!obj || props.jsonError) return []
  const errors: string[] = []

  if ('direction' in obj) {
    const d = obj.direction
    if (typeof d === 'string' && !VALID_DIRECTIONS.has(d))
      errors.push(`direction "${d}" 无效，应为: horizontal / vertical`)
  }

  function walkTones(node: unknown, path: string) {
    if (!node || typeof node !== 'object') return
    if (Array.isArray(node)) {
      ;(node as unknown[]).forEach((item, i) => walkTones(item, `${path}[${i}]`))
      return
    }
    const n = node as Record<string, unknown>
    for (const [key, val] of Object.entries(n)) {
      if (key === 'tone') {
        if (typeof val === 'string' && !VALID_TONES.has(val))
          errors.push(`${path || 'root'}.tone "${val}" 无效`)
      } else {
        walkTones(val, path ? `${path}.${key}` : key)
      }
    }
  }
  walkTones(obj, '')
  return errors
})

// ── Field chips ──────────────────────────────────────────────────────────────

const TONE_CHIPS = [
  { value: 'blue',    color: '#3b82f6' },
  { value: 'emerald', color: '#10b981' },
  { value: 'violet',  color: '#8b5cf6' },
  { value: 'amber',   color: '#f59e0b' },
  { value: 'rose',    color: '#f43f5e' },
  { value: 'gray',    color: '#6b7280' },
] as const

const showDirectionChips = computed(() => props.editorKind === 'step_flow')

function insertAtCursor(text: string) {
  const el   = props.jsonTextareaRef
  const base = el ? el.value : props.jsonText
  const s    = el?.selectionStart ?? base.length
  const e    = el?.selectionEnd   ?? base.length
  const newText = base.slice(0, s) + text + base.slice(e)
  if (el) el.value = newText
  emit('textarea-input', newText)
  nextTick(() => {
    if (!el) return
    el.focus()
    el.selectionStart = el.selectionEnd = s + text.length
  })
}

const insertTone = (tone: string) => insertAtCursor(`"${tone}"`)
const insertDir  = (dir: string)  => insertAtCursor(`"${dir}"`)

// ── Textarea keyboard handler ────────────────────────────────────────────────

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

      <!-- ── Panel 1: Slide list ──────────────────────────────────── -->
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

      <!-- ── Panel 2: Live preview ────────────────────────────────── -->
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

      <!-- ── Panel 3: JSON editor ─────────────────────────────────── -->
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

          <!-- Kind row -->
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

          <!-- Quick fields -->
          <div class="ej-quick-fields">
            <div class="ej-qf-item ej-qf-item--grow">
              <label class="ej-qf-label">标题</label>
              <input
                type="text"
                class="ej-qf-input"
                :value="quickTitle"
                placeholder="幻灯片标题…"
                @input="handleQuickField('title', ($event.target as HTMLInputElement).value)"
              />
            </div>
            <template v-if="hasSubtitle">
              <div class="ej-qf-sep" />
              <div class="ej-qf-item ej-qf-item--grow">
                <label class="ej-qf-label">副标题</label>
                <input
                  type="text"
                  class="ej-qf-input"
                  :value="quickSubtitle"
                  placeholder="副标题…"
                  @input="handleQuickField('subtitle', ($event.target as HTMLInputElement).value)"
                />
              </div>
            </template>
            <template v-if="hasSection">
              <div class="ej-qf-sep" />
              <div class="ej-qf-item ej-qf-item--section">
                <label class="ej-qf-label">章节号</label>
                <input
                  type="text"
                  class="ej-qf-input"
                  :value="quickSection"
                  placeholder="01"
                  @input="handleQuickField('section', ($event.target as HTMLInputElement).value)"
                />
              </div>
            </template>
          </div>

          <!-- JSON textarea with line numbers -->
          <div class="ej-field-group ej-field-group--grow">
            <div class="ej-label-row">
              <label class="ej-label">Blueprint JSON</label>
              <span class="ej-hint">仅显示此类型相关字段</span>
              <button class="btn btn-ghost ej-fmt-btn" @click="$emit('format-json')">
                <span class="i-carbon:clean" /> 格式化
              </button>
            </div>
            <div class="ej-wrap" :class="{ 'ej-wrap--error': !!jsonError }">
              <!-- Gutter + textarea row -->
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
              <!-- Parse error -->
              <div v-if="jsonError" class="ej-error">
                <span class="i-carbon:warning-filled ej-error-icon" />
                <span class="ej-error-text">
                  <template v-if="errorLine !== null">第 {{ errorLine }} 行：</template>{{ jsonError }}
                </span>
              </div>
              <!-- Semantic warnings -->
              <div v-if="semanticErrors.length > 0 && !jsonError" class="ej-semantic-errors">
                <div v-for="err in semanticErrors" :key="err" class="ej-semantic-item">
                  <span class="i-carbon:warning ej-semantic-icon" />{{ err }}
                </div>
              </div>
            </div>
          </div>

          <!-- Field chips -->
          <div class="ej-chips-section">
            <div class="ej-chips-row">
              <span class="ej-chips-label">tone</span>
              <button
                v-for="chip in TONE_CHIPS"
                :key="chip.value"
                class="ej-tone-chip"
                :title="`插入 &quot;${chip.value}&quot;`"
                @click="insertTone(chip.value)"
              >
                <span class="ej-chip-dot" :style="{ background: chip.color }" />
                {{ chip.value }}
              </button>
            </div>
            <div v-if="showDirectionChips" class="ej-chips-row">
              <span class="ej-chips-label">direction</span>
              <button class="ej-dir-chip" @click="insertDir('horizontal')">
                <span class="i-carbon:arrow-right" /> horizontal
              </button>
              <button class="ej-dir-chip" @click="insertDir('vertical')">
                <span class="i-carbon:arrow-down" /> vertical
              </button>
            </div>
          </div>

          <!-- Media library picker -->
          <div v-if="mediaItems.length > 0" class="ej-field-group ej-media-group">
            <div class="ej-label-row">
              <label class="ej-label">素材库</label>
              <span class="ej-hint">点击插入路径到光标处</span>
            </div>
            <div class="ej-media-strip">
              <div
                v-for="m in mediaItems"
                :key="m.id"
                class="ej-media-item"
                :title="m.name"
                @click="$emit('insert-media-url', m.ref)"
              >
                <img v-if="m.type === 'image'" :src="m.url" class="ej-media-thumb" />
                <div v-else class="ej-media-video-thumb">
                  <span class="i-carbon:video-filled" />
                </div>
                <span class="ej-media-label">{{ m.name }}</span>
              </div>
            </div>
          </div>

          <!-- Save -->
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

  <!-- Kind picker modal -->
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
          <template v-else>选择新类型后将重新过滤 JSON 字段，兼容字段会保留</template>
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
