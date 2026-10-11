// process — port of KeynoteProcessSlide.vue (+ light-notes phase tabs and
// ruled checklists): one column per phase — a toned head (chip, icon, title),
// a numbered step list that takes the remaining height, and a highlight.
//
// The three density presets (normal / compact / dense) are chosen from the
// content exactly as the template does; the fit factor then shrinks text if a
// column still does not fit.

#import "../lib/components.typ": *

#let presets = (
  normal: (
    gap: 1, col-gap: 0.75, head-min: 106, title-mt: 0.7, title-k: 1, title-lh: 1.15,
    wrap-pad: (0.95, 0.85), wrap-title-mb: 0.65, row-pad: 0.5, row-gap: 0.7, dot: 1.5, dot-k: 1,
    desc-mt: 0.22, desc-lh: 1.5, desc-k: 1, hl-min: 58, hl-lh: 1.4,
  ),
  compact: (
    gap: 0.82, col-gap: 0.62, head-min: 96, title-mt: 0.58, title-k: 0.96, title-lh: 1.15,
    wrap-pad: (0.82, 0.72), wrap-title-mb: 0.65, row-pad: 0.42, row-gap: 0.58, dot: 1.5, dot-k: 1,
    desc-mt: 0.18, desc-lh: 1.38, desc-k: 1, hl-min: 52, hl-lh: 1.4,
  ),
  dense: (
    gap: 0.68, col-gap: 0.5, head-min: 88, title-mt: 0.48, title-k: 0.9, title-lh: 1.12,
    wrap-pad: (0.72, 0.62), wrap-title-mb: 0.48, row-pad: 0.34, row-gap: 0.5, dot: 1.4, dot-k: 0.94,
    desc-mt: 0.14, desc-lh: 1.3, desc-k: 0.94, hl-min: 46, hl-lh: 1.3,
  ),
)

#let density(phases) = {
  let steps = phases.map(p => p.at("steps", default: ())).map(s => if s == none { () } else { s })
  let max-steps = calc.max(0, ..steps.map(s => s.len()))
  let max-desc = calc.max(0, ..steps.flatten().map(s => str(s.at("desc", default: "") + "").trim().clusters().len()))
  if max-steps >= 4 or max-desc >= 42 { presets.dense } else if max-steps >= 3 or max-desc >= 24 {
    presets.compact
  } else { presets.normal }
}

#let phase-tone(ctx, phase) = tone-of(ctx, phase.at("tone", default: none))

#let head(ctx, phase, d, width, f) = {
  let th = ctx.theme
  let tone = phase-tone(ctx, phase)
  let content = {
    grid(
      columns: (auto, 1fr),
      align: (left + horizon, right + horizon),
      box(
        fill: tone.solid,
        stroke: 1 * px + tone.border,
        radius: 2 * px,
        inset: (x: 0.7 * rem * f, y: 0.32 * rem * f),
        txt(ctx, "number", ctx.ty.meta * f, 1.5, str(phase.at("phase", default: "")), fill: tone.contrast, tracking: 0.01em),
      ),
      if has(phase.at("icon", default: none)) { icon(phase.icon, tone.text.transparentize(15%), ctx.ty.body * f) },
    )
    v(d.title-mt * rem * f)
    txt(ctx, "title", ctx.ty.card-title * d.title-k * f, d.title-lh, phase.at("title", default: ""), fill: tone.text)
    if has(phase.at("subtitle", default: none)) {
      v(0.4 * rem * f)
      block(width: 80%, txt(ctx, "subtitle", ctx.ty.caption * f, 1.45, phase.subtitle, fill: th.muted))
    }
  }
  let inner = (x: 16 * px, y: 12 * px * f)
  let natural = measure(block(width: width - 2 * inner.x, content)).height + 2 * inner.y + 2 * px
  block(
    width: width,
    height: calc.max(natural, d.head-min * px * f),
    fill: tone.bg,
    stroke: 1 * px + tone.border,
    radius: (top: 3 * px),
    inset: inner,
    content,
  )
}

