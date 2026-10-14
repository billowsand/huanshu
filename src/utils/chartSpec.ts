/* ============================================================
 * 图表数据的类型（ChartSpec）
 * 支持类型：line / bar / donut / grouped_bar / stacked_area
 *
 * 图表本身由 Typst 原生绘制（crates/slide-typst/typst/lib/chart.typ）。
 * 校验规则在 Rust 侧（`huanshu_core::chart_check`），生成流水线与编辑器共用同一份：
 * 编辑器通过 `chart_problems` 命令取中文报错。
 * 这里只留类型定义，供 Step3 的字段定义与调试面板使用。
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
