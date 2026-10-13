// Shared primitives for the Typst slide renderer.
//
// Geometry is authored in CSS pixels of the 1280×720 Vue canvas so values can be
// copied from the Vue templates; one px maps to 0.75pt, which makes a 16:9 page
// exactly 13.333in × 7.5in (the PowerPoint widescreen size).

#let px = 0.75pt
#let rem = 16 * px

#let canvas-sizes = (
  ratio_16x9: (w: 1280, h: 720),
  ratio_32x9: (w: 2560, h: 720),
  ratio_48x9: (w: 3840, h: 720),
)

// ── Per-slide type scale (port of SlideRenderer.vue `contentLoad`) ───────────

// `chart`: a chart's data rows are drawn in a fixed-size panel, not set as
// text; counting them would push every chart page to the smallest scale.
// Keep in sync with `skipKeys` in SlideRenderer.vue.
#let load-skip-keys = (
  "kind", "aspect_ratio", "tone", "icon", "label_tone", "footer_tone",
  "titleTone", "top_bar_class", "topBarClass", "chart",
)

// LaTeX math (`$...$` / `$$...$$`) reaches templates already converted to Typst
// math source by Rust (see src/math.rs), wrapped as `\u{1}I…\u{2}` (inline) or
// `\u{1}D…\u{2}` (display) spans; each span is eval'd in the vendored mitex
// scope. Spans whose LaTeX failed conversion keep their literal `$` text.
#import "/vendor/mitex/specs/mod.typ": mitex-scope

#let math-span = regex("(?s)\u{1}([ID])(.*?)\u{2}")

#let strip-tags(s) = s.replace(regex("<[^>]*>"), "").replace(math-span, "xx")

#let content-load(value) = {
  if type(value) == str {
    strip-tags(value).clusters().len()
  } else if type(value) == array {
    value.len() * 10 + value.map(content-load).sum(default: 0)
  } else if type(value) == dictionary {
    value
      .pairs()
      .filter(((key, _)) => key not in load-skip-keys)
      .map(((_, child)) => content-load(child))
      .sum(default: 0)
  } else {
    0
  }
}

#let type-scale-base = 1.25

#let slide-type-scale(slide) = {
  let load = content-load(slide)
  if load <= 150 { type-scale-base * 1.14 } else if load <= 260 { type-scale-base * 1.06 } else { type-scale-base }
}

// The `--type-*` variables of `.slide-shell` (src/style.css), evaluated for one
// scale. Templates multiply these by a local fit factor instead of hard-coding
// sizes, mirroring the CSS rule that every size derives from `--type-scale`.
#let type-sizes(scale) = (
  title: 2 * rem * scale,
  subtitle: 1.0625 * rem * scale,
  body: 1.125 * rem * scale,
  body-tight: 1.0625 * rem * scale,
  meta: 1 * rem * scale,
  caption: 0.9375 * rem * scale,
  body-strong: 1.1875 * rem * scale,
  card-title: 1.25 * rem * scale,
  note: 1 * rem * scale,
  cover-title: 3.25 * rem * scale,
  cover-subtitle: 1.375 * rem * scale,
  cover-en: 0.875 * rem * scale,
  icon-sm: 1 * rem * scale,
  icon-md: 1.125 * rem * scale,
  icon-lg: 1.25 * rem * scale,
  icon-xl: 1.5 * rem * scale,
  icon-2xl: 1.75 * rem * scale,
  icon-hero: 3.75 * rem * scale,
  section-num: 7 * rem * scale,
  grid-header: 1.171875 * rem * scale,
  grid-body: 0.9375 * rem * scale,
  grid-icon: 1.125 * rem * scale,
)

// ── Text helpers ──────────────────────────────────────────────────────────────

// Emulates a CSS line box: every line is `lh × size` tall and the half-leading
// sits above the first and below the last line, so stacked blocks get the same
// rhythm as the browser. Requires the global 1em text edges set in main.typ.
#let line-box(size, lh, body, width: auto) = block(
  width: width,
  inset: (y: (lh - 1) / 2 * size),
  {
    set text(size: size)
    set par(leading: (lh - 1) * size)
    body
  },
)

#let entity-map = (
  "&amp;": "&",
  "&lt;": "<",
  "&gt;": ">",
  "&quot;": "\"",
  "&#39;": "'",
  "&nbsp;": " ",
)

#let decode-entities(s) = {
  for (entity, ch) in entity-map { s = s.replace(entity, ch) }
  s
}

