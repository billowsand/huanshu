// 便签·稿纸 (light-notes). Values are sampled from src/themes/notes.css and
// src/themes/notes-layouts.css; keep the two in sync when the CSS theme changes.

#import "../lib/base.typ": px

#let tone(solid, bg, border) = (solid: solid, text: solid, bg: bg, border: border, contrast: rgb("#fbf7ec"))

#let theme = (
  name: "light-notes",
  // Style family: which CSS layer styles the slides — "notes" (notes-layouts.css),
  // "terminal" (terminal.css), "glass" (raw template styles in style.css).
  // Layouts branch on it via `is-terminal` / `is-glass` (lib/components.typ).
  family: "notes",
  paper: rgb("#fbf7ec"),
  card: rgb("#fefbf5"),
  ink: rgb("#786051"),
  muted: rgb("#89786b"),
  edge: rgb("#ded5c7"),
  primary: rgb("#746254"),
  primary-bg: rgb("#eee7da"),
  primary-border: rgb("#c8bbaa"),
  primary-strong: rgb("#655547"),
  // `.spotlight-media`: photo mount — cream mat inside a hairline frame.
  media-bg: rgb("#f6eddd"),
  stripe: rgb("#faf4e8"),
  // `.overview-card` margin band and `.overview-note` fill.
  sheet-tint: rgb("#f8f0e2"),
  // `.focus-example` / `.split-right-card`: the warmer annotation sheet.
  sheet: rgb("#faf3e4"),
  // `.outcome-card` inner outline (certificate frame).
  outline: rgb(182, 164, 146, 51),
  section-num-stroke: rgb("#b6a49270"),
  tape: rgb("#e9d5af8c"),
  tape-edge: rgb("#b6a49259"),
  // `.section-intro__lead`: the cocoa chapter sheet and its light inks.
  lead: (
    fill: gradient.linear(rgb("#7f6f60"), rgb("#746254"), angle: 45deg),
    border: rgb("#655547"),
    ink: rgb("#fbf7ec"),
    soft: rgb("#efe2cd"),
    rule: rgb("#fbf7ec38"),
    note-rule: rgb("#d6c0a2"),
    badge-bg: rgb("#eee7da"),
    badge-border: rgb("#d6c0a2"),
    badge-ink: rgb("#746254"),
  ),
  // Chart tokens: the `--chart-*` variables of notes.css plus `--success`, read
  // off the Vue reference page. `palette` is exactly six colours; the chart
  // drawing (lib/chart.typ) cycles them for multi-series and donut sectors.
  chart: (
    palette: (
      rgb("#746254"),
      rgb("#54633d"),
      rgb("#826039"),
      rgb("#4e6570"),
      rgb("#8a5a44"),
      rgb("#9c4a3c"),
    ),
    grid: rgb("#e8e0cf"),
    axis: rgb("#c8bbaa"),
    success: rgb("#54633d"),
  ),
  // `.slide-shell` stationery: ruled lines, notebook margin, sheet edge.
  rule: rgb(170, 146, 125, 41),
  margin-fill: rgb("#eee8dc99"),
  margin-line: rgb(116, 98, 82, 46),
  // Corner radii in CSS px: `paper-card` / `sheet` / `tag-pill` + `label-bar`.
  radius: (card: 4, sheet: 2, pill: 2),
  // Letter spacing: `slide-header` title / `tag-pill` + `label-bar`.
  tracking: (title: -0.02em, label: 0.035em),
  // Stationery decorations other themes may switch off.
  deco: (
    ruled: true, // `sheet(ruled:)` draws the ruled-paper lines
    tape: true, // cover's paper tape
    double-frame: true, // double-stroked badge / stamp / example frames
    certificate: true, // outcome cards' inner certificate outline
    stamp: "double", // step-stamp: "double" | "single"
    section-num: "outline", // "outline" (giant stroked) | "meta" (small muted)
  ),
  // Tone families of `[data-tone]` in notes-layouts.css; toneVars() in the Vue
  // templates resolves to the same values under this theme.
  tones: (
    neutral: tone(rgb("#746254"), rgb("#eee7da"), rgb("#c8bbaa")),
    blue: tone(rgb("#4e6570"), rgb("#eaf0f1"), rgb("#b2c3c7")),
    green: tone(rgb("#54633d"), rgb("#edf0e2"), rgb("#b7c1a0")),
    amber: tone(rgb("#826039"), rgb("#f6ebd8"), rgb("#cfb489")),
    red: tone(rgb("#9b4e43"), rgb("#f7e9e3"), rgb("#d4b0a5")),
  ),
)

// Page background: cream sheet, notebook margin at 45px, a rule every 36px.
#let background(w, h) = {
  place(rect(width: w * px, height: h * px, fill: theme.paper))
  place(rect(width: 45 * px, height: h * px, fill: theme.margin-fill))
  place(dx: 45.5 * px, line(start: (0pt, 0pt), end: (0pt, h * px), stroke: 1 * px + theme.margin-line))
  for i in range(1, calc.floor(h / 36) + 1) {
    let y = (i * 36 - 0.5) * px
    place(dy: y, line(start: (0pt, 0pt), end: (w * px, 0pt), stroke: 1 * px + theme.rule))
  }
  place(rect(width: w * px, height: h * px, stroke: 1 * px + theme.edge))
}
