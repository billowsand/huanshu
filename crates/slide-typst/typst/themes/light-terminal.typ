// 终端·纸 (light-terminal). Values are sampled from src/themes/terminal.css
// and confirmed against the preview page with measure()/getComputedStyle; keep
// the two in sync when the CSS theme changes.

#import "../lib/base.typ": px

#let tone(solid, bg, border, contrast: rgb("#fff9ee")) = (
  solid: solid,
  text: solid,
  bg: bg,
  border: border,
  contrast: contrast,
)

#let theme = (
  name: "light-terminal",
  // `--terminal-bg` (page) / `--terminal-panel` (glass-card fill).
  paper: rgb("#faf9f3"),
  card: rgb("#faf9f3"),
  ink: rgb("#25291f"),
  muted: rgb("#656b5c"),
  edge: rgb("#c9cebe"),
  primary: rgb("#9d4a1c"),
  primary-bg: rgb("#eee0d1"),
  primary-border: rgb("#c4a383"),
  // `--terminal-primary-hover`: the darker accent for emphasized items.
  primary-strong: rgb("#803b16"),
  // `.spotlight-media` is a plain glass panel; the mat matches the frame fill.
  media-bg: rgb("#faf9f3"),
  // No row striping on the terminal compare table.
  stripe: none,
  // Overview band / annotation sheets are plain panels in this theme.
  sheet-tint: rgb("#faf9f3"),
  sheet: rgb("#faf9f3"),
  // No certificate outline (deco.certificate: false).
  outline: none,
  section-num-stroke: rgb("#c9cebe"),
  // Unused (deco.tape: false); kept so every theme dictionary has the same keys.
  tape: rgb("#eee0d1"),
  tape-edge: rgb("#c4a383"),
  // `.section-intro__lead`: a plain glass panel; badge is TagBadge color=green,
  // the note rule is `--success-border`.
  lead: (
    fill: rgb("#faf9f3"),
    border: rgb("#c9cebe"),
    ink: rgb("#25291f"),
    soft: rgb("#656b5c"),
    rule: rgb("#c9cebe"),
    note-rule: rgb("#a6b592"),
    badge-bg: rgb("#e7edde"),
    badge-border: rgb("#a6b592"),
    badge-ink: rgb("#49682c"),
  ),
  // `.slide-shell` decorations: the 24px grid doubles as the "rule" token; no
  // notebook margin in this theme (keys kept for parity with light-notes).
  rule: rgb(101, 107, 92, 20), // `--terminal-grid` rgba(101,107,92,0.08)
  margin-fill: rgb("#eaeade"),
  margin-line: rgb("#c9cebe"),
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
  // share the primary family (they collide on screen too).
  tones: (
    neutral: tone(rgb("#9d4a1c"), rgb("#eee0d1"), rgb("#c4a383"), contrast: rgb("#fff9ee")),
    blue: tone(rgb("#36627b"), rgb("#e3ebee"), rgb("#9aafba")),
    green: tone(rgb("#49682c"), rgb("#e7edde"), rgb("#a6b592")),
    amber: tone(rgb("#9d4a1c"), rgb("#eee0d1"), rgb("#c4a383"), contrast: rgb("#fff9ee")),
    red: tone(rgb("#a64239"), rgb("#f2e2dc"), rgb("#c6a29a")),
  ),
)

// Page background: `--terminal-bg` sheet, a 1px grid line every 24px in both
// directions (`background-size: 24px 24px`), and the `.slide-shell__frame`
// hairline border (shell padding 12px top/bottom, 14px left/right).
#let background(w, h) = {
  place(rect(width: w * px, height: h * px, fill: rgb("#f4f3eb")))
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
