// timeline — port of KeynoteTimelineSlide.vue (+ light-notes rivets and date tags):
// events spread evenly along a horizontal axis, cards alternating above (even)
// and below (odd), each joined to its node by a short stem, the date opposite.
//
// Unlike the screen version, every node sits exactly on the axis: the bands
// above and below are sized from the tallest card on each side, so tall cards
// no longer push their node off the line.

#import "../lib/components.typ": *

#let node-r = 8 // `.tl-node`: 1rem dot
#let stem = 20 // `.tl-stem`: 1.25rem
#let card-gap = 12 // `pb-3` / `pt-3` between card and stem
#let date-gap = 4 // `pt-1` / `pb-1` between stem and date

#let event-tone(ctx, event) = tone-of(ctx, event.at("tone", default: none))

#let event-card(ctx, event, below, width, f) = {
  let th = ctx.theme
  let tone = event-tone(ctx, event)
  // The 2px rule faces away from the axis: tone border on light-notes, the
  // solid tone elsewhere (those themes tint the card fill as well). Glass
  // rounds the card to 10px.
  let strong-side = if is-terminal(ctx) or is-glass(ctx) { 2 * px + tone.solid } else { 2 * px + tone.border }
  let stroke = (rest: 1 * px + tone.border)
  stroke.insert(if below { "bottom" } else { "top" }, strong-side)
  block(
    width: width,
    fill: if is-terminal(ctx) or is-glass(ctx) { tone.bg } else { th.paper },
    stroke: stroke,
    radius: if is-glass(ctx) { 10 * px } else if is-terminal(ctx) { th.radius.card * px } else { 4 * px },
    inset: (x: 12 * px, y: 8 * px * f),
    {
      if has(event.at("icon", default: none)) {
        icon(event.icon, tone.text, ctx.ty.icon-md * f)
        v(4 * px * f)
      }
      txt(ctx, "title", ctx.ty.card-title * f, 1.375, event.at("title", default: ""), fill: tone.text)
      if has(event.at("body", default: none)) {
        v(4 * px * f)
        txt(ctx, "body", ctx.ty.caption * f, 1.375, rich(event.body, emphasis: tone.text), fill: th.ink.transparentize(40%))
      }
    },
  )
}

#let date-label(ctx, event, width, f) = {
  let tone = event-tone(ctx, event)
  // `.tl-date` on the glass themes: plain ink at 50% opacity (the template's
  // `opacity-50`), no underline.
  if is-glass(ctx) {
    return block(width: width, align(center, txt(
      ctx,
      "number",
      ctx.ty.caption * f,
      1.5,
      str(event.at("date", default: "")),
      fill: ctx.theme.ink.transparentize(50%),
      tracking: 0.04em,
    )))
  }
  block(width: width, align(center, txt(
    ctx,
    "number",
    ctx.ty.caption * f,
    1.5,
    underline(stroke: (paint: tone.text, thickness: 1 * px, dash: "dotted"), offset: 0.3em, str(event.at(
      "date",
      default: "",
    ))),
    fill: tone.text,
    tracking: 0.04em,
  )))
}

// Geometry for fit factor `f`: band heights above/below the axis.
#let measure-bands(ctx, events, col-w, f) = {
  let card-w = col-w - 16 * px
  let heights = events.enumerate().map(((i, e)) => measure(event-card(ctx, e, calc.odd(i), card-w, f)).height)
  let date-h = calc.max(..events.map(e => measure(date-label(ctx, e, col-w, f)).height))
  let tallest(odd) = calc.max(0pt, ..heights.enumerate().filter(((i, _)) => calc.odd(i) == odd).map(((_, h)) => h))
  let date-band = (node-r + stem + date-gap) * px + date-h
  let card-band(odd) = (node-r + stem + card-gap) * px + tallest(odd)
  (
    heights: heights,
    date-h: date-h,
    up: calc.max(card-band(false), date-band),
    down: if events.len() > 1 { calc.max(card-band(true), date-band) } else { date-band },
  )
}

