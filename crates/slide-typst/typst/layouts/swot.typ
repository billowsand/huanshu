// swot — port of KeynoteSwotSlide.vue with the light-notes "memo sheet" variant
// (flags in the document flow, summaries as left-ruled notes).

#import "../lib/components.typ": *

#let order = ("strengths", "weaknesses", "opportunities", "threats")

#let quadrant(ctx, q, f) = {
  let th = ctx.theme
  let tone = tone-of(ctx, q.at("tone", default: none))
  let summary = q.at("summary", default: none)
  let summary-block = if has(summary) {
    block(
      width: 100%,
      fill: tone.bg,
      stroke: (left: 2 * px + tone.solid),
      inset: (x: 0.5 * rem, y: 0.15 * rem),
      txt(ctx, "body", ctx.ty.caption * f, 1.3, summary, fill: tone.text),
    )
  }
  let summary-first = q.at("key", default: "") in ("strengths", "weaknesses")
  let flag = block(
    width: 100%,
    fill: tone.bg,
    stroke: (left: 3 * px + tone.solid, rest: 1 * px + tone.border),
    radius: 2 * px,
    inset: (x: 0.4 * rem, y: 0.15 * rem),
    grid(
      columns: (auto, 1fr),
      column-gutter: 0.55 * rem,
      align: horizon,
      box(width: 1.3 * rem, height: 1.3 * rem, fill: tone.solid, radius: 2 * px, align(center + horizon, icon(
        q.at("icon", default: "i-carbon:idea"),
        tone.contrast,
        ctx.ty.icon-sm * 0.8,
      ))),
      txt(ctx, "title", ctx.ty.body-tight * 1.15 * f, 1.1, q.title, fill: tone.text),
    ),
  )
  let items = stack(spacing: 0.2 * rem * f, ..q
    .at("items", default: ())
    .map(item => grid(
      columns: (auto, 1fr),
      column-gutter: 0.65 * rem,
      pad(top: 0.42 * rem * f, box(width: 0.55 * rem, height: 0.55 * rem, radius: 1 * px, fill: tone.solid)),
      txt(ctx, "body", 1 * rem * f, 1.625, item),
    )))
  let parts = (flag,)
  if summary-first and summary-block != none { parts.push(summary-block) }
  parts.push(items)
  if not summary-first and summary-block != none { parts.push(summary-block) }
  stack(spacing: 0.25 * rem + 0.25 * rem * f, ..parts)
}

#let render(ctx, slide) = {
  let data = slide.at("swot_data", default: none)
  let quadrants = if data == none { () } else { data.at("quadrants", default: ()) }
  let quadrants = quadrants.sorted(key: q => {
    let i = order.position(k => k == q.at("key", default: ""))
    if i == none { order.len() } else { i }
  })
  let strategy = if data == none { none } else { data.at("strategy", default: none) }

  content-page(ctx, slide, subtitle-gap: 24, gap-after-header: 16, (w, h) => {
    let strategy-block = if has(strategy) { note-bar(ctx, "i-carbon:direction-loop-right", strategy) }
    let strategy-h = if strategy-block == none { 0pt } else { measure(block(width: w, strategy-block)).height + 1 * rem }
    let grid-h = h - strategy-h
    let gap = 1 * rem
    let cell-w = (w - gap) / 2
    let cell-h = (grid-h - gap) / 2
    let pad = 0.5 * rem
    // One factor for all four sheets keeps the quadrants visually consistent.
    let tallest(f) = calc.max(0pt, ..quadrants.map(q => measure(block(width: cell-w - 2 * pad, quadrant(ctx, q, f))).height))
    let f = fit-scale(tallest, cell-h - 2 * pad)
    if f == none {
      overflow-marker(ctx.index, tallest(fit-min) + 2 * pad, cell-h)
      f = fit-min
    }
    grid(
      columns: (cell-w, cell-w),
      rows: (cell-h, cell-h),
      gutter: gap,
      ..quadrants.map(q => paper-card(
        ctx,
        height: 100%,
        stroke: 1 * px + tone-of(ctx, q.at("tone", default: none)).border,
        radius: 2 * px,
        inset: pad,
        clip: true,
        quadrant(ctx, q, f),
      )),
    )
    if strategy-block != none {
      v(1 * rem)
      strategy-block
    }
  })
}
