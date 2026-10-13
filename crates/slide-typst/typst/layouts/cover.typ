// cover — port of KeynoteCoverSlide.vue (+ light-notes `.cover-layout` stationery).

#import "../lib/components.typ": *

#let render(ctx, slide) = {
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
