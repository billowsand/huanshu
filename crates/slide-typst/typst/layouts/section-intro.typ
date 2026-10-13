// section_intro — port of KeynoteSectionIntroSlide.vue (+ light-notes stationery):
// a cocoa lead sheet (badge, title, sub-topic count, note) beside indexed cards.
//
// Card grid per count (16:9): ≤2 one column; 3 one column with index | text |
// icon rows; 4 two columns; 5 two on top, three below; 6 three columns.
// Wider ratios use 3 (32:9) or 4 (48:9) columns. Unlike the screen version,
// card titles are never cut off: everything shrinks together until it fits.

#import "../lib/components.typ": *

#let card-limits = (ratio_16x9: 6, ratio_32x9: 8, ratio_48x9: 12)

// Lead column share of the width (`gridCols` in the Vue template).
#let lead-shares = (ratio_16x9: 0.7 / 2.0, ratio_32x9: 0.25 / 1.75, ratio_48x9: 0.2 / 2.2)

// ── Lead sheet ────────────────────────────────────────────────────────────────

#let lead-parts(ctx, slide, count, f) = {
  let lead = ctx.theme.lead
  let ty = ctx.ty
  let terminal = is-terminal(ctx)
  let parts = ()
  let badge = slide.at("badge", default: none)
  let badge = if has(badge) { badge } else { "章节导览" }
  parts.push(label-bar(ctx, badge, colors: (bg: lead.badge-bg, border: lead.badge-border, text: lead.badge-ink)))
  parts.push({
    txt(ctx, "title", ty.title * f, 1.18, slide.title, fill: lead.ink, tracking: ctx.theme.tracking.title)
    v(0.85 * rem * f)
    // `.section-intro__divider` is a `.slide-title-divider` (160px wide); on
    // light-notes it keeps the lead sheet's soft tint instead of primary.
    divider-line(ctx, 10 * rem, height: 2 * px, faded: true, paint: lead.soft.transparentize(35%))
  })
  if has(slide.at("subtitle", default: none)) {
    // `max-width: 24rem`: never wider than the sheet itself.
    parts.push(layout(size => block(
      width: calc.min(24 * rem, size.width),
      // `.section-intro__subtitle` has no color of its own: it inherits the
      // lead sheet's ink (the glass themes) instead of the muted soft ink.
      txt(ctx, "subtitle", ty.note * f, 1.7, rich(slide.subtitle), fill: if is-glass(ctx) {
        lead.ink
      } else { lead.soft }),
    )))
  }
  parts.push({
    line(length: 100%, stroke: 1 * px + lead.rule)
    v(1 * rem * f)
    // Terminal overrides `.section-intro__meta-label` to the 0.06em label tracking.
    txt(ctx, "caption", ty.caption * f, 1.5, "本章关注", fill: lead.soft, tracking: if terminal {
      ctx.theme.tracking.label
    } else { 0.22em })
    v(0.45 * rem * f)
    // `.section-intro__meta-value` is `--success-light` outside light-notes.
    txt(ctx, "body", ty.body-strong * f, 1.5, str(count) + " 个子主题", fill: if terminal or is-glass(ctx) {
      ctx.theme.tones.green.text
    } else { lead.ink })
  })
  if has(slide.at("note", default: none)) {
    parts.push(block(
      stroke: (left: 3 * px + lead.note-rule),
      inset: (left: 0.85 * rem),
      // `.section-intro__note` keeps the plain ink color outside light-notes.
      txt(ctx, "body", ty.meta * f, 1.45, rich(slide.note, emphasis: lead.ink), fill: if terminal or is-glass(
        ctx,
      ) {
        ctx.theme.ink
      } else { lead.soft }),
    ))
  }
  parts
}

