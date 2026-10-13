/* ============================================================
 * ChartSpec → ECharts option 的唯一实现（双端共享）
 * 支持类型：line / bar / donut / grouped_bar / stacked_area
 * - webview 预览：主题 token 来自 CSS 变量（themeFromCssVars）
 * - PDF 导出：同一 buildChartOption 以 static 模式 SSR 出 SVG，
 *   经 asset 管道交给 typst image() 嵌入，保证与预览一致
 * mode: 'screen'（带动画）| 'static'（导出用，动画关闭、无交互）
 * ============================================================ */
import * as echarts from 'echarts/core'
import { BarChart, LineChart, PieChart } from 'echarts/charts'
import { GridComponent, LegendComponent, MarkPointComponent, TooltipComponent } from 'echarts/components'
import { SVGRenderer } from 'echarts/renderers'

// 按需注册：整包 'echarts' 约 1MB，这里只用到三种系列 + 坐标/图例/提示/标注 + SVG 渲染器。
// 新增图表类型或组件时在这里补注册，否则 setOption 会静默不画。
echarts.use([BarChart, LineChart, PieChart, GridComponent, LegendComponent, MarkPointComponent, TooltipComponent, SVGRenderer])

/** 屏幕端与 SSR 共用同一个已注册组件的 echarts 实例 */
export { echarts }

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

/** 图表主题 token（键名与 Rust 侧 chart-tokens 对齐） */
export interface ChartTheme {
  bg: string
  text_primary: string
  text_secondary: string
  text_tertiary: string
  color_primary: string
  color_success: string
  color_danger: string
  chart_grid: string
  chart_axis: string
  chart_palette: string[]
  /** 图表文字字体：幻灯片「正文」字体角色（--font-body），屏幕与 PDF 里的 SVG 文字一致 */
  font_family: string
}

/** 幻灯片字号基准（`.slide-shell` 的默认 --type-scale）。 */
export const TYPE_SCALE_BASE = 1.25

/**
 * @param typeScale 该页的 --type-scale：图表文字和其他文字一起随内容量缩放。
 *   屏幕从图表容器上读 CSS 变量，PDF 用 typst 测量时报告的同一个值（`pdf_chart_boxes`）。
 */
