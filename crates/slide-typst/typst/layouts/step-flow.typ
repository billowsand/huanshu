// step_flow — port of KeynoteStepFlowSlide.vue (horizontal and vertical).

#import "../lib/components.typ": *

#let step-tone(ctx, step) = tone-of(ctx, step.at("tone", default: none), default: "blue")

#let horizontal(ctx, steps, w, h) = {
  let th = ctx.theme
  let ty = ctx.ty
  let n = calc.max(steps.len(), 1)
  let col-w = w / n
  let inner-w = col-w - 1 * rem
  let node-h = 5 * rem

  // `.sf-hcard`: a ruled paper card (GlassCard `px-3 py-2`) whose title and
  // body rows line up across steps, like the template's subgrid.
  let card-pad = (x: 12 * px, y: 8 * px)
  let text-w = inner-w - 2 * card-pad.x
  let build(f) = {
    let title(step) = txt(ctx, "title", ty.card-title * f, 1.375, step.title, fill: step-tone(ctx, step).text)
    let body(step) = if has(step.at("body", default: none)) {
      v(0.25 * rem)
      txt(ctx, "body", ty.caption * f, 1.625, step.body, fill: th.ink.transparentize(40%))
    }
    let title-h = calc.max(..steps.map(s => measure(block(width: text-w, title(s))).height))
    let body-h = calc.max(0pt, ..steps.map(s => measure(block(width: text-w, body(s))).height))
    let track-x = col-w / 2
    block(width: w, {
      // `.sf-track`: hairline joining the node centers, drawn under the nodes.
      // Terminal sets it to the solid border color (`opacity: 1`); the glass
      // themes draw a transparent→edge→transparent gradient at 45% opacity.
      if n > 1 {
        let track = if is-glass(ctx) {
          let fade = th.edge.transparentize(55%)
          rect(width: w - 2 * track-x, height: 2 * px, radius: 999 * px, fill: gradient.linear(
            (fade.transparentize(100%), 0%),
            (fade, 20%),
            (fade, 80%),
            (fade.transparentize(100%), 100%),
          ))
        } else {
          line(length: w - 2 * track-x, stroke: 2 * px + if is-terminal(ctx) {
            th.edge
          } else { th.edge.transparentize(55%) })
        }
        place(top + left, dx: track-x, dy: 2.24 * rem, track)
      }
      grid(
        columns: (col-w,) * n,
        ..steps
          .enumerate()
          .map(((i, step)) => {
            let tone = step-tone(ctx, step)
            pad(x: 0.5 * rem, {
              block(
                width: 100%,
                height: node-h,
                fill: tone.bg,
                stroke: 1 * px + tone.border,
                // `.sf-hnode`: 1rem pill; terminal forces every radius to 2px.
                radius: if is-glass(ctx) { 16 * px } else if is-terminal(ctx) { 2 * px } else { 3 * px },
                {
                  align(center + horizon, icon(step.at("icon", default: "i-carbon:arrow-right"), tone.text, ty.icon-hero))
                  // `.sf-num`: stamped index tab over the node's top-left corner.
                  // On the glass themes it is a borderless circle with a
                  // hardcoded rgba(0,0,0,0.7) digit.
                  place(top + left, dx: -0.35 * rem, dy: -0.35 * rem, box(
                    width: 1.9 * rem,
                    height: 1.9 * rem,
                    fill: tone.solid,
                    stroke: if is-glass(ctx) { none } else { 1 * px + th.paper },
                    radius: if is-glass(ctx) { 999 * px } else { 2 * px },
                    align(center + horizon, text(
                      font: ctx.fonts.number,
                      size: ty.caption,
                      fill: if is-glass(ctx) { rgb(0, 0, 0, 178) } else { tone.contrast },
                      str(i + 1),
                    )),
                  ))
                },
              )
              v(1 * rem)
              block(
                width: 100%,
                // `.sf-hcard`: paper on light-notes; tone-tinted elsewhere.
                fill: if is-terminal(ctx) or is-glass(ctx) { tone.bg } else { th.paper },
                stroke: 1 * px + tone.border,
                radius: if is-glass(ctx) { th.radius.card * px } else { 2 * px },
                inset: (x: card-pad.x, y: card-pad.y * f),
                align(center, {
                  block(height: title-h, title(step))
                  block(height: body-h, body(step))
                }),
              )
            })
          }),
      )
    })
  }
  fit-height(w, h, build, slide-index: ctx.index, center: true)
}

#let vertical(ctx, steps, w, h) = {
  let th = ctx.theme
  let ty = ctx.ty
  let build(f) = stack(..steps
    .enumerate()
    .map(((i, step)) => {
      let tone = step-tone(ctx, step)
      let last = i == steps.len() - 1
      let card = paper-card(ctx, fill: tone.bg, stroke: 1 * px + tone.border, inset: (x: 1 * rem, y: 0.75 * rem * f), {
        let icon-name = step.at("icon", default: none)
        grid(
          columns: if has(icon-name) { (auto, 1fr) } else { (1fr,) },
          column-gutter: 0.5 * rem,
          align: horizon,
          ..if has(icon-name) { (icon(icon-name, tone.text, ty.icon-md * f),) },
          txt(ctx, "title", ty.card-title * f, 1.375, step.title, fill: tone.text),
        )
        if has(step.at("body", default: none)) {
          txt(ctx, "body", ty.caption * f, 1.625, step.body, fill: th.ink.transparentize(30%))
        }
      })
      let card-h = measure(block(width: w - 2.5 * rem - 1 * rem, card)).height
      let gap = if last { 0pt } else { 1 * rem * f }
      grid(
        columns: (2.5 * rem, 1fr),
        column-gutter: 1 * rem,
        align: top,
        {
          // Terminal flattens `rounded-full` nodes into 2px squares.
          let node-body = align(center + horizon, text(font: ctx.fonts.number, size: ty.caption, fill: tone.text, str(
            i + 1,
          )))
          let node = if is-terminal(ctx) {
            box(width: 2.25 * rem, height: 2.25 * rem, fill: tone.bg, stroke: 1 * px + tone.border, radius: 2 * px, node-body)
          } else {
            circle(radius: 1.125 * rem, fill: tone.bg, stroke: 1 * px + tone.border, node-body)
          }
          align(center, node)
          if not last {
            align(center, line(
              angle: 90deg,
              length: calc.max(14 * px, card-h + gap - 2.25 * rem - 0.5 * rem),
              stroke: 2 * px + th.edge,
            ))
          }
        },
        block(below: gap, card),
      )
    }))
  fit-height(w, h, build, slide-index: ctx.index, center: true)
}

#let render(ctx, slide) = content-page(ctx, slide, footer: slide.at("footer", default: none), (w, h) => {
  let steps = slide.at("steps", default: ())
  if steps.len() == 0 { return }
  if slide.at("direction", default: "horizontal") == "vertical" {
    vertical(ctx, steps, w, h)
  } else {
    horizontal(ctx, steps, w, h)
  }
})
