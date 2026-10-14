/* ============================================================
 * 图表数据的类型与校验（ChartSpec）
 * 支持类型：line / bar / donut / grouped_bar / stacked_area
 *
 * 图表本身由 Typst 原生绘制（crates/slide-typst/typst/lib/chart.typ），
 * 这里只剩两件跟屏幕渲染无关的事：
 * - `ChartSpec` 等类型（Step3 编辑器的字段定义、调试面板都用它）
 * - `chartProblems()`：编辑器里手改图表 JSON 时的语义校验
 * ============================================================ */

export type ChartType = 'line' | 'bar' | 'donut' | 'grouped_bar' | 'stacked_area'

export interface ChartEncoding {
  x?: string
  y?: string
  /** donut：分类列名 / 数值列名 */
  label?: string
  value?: string
}

export interface ChartSeries {
  /** data 行里的数值列名 */
  key: string
  /** 图例显示名 */
  name: string
}

export interface ChartEmphasis {
  highlight?: string[]
  trend?: 'up' | 'down'
}

export type ChartSource = 'user_input' | 'llm_generated' | 'tool' | string

export interface ChartSpec {
  type: ChartType
  title: string
  /** 行数据数组；每行是一个 { 列名: 值 } 对象 */
  data: Array<Record<string, string | number>>
  encoding: ChartEncoding
  /** 多系列图表（grouped_bar / stacked_area）的系列定义，按调色板顺序取色 */
  series?: ChartSeries[]
  emphasis?: ChartEmphasis
  /** user_input | llm_generated | tool —— 决定来源角标文案 */
  source?: ChartSource
  source_note?: string
}

const CHART_TYPES: ChartType[] = ['line', 'bar', 'donut', 'grouped_bar', 'stacked_area']

/**
 * 图表数据的结构问题，与 Rust `ChartSpec::problems()`（src-tauri/src/types.rs）逐条对应，改一边必须改另一边：
 * 生成时解析 Markdown 用 Rust 版报错，编辑器里手改 JSON 用这一版提示。空数组表示可渲染。
 */
export function chartProblems(value: unknown): string[] {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return ['chart 必须是对象']
  const spec = value as Partial<ChartSpec>
  if (!CHART_TYPES.includes(spec.type as ChartType)) {
    return [`type "${String(spec.type)}" 无效，应为 ${CHART_TYPES.join(' / ')}`]
  }
  const out: string[] = []
  if (typeof spec.title !== 'string' || !spec.title.trim()) out.push('title 不能为空')
  if (!Array.isArray(spec.data) || spec.data.length === 0) out.push('data 不能为空')
  const enc = (spec.encoding ?? {}) as ChartEncoding
  const present = (key?: string) => typeof key === 'string' && key.trim() !== ''
  const numeric: Array<[string, string]> = []
  if (spec.type === 'line' || spec.type === 'bar') {
    if (!present(enc.x) || !present(enc.y)) out.push('折线图/柱状图需要 encoding.x 与 encoding.y')
    else numeric.push([enc.y!, 'encoding.y'])
  } else if (spec.type === 'donut') {
    if (!present(enc.label) || !present(enc.value)) out.push('环形图需要 encoding.label 与 encoding.value')
    else numeric.push([enc.value!, 'encoding.value'])
  } else {
    if (!present(enc.x)) out.push('分组柱状图/堆叠面积图需要 encoding.x')
    const series = Array.isArray(spec.series) ? spec.series : []
    if (series.length === 0) out.push('分组柱状图/堆叠面积图至少需要一个 series')
    for (const s of series) {
      if (!present(s?.key)) out.push('series 条目的 key 不能为空')
      else numeric.push([s.key, 'series'])
    }
  }
  const isNumeric = (v: unknown) =>
    (typeof v === 'number' && Number.isFinite(v)) || (typeof v === 'string' && v.trim() !== '' && Number.isFinite(Number(v)))
  ;(Array.isArray(spec.data) ? spec.data : []).forEach((row, i) => {
    if (!row || typeof row !== 'object' || Array.isArray(row)) {
      out.push(`data 第 ${i + 1} 行不是对象`)
      return
    }
    for (const [key, what] of numeric) {
      if (!isNumeric((row as Record<string, unknown>)[key])) out.push(`data 第 ${i + 1} 行的 ${what}「${key}」不是数字`)
    }
  })
  return out
}
