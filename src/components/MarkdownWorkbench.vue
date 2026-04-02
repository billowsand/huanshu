<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import MarkdownIt from 'markdown-it'
import { EditorState } from '@codemirror/state'
import {
  EditorView,
  drawSelection,
  dropCursor,
  highlightActiveLine,
  highlightActiveLineGutter,
  highlightSpecialChars,
  keymap,
  lineNumbers,
  placeholder,
  rectangularSelection,
} from '@codemirror/view'
import { defaultHighlightStyle, syntaxHighlighting } from '@codemirror/language'
import { markdown } from '@codemirror/lang-markdown'
import { defaultKeymap, history, historyKeymap, indentWithTab } from '@codemirror/commands'

const props = defineProps<{
  modelValue: string
  placeholder?: string
}>()

const emit = defineEmits<{
  'update:modelValue': [value: string]
}>()

const editorHost = ref<HTMLElement | null>(null)
const previewScroll = ref<HTMLElement | null>(null)
const mobileMode = ref<'edit' | 'preview'>('edit')
let view: EditorView | null = null
let syncingFromEditor = false
let syncingFromPreview = false
let syncReleaseTimer: ReturnType<typeof setTimeout> | null = null

const md = new MarkdownIt({
  html: false,
  breaks: true,
  linkify: true,
  typographer: true,
})

const renderedPreview = computed(() => {
  const raw = props.modelValue?.trim()
  if (!raw)
    return '<div class="md-preview-empty">预览区将实时显示 Markdown 渲染效果</div>'
  return md.render(raw)
})

const editorTheme = EditorView.theme({
  '&': {
    height: '100%',
    backgroundColor: 'transparent',
    color: 'var(--studio-text)',
    fontSize: '13px',
  },
  '.cm-scroller': {
    overflow: 'auto',
    fontFamily: 'var(--font-mono)',
    lineHeight: '1.8',
  },
  '.cm-content': {
    padding: '14px 18px 24px',
    minHeight: '100%',
    caretColor: 'var(--studio-primary)',
  },
  '.cm-gutters': {
    backgroundColor: 'rgba(255,255,255,0.02)',
    color: 'var(--studio-muted)',
    borderRight: '1px solid var(--studio-border)',
  },
  '.cm-lineNumbers .cm-gutterElement': {
    paddingInline: '8px 10px',
  },
  '.cm-activeLineGutter': {
    backgroundColor: 'rgba(255,255,255,0.04)',
  },
  '.cm-activeLine': {
    backgroundColor: 'rgba(255,255,255,0.03)',
  },
  '.cm-selectionBackground, &.cm-focused .cm-selectionBackground, .cm-content ::selection': {
    backgroundColor: 'rgba(120, 167, 255, 0.28)',
  },
  '.cm-cursor, .cm-dropCursor': {
    borderLeftColor: 'var(--studio-primary)',
  },
  '.tok-heading': {
    color: '#f5d76e',
    fontWeight: '700',
  },
  '.tok-strong': {
    color: '#ffd78a',
    fontWeight: '700',
  },
  '.tok-emphasis': {
    color: '#8fd6ff',
    fontStyle: 'italic',
  },
  '.tok-link': {
    color: '#87c3ff',
    textDecoration: 'underline',
  },
  '.tok-monospace': {
    color: '#ffb37a',
  },
  '.tok-meta': {
    color: '#7f8ca6',
  },
  '.tok-list': {
    color: '#7fe3c4',
  },
})

function buildState(content: string) {
  const extensions = [
    lineNumbers(),
    highlightActiveLineGutter(),
    highlightSpecialChars(),
    history(),
    drawSelection(),
    dropCursor(),
    EditorState.allowMultipleSelections.of(true),
    rectangularSelection(),
    syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
    markdown(),
    EditorView.lineWrapping,
    highlightActiveLine(),
    keymap.of([
      indentWithTab,
      ...defaultKeymap,
      ...historyKeymap,
    ]),
    editorTheme,
    EditorView.updateListener.of((update) => {
      if (!update.docChanged)
        return
      const next = update.state.doc.toString()
      if (next !== props.modelValue)
        emit('update:modelValue', next)
    }),
  ]

  if (props.placeholder)
    extensions.push(placeholder(props.placeholder))

  return EditorState.create({
    doc: content,
    extensions,
  })
}

