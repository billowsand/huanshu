// issue_stack — port of KeynoteIssueStackSlide.vue (+ light-notes "filing strips"):
// equal-height problem cards, each a toned heading over a summary and keyword
// pills. From four cards on, each card turns into a row: heading left (22%),
// content right (78%).

#import "../lib/components.typ": *

#let issue-tone(ctx, card) = tone-of(ctx, card.at("tone", default: none), default: "blue")

#let heading(ctx, card, f) = {
  let tone = issue-tone(ctx, card)
  let icon-name = card.at("icon", default: none)
  grid(
    columns: if has(icon-name) { (auto, 1fr) } else { (1fr,) },
    column-gutter: 0.55 * rem,
    align: horizon,
    ..if has(icon-name) { (icon(icon-name, tone.text, ctx.ty.icon-sm * f),) },
    txt(ctx, "title", ctx.ty.body * 1.25 * f, 1.3, card.title, fill: tone.text),
  )
}

#let pill(ctx, tone, value, f) = box(
  fill: tone.bg,
  // light-notes makes the pills dashed; every other theme keeps them solid,
  // at the theme's pill radius (999px on the glass themes).
  stroke: if is-terminal(ctx) or is-glass(ctx) { 1 * px + tone.border } else {
    (paint: tone.border, thickness: 1 * px, dash: "dashed")
  },
  radius: if is-terminal(ctx) or is-glass(ctx) { ctx.theme.radius.pill * px } else { 4 * px },
  inset: (x: 0.65 * rem * f, y: 0.16 * rem * f),
  // The screen's `.issue-card__pill` has `opacity: 0.88` outside light-notes.
  txt(ctx, "caption", ctx.ty.caption * f, 1.2, value, fill: if is-terminal(ctx) or is-glass(ctx) {
    tone.text.transparentize(12%)
  } else { tone.text }),
)

#let details(ctx, card, rows-mode, f) = {
  let tone = issue-tone(ctx, card)
  let parts = ()
  if has(card.at("body", default: none)) {
    parts.push(txt(ctx, "body", ctx.ty.body * f, if rows-mode { 1.35 } else { 1.4 }, rich(card.body, emphasis: tone.text)))
  }
  let items = card.at("items", default: ())
  if has(items) {
    let gap = (if rows-mode { 0.35 } else { 0.45 }) * rem * f
    // Inline boxes wrap like the screen's `flex-wrap`; leading is the row gap.
    parts.push({
      set par(leading: gap)
      items.map(item => pill(ctx, tone, item, f)).join(h(0.45 * rem * f))
    })
  }
  stack(spacing: 0.4 * rem * f, ..parts)
}

// Natural (unconstrained) height of one card's content.
#let natural-height(ctx, card, rows-mode, width, f) = {
  if rows-mode {
    let head-w = width * 0.22
    calc.max(
      measure(block(width: head-w - 2 * rem, heading(ctx, card, f))).height + 2 * 0.8 * rem * f,
      measure(block(width: width - head-w - 2.2 * rem, details(ctx, card, true, f))).height + 2 * 0.65 * rem * f,
    )
  } else {
    // Parenthesized: a line starting with `+` would be a new statement in a code block.
    (
      measure(block(width: width - 2.4 * rem, heading(ctx, card, f))).height
        + 2 * 0.4 * rem * f
        + measure(block(width: width - 2.5 * rem, details(ctx, card, false, f))).height
        + 2 * 0.55 * rem * f
    )
  }
}

#let issue-card(ctx, card, rows-mode, width, height, f) = {
  let tone = issue-tone(ctx, card)
  let frame(body) = block(
    width: width,
    height: height,
    fill: ctx.theme.paper,
    // Terminal and glass `.issue-card`: a plain panel without the 3px tone bar.
    stroke: if is-terminal(ctx) or is-glass(ctx) { 1 * px + ctx.theme.edge } else {
      (left: 3 * px + tone.solid, rest: 1 * px + ctx.theme.edge)
    },
    radius: if is-glass(ctx) { ctx.theme.radius.card * px } else { 2 * px },
    clip: true,
    body,
  )
  if rows-mode {
    let head-w = width * 0.22
    frame(grid(
      columns: (head-w, 1fr),
      rows: height,
      block(
        width: 100%,
        height: 100%,
        fill: tone.bg,
        stroke: (right: 1 * px + tone.border),
        inset: (x: 1 * rem, y: 0.8 * rem * f),
        center-v(head-w - 2 * rem, height - 1.6 * rem * f, heading(ctx, card, f)),
      ),
      block(
        width: 100%,
        height: 100%,
        inset: (x: 1.1 * rem, y: 0.65 * rem * f),
        center-v(width - head-w - 2.2 * rem, height - 1.3 * rem * f, details(ctx, card, true, f)),
      ),
    ))
  } else {
    let head = block(
      width: 100%,
      fill: tone.bg,
      stroke: (bottom: 1 * px + tone.border),
      inset: (x: 1.2 * rem, y: 0.4 * rem * f),
      heading(ctx, card, f),
    )
    let head-h = measure(block(width: width, head)).height
    frame({
      head
      block(
        width: 100%,
        height: height - head-h,
        inset: (x: 1.25 * rem, y: 0.55 * rem * f),
        center-v(width - 2.5 * rem, height - head-h - 1.1 * rem * f, details(ctx, card, false, f)),
      )
    })
  }
}

#let render(ctx, slide) = content-page(ctx, slide, subtitle-gap: 12, gap-after-header: 20, (width, avail) => {
  let cards = slide.at("cards", default: ())
  let cards = if cards == none { () } else { cards }
  if cards.len() == 0 { return }
  let rows-mode = cards.len() >= 4
  let footer = slide.at("footer", default: none)
  // `.issue-footer`: left-aligned under the stack, unlike the centered footnote.
  let footer-block = if has(footer) {
    v(0.85 * rem)
    txt(ctx, "caption", ctx.ty.caption, 1.45, rich(footer))
  }
  let footer-h = if footer-block == none { 0pt } else { measure(block(width: width, footer-block)).height }
  let gap = 0.75 * rem
  let card-h = (avail - footer-h - gap * (cards.len() - 1)) / cards.len()

  let tallest(f) = calc.max(..cards.map(card => natural-height(ctx, card, rows-mode, width, f)))
  let f = fit-factor(f => tallest(f) <= card-h)
  if f == none {
    overflow-marker(ctx.index, tallest(fit-min), card-h)
    f = fit-min
  }
  stack(spacing: gap, ..cards.map(card => issue-card(ctx, card, rows-mode, width, card-h, f)))
  footer-block
})