export function buildChartOption(spec: ChartSpec, theme: ChartTheme, mode: 'screen' | 'static', typeScale = TYPE_SCALE_BASE) {
  const staticMode = mode === 'static'
  const FONT_FAMILY = theme.font_family
  // 字号写成「scale=1 时的 px」，乘以 --type-scale：基准 1.25 下轴标签 12 → 15px（--type-caption 的 0.8）
  const fs = (px: number) => Math.round(px * typeScale * 10) / 10
  const common = {
    backgroundColor: 'transparent',
    animation: !staticMode,
    textStyle: { fontFamily: FONT_FAMILY },
  }
  const axisBase = {
    axisLine: { lineStyle: { color: theme.chart_axis } },
    axisTick: { show: false },
    axisLabel: { color: theme.text_secondary, fontSize: fs(12), fontFamily: FONT_FAMILY },
    splitLine: { lineStyle: { color: theme.chart_grid } },
  }
  const legendBase = {
    bottom: 0,
    icon: 'roundRect',
    itemWidth: fs(12),
    itemHeight: fs(8),
    itemGap: fs(18),
    textStyle: { color: theme.text_secondary, fontSize: fs(12), fontFamily: FONT_FAMILY },
  }
  // 主题 token 应为 #rgb / #rrggbb；其他写法（rgb()、var()…）原样返回，宁可不透明也不要 NaN。
  const hexA = (color: string, a: number) => {
    let h = color.trim().replace('#', '')
    if (!/^([0-9a-f]{3}|[0-9a-f]{6})$/i.test(h)) return color
    if (h.length === 3) h = h.split('').map((c) => c + c).join('')
    const n = (i: number) => parseInt(h.slice(i, i + 2), 16)
    return `rgba(${n(0)},${n(2)},${n(4)},${a})`
  }
  const palette = (i: number) => theme.chart_palette[i % theme.chart_palette.length]

  if (spec.type === 'line') {
    const xKey = spec.encoding.x ?? ''
    const yKey = spec.encoding.y ?? ''
    const xs = spec.data.map((d) => d[xKey])
    const ys = spec.data.map((d) => Number(d[yKey]))
    const highlighted = new Set(
      (spec.emphasis?.highlight ?? []).map((h) => xs.findIndex((x) => String(x) === h)).filter((i) => i >= 0),
    )
    return {
      ...common,
      grid: { left: 48, right: 40, top: 40, bottom: 40 },
      tooltip: { trigger: 'axis' },
      xAxis: { ...axisBase, type: 'category', data: xs, boundaryGap: false, splitLine: { show: false } },
      yAxis: { ...axisBase, type: 'value', axisLine: { show: false } },
      series: [{
        // 高亮点的数值已经写在 markPoint 气泡里，数据标签再写一遍会叠在气泡下面
        type: 'line', smooth: true, symbol: 'circle', symbolSize: 8,
        data: ys.map((value, i) => (highlighted.has(i) ? { value, label: { show: false } } : value)),
        lineStyle: { width: 3, color: theme.color_primary },
        itemStyle: { color: theme.color_primary },
        areaStyle: {
          color: {
            type: 'linear', x: 0, y: 0, x2: 0, y2: 1,
            colorStops: [
              { offset: 0, color: hexA(theme.color_primary, 0.2) },
              { offset: 1, color: hexA(theme.color_primary, 0.02) },
            ],
          },
        },
        label: { show: true, color: theme.text_secondary, fontSize: fs(12), fontFamily: FONT_FAMILY, formatter: '{c}' },
        markPoint: spec.emphasis?.highlight?.length
          ? {
              symbol: 'pin', symbolSize: fs(46),
              itemStyle: { color: theme.color_success },
              label: { color: '#fff', fontSize: fs(11), formatter: '{c}', fontFamily: FONT_FAMILY },
              // 高亮点只标自己的数值：highlight 可能是低点、拐点，不能一律写「新高」；找不到的类目跳过
              data: [...highlighted].map((i) => ({ name: String(xs[i]), coord: [xs[i], ys[i]], value: ys[i] })),
            }
          : undefined,
      }],
    }
  }

  if (spec.type === 'bar') {
    const xKey = spec.encoding.x ?? ''
    const yKey = spec.encoding.y ?? ''
    const names = spec.data.map((d) => d[xKey])
    const vals = spec.data.map((d) => Number(d[yKey]))
    const hl = spec.emphasis?.highlight ?? []
    return {
      ...common,
      grid: { left: 48, right: 24, top: 36, bottom: 40 },
      tooltip: { trigger: 'axis', axisPointer: { type: 'shadow' } },
      xAxis: { ...axisBase, type: 'category', data: names, splitLine: { show: false } },
      yAxis: { ...axisBase, type: 'value', axisLine: { show: false } },
      series: [{
        type: 'bar', data: vals, barWidth: '46%',
        itemStyle: {
          borderRadius: [6, 6, 0, 0],
          // 没有 highlight 时全部实色；有 highlight 时只突出这些类目
          color: (p: { dataIndex: number }) =>
            !hl.length || hl.includes(String(names[p.dataIndex])) ? theme.color_primary : hexA(theme.color_primary, 0.35),
        },
        label: { show: true, position: 'top', color: theme.text_secondary, fontSize: fs(12), fontFamily: FONT_FAMILY },
      }],
    }
  }

  // 多系列通用：grouped_bar（并排柱）与 stacked_area（堆叠面积）
  if (spec.type === 'grouped_bar' || spec.type === 'stacked_area') {
    const xKey = spec.encoding.x ?? ''
    const xs = spec.data.map((d) => d[xKey])
    const series = (spec.series ?? []).map((s, i) => {
      if (spec.type === 'grouped_bar') {
        // 不写死 barWidth：固定 30% 时三个系列加间距超过类目宽度，相邻季度的柱子会贴在一起；
        // 交给 ECharts 按 barCategoryGap 均分，系列数多少都放得下。
        return {
          name: s.name, type: 'bar', barGap: '12%', barCategoryGap: '32%',
          data: spec.data.map((d) => Number(d[s.key])),
          itemStyle: { color: palette(i), borderRadius: [4, 4, 0, 0] },
        }
      }
      return {
        name: s.name, type: 'line', smooth: true, symbol: 'none', stack: 'total',
        lineStyle: { width: 2, color: palette(i) },
        areaStyle: { color: hexA(palette(i), 0.45) },
        emphasis: { focus: 'series' },
        data: spec.data.map((d) => Number(d[s.key])),
      }
    })
    return {
      ...common,
      color: (spec.series ?? []).map((_, i) => palette(i)),
      // 底部留给 x 轴标签 + 图例两行文字，随字号缩放
      grid: { left: 48, right: 24, top: 36, bottom: fs(40) },
      tooltip: { trigger: 'axis', axisPointer: { type: 'shadow' } },
      legend: legendBase,
      xAxis: { ...axisBase, type: 'category', data: xs, splitLine: { show: false } },
      yAxis: { ...axisBase, type: 'value', axisLine: { show: false } },
      series,
    }
  }

  if (spec.type === 'donut') {
    const labelKey = spec.encoding.label ?? ''
    const valueKey = spec.encoding.value ?? ''
    const total = spec.data.reduce((s, d) => s + Number(d[valueKey]), 0)
    const hlName = spec.emphasis?.highlight?.[0]
    return {
      ...common,
      tooltip: { formatter: '{b}: {c}（{d}%）' },
      legend: { ...legendBase, bottom: 0 },
      series: [{
        type: 'pie', radius: ['48%', '66%'], center: ['50%', '46%'],
        itemStyle: { borderColor: theme.bg, borderWidth: 2, borderRadius: 6 },
        label: { show: true, color: theme.text_secondary, fontSize: fs(12), fontFamily: FONT_FAMILY, formatter: '{b}\n{d}%', lineHeight: fs(16), width: fs(130), overflow: 'break' },
        labelLine: { length: 14, length2: 10, lineStyle: { color: theme.chart_axis } },
        emphasis: { scale: true, scaleSize: 6 },
        data: spec.data.map((d, i) => ({
          name: d[labelKey], value: Number(d[valueKey]),
          itemStyle: { color: d[labelKey] === hlName ? theme.color_primary : palette(i) },
        })),
      }, {
        type: 'pie', radius: ['0%', '38%'], center: ['50%', '46%'], silent: true,
        label: {
          show: true, position: 'center',
          // 数值不一定是百分比（可能是件数、金额）：中心只显示合计，占比交给扇区标签的 {d}%
          formatter: `{a|${Number(total.toFixed(2))}}\n{b|合计}`,
          rich: {
            a: { fontSize: fs(26), fontWeight: 700, color: theme.text_primary, fontFamily: FONT_FAMILY },
            b: { fontSize: fs(12), color: theme.text_tertiary, padding: [6, 0, 0, 0], fontFamily: FONT_FAMILY },
          },
        },
        data: [{ value: 1, itemStyle: { color: 'transparent' } }],
      }],
    }
  }

  throw new Error(`unsupported chart type: ${spec.type}`)
}