#let highlight(ctx, phase, d, width, f) = {
  if not has(phase.at("highlight", default: none)) { return none }
  let tone = phase-tone(ctx, phase)
  let inner = (x: 16 * px, y: 12 * px * f)
  let content = grid(
    columns: (auto, 1fr),
    column-gutter: 0.65 * rem,
    align: horizon,
    icon("i-carbon:checkmark-filled", tone.text, ctx.ty.body * f),
    txt(ctx, "body", ctx.ty.body-tight * f, d.hl-lh, phase.highlight, fill: tone.text),
  )
  let natural = measure(block(width: width - 2 * inner.x, content)).height + 2 * inner.y + 3 * px
  block(
    width: width,
    height: calc.max(natural, d.hl-min * px * f),
    fill: tone.bg,
    stroke: (top: 2 * px + tone.solid, rest: 1 * px + tone.border),
    radius: 2 * px,
    inset: inner,
    // Centered like the template's flex row inside its min-height.
    center-v(width - 2 * inner.x, calc.max(natural, d.hl-min * px * f) - 2 * inner.y - 3 * px, content),
  )
}

// The step list without its frame: title row, then numbered rows joined by a rail.
#let steps-list(ctx, phase, d, width, f) = {
  let th = ctx.theme
  let tone = phase-tone(ctx, phase)
  let steps = phase.at("steps", default: ())
  let steps = if steps == none { () } else { steps }
  let dot = d.dot * rem * f
  let row(i, step) = {
    let copy = {
      txt(ctx, "body", ctx.ty.body-tight * f, 1.35, step.at("label", default: ""))
      if has(step.at("desc", default: none)) {
        v(d.desc-mt * rem * f)
        txt(ctx, "body", ctx.ty.caption * d.desc-k * f, d.desc-lh, step.desc, fill: th.muted)
      }
    }
    grid(
      columns: (dot, 1fr),
      column-gutter: d.row-gap * rem * f,
      pad(top: 0.05 * rem, box(
        width: dot,
        height: dot,
        fill: tone.solid,
        stroke: 1 * px + tone.border,
        radius: 2 * px,
        align(center + horizon, text(
          font: ctx.fonts.number,
          size: ctx.ty.caption * d.dot-k * f,
          fill: tone.contrast,
          top-edge: "cap-height",
          bottom-edge: "baseline",
          str(i + 1),
        )),
      )),
      copy,
    )
  }
  let pad-y = d.row-pad * rem * f
  let rows = steps.enumerate().map(((i, s)) => row(i, s))
  let heights = rows.map(r => measure(block(width: width, r)).height)
  // Title row: label and zero-padded count, over a dashed rule.
  block(
    width: 100%,
    stroke: (bottom: (paint: th.edge, thickness: 1 * px, dash: "dashed")),
    below: d.wrap-title-mb * rem * f,
    grid(
      columns: (1fr, auto),
      txt(ctx, "caption", ctx.ty.caption * f, 1.5, "关键动作", fill: th.muted, tracking: 0.08em),
      txt(ctx, "number", ctx.ty.caption * f, 1.5, (if steps.len() < 10 { "0" } else { "" }) + str(steps.len()), fill: th.muted),
    ),
  )
  for (i, r) in rows.enumerate() {
    block(width: 100%, inset: (y: pad-y), below: 0.1 * rem * f, {
      // `.step-conn`: a faint rail from this dot down to the next one.
      if i < rows.len() - 1 {
        place(top + left, dx: dot / 2 - 1 * px, dy: 0.05 * rem + dot + 3 * px, line(
          angle: 90deg,
          length: calc.max(0pt, heights.at(i) - dot + 2 * pad-y + 0.1 * rem * f - 6 * px),
          stroke: 2 * px + tone.solid.transparentize(74%),
        ))
      }
      r
    })
  }
}

#let column(ctx, phase, d, width, height, f) = {
  let th = ctx.theme
  let col-gap = d.col-gap * rem * f
  let head-block = head(ctx, phase, d, width, f)
  let hl = highlight(ctx, phase, d, width, f)
  let head-h = measure(head-block).height
  let hl-h = if hl == none { 0pt } else { measure(hl).height + col-gap }
  let wrap-h = height - head-h - col-gap - hl-h
  let (pad-top, pad-bottom) = d.wrap-pad
  let wrap-pad = (x: pad-top * rem * f, top: pad-top * rem * f, bottom: pad-bottom * rem * f)
  stack(
    spacing: col-gap,
    head-block,
    block(
      width: width,
      height: wrap-h,
      fill: th.paper,
      stroke: 1 * px + th.edge,
      radius: (bottom: 3 * px),
      inset: wrap-pad,
      clip: true,
      steps-list(ctx, phase, d, width - 2 * wrap-pad.x, f),
    ),
    ..if hl != none { (hl,) },
  )
}

