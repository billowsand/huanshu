// outcome_grid — port of KeynoteOutcomeGridSlide.vue (+ light-notes certificate
// cards): one row of centered result cards — toned top bar, icon, title, copy
// and a double-ruled tag — with every field row aligned across the cards (the
// template's subgrid), and an optional note bar.

#import "../lib/components.typ": *

// normal ≤3 cards, compact = 4, mini ≥5; sizes in rem before the type scale.
#let densities = (
  normal: (pad-y: 1.25, pad-x: 1, icon: 2.25, body: 1, gap: 1),
  compact: (pad-y: 1, pad-x: 0.75, icon: 1.875, body: 0.9375, gap: 0.75),
  mini: (pad-y: 0.75, pad-x: 0.5, icon: 1.5, body: 0.875, gap: 0.5),
)

#let card-pad = (x: 16, y: 12) // GlassCard's default `px-4 py-3`
#let row-gap = 0.75 // `.outcome-content` row gap, rem

#let card-tone(ctx, card) = tone-of(ctx, card.at("tone", default: none))

// `.outcome-tag`: a toned pill; light-notes gives it a 3px double border
// (`deco.double-frame`), other themes a plain 1px frame.
#let outcome-tag(ctx, tone, value, size) = {
  let label = txt(ctx, "caption", size, 1.5, value, fill: tone.text, tracking: ctx.theme.tracking.label)
  if ctx.theme.deco.double-frame {
    box(
      fill: tone.bg,
      stroke: 1 * px + tone.border,
      radius: ctx.theme.radius.pill * px,
      inset: 1 * px,
      box(stroke: 1 * px + tone.border, radius: 1 * px, inset: (x: 10 * px, y: 0pt), label),
    )
  } else {
    box(
      fill: tone.bg,
      stroke: 1 * px + tone.border,
      radius: ctx.theme.radius.pill * px,
      inset: (x: 10 * px, y: 0pt),
      label,
    )
  }
}

// The four aligned fields of one card; `none` where the card has no value.
#let fields(ctx, card, d, f) = {
  let th = ctx.theme
  let tone = card-tone(ctx, card)
  let s = ctx.ty.meta / rem
  let body = d.body * rem * s * f
  let centered(x) = if x == none { none } else { align(center, x) }
  (
    centered(icon(card.at("icon", default: "i-carbon:checkmark-filled"), tone.text, d.icon * rem * s * f)),
    centered(txt(ctx, "title", body * 1.25, 1.3, card.at("title", default: ""))),
    if has(card.at("body", default: none)) {
      centered(txt(ctx, "body", body, 1.625, rich(card.body, emphasis: tone.text), fill: th.muted))
    },
    if has(card.at("tag", default: none)) { centered(outcome-tag(ctx, tone, card.tag, ctx.ty.subtitle * f)) },
  )
}

#let layout-cards(ctx, cards, d, width, f) = {
  let th = ctx.theme
  let n = cards.len()
  let gap = d.gap * rem * f
  let col-w = (width - gap * (n - 1)) / n
  let s = ctx.ty.meta / rem
  let pad-x = d.pad-x * rem * s
  let pad-y = d.pad-y * rem * s * f
  let inner-w = col-w - 2 * card-pad.x * px - 2 * pad-x
  let cells = cards.map(card => fields(ctx, card, d, f))
  let heights = range(4).map(r => calc.max(0pt, ..cells.map(c => if c.at(r) == none { 0pt } else {
    measure(block(width: inner-w, c.at(r))).height
  })))
  // Rows that no card fills take no space, like empty subgrid tracks.
  let rows = range(4).filter(r => heights.at(r) > 0pt)
  let gaps = calc.max(rows.len() - 1, 0) * row-gap * rem * f
  let content-h = rows.map(r => heights.at(r)).sum(default: 0pt) + gaps
  let card-h = 2 * card-pad.y * px * f + 4 * px + 2 * pad-y + content-h
  let card-block(card, cell) = {
    let tone = card-tone(ctx, card)
    // Terminal and glass cards are plain panels: no tone border, no top bar
    // (the screen's `outcome-top-bar` only renders when the data sets a bar
    // class).
    let flat = is-terminal(ctx) or is-glass(ctx)
    block(width: col-w, height: card-h, fill: th.paper, stroke: if is-editorial(ctx) {
      (left: 1 * px + th.edge)
    } else { 1 * px + if flat {
      th.edge
    } else { tone.border } }, radius: if flat { th.radius.card * px } else { 4 * px }, {
      // `outline: 1px; outline-offset: -5px` — a hairline frame 4–5px inside.
      if th.deco.certificate {
        place(top + left, dx: 4.5 * px, dy: 4.5 * px, rect(
          width: col-w - 9 * px,
          height: card-h - 9 * px,
          stroke: 1 * px + th.outline,
        ))
      }
      pad(x: card-pad.x * px, y: card-pad.y * px * f, {
        if not flat { block(width: 100%, height: 4 * px, fill: tone.solid) }
        pad(x: pad-x, y: pad-y, stack(
          spacing: row-gap * rem * f,
          ..rows.map(r => block(width: 100%, height: heights.at(r), cell.at(r))),
        ))
      })
    })
  }
  grid(columns: (col-w,) * n, column-gutter: gap, ..cards.zip(cells).map(((card, cell)) => card-block(card, cell)))
}

#let render(ctx, slide) = content-page(ctx, slide, subtitle-gap: 12, (width, avail) => {
  let cards = slide.at("cards", default: ())
  let cards = if cards == none { () } else { cards }
  if cards.len() == 0 { return }
  let n = cards.len()
  let d = if n >= 5 { densities.mini } else if n == 4 { densities.compact } else { densities.normal }
  let note = slide.at("note", default: none)
  let build(f) = {
    v(16 * px * f) // `mt-4`
    layout-cards(ctx, cards, d, width, f)
    if has(note) {
      v(16 * px * f)
      note-bar(ctx, "i-carbon:collaborate", note, f: f)
    }
  }
  fit-height(width, avail, build, slide-index: ctx.index, center: true)
})