function getScrollRatio(el: HTMLElement) {
  const maxScroll = el.scrollHeight - el.clientHeight
  if (maxScroll <= 0) return 0
  return el.scrollTop / maxScroll
}

function applyScrollRatio(el: HTMLElement, ratio: number) {
  const maxScroll = el.scrollHeight - el.clientHeight
  if (maxScroll <= 0) {
    el.scrollTop = 0
    return
  }
  el.scrollTop = Math.max(0, Math.min(maxScroll, ratio * maxScroll))
}

function releaseSyncLock(source: 'editor' | 'preview') {
  if (syncReleaseTimer) clearTimeout(syncReleaseTimer)
  syncReleaseTimer = setTimeout(() => {
    if (source === 'editor') syncingFromEditor = false
    else syncingFromPreview = false
  }, 80)
}

function syncPreviewToEditor() {
  if (!view || !previewScroll.value || syncingFromEditor) return
  syncingFromPreview = true
  applyScrollRatio(view.scrollDOM, getScrollRatio(previewScroll.value))
  releaseSyncLock('preview')
}

function syncEditorToPreview() {
  if (!view || !previewScroll.value || syncingFromPreview) return
  syncingFromEditor = true
  applyScrollRatio(previewScroll.value, getScrollRatio(view.scrollDOM))
  releaseSyncLock('editor')
}

function handleEditorScroll() {
  syncEditorToPreview()
}

function handlePreviewScroll() {
  syncPreviewToEditor()
}

onMounted(() => {
  if (!editorHost.value)
    return
  view = new EditorView({
    state: buildState(props.modelValue),
    parent: editorHost.value,
  })
  view.scrollDOM.addEventListener('scroll', handleEditorScroll, { passive: true })
  previewScroll.value?.addEventListener('scroll', handlePreviewScroll, { passive: true })
})

watch(
  () => props.modelValue,
  async (next) => {
    if (!view)
      return
    const current = view.state.doc.toString()
    if (next === current)
      return
    view.dispatch({
      changes: { from: 0, to: current.length, insert: next },
    })
    await nextTick()
    syncEditorToPreview()
  },
)

onBeforeUnmount(() => {
  if (syncReleaseTimer) clearTimeout(syncReleaseTimer)
  if (view) {
    view.scrollDOM.removeEventListener('scroll', handleEditorScroll)
  }
  previewScroll.value?.removeEventListener('scroll', handlePreviewScroll)
  view?.destroy()
  view = null
})
</script>

<template>
  <div class="md-workbench" :data-mobile-mode="mobileMode">
    <!-- Mobile tab switch (hidden on desktop) -->
    <div class="md-switch">
      <button
        class="md-tab"
        :class="{ active: mobileMode === 'edit' }"
        @click="mobileMode = 'edit'"
      >
        编辑
      </button>
      <button
        class="md-tab"
        :class="{ active: mobileMode === 'preview' }"
        @click="mobileMode = 'preview'"
      >
        预览
      </button>
    </div>

    <!-- Split panes -->
    <div class="md-panes">
      <section class="md-pane md-pane--editor">
        <div class="md-pane-label">
          <span class="i-carbon:edit" style="font-size:0.7rem;opacity:0.6" />
          编辑
        </div>
        <div ref="editorHost" class="md-editor-host" />
      </section>

      <section class="md-pane md-pane--preview">
        <div class="md-pane-label">
          <span class="i-carbon:view" style="font-size:0.7rem;opacity:0.6" />
          预览
        </div>
        <div ref="previewScroll" class="md-preview-scroll">
          <div class="md-preview" v-html="renderedPreview" />
        </div>
      </section>
    </div>
  </div>
</template>

<style scoped>
/* ── Root ── */
.md-workbench {
  display: flex;
  flex-direction: column;
  overflow: hidden;
  /* No height:100% — parent provides height via flex or grid */
}

/* ── Mobile switch ── */
.md-switch {
  display: none;
  flex-shrink: 0;
  gap: 0.375rem;
  padding: 0.5rem 0.75rem;
  border-bottom: 1px solid var(--studio-border);
  background: var(--studio-panel);
}

