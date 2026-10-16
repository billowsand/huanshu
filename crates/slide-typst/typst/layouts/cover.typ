// cover — port of KeynoteCoverSlide.vue (+ light-notes `.cover-layout` stationery).

#import "../lib/components.typ": *

#let editorial-cover(ctx, slide) = layout(size => {
  let th = ctx.theme
  let number = if ctx.index < 9 { "0" + str(ctx.index + 1) } else { str(ctx.index + 1) }
  let main-w = calc.min(size.width * 0.7, 1080 * px)
  let gap = 40 * px
  let side-w = size.width - main-w - gap
  txt(ctx, "caption", 12 * px, 1.5, number + " / PRESENTATION", fill: th.muted, tracking: 0.12em)
  v(32 * px)
  let body-h = size.height - 60 * px
  grid(columns: (main-w, side-w), column-gutter: gap, align: horizon,
    fit-height(main-w, body-h, f => {
      let badges = slide.at("badges", default: none)
      if has(badges) {
        txt(ctx, "caption", ctx.ty.caption * f, 1.5, badges.join(" / "), fill: th.primary, tracking: th.tracking.label)
        v(20 * px * f)
      }
      headline(ctx, ctx.ty.cover-title * 1.25 * f, 1.15, slide.title, tracking: th.tracking.title)
      if has(slide.at("subtitle", default: none)) {
        v(24 * px * f)
        txt(ctx, "subtitle", ctx.ty.cover-subtitle * f, 1.5, slide.subtitle, fill: th.muted)
      }
    }, slide-index: ctx.index, center: true),
    block(width: side-w, height: body-h, stroke: (left: 1 * px + th.edge), inset: (left: 28 * px),
      align(center + horizon, text(font: ctx.fonts.number, size: calc.min(210 * px, side-w * 0.62),
        fill: rgb(0, 0, 0, 0), stroke: 1 * px + th.primary.transparentize(25%), weight: "bold", number))),
  )
})

#let render(ctx, slide) = {
  if is-editorial(ctx) { return editorial-cover(ctx, slide) }
  let th = ctx.theme
  let ty = ctx.ty
  let w = ctx.canvas.w
  let content-w = w - frame-inset.left - frame-inset.right

  // Paper tape: `.cover-layout::before`, 2rem below the frame top, centered.
  if th.deco.tape {
    place(top + left, dx: (content-w / 2 - 48) * px, dy: (34 + 32 - frame-inset.top) * px, rotate(-3deg, rect(
      width: 6 * rem,
      height: 1.35 * rem,
      fill: th.tape,
      stroke: (
        left: (paint: th.tape-edge, thickness: 1 * px, dash: "dashed"),
        right: (paint: th.tape-edge, thickness: 1 * px, dash: "dashed"),
      ),
    )))
  }
  // Frame-wide rule at the bottom of `.slide-shell__frame`.
  place(top + left, dx: -28 * px, dy: (707.5 - frame-inset.top) * px, divider-line(ctx, (w - 28) * px))

  let badges = slide.at("badges", default: none)
  let subtitle = slide.at("subtitle", default: none)
  let parts = ()
  if has(badges) {
    parts.push(block(below: 8 * px, badges
      .map(b => {
        // `.amber-badge.tag-pill`: text is `--primary-light` (neutral tone).
        let label = txt(ctx, "caption", ty.subtitle, 1.5, b, fill: th.tones.neutral.text, tracking: th.tracking.label)
        if th.deco.double-frame {
          double-box(th.edge, inset: (x: 12 * px, y: 2 * px), label)
        } else if is-glass(ctx) {
          // `.amber-badge`: a 135deg gradient from the primary tint to clear.
          box(
            fill: gradient.linear((th.tones.neutral.bg, 0%), (th.tones.neutral.bg.transparentize(100%), 100%), angle: 45deg),
            stroke: 1 * px + th.tones.neutral.border,
            radius: th.radius.pill * px,
            inset: (x: 12 * px, y: 2 * px),
            label,
          )
        } else if is-editorial(ctx) {
          // Editorial `.amber-badge`: a solid cobalt label with white text.
          box(fill: th.primary, stroke: 1 * px + th.primary, radius: th.radius.pill * px, inset: (x: 12 * px, y: 2 * px), txt(
            ctx,
            "caption",
            ty.subtitle,
            1.5,
            b,
            fill: th.tones.neutral.contrast,
            tracking: th.tracking.label,
          ))
        } else {
          // Terminal `.amber-badge`: filled with the primary tint.
          box(fill: th.tones.neutral.bg, stroke: 1 * px + th.tones.neutral.border, radius: th.radius.pill * px, inset: (
            x: 12 * px,
            y: 2 * px,
          ), label)
        }
      })
      .join(h(12 * px))))
  }
  parts.push({
    headline(ctx, ty.cover-title, 1.15, slide.title, tracking: th.tracking.title)
    if has(subtitle) {
      v(0.75 * rem)
      // `.cover-subtitle`: muted on light-notes, ink on the editorial theme
      // (the deck line under a two-tone headline), 85% primary elsewhere
      // (style.css sets `color: var(--primary); opacity: 0.85`).
      txt(ctx, "subtitle", ty.cover-subtitle, 1.5, subtitle, fill: if is-editorial(ctx) { th.ink } else if is-terminal(ctx) or is-glass(ctx) {
        th.primary.transparentize(15%)
      } else { th.muted.transparentize(15%) })
    }
  })
  parts.push(block(above: 8 * px, divider-line(ctx, 256 * px)))

  block(width: 100%, height: 100%, align(center + horizon, stack(spacing: 24 * px, ..parts.map(p => align(
    center,
    p,
  )))))
}
