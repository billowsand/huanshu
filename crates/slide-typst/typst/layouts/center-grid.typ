// center_grid — port of KeynoteCenterGridSlide.vue (+ light-notes memo tiles):
// a centered column — badge, title with an optional accent line, a short rule,
// a grid of toned memo tiles and a footnote — vertically centered on the page.
//
// The screen version caps the grid at `max-w-2xl` (672px) for every aspect
// ratio, which squeezes eight or twelve columns into slivers on 32:9 and 48:9;
// here the cap grows with the canvas width.

#import "../lib/components.typ": *

#let item-tone(ctx, item) = tone-of(ctx, item.at("tone", default: none))

#let tile-body(ctx, item, f) = {
  let th = ctx.theme
  align(center, stack(
    spacing: 8 * px * f,
    ..(
      if has(item.at("icon", default: none)) { icon(item.icon, item-tone(ctx, item).solid, ctx.ty.icon-2xl * f) },
      txt(ctx, "title", ctx.ty.card-title * f, 1.3, item.at("title", default: "")),
      if has(item.at("desc", default: none)) {
        txt(ctx, "body", ctx.ty.meta * f, 1.45, item.desc, fill: th.ink.transparentize(40%))
      },
    ).filter(x => x != none),
  ))
}

#let tile-grid(ctx, items, cols, width, f) = {
  let th = ctx.theme
  let gap = 16 * px * f
  let col-w = (width - gap * (cols - 1)) / cols
  let inset = (x: 16 * px * f, top: 17 * px * f, bottom: 16 * px * f) // `p-4` under a 3px top edge
  let rows = range(0, items.len(), step: cols).map(i => items.slice(i, calc.min(i + cols, items.len())))
  stack(spacing: gap, ..rows.map(row => {
    // Grid rows stretch: every tile in a row takes the tallest one's height.
    let height = calc.max(..row.map(item => measure(block(width: col-w - 2 * inset.x, tile-body(ctx, item, f))).height))
    let height = height + inset.top + inset.bottom
    // A short last row stays on the column tracks, left-aligned like CSS grid.
    grid(columns: (col-w,) * cols, column-gutter: gap, ..row.map(item => sheet(
      ctx,
      col-w,
      height,
      inset: inset,
      stroke: (top: 3 * px + item-tone(ctx, item).solid, rest: 1 * px + th.edge),
      ruled: true,
      tile-body(ctx, item, f),
    )))
  }))
}

#let column(ctx, slide, width, f) = {
  let th = ctx.theme
  let items = slide.at("center_items", default: ())
  let items = if items == none { () } else { items }
  let badge = slide.at("badge", default: none)
  let accent = slide.at("accent", default: none)
  let aspect = aspect-of(ctx)
  let wf = if aspect == "ratio_48x9" { 3 } else if aspect == "ratio_32x9" { 2 } else { 1 }
  let cols = calc.max(1, calc.min(4 * wf, items.len()))
  let gap = 20 * px * f // `gap-5`

  align(center, tag-pill(ctx, if has(badge) { badge } else { "总结" }, size: ctx.ty.subtitle * f))
  v(gap)
  // Title and accent are separate lines (`<br>`), each wrapped in balance.
  let title-line(value, fill) = align(center, balanced(width, txt(
    ctx,
    "title",
    ctx.ty.title * f,
    1.18,
    value,
    fill: fill,
    tracking: -0.02em,
  )))
  title-line(slide.title, th.ink)
  if has(accent) { title-line(accent, th.primary) }
  v(12 * px * f)
  align(center, line(length: calc.min(256 * px, width), stroke: 1 * px + th.primary.transparentize(35%)))
  v(16 * px * f + gap) // `.slide-header` margin-bottom, then the column gap
  if items.len() > 0 {
    let grid-w = calc.min(672 * px * wf, width)
    align(center, block(width: grid-w, tile-grid(ctx, items, cols, grid-w, f)))
  }
  let footer = slide.at("footer", default: none)
  if has(footer) {
    v(gap + 8 * px * f)
    align(center, txt(ctx, "caption", ctx.ty.meta * f, 1.45, rich(footer)))
  }
}

#let render(ctx, slide) = layout(size => center-v(size.width, size.height, fit-height(
  size.width,
  size.height,
  f => column(ctx, slide, size.width, f),
  slide-index: ctx.index,
)))
