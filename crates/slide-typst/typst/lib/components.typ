// Building blocks shared by the slide layouts. Every function takes `ctx`:
//   theme  – color tokens of the active theme (themes/*.typ)
//   ty     – type sizes for this slide's type scale (base.typ `type-sizes`)
//   fonts  – font fallback lists per role (title, subtitle, body, caption, number, code)
//   canvas – (w, h) of the slide in CSS px

#import "base.typ": *

// Content area of a standard layout, measured from the Vue canvas:
// `.slide-shell` padding 12/14 + `.slide-shell__frame` padding 22/28/24, plus the
// templates' `py-4`. Layouts without `py-4` (cover) compensate locally.
#let frame-inset = (left: 42, right: 42, top: 50, bottom: 52)

// The deck's aspect ratio, from the page canvas. Layouts use this rather than
// a slide's own `aspect_ratio` field, which older or hand-edited slides may lack.
#let aspect-of(ctx) = if ctx.canvas.w >= 3840 { "ratio_48x9" } else if ctx.canvas.w >= 2560 {
  "ratio_32x9"
} else { "ratio_16x9" }

#let tone-aliases = (
  indigo: "blue",
  purple: "blue",
  violet: "blue",
  teal: "green",
  emerald: "green",
  rose: "red",
)

#let tone-of(ctx, name, default: "amber") = {
  let resolve(n) = tone-aliases.at(n, default: n)
  let fallback = ctx.theme.tones.at(resolve(default), default: ctx.theme.tones.amber)
  if has(name) { ctx.theme.tones.at(resolve(name), default: fallback) } else { fallback }
}

// Text in a role font with CSS-like line height.
#let txt(ctx, role, size, lh, body, fill: auto, ..args) = line-box(
  size,
  lh,
  text(font: ctx.fonts.at(role), fill: if fill == auto { ctx.theme.ink } else { fill }, ..args, body),
)

// Large outlined chapter number in the top-right corner (`.section-num`).
#let section-num(ctx, value) = {
  if not has(value) { return }
  place(
    top + right,
    dx: -4 * px,
    dy: -22 * px,
    text(
      font: ctx.fonts.number,
      size: ctx.ty.section-num,
      fill: rgb(0, 0, 0, 0),
      stroke: 1 * px + ctx.theme.section-num-stroke,
      top-edge: 0.9em,
      str(value),
    ),
  )
}

// Full-width neutral label bar: the TagBadge subtitle (`.tag-pill` under
// light-notes) stretches across its flex column instead of hugging the text.
#let label-bar(ctx, value, size: auto, colors: none) = {
  let t = if colors == none { ctx.theme.tones.neutral } else { colors }
  block(
    width: 100%,
    fill: t.bg,
    stroke: 1 * px + t.border,
    radius: 2 * px,
    inset: (x: 12 * px, y: 2 * px),
    txt(ctx, "subtitle", if size == auto { ctx.ty.subtitle } else { size }, 1.5, value, fill: t.text, tracking: 0.035em),
  )
}

// Title, rule and optional subtitle (`.slide-header` + TagBadge subtitle).
#let slide-header(ctx, slide, subtitle-gap: 8) = {
  let th = ctx.theme
  txt(ctx, "title", ctx.ty.title, 1.18, slide.title, tracking: -0.02em)
  v(0.6 * rem)
  line(length: 100%, stroke: 1 * px + th.primary.transparentize(35%))
  v(1 * rem)
  if has(slide.at("subtitle", default: none)) {
    v(subtitle-gap * px)
    label-bar(ctx, slide.subtitle)
  }
}

// Centered footnote under the content (`.slide-footnote`).
#let footnote-line(ctx, value, gap: 16) = {
  if not has(value) { return none }
  v(gap * px)
  align(center, txt(ctx, "caption", ctx.ty.caption, 1.5, rich(value)))
}

// The standard content page: header on top, optional footnote at the bottom,
// and `body(width, height)` laid out in exactly the space left between them.
#let content-page(ctx, slide, body, footer: none, footer-gap: 16, subtitle-gap: 8, gap-after-header: 0) = layout(size => {
  let header = block(width: size.width, slide-header(ctx, slide, subtitle-gap: subtitle-gap))
  let foot = footnote-line(ctx, footer, gap: footer-gap)
  let header-h = measure(header).height
  let foot-h = if foot == none { 0pt } else { measure(block(width: size.width, foot)).height }
  let avail = size.height - header-h - foot-h - gap-after-header * px
  section-num(ctx, slide.at("section", default: none))
  header
  v(gap-after-header * px)
  block(width: size.width, height: avail, body(size.width, avail))
  foot
})

