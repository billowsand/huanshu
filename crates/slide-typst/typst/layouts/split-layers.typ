// split_layers — port of KeynoteSplitLayersSlide.vue (+ light-notes labeled
// sheets): numbered explanation cards on the left, joined by fine ink lines;
// on the right a sheet with a stack of toned layers (bottom = most concrete)
// separated by faint arrows, and an optional footnote.

#import "../lib/components.typ": *

#let card-pad = (x: 16, y: 12) // GlassCard's default `px-4 py-3`
#let card-gap = 12.8 // `.split-left-col` gap: 0.8rem
#let connector = (top: 12, left: 23.2, length: 12.8) // `.split-left-connector`

#let pad2(n) = if n < 10 { "0" + str(n) } else { str(n) }

#let left-content(ctx, item, idx, f) = {
  let th = ctx.theme
  let caption = ctx.ty.caption * f
  let step = item.at("step", default: none)
  let marker = step-stamp(
    ctx,
    if has(step) { step } else { pad2(idx + 1) },
    size: 46.4,
    f: f,
    text-size: ctx.ty.caption,
    border: th.edge,
    tracking: 0.08em,
    radius: 4,
  )
  let copy = {
    grid(
      columns: (1fr, auto),
      column-gutter: 12 * px * f,
      txt(ctx, "title", ctx.ty.card-title * f, 1.3, item.at("title", default: "")),
      // Baseline-aligned with the title: the smaller line box sits lower.
      pad(top: 4 * px * f, txt(
        ctx,
        "number",
        caption,
        1.5,
        pad2(idx + 1),
        fill: th.muted,
        tracking: 0.14em,
      )),
    )
    v(0.32 * rem * f)
    txt(ctx, "body", caption, 1.6, rich(item.at("body", default: "")), fill: th.ink.transparentize(22%))
  }
  let cols = (auto,)
  let cells = (marker,)
  if has(item.at("icon", default: none)) {
    cols.push(auto)
    cells.push(pad(top: 4 * px * f, icon(item.icon, th.primary, ctx.ty.icon-2xl * f)))
  }
  grid(columns: cols + (1fr,), column-gutter: 12 * px * f, ..cells, copy)
}

#let left-card(ctx, item, idx, last, width, extra, f) = {
  let th = ctx.theme
  let inner-w = width - 2 * card-pad.x * px
  let body = {
    left-content(ctx, item, idx, f)
    if not last {
      v(connector.top * px * f)
      pad(left: connector.left * px * f, line(angle: 90deg, length: connector.length * px * f, stroke: 1 * px + th.edge))
    }
  }
  let height = measure(block(width: inner-w, body)).height + 2 * card-pad.y * px * f + extra
  sheet(ctx, width, height, inset: (x: card-pad.x * px, y: card-pad.y * px * f), radius: 4 * px, body)
}

#let left-column(ctx, items, width, avail, f) = {
  let gap = card-gap * px * f
  let n = items.len()
  let natural = items.enumerate().map(((i, it)) => measure(left-card(ctx, it, i, i == n - 1, width, 0pt, f)).height)
  let total = natural.sum(default: 0pt) + gap * calc.max(n - 1, 0)
  (total, extra => stack(spacing: gap, ..items.enumerate().map(((i, it)) => left-card(ctx, it, i, i == n - 1, width, extra, f))))
}

#let layer-row(ctx, layer, f) = {
  let th = ctx.theme
  let tone = tone-of(ctx, layer.at("tone", default: none))
  let size = ctx.ty.caption * f
  block(
    width: 100%,
    fill: th.paper,
    // `box-shadow: inset 3px 0 0` in the tone ink, inside a toned hairline.
    stroke: (left: 3 * px + tone.solid, rest: 1 * px + tone.border),
    radius: 2 * px,
    inset: (left: 14 * px, right: 12 * px, y: 8 * px * f),
    grid(
      columns: (1fr, auto),
      column-gutter: 12 * px * f,
      txt(ctx, "body", size, 1.5, layer.at("title", default: "")),
      txt(ctx, "caption", size, 1.5, str(layer.at("meta", default: "")), fill: th.muted),
    ),
  )
}

#let right-content(ctx, slide, f) = {
  let th = ctx.theme
  let layers = slide.at("layers", default: ())
  let layers = if layers == none { () } else { layers }
  let title = slide.at("layers_title", default: none)
  txt(ctx, "title", ctx.ty.card-title * f, 1.3, if has(title) { title } else { "架构层次" })
  v(16 * px * f) // `mb-1` + `gap-3`
  let arrow-h = ctx.ty.icon-xl * 1.5 * f
  let arrow-s = ctx.ty.icon-xl * 0.75 * f
  for (i, layer) in layers.enumerate() {
    layer-row(ctx, layer, f)
    if i < layers.len() - 1 {
      let next = tone-of(ctx, layers.at(i + 1).at("tone", default: none))
      v(8 * px * f)
      block(width: 100%, height: arrow-h, inset: (top: (arrow-h - arrow-s) / 2), align(center, icon(
        "i-carbon:arrow-up",
        next.text.transparentize(70%),
        arrow-s,
      )))
      v(8 * px * f)
    }
  }
  let footer = slide.at("footer", default: none)
  if has(footer) {
    v(16 * px * f) // `mt-1` + `gap-3`
    align(center, txt(ctx, "caption", ctx.ty.caption * f, 1.5, rich(footer)))
  }
}

#let render(ctx, slide) = content-page(ctx, slide, subtitle-gap: 12, (width, avail) => {
  let th = ctx.theme
  let items = slide.at("left_items", default: ())
  let items = if items == none { () } else { items }
  let aspect = aspect-of(ctx)
  let (fr, gap) = if aspect == "ratio_48x9" { (4, 1.5) } else if aspect == "ratio_32x9" { (3, 1.35) } else { (1, 1.25) }
  let gap = gap * rem
  let left-w = (width - gap) / (1 + fr)
  let right-w = width - gap - left-w
  let top = 12 * px // `mt-3`
  let height = avail - top
  let right-pad = 16 * px
  let right-h(f) = measure(block(width: right-w - 2 * right-pad, right-content(ctx, slide, f))).height + 2 * right-pad
  let fits(f) = left-column(ctx, items, left-w, height, f).at(0) <= height and right-h(f) <= height
  let f = fit-factor(fits)
  if f == none {
    f = fit-min
    let need = calc.max(left-column(ctx, items, left-w, height, f).at(0), right-h(f))
    overflow-marker(ctx.index, need, height)
  }
  let (total, build-left) = left-column(ctx, items, left-w, height, f)
  // The column's grid rows stretch: spare height is shared by every card.
  let extra = if items.len() == 0 { 0pt } else { calc.max(0pt, (height - total) / items.len()) }
  v(top)
  grid(
    columns: (left-w, right-w),
    column-gutter: gap,
    block(height: height, clip: true, build-left(extra)),
    sheet(ctx, right-w, height, fill: th.sheet, inset: (rest: right-pad), right-content(ctx, slide, f)),
  )
})
