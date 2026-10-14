<script setup lang="ts">
import { computed } from 'vue'
import { useEditorSchemaStore } from '../stores/editorSchema'
import { usePdfExport } from '../composables/usePdfExport'
import { APP_THEMES } from '../themes/catalog'
import type { SlideKind } from './types'

const schemaStore = useEditorSchemaStore()

// Progress + result of the shared PDF export (composables/usePdfExport.ts).
// Mounted once per view; renders nothing while idle.
const { exporting, report, error, note, reveal, dismiss } = usePdfExport()

const visible = computed(() => exporting.value || report.value !== null || error.value !== '')

const kindLabel = (kind: string) => schemaStore.kindMeta[kind as SlideKind]?.label ?? kind
const pageList = (indices: number[]) => indices.map(i => `第 ${i + 1} 页`).join('、')

const placeholderGroups = computed(() => {
  const groups = new Map<string, number[]>()
  for (const { index, kind } of report.value?.placeholder_slides ?? []) {
    groups.set(kind, [...(groups.get(kind) ?? []), index])
  }
  return [...groups].map(([kind, indices]) => ({ label: kindLabel(kind), pages: pageList(indices) }))
})

const hasWarnings = computed(() => {
  const r = report.value
  return !!r && (r.placeholder_slides.length > 0 || r.overflow_slides.length > 0 || r.missing_assets.length > 0 || !!r.theme_fallback)
})

// Every slide theme has a PDF layout now; an unknown id falls back to
// light-notes and the report names it (`theme_fallback`).
const themeHint = computed(() => {
  const fallback = report.value?.theme_fallback
  if (!fallback) return 'PDF 版式跟随当前主题。'
  const name = APP_THEMES.find(t => t.id === fallback)?.name ?? fallback
  return `主题「${name}」还没有 PDF 版式，已回落到「便签·稿纸」。`
})

const fileName = computed(() => report.value?.path.split(/[\\/]/).pop() ?? '')
</script>

<template>
  <Teleport to="body">
    <Transition name="modal">
      <div v-if="visible" class="modal-overlay" @click.self="!exporting && dismiss()">
        <div class="modal-box pdf-modal" role="dialog" aria-modal="true" aria-labelledby="pdf-export-title">
          <template v-if="exporting">
            <div class="modal-icon">
              <span class="i-carbon:renew spin" />
            </div>
            <h3 id="pdf-export-title" class="modal-title">正在生成 PDF…</h3>
            <p class="modal-desc">首次导出需要扫描系统字体，会稍慢一些。</p>
          </template>

          <template v-else-if="error">
            <button class="modal-close" aria-label="关闭" @click="dismiss">
              <span class="i-carbon:close" />
            </button>
            <div class="modal-icon modal-icon--error">
              <span class="i-carbon:warning-alt" />
            </div>
            <h3 id="pdf-export-title" class="modal-title">导出失败</h3>
            <p class="pdf-error">{{ error }}</p>
            <div class="modal-actions">
              <button class="btn btn-primary" @click="dismiss">知道了</button>
            </div>
          </template>

          <template v-else-if="report">
            <button class="modal-close" aria-label="关闭" @click="dismiss">
              <span class="i-carbon:close" />
            </button>
            <div class="modal-icon" :class="{ 'modal-icon--warn': hasWarnings }">
              <span :class="hasWarnings ? 'i-carbon:warning-alt' : 'i-carbon:checkmark'" />
            </div>
            <h3 id="pdf-export-title" class="modal-title">
              {{ hasWarnings ? 'PDF 已导出，有几处需要留意' : 'PDF 已导出' }}
            </h3>
            <p class="modal-desc">
              {{ fileName }} · {{ report.pages }} 页 · {{ (report.elapsed_ms / 1000).toFixed(1) }} 秒
            </p>

            <ul v-if="hasWarnings || note" class="pdf-notes">
              <li v-if="note">
                <span class="i-carbon:information" />
                <span>{{ note }}</span>
              </li>
              <li v-if="placeholderGroups.length">
                <span class="i-carbon:document-blank" />
                <span>
                  以下模板还没有 PDF 版式，导出为占位页：
                  <template v-for="(group, i) in placeholderGroups" :key="group.label">
                    {{ i > 0 ? '；' : '' }}{{ group.label }}（{{ group.pages }}）
                  </template>
                </span>
              </li>
              <li v-if="report.overflow_slides.length">
                <span class="i-carbon:fit-to-screen" />
                <span>{{ pageList(report.overflow_slides) }} 内容过多，已缩到最小字号仍放不下，超出部分被裁掉。</span>
              </li>
              <li v-for="asset in report.missing_assets" :key="`${asset.slide}-${asset.reference}`">
                <span class="i-carbon:image" />
                <span>第 {{ asset.slide + 1 }} 页图片 <code>{{ asset.reference }}</code> 未导出：{{ asset.reason }}</span>
              </li>
              <li v-if="report.theme_fallback">
                <span class="i-carbon:color-palette" />
                <span>{{ themeHint }}</span>
              </li>
            </ul>
            <p v-if="!report.theme_fallback" class="pdf-theme-hint">{{ themeHint }}</p>

            <div class="modal-actions">
              <button class="btn btn-ghost" @click="reveal">
                <span class="i-carbon:folder" /> 在文件夹中显示
              </button>
              <button class="btn btn-primary" @click="dismiss">完成</button>
            </div>
          </template>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.modal-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.5);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.modal-box {
  background: var(--studio-panel);
  border: 1px solid var(--studio-border);
  border-radius: 14px;
  padding: 24px;
  width: 520px;
  max-width: 92vw;
  position: relative;
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.35);
  text-align: center;
}

