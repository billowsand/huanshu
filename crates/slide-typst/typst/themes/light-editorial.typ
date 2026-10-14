// 报刊·钴蓝 (light-editorial). Values are sampled from src/themes/editorial.css;
// keep the two in sync when the CSS theme changes.
//
// First-phase port: the theme rides the terminal geometry (`is-terminal` is
// true for the "editorial" family), so cards keep a hairline frame here while
// the screen draws only the rule above them. Editorial-specific branches so
// far: the two-tone cover / closing headline (`title-tail`), solid cobalt
// badges and ink title rules (`divider-line`).

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
  paper: rgb("#f3efe6"),
  card: rgb("#f3efe6"),
  ink: rgb("#1c2723"),
  muted: rgb("#63675e"),
  edge: rgb("#cbc5b8"),
  primary: rgb("#1d4ed8"),
  primary-bg: rgb("#e1e7f3"),
  primary-border: rgb("#b7c5e6"),
  primary-strong: rgb("#1a43bf"),
  media-bg: rgb("#f3efe6"),
  stripe: none,
  sheet-tint: rgb("#f3efe6"),
  sheet: rgb("#f3efe6"),
  outline: none,
  section-num-stroke: rgb("#cbc5b8"),
  // Unused (deco.tape: false); kept so every theme dictionary has the same keys.
  tape: rgb("#e1e7f3"),
  tape-edge: rgb("#b7c5e6"),
  // `.section-intro__lead`: the pale cobalt aside band without a border; the
  // badge is TagBadge color=green.
  lead: (
    fill: rgb("#e1e7f3"),
    border: rgb("#e1e7f3"),
    ink: rgb("#1c2723"),
    soft: rgb("#63675e"),
    rule: rgb("#b7c5e6"),
    note-rule: rgb("#b7c5e6"),
    badge-bg: rgb("#e2ede5"),
    badge-border: rgb("#a9c7b4"),
    badge-ink: rgb("#2e6e4f"),
  ),
  // Chart tokens: editorial.css has no chart JSON fixture (the SVG fixtures were
  // generated for light-notes only), so these are copied from the theme's
  // `--chart-grid` / `--chart-axis` / `--chart-palette-*` and `--success`.
  chart: (
    palette: (
      rgb("#1d4ed8"),
      rgb("#2e6e4f"),
      rgb("#9a6212"),
      rgb("#a8322a"),
      rgb("#5b6f86"),
      rgb("#7a5c99"),
    ),
    grid: rgb("#e4ded2"),
    axis: rgb("#cbc5b8"),
    success: rgb("#2e6e4f"),
  ),
  // No ruled paper or notebook margin (keys kept for parity with light-notes).
  rule: rgb("#cbc5b8"),
  margin-fill: rgb("#f3efe6"),
  margin-line: rgb("#cbc5b8"),
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
    neutral: tone(rgb("#1d4ed8"), rgb("#e1e7f3"), rgb("#b7c5e6")),
    blue: tone(rgb("#23407a"), rgb("#e3e8f1"), rgb("#afbcd6")),
    green: tone(rgb("#2e6e4f"), rgb("#e2ede5"), rgb("#a9c7b4")),
    amber: tone(rgb("#9a6212"), rgb("#f3e7d2"), rgb("#d8be91")),
    red: tone(rgb("#a8322a"), rgb("#f4e1dc"), rgb("#d7a9a1")),
  ),
)

// Page background: plain newsprint and the 6px cobalt band across the top
// (`.slide-shell::before`); the shell frame has no border in this theme.
#let background(w, h) = {
  place(rect(width: w * px, height: h * px, fill: theme.paper))
  place(rect(width: w * px, height: 6 * px, fill: theme.primary))
}
