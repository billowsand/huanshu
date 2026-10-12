// closing — port of KeynoteClosingSlide.vue (+ light-notes kicker and stamp):
// badges, an "END" kicker, the title, subtitle, a short rule and a note,
// left-aligned and vertically centered, with a stamped brand mark bottom-right.

#import "../lib/components.typ": *

#let parts(ctx, slide, f) = {
  let th = ctx.theme
  let ty = ctx.ty
  let out = ()
  let badges = slide.at("badges", default: none)
  let terminal = is-terminal(ctx)
  let glass = is-glass(ctx)
  if has(badges) {
    out.push((
      block(below: 0pt, badges
        .map(b => box(
          // `.amber-badge`: a 135deg primary-tint gradient on the glass themes,
          // a flat tint elsewhere.
          fill: if glass {
            gradient.linear((th.tones.neutral.bg, 0%), (th.tones.neutral.bg.transparentize(100%), 100%), angle: 45deg)
          } else { th.tones.neutral.bg },
          stroke: 1 * px + th.tones.neutral.border,
          radius: if glass { th.radius.pill * px } else { 2 * px },
          inset: (x: 12 * px, y: 2 * px),
          // `.tag-pill` letter spacing comes from the theme; the text is
          // `--primary-light` (the neutral tone's text).
          txt(ctx, "caption", ty.subtitle * f, 1.5, b, fill: th.tones.neutral.text, tracking: th.tracking.label),
        ))
        .join(h(12 * px))),
      1.5 * rem,
    ))
  }
  out.push((
    {
      // `.closing-kicker`: light-notes adds a 3px left bar and 0.3em tracking;
      // the terminal themes render plain primary text at 0.06em. The dark glass
      // themes keep the raw template style: hardcoded amber at 75%, 0.52em.
      let kicker = if glass {
        txt(ctx, "caption", ty.meta * f, 1.5, "END", fill: rgb(245, 158, 11, 191), tracking: 0.52em)
      } else {
        txt(ctx, "caption", ty.meta * f, 1.5, "END", fill: th.primary, tracking: if terminal {
          th.tracking.label
        } else { 0.3em })
      }
      if terminal or glass { kicker } else {
        block(stroke: (left: 3 * px + th.primary), inset: (left: 0.8 * rem), kicker)
      }
      v(1 * rem * f)
      txt(ctx, "title", 2.8 * rem * (ty.meta / rem) * f, 1.08, slide.title, tracking: if terminal {
        th.tracking.title
      } else { -0.04em })
      if has(slide.at("subtitle", default: none)) {
        v(1 * rem * f)
        layout(size => block(
          width: calc.min(40 * rem, size.width),
          // `.closing-subtitle`: hardcoded rgba(226,232,240,0.84) on the glass
          // themes; muted elsewhere.
          txt(ctx, "subtitle", ty.body-strong * f, 1.7, slide.subtitle, fill: if glass {
            rgb(226, 232, 240, 214)
          } else { th.muted }),
        ))
      }
    },
    1.25 * rem,
  ))
  out.push((divider-line(ctx, 288 * px), 1.2 * rem))
  if has(slide.at("note", default: none)) {
    out.push((
      layout(size => block(
        width: calc.min(34 * rem, size.width),
        // `.closing-note`: light-notes adds a 1px left rule; terminal themes
        // render the note as plain muted text; the glass themes keep the raw
        // hardcoded rgba(148,163,184,0.92) without a rule.
        stroke: if terminal or glass { none } else { (left: 1 * px + th.edge) },
        inset: if terminal or glass { (left: 0 * rem) } else { (left: 0.8 * rem) },
        txt(ctx, "body", ty.body-tight * f, 1.75, rich(slide.note), fill: if glass {
          rgb(148, 163, 184, 235)
        } else { th.muted }),
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
  let glass = is-glass(ctx)
  let stamped = txt(
    ctx,
    "caption",
    ctx.ty.caption,
    1.5,
    "幻述",
    // `.closing-mark`: hardcoded rgba(148,163,184,0.54) at 0.16em on the glass
    // themes (the raw template style; neither theme CSS overrides it).
    fill: if is-terminal(ctx) { th.muted } else if glass { rgb(148, 163, 184, 138) } else { th.primary },
    tracking: if is-terminal(ctx) { th.tracking.label } else { 0.16em },
  )
  let mark = if th.deco.double-frame {
    double-box(th.primary-border, inset: (x: 0.55 * rem, y: 0.35 * rem), stamped)
  } else if is-terminal(ctx) or glass {
    // Terminal and glass `.closing-mark`: plain text without a stamp frame.
    stamped
  } else {
    box(stroke: 1 * px + th.primary-border, radius: th.radius.pill * px, inset: (x: 0.55 * rem, y: 0.35 * rem), stamped)
  }
  place(top + right, dx: -28 * px, dy: size.height - measure(mark).height, mark)
  pad(x: 48 * px, center-v(width, size.height, fit-height(
    width,
    size.height,
    f => column(ctx, slide, f),
    slide-index: ctx.index,
  )))
})
