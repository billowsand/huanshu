// cover — port of KeynoteCoverSlide.vue (+ light-notes `.cover-layout` stationery).

#import "../lib/components.typ": *

#let render(ctx, slide) = {
  let th = ctx.theme
  let ty = ctx.ty
  let w = ctx.canvas.w
  let content-w = w - frame-inset.left - frame-inset.right

  // Paper tape: `.cover-layout::before`, 2rem below the frame top, centered.
  place(top + left, dx: (content-w / 2 - 48) * px, dy: (34 + 32 - frame-inset.top) * px, rotate(-3deg, rect(
    width: 6 * rem,
    height: 1.35 * rem,
    fill: th.tape,
    stroke: (
      left: (paint: th.tape-edge, thickness: 1 * px, dash: "dashed"),
      right: (paint: th.tape-edge, thickness: 1 * px, dash: "dashed"),
    ),
  )))
  // Frame-wide rule at the bottom of `.slide-shell__frame`.
  place(top + left, dx: -28 * px, dy: (707.5 - frame-inset.top) * px, line(
    length: (w - 28) * px,
    stroke: 1 * px + th.primary.transparentize(35%),
  ))

  let badges = slide.at("badges", default: none)
  let subtitle = slide.at("subtitle", default: none)
  let parts = ()
  if has(badges) {
    parts.push(block(below: 8 * px, badges
      .map(b => double-box(th.edge, inset: (x: 12 * px, y: 2 * px), txt(
        ctx,
        "caption",
        ty.subtitle,
        1.5,
        b,
        fill: th.primary,
        tracking: 0.035em,
      )))
      .join(h(12 * px))))
  }
  parts.push({
    txt(ctx, "title", ty.cover-title, 1.15, slide.title, tracking: -0.02em)
    if has(subtitle) {
      v(0.75 * rem)
      txt(ctx, "subtitle", ty.cover-subtitle, 1.5, subtitle, fill: th.muted.transparentize(15%))
    }
  })
  parts.push(block(above: 8 * px, line(length: 256 * px, stroke: 1 * px + th.primary.transparentize(35%))))

  block(width: 100%, height: 100%, align(center + horizon, stack(spacing: 24 * px, ..parts.map(p => align(
    center,
    p,
  )))))
}