// Paper card used by `.glass-card` under this theme.
#let paper-card(ctx, body, inset: 0pt, stroke: auto, fill: auto, radius: 4 * px, width: 100%, ..args) = block(
  width: width,
  inset: inset,
  radius: radius,
  fill: if fill == auto { ctx.theme.paper } else { fill },
  stroke: if stroke == auto { 1 * px + ctx.theme.edge } else { stroke },
  ..args,
  body,
)

// `--notes-rule` card background: a faint 1px line in every 32px step,
// counted from the bottom edge like the CSS `repeating-linear-gradient(0deg)`.
#let rule-lines(ctx, width, height) = {
  for i in range(1, calc.floor(height / (32 * px)) + 1) {
    place(top + left, dy: height - (i * 32 - 0.5) * px, line(length: width, stroke: 1 * px + ctx.theme.rule))
  }
}

// A card of known size: optional ruled background, `body` padded by `inset`
// (a dictionary of `pad` arguments). Explicit sizes let rows share heights.
#let sheet(ctx, width, height, body, inset: (:), fill: auto, stroke: auto, radius: 2 * px, ruled: false) = block(
  width: width,
  height: height,
  fill: if fill == auto { ctx.theme.paper } else { fill },
  stroke: if stroke == auto { 1 * px + ctx.theme.edge } else { stroke },
  radius: radius,
  clip: true,
  {
    if ruled { rule-lines(ctx, width, height) }
    pad(..inset, body)
  },
)

// `StepNumber` / `.split-left-step` under light-notes: a paper square inside a
// 3px double border (two 1px strokes with a 1px gap).
#let step-stamp(ctx, value, size: 48, f: 1, text-size: auto, border: auto, tracking: 0em, radius: 3) = {
  let s = size * px * f
  let color = if border == auto { ctx.theme.tones.neutral.border } else { border }
  box(width: s, height: s, fill: ctx.theme.paper, stroke: 1 * px + color, radius: radius * px, inset: 2 * px, box(
    width: 100%,
    height: 100%,
    stroke: 1 * px + color,
    radius: calc.max(radius - 2, 0) * px,
    align(center + horizon, txt(
      ctx,
      "number",
      (if text-size == auto { ctx.ty.body } else { text-size }) * f,
      1.2,
      str(value),
      fill: ctx.theme.primary,
      tracking: tracking,
    )),
  ))
}

// Icon + note sentence bar (`glass-card px-5 py-3 flex items-center gap-3`).
#let note-bar(ctx, icon-name, value, f: 1) = paper-card(
  ctx,
  inset: (x: 20 * px, y: 12 * px * f),
  grid(
    columns: (auto, 1fr),
    column-gutter: 12 * px,
    align: horizon,
    icon(icon-name, ctx.theme.primary, ctx.ty.icon-xl * f),
    txt(ctx, "body", ctx.ty.note * f, 1.5, rich(value, emphasis: ctx.theme.primary)),
  ),
)

// Neutral or toned label pill (`.tag-pill` under light-notes).
#let tag-pill(ctx, value, tone: none, size: auto) = {
  let t = if tone == none { ctx.theme.tones.neutral } else { tone }
  box(
    fill: t.bg,
    stroke: 1 * px + t.border,
    radius: 2 * px,
    inset: (x: 12 * px, y: 2 * px),
    txt(ctx, "caption", if size == auto { ctx.ty.subtitle } else { size }, 1.5, value, fill: t.text, tracking: 0.035em),
  )
}

// Fallback page for kinds that have no Typst layout yet: keeps the deck
// complete and makes the gap visible instead of silently dropping the slide.
#let unsupported-page(ctx, slide) = content-page(ctx, slide, (w, h) => align(center + horizon, block(
  width: 70%,
  inset: 24 * px,
  stroke: (paint: ctx.theme.edge, thickness: 1 * px, dash: "dashed"),
  radius: 4 * px,
  align(center, {
    txt(ctx, "caption", ctx.ty.meta, 1.5, "Typst 版式尚未实现", fill: ctx.theme.muted)
    txt(ctx, "code", ctx.ty.body, 1.5, "kind: " + str(slide.kind))
  }),
)))
