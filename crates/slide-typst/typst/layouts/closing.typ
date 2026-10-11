// closing — port of KeynoteClosingSlide.vue (+ light-notes kicker and stamp):
// badges, an "END" kicker, the title, subtitle, a short rule and a note,
// left-aligned and vertically centered, with a stamped brand mark bottom-right.

#import "../lib/components.typ": *

#let parts(ctx, slide, f) = {
  let th = ctx.theme
  let ty = ctx.ty
  let out = ()
  let badges = slide.at("badges", default: none)
  if has(badges) {
    out.push((
      block(below: 0pt, badges
        .map(b => box(
          fill: th.tones.neutral.bg,
          stroke: 1 * px + th.tones.neutral.border,
          radius: 2 * px,
          inset: (x: 12 * px, y: 2 * px),
          txt(ctx, "caption", ty.subtitle * f, 1.5, b, fill: th.primary, tracking: 0.035em),
        ))
        .join(h(12 * px))),
      1.5 * rem,
    ))
  }
  out.push((
    {
      block(
        stroke: (left: 3 * px + th.primary),
        inset: (left: 0.8 * rem),
        txt(ctx, "caption", ty.meta * f, 1.5, "END", fill: th.primary, tracking: 0.3em),
      )
      v(1 * rem * f)
      txt(ctx, "title", 2.8 * rem * (ty.meta / rem) * f, 1.08, slide.title, tracking: -0.04em)
      if has(slide.at("subtitle", default: none)) {
        v(1 * rem * f)
        layout(size => block(
          width: calc.min(40 * rem, size.width),
          txt(ctx, "subtitle", ty.body-strong * f, 1.7, slide.subtitle, fill: th.muted),
        ))
      }
    },
    1.25 * rem,
  ))
  out.push((line(length: 288 * px, stroke: 1 * px + th.primary.transparentize(35%)), 1.2 * rem))
  if has(slide.at("note", default: none)) {
    out.push((
      layout(size => block(
        width: calc.min(34 * rem, size.width),
        stroke: (left: 1 * px + th.edge),
        inset: (left: 0.8 * rem),
        txt(ctx, "body", ty.body-tight * f, 1.75, rich(slide.note), fill: th.muted),
      )),
      0pt,
    ))
  }
  out
}

#let column(ctx, slide, f) = {
  let items = parts(ctx, slide, f)
  for (i, (body, gap)) in items.enumerate() {
    body
    if i < items.len() - 1 { v(gap * f) }
  }
}

#let render(ctx, slide) = layout(size => {
  let th = ctx.theme
  // `px-12` inside the frame; this layout has no `py-4`, but it is centered,
  // and the frame's center equals the content area's.
  let width = size.width - 2 * 48 * px
  // Stamped brand mark: `right-14 bottom-10` of the frame, i.e. on the content
  // area's bottom edge. Positioned from the top with an explicit offset: inside
  // `layout`, `place(bottom)` aligns to the laid-out content, not the area.
  let mark = double-box(th.primary-border, inset: (x: 0.55 * rem, y: 0.35 * rem), txt(
    ctx,
    "caption",
    ctx.ty.caption,
    1.5,
    "幻述",
    fill: th.primary,
    tracking: 0.16em,
  ))
  place(top + right, dx: -28 * px, dy: size.height - measure(mark).height, mark)
  pad(x: 48 * px, center-v(width, size.height, fit-height(
    width,
    size.height,
    f => column(ctx, slide, f),
    slide-index: ctx.index,
  )))
})
