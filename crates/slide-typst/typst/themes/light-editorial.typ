// 报刊·钴蓝 (light-editorial). Values are sampled from src/themes/editorial.css;
// keep the two in sync when the CSS theme changes.
//
// Modern editorial: crisp paper, black headlines, cobalt emphasis, open columns.
// Screen previews and PDF exports share this exact rendering path.

#import "../lib/base.typ": px

#let tone(solid, bg, border, contrast: rgb("#ffffff")) = (
  solid: solid,
  text: solid,
  bg: bg,
  border: border,
  contrast: contrast,
)

#let theme = (
  name: "light-editorial",
  // Style family: which CSS layer styles the slides — "notes" (notes-layouts.css),
  // "terminal" (terminal.css), "glass" (raw template styles in style.css),
  // "editorial" (editorial.css; layouts treat it as terminal plus a few
  // `is-editorial` branches).
  family: "editorial",
  // Newsprint page; `--glass` is the same paper so cards read as open columns.
  paper: rgb("#f6f5f0"),
  card: rgb("#ffffff"),
  ink: rgb("#141414"),
  muted: rgb("#73736f"),
  edge: rgb("#d8d6cf"),
  primary: rgb("#2549d8"),
  primary-bg: rgb("#e9edfc"),
  primary-border: rgb("#bdc9f1"),
  primary-strong: rgb("#1d3db8"),
  media-bg: rgb("#f6f5f0"),
  stripe: none,
  sheet-tint: rgb("#f6f5f0"),
  sheet: rgb("#f6f5f0"),
  outline: none,
  section-num-stroke: rgb("#d8d6cf"),
  // Unused (deco.tape: false); kept so every theme dictionary has the same keys.
  tape: rgb("#e9edfc"),
  tape-edge: rgb("#bdc9f1"),
  // `.section-intro__lead`: the pale cobalt aside band without a border; the
  // badge is TagBadge color=green.
  lead: (
    fill: rgb("#e9edfc"),
    border: rgb("#e9edfc"),
    ink: rgb("#141414"),
    soft: rgb("#73736f"),
    rule: rgb("#bdc9f1"),
    note-rule: rgb("#bdc9f1"),
    badge-bg: rgb("#e2ede5"),
    badge-border: rgb("#a9c7b4"),
    badge-ink: rgb("#2e6e4f"),
  ),
  // Chart tokens: editorial.css has no chart JSON fixture (the SVG fixtures were
  // generated for light-notes only), so these are copied from the theme's
  // `--chart-grid` / `--chart-axis` / `--chart-palette-*` and `--success`.
  chart: (
    palette: (
      rgb("#2549d8"),
      rgb("#2e6e4f"),
      rgb("#9a6212"),
      rgb("#a8322a"),
      rgb("#5b6f86"),
      rgb("#7a5c99"),
    ),
    grid: rgb("#e8e7e1"),
    axis: rgb("#d8d6cf"),
    success: rgb("#2e6e4f"),
  ),
  // No ruled paper or notebook margin (keys kept for parity with light-notes).
  rule: rgb("#d8d6cf"),
  margin-fill: rgb("#f6f5f0"),
  margin-line: rgb("#d8d6cf"),
  // Every card, sheet and pill is square.
  radius: (card: 0, sheet: 0, pill: 0),
  tracking: (title: -0.01em, label: 0.06em),
  deco: (
    ruled: false,
    tape: false,
    double-frame: false,
    certificate: false,
    stamp: "single",
    section-num: "meta",
  ),
  // toneVars(): neutral is the cobalt primary family, amber the `--warning`
  // ochre, blue the `--info` navy.
  tones: (
    neutral: tone(rgb("#2549d8"), rgb("#e9edfc"), rgb("#bdc9f1")),
    blue: tone(rgb("#23407a"), rgb("#e3e8f1"), rgb("#afbcd6")),
    green: tone(rgb("#2e6e4f"), rgb("#e2ede5"), rgb("#a9c7b4")),
    amber: tone(rgb("#9a6212"), rgb("#f3e7d2"), rgb("#d8be91")),
    red: tone(rgb("#a8322a"), rgb("#f4e1dc"), rgb("#d7a9a1")),
  ),
)

// Page background: plain newsprint and a restrained 3px cobalt rule across the top
// (`.slide-shell::before`); the shell frame has no border in this theme.
#let background(w, h) = {
  place(rect(width: w * px, height: h * px, fill: theme.paper))
  place(rect(width: w * px, height: 3 * px, fill: theme.primary))
}