#let lead-sheet(ctx, slide, count, width, height) = {
  let lead = ctx.theme.lead
  let pad = 28 * px
  let inner-w = width - 2 * pad
  let inner-h = height - 2 * pad
  let min-gap = 16 * px
  let natural(f) = {
    let parts = lead-parts(ctx, slide, count, f)
    parts.map(p => measure(block(width: inner-w, p)).height).sum() + min-gap * (parts.len() - 1)
  }
  let f = fit-scale(natural, inner-h)
  if f == none {
    overflow-marker(ctx.index, natural(fit-min), inner-h)
    f = fit-min
  }
  block(
    width: width,
    height: height,
    fill: lead.fill,
    stroke: 1 * px + lead.border,
    // Terminal forces every card radius to 2px; glass uses its 12px cards.
    radius: if is-terminal(ctx) or is-glass(ctx) { ctx.theme.radius.card * px } else { 3 * px },
    inset: pad,
    clip: true,
    // `justify-content: space-between`: first part on top, last at the bottom.
    stack(..lead-parts(ctx, slide, count, f).intersperse(1fr)),
  )
}

// ── Cards ─────────────────────────────────────────────────────────────────────

#let card-index(ctx, tone, idx, size) = txt(
  ctx,
  "number",
  size,
  1.5,
  if idx < 9 { "0" } + str(idx + 1),
  // Terminal forces `.section-intro__card-index` to muted text at 0.06em. The
  // base style (glass themes) is the solid tone with no letter spacing.
  fill: if is-terminal(ctx) { ctx.theme.muted } else { tone.solid },
  tracking: if is-terminal(ctx) { ctx.theme.tracking.label } else if is-glass(ctx) { 0em } else { 0.14em },
)

#let card-text(ctx, card, compact, f) = {
  let ty = ctx.ty
  let th = ctx.theme
  let tone = tone-of(ctx, card.at("tone", default: none))
  let title-size = if compact { ty.body } else { ty.body-strong }
  let text-size = if compact { ty.caption } else { ty.meta }
  let title = txt(ctx, "title", title-size * f, 1.35, card.title)
  let body = if has(card.at("body", default: none)) {
    txt(ctx, "body", text-size * f, if compact { 1.5 } else { 1.7 }, rich(card.body, emphasis: tone.text), fill: th.ink.transparentize(30%))
  } else if has(card.at("items", default: none)) {
    let size = if compact { ty.caption } else { ty.body-tight }
    stack(spacing: 0.5 * rem * f, ..card.items.slice(0, calc.min(2, card.items.len())).map(item => txt(
      ctx,
      "body",
      size * f,
      if compact { 1.5 } else { 1.6 },
      item,
    )))
  }
  (title, body)
}

// Default card: index and icon on top, then title and text.
#let stacked-card(ctx, card, idx, compact, f) = {
  let tone = tone-of(ctx, card.at("tone", default: none))
  let icon-size = (if compact { 2.4 } else { 2.75 }) * rem * f
  let (title, body) = card-text(ctx, card, compact, f)
  stack(
    spacing: (if compact { 0.75 } else { 1 }) * rem * f,
    grid(
      columns: (1fr, icon-size),
      rows: icon-size,
      card-index(ctx, tone, idx, ctx.ty.meta * f),
      icon(card.at("icon", default: "i-carbon:arrow-right"), tone.solid, icon-size),
    ),
    title,
    ..if body != none { (body,) },
  )
}

// Three cards in one column: index | title over text | icon.
#let row-card(ctx, card, idx, f) = {
  let tone = tone-of(ctx, card.at("tone", default: none))
  let icon-size = 2.75 * rem * f
  let (title, body) = card-text(ctx, card, false, f)
  grid(
    columns: (2.25 * rem, 1fr, icon-size),
    column-gutter: 0.75 * rem,
    card-index(ctx, tone, idx, ctx.ty.meta * f),
    stack(spacing: 0.5 * rem * f, title, ..if body != none { (body,) }),
    icon(card.at("icon", default: "i-carbon:arrow-right"), tone.solid, icon-size),
  )
}