// Renders the small HTML subset the LLM may put into `v-html` fields.
// Strings become text content, never Typst markup, so `#`, `*`, `$` … in model
// output cannot execute. Unknown tags are dropped, as the browser would hide them.
//
// LaTeX math (`$...$` / `$$...$$`) reaches templates already converted to Typst
// math source by Rust (see src/math.rs), wrapped as `\u{1}I…\u{2}` (inline) or
// `\u{1}D…\u{2}` (display) spans; each span is eval'd in the vendored mitex
// scope. Spans whose LaTeX failed conversion keep their literal `$` text.
#let rich-plain(s, emphasis: none) = {
  let pattern = regex("(?s)<br\s*/?>|<(strong|b|em|i|mark)(?:\s[^>]*)?>(.*?)</(?:strong|b|em|i|mark)>|<[^>]+>")
  let parts = ()
  let pos = 0
  for m in s.matches(pattern) {
    parts.push(decode-entities(s.slice(pos, m.start)))
    if m.text.starts-with("<br") {
      parts.push(linebreak())
    } else if m.captures.at(0) != none {
      let inner = strip-tags(decode-entities(m.captures.at(1)))
      parts.push(if emphasis == none { inner } else { text(fill: emphasis, inner) })
    }
    pos = m.end
  }
  parts.push(decode-entities(s.slice(pos)))
  parts.map(part => if type(part) == str { [#part] } else { part }).join()
}

#let rich(value, emphasis: none) = {
  if value == none { return [] }
  let s = str(value)
  let parts = ()
  let pos = 0
  for m in s.matches(math-span) {
    parts.push(rich-plain(s.slice(pos, m.start), emphasis: emphasis))
    let equation = eval("$" + m.captures.at(1) + "$", scope: mitex-scope)
    parts.push(math.equation(block: m.captures.at(0) == "D", equation))
    pos = m.end
  }
  parts.push(rich-plain(s.slice(pos), emphasis: emphasis))
  parts.join()
}

#let has(value) = value != none and value != "" and value != ()

// ── Icons ─────────────────────────────────────────────────────────────────────
// Icons are served by the Rust world as `/icons/<set>/<name>/<rrggbb[aa]>.svg` with
// `currentColor` already replaced, so any tint is a plain image load.

#let icon-pattern = regex("^(?:i-)?(?:([a-z0-9-]+):)?([a-z0-9-]+)$")

#let icon(name, fill, size) = {
  // Model output: anything that is not a plain `i-set:name` becomes the
  // fallback icon instead of a path that could leave `/icons/`.
  let m = if type(name) == str { name.match(icon-pattern) }
  let (set-name, icon-name) = if m == none { ("carbon", "circle-dash") } else {
    (if m.captures.at(0) == none { "carbon" } else { m.captures.at(0) }, m.captures.at(1))
  }
  // `rrggbb` or `rrggbbaa`; the alpha becomes the icon's opacity.
  let hex = fill.to-hex().trim("#", at: start)
  box(image("/icons/" + set-name + "/" + icon-name + "/" + hex + ".svg", width: size, height: size))
}

// ── Boxes ─────────────────────────────────────────────────────────────────────

// CSS `border: 3px double` → two 1px strokes with a 1px gap.
#let double-box(stroke-color, inset: 0pt, radius: 2 * px, body) = box(
  stroke: 1 * px + stroke-color,
  radius: radius,
  inset: 1 * px,
  box(stroke: 1 * px + stroke-color, radius: radius, inset: inset, body),
)

// ── Fit to box ────────────────────────────────────────────────────────────────
// `build(f)` renders the content at fit factor `f`. `fit-factor` returns the
// largest `f` in [min, 1] whose measured height fits, by bisection. This
// replaces the browser templates' pre-estimated `contentScale` buckets with an
// exact measurement.
#let fit-min = 0.4

#let fit-factor(fits, min: fit-min, rounds: 7) = {
  if fits(1) { return 1 }
  if not fits(min) { return none }
  let (lo, hi) = (min, 1)
  for _ in range(rounds) {
    let mid = (lo + hi) / 2
    if fits(mid) { lo = mid } else { hi = mid }
  }
  lo
}

// Reported to Rust (`RenderReport::overflows`) when even `fit-min` is too big.
#let overflow-marker(slide-index, needed, available) = [#metadata((
  slide: slide-index,
  needed-pt: needed / 1pt,
  available-pt: available / 1pt,
)) <slide-overflow>]

// Fits `build` into `width × height`. If nothing fits, renders at `fit-min`,
// clips like the browser's `overflow: hidden`, and emits an overflow marker.
#let fit-height(width, height, build, slide-index: none) = {
  let fits(f) = measure(block(width: width, build(f))).height <= height
  let f = fit-factor(fits)
  if f != none { return build(f) }
  let needed = measure(block(width: width, build(fit-min))).height
  overflow-marker(slide-index, needed, height)
  // The inner block gets its full natural height: laid out in the clipped
  // region instead, breakable children would be split into fragments drawn on
  // top of each other.
  block(width: width, height: height, clip: true, block(width: width, height: needed, breakable: false, build(fit-min)))
}

// CSS `text-wrap: balance` (SlideTitle): the narrowest width that keeps the
// line count, so a wrapped title does not leave one or two characters alone.
#let balanced(width, body) = {
  let full = measure(block(width: width, body)).height
  let (lo, hi) = (0.5, 1.0)
  for _ in range(8) {
    let mid = (lo + hi) / 2
    if measure(block(width: width * mid, body)).height <= full { hi = mid } else { lo = mid }
  }
  block(width: width * hi, body)
}

// Vertically centers `body` in `height` without `align(horizon)`, which would
// leak into every nested grid cell and paragraph.
#let center-v(width, height, body) = {
  let h = measure(block(width: width, body)).height
  v(calc.max(0pt, (height - h) / 2))
  body
}
