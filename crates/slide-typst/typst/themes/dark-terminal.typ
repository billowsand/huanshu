// 终端·墨 (dark-terminal). Values are sampled from src/themes/terminal.css
// and confirmed against the preview page with measure()/getComputedStyle; keep
// the two in sync when the CSS theme changes.

#import "../lib/base.typ": px

#let tone(solid, bg, border, contrast: rgb("#151614")) = (
  solid: solid,
  text: solid,
  bg: bg,
  border: border,
  contrast: contrast,
)

#let theme = (
  name: "dark-terminal",
  // `--terminal-bg` (page) / `--terminal-panel` (glass-card fill).
  paper: rgb("#1b1d19"),
  card: rgb("#1b1d19"),
  ink: rgb("#eeeede"),
  muted: rgb("#a1a697"),
  edge: rgb("#393d33"),
  primary: rgb("#ed9b62"),
  primary-bg: rgb("#30251e"),
  primary-border: rgb("#76543b"),
  // `--terminal-primary-hover`: the lighter accent for emphasized items.
  primary-strong: rgb("#f4b787"),
  // `.spotlight-media` is a plain glass panel; the mat matches the frame fill.
  media-bg: rgb("#1b1d19"),
  // No row striping on the terminal compare table.
  stripe: none,
  // Overview band / annotation sheets are plain panels in this theme.
  sheet-tint: rgb("#1b1d19"),
  sheet: rgb("#1b1d19"),
  // No certificate outline (deco.certificate: false).
  outline: none,
  section-num-stroke: rgb("#393d33"),
  // Unused (deco.tape: false); kept so every theme dictionary has the same keys.
  tape: rgb("#30251e"),
  tape-edge: rgb("#76543b"),
  // `.section-intro__lead`: a plain glass panel; badge is TagBadge color=green,
  // the note rule is `--success-border`.
  lead: (
    fill: rgb("#1b1d19"),
    border: rgb("#393d33"),
    ink: rgb("#eeeede"),
    soft: rgb("#a1a697"),
    rule: rgb("#393d33"),
    note-rule: rgb("#546843"),
    badge-bg: rgb("#252e20"),
    badge-border: rgb("#546843"),
    badge-ink: rgb("#a9c58c"),
  ),
  // `.slide-shell` decorations: the 24px grid doubles as the "rule" token; no
  // notebook margin in this theme (keys kept for parity with light-notes).
  rule: rgb(161, 166, 151, 18), // `--terminal-grid` rgba(161,166,151,0.07)
  margin-fill: rgb("#242720"),
  margin-line: rgb("#393d33"),
  // Every card / badge / node is forced to 2px by terminal.css.
  radius: (card: 2, sheet: 2, pill: 2),
  // Titles -0.025em; tag-pill / label-bar 0.06em.
  tracking: (title: -0.025em, label: 0.06em),
  deco: (
    ruled: false,
    tape: false,
    double-frame: false,
    certificate: false,
    stamp: "single",
    section-num: "meta",
  ),
  // data-tone on screen resolves through toneVars() to the --info/--success/
  // --danger/--warning variables; warning aliases primary, so amber and neutral
  // share the primary family (they collide on screen too). The default contrast
  // is `--terminal-solid-contrast`; only the primary family overrides it with
  // `--terminal-primary-contrast`.
  tones: (
    neutral: tone(rgb("#ed9b62"), rgb("#30251e"), rgb("#76543b"), contrast: rgb("#20170f")),
    blue: tone(rgb("#96b9ce"), rgb("#202a30"), rgb("#465e6c")),
    green: tone(rgb("#a9c58c"), rgb("#252e20"), rgb("#546843")),
    amber: tone(rgb("#ed9b62"), rgb("#30251e"), rgb("#76543b"), contrast: rgb("#20170f")),
    red: tone(rgb("#e99388"), rgb("#322321"), rgb("#754b44")),
  ),
)

// Page background: `--terminal-bg` sheet, a 1px grid line every 24px in both
// directions (`background-size: 24px 24px`), and the `.slide-shell__frame`
// hairline border (shell padding 12px top/bottom, 14px left/right).
#let background(w, h) = {
  place(rect(width: w * px, height: h * px, fill: rgb("#151614")))
  for i in range(0, calc.floor(w / 24) + 1) {
    place(dx: i * 24 * px, line(start: (0pt, 0pt), end: (0pt, h * px), stroke: 1 * px + theme.rule))
  }
  for i in range(0, calc.floor(h / 24) + 1) {
    place(dy: i * 24 * px, line(start: (0pt, 0pt), end: (w * px, 0pt), stroke: 1 * px + theme.rule))
  }
  place(dx: 14 * px, dy: 12 * px, rect(
    width: (w - 28) * px,
    height: (h - 24) * px,
    radius: 2 * px,
    stroke: 1 * px + theme.edge,
  ))
}
