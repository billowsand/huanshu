// focus_example — port of KeynoteFocusExampleSlide.vue (+ light-notes example
// sheet): ruled point cards on the left; on the right an annotated example
// sheet with a ranked "correct order" list, muted entries faded out.

#import "../lib/components.typ": *

#let gap = 20 // `gap-5` between the columns

#let point-card(ctx, point, width, f) = {
  let th = ctx.theme
  let inset = (left: 19 * px, right: 16 * px, y: 12 * px * f) // `px-4 py-3` + 3px edge
  let copy = {
    txt(ctx, "title", ctx.ty.card-title * f, 1.3, point.at("title", default: ""))
    if has(point.at("body", default: none)) {
      v(4 * px * f)
      txt(ctx, "body", ctx.ty.caption * f, 1.5, rich(point.body), fill: th.ink.transparentize(30%))
    }
  }
  let body = if has(point.at("icon", default: none)) {
    grid(
      columns: (auto, 1fr),
      column-gutter: 12 * px * f,
      pad(top: 4 * px * f, icon(point.icon, th.primary, ctx.ty.icon-2xl * f)),
      copy,
    )
  } else { copy }
  let height = measure(block(width: width - inset.left - inset.right, body)).height + 2 * inset.y
  // Terminal and glass `.focus-point`: uniform 1px border, no 3px left accent.
  let stroke = if is-terminal(ctx) or is-glass(ctx) { 1 * px + th.edge } else {
    (left: 3 * px + th.edge, rest: 1 * px + th.edge)
  }
  sheet(ctx, width, height, inset: inset, stroke: stroke, ruled: true, body)
}

#let ranking-row(ctx, item, f) = {
  let th = ctx.theme
  let muted = item.at("muted", default: false) == true
  // `opacity: 0.4` on the whole row, border included.
  let fade(c) = if muted { c.transparentize(60%) } else { c }
  let size = ctx.ty.caption * f
  let meta = item.at("meta", default: none)
  block(
    width: 100%,
    stroke: (bottom: (paint: fade(th.edge), thickness: 1 * px, dash: "dashed")),
    inset: (bottom: 2.4 * px * f),
    grid(
      columns: (auto, 1fr),
      column-gutter: 8 * px * f,
      txt(ctx, "number", size, 1.5, str(item.at("index", default: "")), fill: fade(if muted { th.muted } else { th.primary })),
      txt(ctx, "body", size, 1.5, fill: fade(th.ink), {
        rich(str(item.at("label", default: "")))
        if has(meta) { text(fill: fade(th.ink.transparentize(50%)), rich(" （" + str(meta) + "）")) }
      }),
    ),
  )
}

#let example-body(ctx, slide, f) = {
  let th = ctx.theme
  let title = slide.at("example_title", default: none)
  txt(ctx, "title", ctx.ty.card-title * f, 1.3, if has(title) { title } else { "类比" })
  let body = slide.at("example_body", default: none)
  if has(body) {
    v(12 * px * f)
    txt(ctx, "body", ctx.ty.caption * f, 1.625, rich(body), fill: th.ink.transparentize(30%))
  }
  let ranking = slide.at("ranking", default: ())
  if has(ranking) {
    v(16 * px * f)
    divider-line(ctx, 100%)
    v(16 * px * f)
    txt(ctx, "body", ctx.ty.meta * f, 1.5, "正确的关注顺序：")
    v(8 * px * f)
    stack(spacing: 8 * px * f, ..ranking.map(item => ranking-row(ctx, item, f)))
  }
}

#let render(ctx, slide) = content-page(ctx, slide, subtitle-gap: 12, (width, avail) => {
  let th = ctx.theme
  let points = slide.at("points", default: ())
  let points = if points == none { () } else { points }
  let col-w = (width - gap * px) / 2
  let ex-pad = 23 * px // `p-5` inside the 3px double border

  let build(f) = {
    let left = stack(spacing: 12 * px * f, ..points.map(p => point-card(ctx, p, col-w, f)))
    let example = example-body(ctx, slide, f)
    let left-h = measure(block(width: col-w, left)).height
    let ex-h = measure(block(width: col-w - 2 * ex-pad, example)).height + 2 * ex-pad
    // Grid rows stretch: the example sheet is at least as tall as the points.
    let height = calc.max(left-h, ex-h)
    v(16 * px * f) // `mt-4`
    grid(
      columns: (col-w, col-w),
      column-gutter: gap * px,
      left,
      if th.deco.double-frame {
        block(
          width: col-w,
          height: height,
          fill: th.sheet,
          stroke: 1 * px + th.edge,
          radius: 2 * px,
          inset: 2 * px,
          block(width: 100%, height: 100%, stroke: 1 * px + th.edge, radius: 1 * px, inset: ex-pad - 2 * px, example),
        )
      } else {
        block(
          width: col-w,
          height: height,
          fill: th.sheet,
          stroke: 1 * px + th.edge,
          radius: th.radius.sheet * px,
          inset: ex-pad,
          example,
        )
      },
    )
  }
  center-v(width, avail, fit-height(width, avail, build, slide-index: ctx.index))
})
