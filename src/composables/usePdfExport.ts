import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { save } from '@tauri-apps/plugin-dialog'
import type { SlideBlueprint } from '../components/types'
import type { ChartBox } from '../components/chartOptions'
import { renderChartSvg, themeFromCssVars } from '../components/chartOptions'

/** Mirrors `commands::pdf::PdfExportReport` (src-tauri/src/commands/pdf.rs). */
export interface PdfExportReport {
  path: string
  pages: number
  /** Slides whose template has no PDF layout yet; rendered as placeholder pages. */
  placeholder_slides: Array<{ index: number, kind: string }>
  /** Slides clipped because their content did not fit even when shrunk. */
  overflow_slides: number[]
  missing_assets: Array<{ slide: number, reference: string, reason: string }>
  /** The requested theme id when it had no PDF layout and the export fell back to light-notes. */
  theme_fallback: string | null
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
      // chart_focus 页的图表由 webview 用同一套 buildChartOption 以 SSR 模式渲染成 SVG，
      // 经 asset 管道（chart:<index>）嵌入 typst，保证与预览一致。三步：
      // 1. 首页导出不带 slides：先读出项目保存的牌组（SSR 需要 chart 数据）。之后两次调用都
      //    显式传同一份 slides，保证 chart:<index> 与 Rust 侧排版的页序一致。
      // 2. pdf_chart_boxes：typst 按与导出完全相同的参数排版，报告每张图的绘图框和该页 --type-scale。
      // 3. 按框的尺寸与字号比例渲染 SVG，再导出：SVG 1:1 落位，标签与屏幕同字号。
      let slides = request.slides
      if (!slides && request.projectId != null) {
        const project = await invoke<{ blueprints_json: string }>('get_project', { id: request.projectId })
        if (project.blueprints_json.trim()) slides = JSON.parse(project.blueprints_json) as SlideBlueprint[]
      }
      const themeId = document.documentElement.getAttribute('data-theme')
      let chartSvgs: Array<{ index: number, svg: string }> | null = null
      if (slides?.some(slide => slide.kind === 'chart_focus' && slide.chart)) {
        const boxes = await invoke<ChartBox[]>('pdf_chart_boxes', {
          projectId: request.projectId,
          slides,
          theme: themeId,
        })
        const theme = themeFromCssVars(getComputedStyle(document.documentElement))
        chartSvgs = boxes.flatMap((box) => {
          const chart = slides?.[box.slide]?.chart
          return chart ? [{ index: box.slide, svg: renderChartSvg(chart, theme, box) }] : []
        })
      }
      report.value = await invoke<PdfExportReport>('export_pdf', {
        projectId: request.projectId,
        slides: slides ?? null,
        path,
        theme: themeId,
        chartSvgs,
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