.md-tab {
  border: 1px solid var(--studio-border);
  background: transparent;
  color: var(--studio-muted);
  border-radius: 999px;
  padding: 0.3rem 0.75rem;
  font-size: 0.75rem;
  cursor: pointer;
  transition: all 0.15s;
}

.md-tab.active {
  color: white;
  background: var(--studio-accent);
  border-color: var(--studio-accent);
}

/* ── Split panes ── */
.md-panes {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: minmax(0, 1.15fr) minmax(260px, 0.85fr);
  overflow: hidden; /* critical: prevent grid from expanding beyond flex constraint */
}

.md-pane {
  display: flex;
  flex-direction: column;
  overflow: hidden;
  min-height: 0;
}

.md-pane--editor {
  border-right: 1px solid var(--studio-border);
}

/* ── Pane label bar ── */
.md-pane-label {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: 0.375rem;
  padding: 0.35rem 0.875rem;
  font-size: 0.65rem;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.08em;
  color: var(--studio-muted);
  border-bottom: 1px solid var(--studio-border);
  background: rgba(255,255,255,0.015);
}

/* ── Editor host ── */
.md-editor-host {
  flex: 1;
  min-height: 0;
  overflow: hidden;
  /* CodeMirror EditorView root gets height:100% via editorTheme,
     resolves against this element's flex-computed height */
}

/* ── Preview pane ── */
.md-preview-scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}

.md-preview {
  padding: 16px 18px 24px;
  line-height: 1.8;
  color: var(--studio-text);
  font-size: 0.875rem;
}

:deep(.md-preview-empty) {
  color: var(--studio-muted);
  font-size: 0.8rem;
}

:deep(.md-preview h1),
:deep(.md-preview h2),
:deep(.md-preview h3),
:deep(.md-preview h4) {
  margin: 0 0 0.75rem;
  line-height: 1.35;
  font-weight: 700;
}

:deep(.md-preview h1) {
  font-size: 1.35rem;
  padding-bottom: 0.5rem;
  border-bottom: 1px solid var(--studio-border);
}

:deep(.md-preview h2) {
  margin-top: 1.4rem;
  font-size: 1.1rem;
  color: #f5d76e;
}

:deep(.md-preview h3) {
  margin-top: 1rem;
  font-size: 0.975rem;
  color: #8fd6ff;
}

:deep(.md-preview h4) {
  margin-top: 0.8rem;
  font-size: 0.9rem;
  color: #b9c5dd;
}

:deep(.md-preview p),
:deep(.md-preview ul),
:deep(.md-preview ol),
:deep(.md-preview blockquote),
:deep(.md-preview pre) {
  margin: 0 0 0.8rem;
}

:deep(.md-preview ul),
:deep(.md-preview ol) {
  padding-left: 1.2rem;
}

:deep(.md-preview li + li) {
  margin-top: 0.25rem;
}

:deep(.md-preview code) {
  font-family: var(--font-mono);
  font-size: 0.82em;
  background: rgba(255,255,255,0.06);
  border: 1px solid rgba(255,255,255,0.08);
  border-radius: 5px;
  padding: 0.1rem 0.35rem;
}

:deep(.md-preview pre) {
  overflow: auto;
  padding: 0.8rem 1rem;
  background: rgba(0,0,0,0.2);
  border: 1px solid rgba(255,255,255,0.08);
  border-radius: 8px;
}

:deep(.md-preview pre code) {
  padding: 0;
  border: none;
  background: transparent;
}

:deep(.md-preview blockquote) {
  padding: 0.65rem 0.875rem;
  border-left: 3px solid var(--studio-primary);
  background: rgba(255,255,255,0.03);
  color: var(--studio-muted);
}

:deep(.md-preview hr) {
  border: none;
  border-top: 1px solid var(--studio-border);
  margin: 1rem 0;
}

/* ── Mobile layout ── */
@media (max-width: 980px) {
  .md-switch {
    display: flex;
  }

  .md-panes {
    grid-template-columns: minmax(0, 1fr);
  }

  .md-workbench[data-mobile-mode='edit'] .md-pane--preview,
  .md-workbench[data-mobile-mode='preview'] .md-pane--editor {
    display: none;
  }
}
</style>