#let axis(ctx, events, width, f) = {
  let th = ctx.theme
  let n = events.len()
  let col-w = width / n
  let card-w = col-w - 16 * px
  let bands = measure-bands(ctx, events, col-w, f)
  let up = bands.up
  block(width: width, height: up + bands.down, {
    // `.tl-axis-line`: a faint printed rule on light-notes; the terminal CSS
    // forces it to the solid border color; on the glass themes it is a
    // transparent→edge→transparent gradient at 50% opacity.
    if is-glass(ctx) {
      let fade = th.edge.transparentize(50%)
      place(top + left, dy: up - 1 * px, rect(width: width, height: 2 * px, radius: 2 * px, fill: gradient.linear(
        (fade.transparentize(100%), 0%),
        (fade, 15%),
        (fade, 85%),
        (fade.transparentize(100%), 100%),
      )))
    } else {
      place(top + left, dy: up - 1 * px, line(length: width, stroke: 2 * px + if is-terminal(ctx) {
        th.edge
      } else { th.edge.transparentize(50%) }))
    }
    for (i, event) in events.enumerate() {
      let tone = event-tone(ctx, event)
      let cx = col-w * (i + 0.5)
      let below = calc.odd(i)
      // `.tl-stem`: on the glass themes the template tints the stem with the
      // solid tone — 44% on the card side, 19% on the date side.
      let stem-stroke(strong) = 2 * px + if is-glass(ctx) {
        tone.solid.transparentize(if strong { 56% } else { 81% })
      } else { tone.border }
      let stem-line(from, to, strong: true) = place(top + left, dx: cx - 1 * px, dy: from, line(
        angle: 90deg,
        length: to - from,
        stroke: stem-stroke(strong),
      ))
      let r = node-r * px
      // Above the axis is the card side for even events, the date side for
      // odd ones (cards alternate around the axis).
      stem-line(up - r - stem * px, up - r, strong: not below)
      stem-line(up + r, up + r + stem * px, strong: below)
      let card = event-card(ctx, event, below, card-w, f)
      let date = date-label(ctx, event, col-w, f)
      let reach = (node-r + stem) * px
      if below {
        place(top + left, dx: cx - card-w / 2, dy: up + reach + card-gap * px, card)
        place(top + left, dx: cx - col-w / 2, dy: up - reach - date-gap * px - bands.date-h, date)
      } else {
        // Cards above hang from the stem, bottom-aligned like `justify-end`.
        place(top + left, dx: cx - card-w / 2, dy: up - reach - card-gap * px - bands.heights.at(i), card)
        place(top + left, dx: cx - col-w / 2, dy: up + reach + date-gap * px, date)
      }
      // `.tl-node`: a solid dot ringed by paper and a hairline (box-shadow
      // rings) on light-notes; a flat 16px square on the terminal themes; a
      // plain solid circle with a 2px tone ring on the glass themes.
      if is-terminal(ctx) {
        place(top + left, dx: cx - r, dy: up - r, rect(
          width: 2 * r,
          height: 2 * r,
          fill: tone.solid,
          stroke: 2 * px + tone.border,
          radius: 2 * px,
        ))
      } else if is-glass(ctx) {
        place(top + left, dx: cx - r, dy: up - r, circle(radius: r, fill: tone.solid, stroke: 2 * px + tone.border))
      } else {
        place(top + left, dx: cx - r - 4 * px, dy: up - r - 4 * px, circle(
          radius: r + 4 * px,
          fill: th.paper,
          stroke: 1 * px + tone.border,
        ))
        place(top + left, dx: cx - r, dy: up - r, circle(radius: r - 1 * px, fill: tone.solid, stroke: 2 * px + tone.border))
      }
    }
  })
}

#let render(ctx, slide) = content-page(ctx, slide, footer: slide.at("footer", default: none), footer-gap: 12, (width, avail) => {
  let events = slide.at("timeline_events", default: ())
  let events = if events == none { () } else { events }
  if events.len() == 0 { return }
  let col-w = width / events.len()
  let total(f) = {
    let b = measure-bands(ctx, events, col-w, f)
    b.up + b.down
  }
  let f = fit-factor(f => total(f) <= avail)
  if f == none {
    overflow-marker(ctx.index, total(fit-min), avail)
    f = fit-min
  }
  center-v(width, avail, axis(ctx, events, width, f))
})
