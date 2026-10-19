// chart_focus — port of KeynoteChartSlide.vue: interpretation points on the
// left (5/12), a chart card on the right (7/12). The chart is drawn natively by
// lib/chart.typ (a port of the app's ECharts option builder) inside the card's
// plot box; the card chrome (fill/stroke/title/caption) reads the theme here.

#import "../lib/components.typ": *
#import "../lib/chart.typ": as-text, chart-plot, chart-read

#let gap = 20 // `gap-5` between the columns

// Shown when there is no chart or its data cannot be drawn (`chart-read`).
#let placeholder(ctx, w, h, label, detail: none) = block(
  width: w,
  height: h,
  fill: ctx.theme.paper,
  stroke: 1 * px + ctx.theme.edge,
  radius: ctx.theme.radius.card * px,
  {
    let body = {
      let ink = ctx.theme.ink.transparentize(60%)
      align(center, icon("i-carbon:chart-line-smooth", ink, calc.min(ctx.ty.icon-hero, h * 0.3)))
      v(12 * px)
      align(center, txt(ctx, "caption", ctx.ty.meta, 1.45, label, fill: ink))
      if detail != none {
        align(center, txt(ctx, "code", ctx.ty.caption * 0.8, 1.4, detail, fill: ink))
      }
    }
    center-v(w, h, body)
  },
)

// Measured against the Vue page (light-notes, 16:9): title `.slide-card-title`
// (card-title size, line-height 1.3), body `.slide-text-caption` at 70%
// opacity after `mt-1`, `gap-3` to the step number (`.cf-step`: card-title
// size so it sits on the title line) or, without a step, an 8px square
// (`.cf-dot`, `mt-2`). Same metrics as focus_example's point cards.
#let point-row(ctx, point, index, f) = {
  let th = ctx.theme
  let body = {
    txt(ctx, "title", ctx.ty.card-title * f, 1.3, point.at("title", default: ""))
    let desc = point.at("body", default: none)
    if has(desc) {
      v(4 * px * f)
      txt(ctx, "body", ctx.ty.caption * f, 1.5, rich(desc), fill: th.ink.transparentize(30%))
    }
  }
  let marker = txt(ctx, "number", ctx.ty.card-title * f, 1.3, str(index + 1), fill: th.primary)
  grid(columns: (auto, 1fr), column-gutter: 12 * px * f, marker, body)
}

// `figcaption` under the chart: source badge, mirroring chartSourceLabel() in
// chartOptions.ts. `source` is an internal enum and is never shown verbatim.
// `.slide-text-caption` size; the notes theme colors caption text with the ink
// (measured), the other themes keep `.cf-caption`'s --text-muted.
#let source-caption(ctx, chart, f) = {
  let th = ctx.theme
  let source = chart.at("source", default: none)
  let note = chart.at("source_note", default: none)
  let text = if source == "llm_generated" {
    "⚠ 示例数据"
  } else if has(note) and as-text(note).trim() != "" {
    "数据来源：" + as-text(note).trim()
  } else if source == "tool" {
    "数据来源：工具生成"
  } else {
    "数据来源：文档"
  }
  let ink = if ctx.theme.family == "notes" { th.ink } else { th.muted }
  txt(ctx, "caption", ctx.ty.caption * f, 1.5, text, fill: ink)
}

#let chart-card(ctx, slide, width, height, f) = {
  let th = ctx.theme
  let chart = slide.at("chart", default: none)
  if type(chart) != dictionary {
    return placeholder(ctx, width, height, "图表待补充", detail: "缺少 chart 字段")
  }
  let spec = chart-read(chart)
  let card-pad = 20 * px // `p-5`
  // Card chrome, measured on the Vue page: title `mb-2`; caption `mt-2`, then a
  // 1px top border and `pt-2` (the stroke takes no layout space here, so it is
  // folded into the inset).
  let title-gap = 8
  let caption-gap = 8
  let rule-pad = 1 + 8
  // Ink, like focus_example's example title: the template's `text="var(--primary)"`
  // attribute does not take effect on screen (measured in all five themes).
  let title = txt(ctx, "title", ctx.ty.card-title * f, 1.3, as-text(chart.at("title", default: "")))
  let caption = source-caption(ctx, chart, f)
  let title-h = measure(block(width: width - 2 * card-pad, title)).height
  let caption-h = measure(block(width: width - 2 * card-pad, caption)).height
  let inner-w = width - 2 * card-pad
  let inner-h = height - 2 * card-pad - title-h - caption-h - (title-gap + caption-gap + rule-pad) * px * f
  // Data that cannot be drawn shows the placeholder with the reason, never a
  // compile error: one bad chart must not fail the export or the thumbnails.
  let plot = if spec.ok {
    chart-plot(ctx, spec, inner-w, inner-h)
  } else {
    placeholder(ctx, inner-w, inner-h, "图表待补充", detail: spec.reason)
  }
  block(
    width: width,
    height: height,
    // `.glass-card` under each theme: the paper color (measured), not the sheet tint.
    fill: th.paper,
    stroke: if is-editorial(ctx) { (left: 1 * px + th.edge) } else { 1 * px + th.edge },
    radius: th.radius.card * px,
    clip: true,
    pad(
      card-pad,
      {
        title
        v(title-gap * px * f)
        box(width: inner-w, height: inner-h, plot)
        v(caption-gap * px * f)
        block(
          width: 100%,
          stroke: (top: 1 * px + th.edge),
          inset: (top: rule-pad * px * f),
          caption,
        )
      },
    ),
  )
}

#let render(ctx, slide) = content-page(ctx, slide, footer: slide.at("footer", default: none), subtitle-gap: 12, (width, avail) => {
  let th = ctx.theme
  let points = slide.at("points", default: ())
  let points = if points == none { () } else { points }
  let left-w = (width - gap * px) * 5 / 12
  let right-w = width - gap * px - left-w

  let build(f) = {
    let left = stack(spacing: 16 * px * f, ..points.enumerate().map(((i, p)) => point-row(ctx, p, i, f)))
    v(16 * px * f) // `mt-4`
    grid(
      columns: (left-w, right-w),
      column-gutter: gap * px,
      center-v(left-w, avail - 16 * px * f, left),
      chart-card(ctx, slide, right-w, avail - 16 * px * f, f),
    )
  }
  fit-height(width, avail, build, slide-index: ctx.index, center: true)
})
