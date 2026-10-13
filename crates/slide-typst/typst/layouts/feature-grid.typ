// feature_grid — port of KeynoteFeatureGridSlide.vue.
//
// The Vue template aligns optional fields across cards with CSS subgrid and
// guesses a shrink factor from character counts. Here each field row is
// measured at the real column width, the tallest cell sets the row height, and
// `fit-height` picks the largest factor that leaves room for the note bar.

#import "../lib/components.typ": *

#let densities = (
  normal: (head-x: 1, head-y: 0.75, head: 1.25, body: 1, pad: 1, icon: 1.25, number: 1.5, gap: 1),
  compact: (head-x: 0.75, head-y: 0.5, head: 1.171875, body: 0.9375, pad: 0.75, icon: 1.125, number: 1.25, gap: 0.75),
  mini: (head-x: 0.5, head-y: 0.5, head: 1.09375, body: 0.875, pad: 0.5, icon: 1, number: 1.125, gap: 0.5),
)

#let field-rows = ("number", "subtitle", "body", "items", "risk", "conclusion", "footer")

#let footer-tag(card) = card.at("footer_tag", default: card.at("footerTag", default: none))

#let field(ctx, card, row, d, f) = {
  let th = ctx.theme
  let tone = tone-of(ctx, card.at("tone", default: none))
  let s = ctx.ty.meta / rem
  let body-size = d.body * rem * s * f
  let value = if row == "footer" { footer-tag(card) } else { card.at(row, default: none) }
  if not has(value) { return none }
  if row == "number" {
    txt(ctx, "number", d.number * rem * s * f, 1.2, str(value), fill: tone.solid.transparentize(50%))
  } else if row == "subtitle" {
    txt(ctx, "subtitle", body-size, 1.5, value, fill: th.muted)
  } else if row == "body" {
    // `.feature-copy` is `opacity-70`: exactly `muted` on light-notes, but the
    // raw ink at 70% on the glass themes (no override CSS applies there).
    txt(ctx, "body", body-size, 1.625, rich(value, emphasis: tone.text), fill: if is-glass(ctx) {
      th.ink.transparentize(30%)
    } else { th.muted })
  } else if row == "items" {
    stack(spacing: 0.5 * rem * f, ..value.map(item => {
      let item = if type(item) == str { (text: item) } else { item }
      let item-tone = tone-of(ctx, item.at("iconTone", default: none), default: card.at("tone", default: "amber"))
      let accent = item.at("accent", default: false) == true
      grid(
        columns: (auto, 1fr),
        column-gutter: 0.5 * rem * f,
        // `mt-0.5` keeps the icon on the first line's cap height.
        pad(top: 0.125 * rem * f + (body-size * 1.5 - d.icon * rem * s * f) / 2, icon(
          item.at("icon", default: "i-carbon:checkmark-filled"),
          item-tone.text,
          d.icon * rem * s * f,
        )),
        txt(ctx, "body", body-size, 1.5, item.text, fill: if accent { tone.text } else { th.ink }),
      )
    }))
  } else if row == "risk" {
    block(
      width: 100%,
      fill: tone.bg,
      // light-notes dashes the risk frame; the screen's inline style is solid.
      // Glass rounds it to a full pill (the template's `rounded-full`).
      stroke: if is-terminal(ctx) or is-glass(ctx) { 1 * px + tone.border } else {
        (paint: tone.border, thickness: 1 * px, dash: "dashed")
      },
      radius: if is-glass(ctx) { ctx.theme.radius.pill * px } else { 2 * px },
      inset: (x: 12 * px * f, y: 4 * px * f),
      align(center, txt(ctx, "body", body-size, 1.5, value, fill: tone.text)),
    )
  } else if row == "conclusion" {
    block(
      width: 100%,
      fill: tone.bg,
      // Terminal and glass: a uniform 1px tone border without the 3px left
      // accent; glass rounds it to 8px (the template's `rounded-lg`).
      stroke: if is-terminal(ctx) or is-glass(ctx) { 1 * px + tone.border } else {
        (left: 3 * px + tone.solid, rest: 1 * px + tone.border)
      },
      radius: if is-glass(ctx) { 8 * px } else { 2 * px },
      inset: (x: 12 * px * f, y: 8 * px * f),
      align(center, txt(ctx, "body", body-size, 1.5, "→ " + value, fill: tone.text)),
    )
  } else if row == "footer" {
    let ft = tone-of(ctx, card.at("footer_tone", default: card.at("footerTone", default: none)), default: card.at(
      "tone",
      default: "amber",
    ))
    tag-pill(ctx, value, tone: ft, size: body-size)
  }
}