/**
 * 从 CSS 变量收集图表主题（webview 侧入口）。
 * 读取 slide-shell 上生效的主题 token，各套主题（dark-yellow / dark-blue /
 * dark-terminal / light-terminal / light-notes / light-editorial）都映射到统一的 --bg/--text/... 名，
 * 图表专有 token（--chart-*）在缺少时退回主色派生值。
 */
export function themeFromCssVars(style: CSSStyleDeclaration): ChartTheme {
  const v = (k: string, fallback = '') => style.getPropertyValue(k).trim() || fallback
  const primary = v('--primary', '#f59e0b')
  const palette = [0, 1, 2, 3, 4, 5].map((i) => v(`--chart-palette-${i}`)).filter(Boolean)
  return {
    bg: v('--bg', '#ffffff'),
    text_primary: v('--text', '#1a1d26'),
    text_secondary: v('--text-muted', '#5a6072'),
    text_tertiary: v('--text-muted', '#8a90a3'),
    color_primary: primary,
    color_success: v('--success', '#10b981'),
    color_danger: v('--danger', '#f43f5e'),
    chart_grid: v('--chart-grid', 'rgba(128,128,128,0.18)'),
    chart_axis: v('--chart-axis', 'rgba(128,128,128,0.35)'),
    chart_palette: palette.length ? palette : [primary],
    font_family: v('--font-body', '"Microsoft YaHei", "PingFang SC", sans-serif'),
  }
}

/**
 * 图表下方的来源角标；typst 侧 chart-focus.typ 的 source-caption 必须同步。
 * source 是内部枚举（user_input / tool / llm_generated），不能原样显示给观众。
 */
export function chartSourceLabel(spec: ChartSpec): string {
  if (spec.source === 'llm_generated') return '⚠ 示例数据'
  if (spec.source_note?.trim()) return `数据来源：${spec.source_note.trim()}`
  if (spec.source === 'tool') return '数据来源：工具生成'
  return '数据来源：文档'
}

/**
 * 图表在 PDF 里的绘图框：typst 排版 chart-focus.typ 时实测的内框（CSS px）与该页 --type-scale，
 * 由 `pdf_chart_boxes` 命令返回（Rust `slide_typst::ChartBox`）。
 * SVG 必须按它渲染：画布比内框大多少，标签就缩小多少（曾固定 920×440，PDF 里 12px 的标签只剩约 8px）。
 */
export interface ChartBox {
  slide: number
  width: number
  height: number
  scale: number
}

/** 用与屏幕预览完全相同的 buildChartOption 以 SSR 模式渲染一张图表（返回 SVG 字符串）。 */
export function renderChartSvg(spec: ChartSpec, theme: ChartTheme, box: Pick<ChartBox, 'width' | 'height' | 'scale'>): string {
  const chart = echarts.init(null, null, {
    renderer: 'svg',
    ssr: true,
    width: Math.max(1, Math.round(box.width)),
    height: Math.max(1, Math.round(box.height)),
  })
  try {
    chart.setOption(buildChartOption(spec, theme, 'static', box.scale))
    return chart.renderToSVGString()
  } finally {
    chart.dispose()
  }
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
