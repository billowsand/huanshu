// compare — port of KeynoteCompareSlide.vue (side-by-side / pros-cons / versus).
//
// The Vue template shrinks fonts in a ResizeObserver loop after paint; here the
// same `--compare-fit` factor is chosen up front by measuring the whole block.

#import "../lib/components.typ": *

#let axis-width = 44

#let pair(left, mid, right) = grid(
  columns: (1fr, axis-width * px, 1fr),
  align: (auto, center + horizon, auto),
  left, mid, right,
)

#let clean(s) = if s == none { "" } else { str(s).replace(regex("\s"), "") }

#let render(ctx, slide) = {
  let data = slide.at("compare_data", default: none)
  let data = if data == none { (:) } else { data }
  let mode = data.at("mode", default: "side-by-side")
  let sides = (
    data.at("left", default: (title: "左侧", tone: "blue", items: ())),
    data.at("right", default: (title: "右侧", tone: "amber", items: ())),
  )
  let tones = sides.enumerate().map(((i, s)) => tone-of(ctx, s.at("tone", default: none), default: if i == 0 {
    "blue"
  } else { "amber" }))
  let n-rows = calc.max(..sides.map(s => s.at("items", default: ()).len()), 0)
  let footer = slide.at("footer", default: none)
  let footer = if clean(footer) != "" and not sides.any(s => clean(s.at("conclusion", default: none)) == clean(
    footer,
  )) { footer }

  // Density presets of `layoutStyle` in the Vue template.
  let copy = (if n-rows <= 2 { 1.35 } else if n-rows == 3 { 1.2 } else { 1.05 }) * rem
  let gap = (if n-rows >= 4 { 6 } else { 12 }) * px
  let pad-y = (if n-rows >= 5 { 2 } else if n-rows == 4 { 4 } else { 18 }) * px
  let desc-gap = (if n-rows >= 4 { 4 } else { 8 }) * px
  let leading = if n-rows >= 4 { 1.4 } else { 1.5 }
  let scale = ctx.ty.meta / rem

  content-page(ctx, slide, footer: footer, gap-after-header: 20, (w, h) => {
    let th = ctx.theme
    let build(f) = {
      let heading = pair(
        ..sides
          .enumerate()
          .map(((i, side)) => block(
            width: 100%,
            fill: tones.at(i).bg,
            // Terminal and glass `.cp-header`: a uniform 1px tone border, no
            // 3px top bar; glass rounds it to 10px.
            stroke: if is-terminal(ctx) or is-glass(ctx) { 1 * px + tones.at(i).border } else {
              (top: 3 * px + tones.at(i).solid, rest: 1 * px + tones.at(i).border)
            },
            radius: if is-glass(ctx) { 10 * px } else if is-terminal(ctx) {
              ctx.theme.radius.card * px
            } else { (top: 2 * px) },
            inset: (x: 18 * px, y: 14 * px * f),
            grid(
              columns: (auto, auto, 1fr),
              column-gutter: 10 * px,
              align: horizon,
              if has(side.at("icon", default: none)) { icon(side.icon, tones.at(i).text, ctx.ty.icon-lg) },
              if mode == "pros-cons" {
                box(stroke: 1 * px + tones.at(i).text, radius: if is-terminal(ctx) or is-glass(ctx) {
                  ctx.theme.radius.pill * px
                } else { 4 * px }, inset: (x: 6 * px, y: 2 * px), txt(
                  ctx,
                  "caption",
                  14 * px,
                  1.5,
                  if i == 0 { "优势" } else { "劣势" },
                  fill: tones.at(i).text,
                ))
              },
              txt(ctx, "title", copy * scale * f * 1.25, 1.25, side.title),
            ),
          ))
          // The side-by-side `↔` is an icon: few CJK fonts carry that glyph and
          // the fallback is often a color emoji font.
          .intersperse(if mode == "versus" {
            txt(ctx, "caption", 16 * px, 1.5, "VS", fill: th.muted)
          } else if mode == "pros-cons" {
            txt(ctx, "caption", 16 * px, 1.5, "±", fill: th.muted)
          } else { icon("i-carbon:arrows-horizontal", th.muted, 16 * px) }),
      )

      let item-cell(i, item, row) = {
        let tone = tones.at(i)
        if item == none {
          return block(width: 100%, height: 100%, align(center + horizon, text(fill: th.muted.transparentize(60%), "—")))
        }
        let highlight = item.at("highlight", default: false) == true
        let icon-name = if highlight { "i-carbon:star-filled" } else if mode == "pros-cons" and i == 1 {
          "i-carbon:close"
        } else { "i-carbon:checkmark-filled" }
        grid(
          columns: (auto, 1fr),
          column-gutter: 10 * px,
          pad(top: 5 * px, icon(icon-name, tone.text, ctx.ty.icon-sm)),
          {
            txt(ctx, "body", copy * scale * f * 1.12, 1.3, item.label)
            if has(item.at("desc", default: none)) {
              v(desc-gap)
              txt(ctx, "body", copy * scale * f, leading, item.desc, fill: th.ink.transparentize(24%))
            }
          },
        )
      }
      let item-box(i, item, row, height) = {
        let tone = tones.at(i)
        let highlight = item != none and item.at("highlight", default: false) == true
        block(
          width: 100%,
          height: height,
          // Non-highlighted `.cp-item` is transparent on the glass themes.
          fill: if highlight { tone.bg } else if is-glass(ctx) {
            none
          } else if calc.odd(row) and th.stripe != none { th.stripe } else { th.paper },
          // Terminal `.cp-item`: 1px borders, the left one in the solid tone.
          // Glass `.cp-item`: a uniform 1px tone border, rounded 10px.
          stroke: if is-glass(ctx) { 1 * px + tone.border } else if is-terminal(ctx) {
            (left: 1 * px + tone.solid, rest: 1 * px + tone.border)
          } else {
            (
              left: 3 * px + tone.solid,
              top: 1 * px + tone.border,
              right: 1 * px + tone.border,
              bottom: (paint: th.edge, thickness: 1 * px, dash: "dashed"),
            )
          },
          radius: if is-glass(ctx) { 10 * px } else { 0 * px },
          inset: (x: 20 * px, y: pad-y * f),
          item-cell(i, item, row),
        )
      }
      let row-index(row) = txt(ctx, "number", 12 * px, 1.5, if row < 9 { "0" } + str(row + 1), fill: th.muted)

      let rows = range(n-rows).map(row => {
        let items = sides.map(s => s.at("items", default: ()).at(row, default: none))
        // Both cells of a row share the taller height so pairs stay aligned.
        let row-h = calc.max(..items
          .enumerate()
          .map(((i, it)) => measure(block(width: (w - axis-width * px) / 2, item-box(i, it, row, auto))).height))
        pair(item-box(0, items.at(0), row, row-h), row-index(row), item-box(1, items.at(1), row, row-h))
      })

      let conclusions = if sides.any(s => has(s.at("conclusion", default: none))) {
        pair(..sides
          .enumerate()
          .map(((i, side)) => if has(side.at("conclusion", default: none)) {
            block(
              width: 100%,
              fill: tones.at(i).bg,
              stroke: 1 * px + tones.at(i).border,
              radius: (bottom: 2 * px),
              inset: (x: 18 * px, y: 12 * px),
              grid(
                columns: (auto, 1fr),
                column-gutter: 10 * px,
                align: horizon,
                icon("i-carbon:idea", tones.at(i).text, ctx.ty.icon-sm),
                txt(ctx, "body", calc.max(16 * px, ctx.ty.caption * f), 1.4, side.conclusion),
              ),
            )
          } else { [] })
          .intersperse([]))
      }

      stack(spacing: gap, heading, ..rows, ..if conclusions != none { (conclusions,) })
    }
    fit-height(w, h, build, slide-index: ctx.index)
  })
}