#let header(ctx, card, d, f) = {
  let tone = tone-of(ctx, card.at("titleTone", default: card.at("tone", default: none)))
  let s = ctx.ty.meta / rem
  let size = d.head * rem * s * f
  let icon-name = card.at("icon", default: none)
  block(
    width: 100%,
    fill: tone.bg,
    // Terminal and glass `.feature-header`: only the 1px bottom tone border;
    // the template's `.feature-top-bar` has no styles outside light-notes.
    stroke: if is-terminal(ctx) or is-glass(ctx) { (bottom: 1 * px + tone.border) } else {
      (top: 3 * px + tone.solid, bottom: 1 * px + tone.border)
    },
    inset: (x: d.head-x * rem * f, y: d.head-y * rem * f),
    grid(
      columns: if has(icon-name) { (auto, 1fr) } else { (1fr,) },
      column-gutter: 0.5 * rem,
      align: horizon,
      ..if has(icon-name) { (icon(icon-name, tone.text, d.icon * rem * s * f),) },
      txt(ctx, "title", size, 1.5, card.title, fill: tone.text),
    ),
  )
}

// GlassCard's default `px-4 py-3`, around the header band and the body.
#let card-pad = (x: 16 * px, y: 12 * px)

// One visual row of cards with shared row heights.
#let card-row(ctx, cards, cols, col-w, d, f) = {
  let gap = d.gap * rem * f
  let pad = d.pad * rem * f
  let content-w = col-w - 2 * card-pad.x
  let inner-w = content-w - 2 * pad
  let rows = field-rows.filter(r => cards.any(c => field(ctx, c, r, d, f) != none))
  let head-h = calc.max(..cards.map(c => measure(block(width: content-w, header(ctx, c, d, f))).height))
  let heights = rows.map(r => calc.max(..cards.map(c => {
    let cell = field(ctx, c, r, d, f)
    if cell == none { 0pt } else { measure(block(width: inner-w, cell)).height }
  })))
  grid(
    columns: (col-w,) * cols,
    column-gutter: gap,
    ..cards.map(card => paper-card(ctx, radius: if is-terminal(ctx) or is-glass(ctx) {
      ctx.theme.radius.card * px
    } else { 3 * px }, inset: (x: card-pad.x, y: card-pad.y * f), clip: true, stack(
      block(height: head-h, width: 100%, fill: tone-of(ctx, card.at("tone", default: none)).bg, header(ctx, card, d, f)),
      block(inset: pad, width: 100%, stack(spacing: 0.75 * rem * f, ..rows
        .zip(heights)
        .map(((r, rh)) => block(height: rh, width: 100%, {
          let cell = field(ctx, card, r, d, f)
          if cell != none { cell }
        })))),
    ))),
  )
}

#let render(ctx, slide) = content-page(ctx, slide, subtitle-gap: 12, (w, h) => {
  let cards = slide.at("cards", default: ())
  let aspect = aspect-of(ctx)
  let max-cols = if aspect == "ratio_48x9" { 12 } else if aspect == "ratio_32x9" { 8 } else { 4 }
  let cols = calc.max(1, calc.min(max-cols, cards.len()))
  let d = if cols >= 5 { densities.mini } else if cols == 4 { densities.compact } else { densities.normal }
  if cards.len() == 0 { return }
  let note = slide.at("note", default: none)

  // Cards and note shrink together so the note never covers the grid.
  let build(f) = {
    let gap = d.gap * rem * f
    let col-w = (w - gap * (cols - 1)) / cols
    let chunks = range(0, cards.len(), step: cols).map(i => cards.slice(i, calc.min(i + cols, cards.len())))
    v(1 * rem * f)
    stack(spacing: gap, ..chunks.map(chunk => card-row(ctx, chunk, chunk.len(), col-w, d, f)))
    if has(note) {
      v(1 * rem * f)
      note-bar(ctx, "i-carbon:idea", note, f: f)
    }
  }

  fit-height(w, h, build, slide-index: ctx.index, center: true)
})