// Cell boxes: (card, index, x, y, width, height) in the card area.
#let card-cells(count, cols, width, height, gap) = {
  let rows-of(n, c) = calc.ceil(n / c)
  if cols == none and count == 5 {
    // Two wide cards on top, three below (`span 3` / `span 2` of 6 tracks).
    let row-h = (height - gap) / 2
    let top-w = (width - gap) / 2
    let bottom-w = (width - 2 * gap) / 3
    range(5).map(i => if i < 2 {
      (i * (top-w + gap), 0pt, top-w, row-h)
    } else {
      ((i - 2) * (bottom-w + gap), row-h + gap, bottom-w, row-h)
    })
  } else {
    let cols = if cols != none { cols } else if count <= 3 { 1 } else if count == 6 { 3 } else { 2 }
    let rows = rows-of(count, cols)
    let cell-w = (width - gap * (cols - 1)) / cols
    let cell-h = (height - gap * (rows - 1)) / rows
    range(count).map(i => (
      calc.rem(i, cols) * (cell-w + gap),
      calc.floor(i / cols) * (cell-h + gap),
      cell-w,
      cell-h,
    ))
  }
}

#let card-grid(ctx, cards, width, height, aspect) = {
  let count = cards.len()
  let standard = aspect == "ratio_16x9"
  let cols = if aspect == "ratio_48x9" { 4 } else if aspect == "ratio_32x9" { 3 } else { none }
  let compact = standard and count >= 5
  let gap = (if aspect == "ratio_48x9" { 1.4 } else if aspect == "ratio_32x9" { 1.2 } else { 1 }) * rem
  let pad = (if compact { 1.1 } else { 1.5 }) * rem
  let cells = card-cells(count, cols, width, height, gap)
  let content(card, idx, f) = if standard and count == 3 {
    row-card(ctx, card, idx, f)
  } else { stacked-card(ctx, card, idx, compact, f) }

  // One factor for every card keeps the grid visually even.
  let fits(f) = cards
    .enumerate()
    .all(((i, card)) => {
      let (_, _, w, h) = cells.at(i)
      measure(block(width: w - 2 * pad, content(card, i, f))).height <= h - 2 * pad - 3 * px
    })
  let f = fit-factor(fits)
  if f == none {
    let (_, _, w, h) = cells.first()
    overflow-marker(ctx.index, measure(block(width: w - 2 * pad, content(cards.first(), 0, fit-min))).height, h)
    f = fit-min
  }

  block(width: width, height: height, {
    for (i, card) in cards.enumerate() {
      let (x, y, w, h) = cells.at(i)
      let tone = tone-of(ctx, card.at("tone", default: none))
      place(top + left, dx: x, dy: y, block(
        width: w,
        height: h,
        fill: ctx.theme.paper,
        // Terminal and glass cards are plain panels: no 3px tone bar on top.
        stroke: if is-terminal(ctx) or is-glass(ctx) { 1 * px + ctx.theme.edge } else {
          (top: 3 * px + tone.solid, rest: 1 * px + ctx.theme.edge)
        },
        radius: if is-glass(ctx) { ctx.theme.radius.card * px } else { 2 * px },
        inset: pad,
        clip: true,
        content(card, i, f),
      ))
    }
  })
}

// ── Page ──────────────────────────────────────────────────────────────────────

#let render(ctx, slide) = layout(size => {
  let aspect = aspect-of(ctx)
  let cards = slide.at("cards", default: ())
  let cards = if cards == none { () } else { cards }
  let cards = cards.slice(0, calc.min(cards.len(), card-limits.at(aspect)))
  // This layout has no `py-4`: it spans the frame minus a 12px band.
  let height = size.height + 8 * px
  let gap = 1.25 * rem
  let lead-w = (size.width - gap) * lead-shares.at(aspect)

  section-num(ctx, slide.at("section", default: none))
  // Placed, not flowed: the 4px reach above and below the content area would
  // otherwise push the page past its bottom margin onto a new page.
  place(top + left, dy: -4 * px, grid(
    columns: (lead-w, size.width - gap - lead-w),
    column-gutter: gap,
    lead-sheet(ctx, slide, cards.len(), lead-w, height),
    if cards.len() > 0 { card-grid(ctx, cards, size.width - gap - lead-w, height, aspect) },
  ))
})