.modal-close {
  position: absolute;
  top: 12px;
  right: 12px;
  background: none;
  border: none;
  cursor: pointer;
  padding: 4px;
  color: var(--studio-muted);
  border-radius: 4px;
}

.modal-close:hover {
  background: var(--studio-border);
  color: var(--studio-text);
}

.modal-icon {
  width: 52px;
  height: 52px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  margin: 0 auto 16px;
  font-size: 24px;
  color: var(--studio-success);
  background: var(--studio-success-bg);
}

.modal-icon--warn {
  color: var(--warning);
  background: var(--warning-bg);
}

.modal-icon--error {
  color: var(--studio-error);
  background: var(--studio-error-bg);
}

.modal-title {
  font-size: 18px;
  font-weight: 700;
  margin: 0 0 8px;
  color: var(--studio-text);
}

.modal-desc {
  font-size: 14px;
  color: var(--studio-muted);
  margin: 0 0 16px;
  overflow-wrap: anywhere;
}

.pdf-notes {
  list-style: none;
  margin: 0;
  padding: 12px 14px;
  border: 1px solid var(--studio-border);
  border-radius: 10px;
  background: var(--studio-surface);
  text-align: left;
  display: flex;
  flex-direction: column;
  gap: 8px;
  max-height: 220px;
  overflow-y: auto;
}

.pdf-notes li {
  display: flex;
  gap: 8px;
  font-size: 13px;
  line-height: 1.5;
  color: var(--studio-text);
}

.pdf-notes li > span:first-child {
  flex-shrink: 0;
  margin-top: 3px;
  color: var(--studio-muted);
}

.pdf-notes code {
  font-family: var(--font-code);
  font-size: 12px;
  padding: 0 4px;
  border-radius: 4px;
  background: var(--studio-primary-bg);
}

.pdf-theme-hint {
  margin: 12px 0 0;
  font-size: 12px;
  color: var(--studio-muted);
}

.pdf-error {
  margin: 0;
  padding: 12px;
  border-radius: 10px;
  background: var(--studio-error-bg);
  color: var(--studio-error);
  font-size: 13px;
  text-align: left;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  max-height: 220px;
  overflow-y: auto;
}

.modal-actions {
  display: flex;
  justify-content: center;
  gap: 12px;
  margin-top: 22px;
}

.modal-actions .btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

.spin {
  animation: pdf-spin 1s linear infinite;
}

@keyframes pdf-spin {
  to { transform: rotate(360deg); }
}

.modal-enter-active,
.modal-leave-active {
  transition: opacity 0.2s ease;
}

.modal-enter-from,
.modal-leave-to {
  opacity: 0;
}

@media (prefers-reduced-motion: reduce) {
  .spin { animation: none; }
}
</style>
