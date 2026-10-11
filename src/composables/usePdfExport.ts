import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { save } from '@tauri-apps/plugin-dialog'
import type { SlideBlueprint } from '../components/types'

/** Mirrors `commands::pdf::PdfExportReport` (src-tauri/src/commands/pdf.rs). */
export interface PdfExportReport {
  path: string
  pages: number
  /** Slides whose template has no PDF layout yet; rendered as placeholder pages. */
  placeholder_slides: Array<{ index: number, kind: string }>
  /** Slides clipped because their content did not fit even when shrunk. */
  overflow_slides: number[]
  missing_assets: Array<{ slide: number, reference: string, reason: string }>
  elapsed_ms: number
}

export interface PdfExportRequest {
  projectId: number | null
  /** Used for the suggested file name and the PDF title. */
  name: string
  /** The deck to export; omit to export the project's saved blueprints. */
  slides?: SlideBlueprint[]
  /** Shown in the result dialog, e.g. when the editor has unapplied edits. */
  note?: string
}

function safeFileName(name: string) {
  return (name.trim() || '未命名演示').replace(/[\\/:*?"<>|]+/g, '_')
}

// One export at a time across the app; the result dialog reads this state.
const exporting = ref(false)
const report = ref<PdfExportReport | null>(null)
const error = ref('')
const note = ref('')

export function usePdfExport() {
  async function exportPdf(request: PdfExportRequest) {
    if (exporting.value) return
    const path = await save({
      defaultPath: `${safeFileName(request.name)}.pdf`,
      filters: [{ name: 'PDF', extensions: ['pdf'] }],
    })
    if (!path) return

    exporting.value = true
    report.value = null
    error.value = ''
    note.value = request.note ?? ''
    try {
      report.value = await invoke<PdfExportReport>('export_pdf', {
        projectId: request.projectId,
        slides: request.slides ?? null,
        path,
        theme: document.documentElement.getAttribute('data-theme'),
      })
    } catch (e) {
      error.value = typeof e === 'string' ? e : e instanceof Error ? e.message : String(e)
    } finally {
      exporting.value = false
    }
  }

  async function reveal() {
    if (!report.value) return
    try {
      await invoke('reveal_exported_file', { path: report.value.path })
    } catch (e) {
      error.value = String(e)
    }
  }

  function dismiss() {
    report.value = null
    error.value = ''
    note.value = ''
  }

  return { exporting, report, error, note, exportPdf, reveal, dismiss }
}
