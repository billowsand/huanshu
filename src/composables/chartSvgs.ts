import { invoke } from '@tauri-apps/api/core'
import type { SlideBlueprint } from '../components/types'
import type { ChartBox } from '../components/chartOptions'
import { renderChartSvg, themeFromCssVars } from '../components/chartOptions'

/**
 * chart_focus 页的图表由 webview 用同一套 buildChartOption 以 SSR 模式渲染成 SVG，经 asset 管道
 * （chart:<index>）嵌入 typst，保证与预览一致。两步：
 * 1. pdf_chart_boxes：typst 按与导出完全相同的参数排版，报告每张图的绘图框和该页 --type-scale。
 * 2. 按框的尺寸与字号比例渲染 SVG，再交给导出：SVG 1:1 落位，标签与屏幕同字号。
 *
 * `chart:<index>` 的下标是 **`slides` 数组里的位置**，所以调用方传入的必须就是接下来交给 Rust 排版的
 * 那一份牌组（缩略图只发子集时，也要用同一个子集调用这里）。没有图表页时返回 null（命令不必调用）。
 */
export async function buildChartSvgs(
  slides: SlideBlueprint[],
  opts: { projectId: number | null, themeId: string | null },
): Promise<Array<{ index: number, svg: string }> | null> {
  if (!slides.some(slide => slide.kind === 'chart_focus' && slide.chart)) return null
  const boxes = await invoke<ChartBox[]>('pdf_chart_boxes', {
    projectId: opts.projectId,
    slides,
    theme: opts.themeId,
  })
  const theme = themeFromCssVars(getComputedStyle(document.documentElement))
  return boxes.flatMap((box) => {
    const chart = slides[box.slide]?.chart
    return chart ? [{ index: box.slide, svg: renderChartSvg(chart, theme, box) }] : []
  })
}
