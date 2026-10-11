// section_list — port of KeynoteSectionListSlide.vue (+ light-notes lined notes):
// a vertical list of ruled cards, each led by a stamped step number (or an
// icon when there is no step), with a title over a one-line summary.

#import "../lib/components.typ": *

#let card-pad = (left: 23, right: 20, y: 16) // `px-5 py-4` inside the 3px left edge

#let item-body(ctx, item, f) = {
  let th = ctx.theme
  let marker = if has(item.at("step", default: none)) {
    step-stamp(ctx, item.step, f: f)
  } else if has(item.at("icon", default: none)) {
    pad(top: 4 * px * f, icon(item.icon, th.primary, ctx.ty.icon-2xl * f))
  }
  let copy = {
    txt(ctx, "title", ctx.ty.card-title * f, 1.3, item.at("title", default: ""))
    v(4 * px * f)
    txt(ctx, "body", ctx.ty.meta * f, 1.5, rich(item.at("body", default: "")), fill: th.ink.transparentize(30%))
  }
  if marker == none { copy } else { grid(columns: (auto, 1fr), column-gutter: 16 * px * f, marker, copy) }
}

#let item-card(ctx, item, width, f) = {
  let inset = (left: card-pad.left * px, right: card-pad.right * px, y: card-pad.y * px * f)
  let body = item-body(ctx, item, f)
  let height = measure(block(width: width - inset.left - inset.right, body)).height + 2 * inset.y
  sheet(
    ctx,
    width,
    height,
    inset: inset,
    stroke: (left: 3 * px + ctx.theme.edge, rest: 1 * px + ctx.theme.edge),
    ruled: true,
    body,
  )
}

#let render(ctx, slide) = content-page(ctx, slide, subtitle-gap: 12, (width, avail) => {
  let items = slide.at("list_items", default: ())
  let items = if items == none { () } else { items }
  if items.len() == 0 { return }
  let build(f) = {
    v(12 * px * f) // `mt-3`, inside the centered block as in the template
    stack(spacing: 16 * px * f, ..items.map(item => item-card(ctx, item, width, f)))
  }
  center-v(width, avail, fit-height(width, avail, build, slide-index: ctx.index))
})
