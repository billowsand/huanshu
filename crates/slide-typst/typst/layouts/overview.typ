// overview — port of KeynoteOverviewSlide.vue: numbered chapter cards on a
// twelve-track grid (every row divides evenly, so no row has empty cells) and
// an optional note. One fit factor for all cards, as the screen version does.

#import "../lib/components.typ": *

// Cards per row (`rows` in the Vue template).
#let row-plan(n, long) = {
  if n <= 2 { return (calc.max(n, 1),) }
  if long {
    if n == 3 { return (1, 1, 1) }
    return range(calc.ceil(n / 2)).map(i => calc.min(2, n - i * 2))
  }
  let presets = ("3": (3,), "4": (2, 2), "5": (2, 3), "6": (3, 3), "7": (3, 4), "8": (4, 4))
  presets.at(str(n), default: range(calc.ceil(n / 3)).map(i => calc.min(3, n - i * 3)))
}

#let card-body(ctx, item, stacked, body-size, f) = {
  let th = ctx.theme
  let scale = ctx.ty.meta / rem
  // light-notes underlines the number in primary; every other theme renders
  // the base style: ink at 55% opacity, no underline.
  let number = text(
    font: ctx.fonts.number,
    size: 2.2 * rem * scale * f,
    fill: if is-terminal(ctx) or is-glass(ctx) { th.ink.transparentize(45%) } else { th.primary },
    top-edge: "cap-height",
    bottom-edge: "baseline",
    if is-terminal(ctx) or is-glass(ctx) { str(item.at("number", default: "")) } else {
      underline(stroke: 1 * px + th.edge, offset: 0.2em, str(item.at("number", default: "")))
    },
  )
  let copy = {
    txt(ctx, "title", body-size * scale * f * 1.25, 1.3, item.at("title", default: ""))
    if has(item.at("desc", default: none)) {
      v(body-size * scale * f * 1.25 * 0.5) // the title's `margin-bottom: 0.5em`
      txt(ctx, "body", body-size * scale * f, 1.45, item.desc, fill: th.muted)
    }
  }
  if stacked {
    stack(spacing: 1.2 * rem * f, number, copy)
  } else {
    grid(columns: (auto, 1fr), column-gutter: 1.2 * rem * f, align: horizon, number, copy)
  }
}

#let render(ctx, slide) = {
  let slide = if has(slide.at("section", default: none)) { slide } else { slide + (section: "00") }
  content-page(ctx, slide, (width, avail) => {
    let th = ctx.theme
    let items = slide.at("overview_items", default: ())
    let items = if items == none { () } else { items }
    if items.len() == 0 { return }
    let n = items.len()
    let aspect = aspect-of(ctx)
    let wf = if aspect == "ratio_48x9" { 3 } else if aspect == "ratio_32x9" { 2 } else { 1 }
    let long = items.any(it => (
      str(it.at("title", default: "")).clusters().len() > 22 * wf
        or str(it.at("desc", default: "")).clusters().len() > 56 * wf
    ))
    let rows = row-plan(n, long)
    let stacked = not long and n >= 3
    let body-size = (if n <= 4 { 1.2 } else { 1.05 }) * rem

    let note = slide.at("note", default: none)
    let note-block = if has(note) {
      block(
        width: 100%,
        fill: th.sheet-tint,
        // light-notes dashes the note border; terminal and glass keep the
        // plain panel.
        stroke: if is-terminal(ctx) or is-glass(ctx) { 1 * px + th.edge } else {
          (paint: th.edge, thickness: 1 * px, dash: "dashed")
        },
        radius: if is-terminal(ctx) or is-glass(ctx) { th.radius.sheet * px } else { 4 * px },
        inset: (x: 20 * px, y: 12 * px),
        grid(
          columns: (auto, 1fr),
          column-gutter: 12 * px,
          align: horizon,
          icon("i-carbon:idea", th.primary, ctx.ty.icon-xl),
          txt(ctx, "body", ctx.ty.note, 1.5, rich(note, emphasis: th.primary), fill: th.ink.transparentize(20%)),
        ),
      )
    }
    let note-h = if note-block == none { 0pt } else { measure(block(width: width, note-block)).height + 1 * rem }
    let grid-h = avail - note-h

    // Cell geometry for a fit factor (the gap and padding shrink with it).
    let cells(f) = {
      let gap = 1 * rem * f
      let row-h = (grid-h - gap * (rows.len() - 1)) / rows.len()
      let out = ()
      let idx = 0
      for (r, cols) in rows.enumerate() {
        let cell-w = (width - gap * (cols - 1)) / cols
        for c in range(cols) {
          out.push((idx, c * (cell-w + gap), r * (row-h + gap), cell-w, row-h))
          idx += 1
        }
      }
      out
    }
    let card-pad(f) = 1.35 * rem * f
    // (needed, available) content height of the tightest card.
    let tightest(f) = cells(f)
      .map(((i, _, _, w, h)) => (
        measure(block(width: w - 2 * card-pad(f) - 6 * px, card-body(ctx, items.at(i), stacked, body-size, f))).height,
        h - 2 * card-pad(f),
      ))
      .sorted(key: ((need, have)) => have - need)
      .first()
    let f = fit-factor(f => { let (need, have) = tightest(f); need <= have })
    if f == none {
      let (need, have) = tightest(fit-min)
      overflow-marker(ctx.index, need, have)
      f = fit-min
    }

    block(width: width, height: grid-h, {
      for (i, x, y, w, h) in cells(f) {
        let body = card-body(ctx, items.at(i), stacked, body-size, f)
        let inner-w = w - 2 * card-pad(f) - 6 * px
        let content-h = measure(block(width: inner-w, body)).height
        place(top + left, dx: x, dy: y, block(
          width: w,
          height: h,
          // `.overview-card`: paper sheet with a 6px tinted margin band on
          // light-notes; a plain panel with even padding elsewhere.
          fill: if is-terminal(ctx) or is-glass(ctx) { th.paper } else {
            gradient.linear((th.sheet-tint, 0%), (th.sheet-tint, 6 * px / w * 100%), (th.paper, 6 * px / w * 100%), (th.paper, 100%))
          },
          stroke: 1 * px + th.edge,
          radius: if is-glass(ctx) { th.radius.card * px } else { 2 * px },
          inset: if is-terminal(ctx) or is-glass(ctx) { card-pad(f) } else {
            (left: card-pad(f) + 6 * px, right: card-pad(f), y: card-pad(f))
          },
          clip: true,
          { v(calc.max(0pt, (h - 2 * card-pad(f) - content-h) / 2)); body },
        ))
      }
    })
    if note-block != none {
      v(1 * rem)
      note-block
    }
  })
}