// Height a column needs at factor `f` (its step list at natural height).
#let column-need(ctx, phase, d, width, f) = {
  let col-gap = d.col-gap * rem * f
  let (pad-top, pad-bottom) = d.wrap-pad
  let hl = highlight(ctx, phase, d, width, f)
  (
    measure(head(ctx, phase, d, width, f)).height
      + col-gap
      + measure(block(width: width - 2 * pad-top * rem * f, steps-list(ctx, phase, d, width - 2 * pad-top * rem * f, f))).height
      + (pad-top + pad-bottom) * rem * f
      + if hl == none { 0pt } else { col-gap + measure(hl).height }
  )
}

#let render(ctx, slide) = layout(size => {
  let th = ctx.theme
  let phases = slide.at("phases", default: ())
  let phases = if phases == none { () } else { phases }
  let d = density(phases)
  let gap = d.gap * rem
  // No `py-4` here: the slide spans the frame from 4px below its top edge to
  // its bottom edge, i.e. 12px above and 16px below this content area.
  let width = size.width
  let height = size.height + 28 * px

  let header = block(width: width, {
    txt(ctx, "title", ctx.ty.title, 1.18, slide.title, tracking: -0.02em)
    v(0.6 * rem)
    line(length: 100%, stroke: 1 * px + th.primary.transparentize(35%))
  })
  // Subtitle tag and summary card share one row (`.process-topbar`).
  let topbar = {
    let parts = ()
    if has(slide.at("subtitle", default: none)) {
      parts.push(box(
        fill: th.tones.neutral.bg,
        stroke: 1 * px + th.tones.neutral.border,
        radius: 2 * px,
        inset: (x: 12 * px, y: 2 * px),
        txt(ctx, "subtitle", ctx.ty.subtitle, 1.5, slide.subtitle, fill: th.tones.neutral.text, tracking: 0.035em),
      ))
    }
    if has(slide.at("summary", default: none)) {
      parts.push(block(
        fill: th.paper,
        stroke: 1 * px + th.edge,
        radius: 4 * px,
        inset: (x: 1 * rem, y: 0.75 * rem),
        grid(
          columns: (42 * px, 1fr),
          column-gutter: 0.75 * rem,
          align: horizon,
          line(length: 42 * px, stroke: 1 * px + th.primary.transparentize(35%)),
          txt(ctx, "body", ctx.ty.body-tight, 1.45, slide.summary, fill: th.muted),
        ),
      ))
    }
    // One row like the flex topbar: the tag keeps its width, the summary takes the rest.
    if parts.len() > 0 {
      grid(columns: (auto,) * (parts.len() - 1) + (1fr,), column-gutter: 0.875 * rem, align: horizon, ..parts)
    }
  }
  let footer = if has(slide.at("footer", default: none)) {
    block(width: width, inset: (top: 0.15 * rem), align(center, txt(ctx, "body", ctx.ty.body-tight, 1.5, rich(slide.footer), fill: th.muted)))
  }
  let topbar-h = if topbar == none { 0pt } else { measure(block(width: width, topbar)).height }
  let footer-h = if footer == none { 0pt } else { measure(footer).height + gap }
  // The topbar is a flex child even when empty, so both gaps always apply.
  let board-h = height - 4 * px - measure(header).height - gap - topbar-h - gap - footer-h - 4 * px
  let n = calc.max(phases.len(), 1)
  let col-gap = 1 * rem
  let col-w = (width - col-gap * (n - 1)) / n

  let tightest(f) = calc.max(0pt, ..phases.map(p => column-need(ctx, p, d, col-w, f)))
  let f = fit-factor(f => tightest(f) <= board-h)
  if f == none {
    overflow-marker(ctx.index, tightest(fit-min), board-h)
    f = fit-min
  }

  section-num(ctx, slide.at("section", default: none))
  place(top + left, dy: -12 * px, block(width: width, height: height, inset: (top: 4 * px), {
    header
    v(gap)
    if topbar != none { topbar }
    v(gap)
    v(4 * px)
    grid(
      columns: (col-w,) * n,
      column-gutter: col-gap,
      ..phases.map(p => column(ctx, p, d, col-w, board-h, f)),
    )
    if footer != none {
      v(gap)
      footer
    }
  }))
})
