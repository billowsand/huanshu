// 暗·星图 (dark-blue). Token values are the `[data-theme="dark-blue"]`
// variables in src/style.css (L519-573), confirmed against the preview page
// with measure()/getComputedStyle; keep the two in sync when the CSS changes.
//
// Structure mirrors dark-yellow (see that file): no per-layout CSS applies, so
// the raw Vue template styles win — flat `var(--glass)` panels, 12px card
// radius, 999px pills, primary→transparent gradient rules.

#import "../lib/base.typ": px

#let tone(solid, text, bg, border, contrast) = (
  solid: solid,
  text: text, // `--*-light` — toneVars().text resolves to the light variant
  bg: bg,
  border: border,
  contrast: contrast, // `--*-contrast`
)

#let glass = rgb(14, 165, 233, 8) // `--glass` rgba(14,165,233,0.03)
#let glass-border = rgb(14, 165, 233, 26) // `--glass-border` rgba(14,165,233,0.1)

#let theme = (
  name: "dark-blue",
  // Style family: which CSS layer styles the slides — "notes" (notes-layouts.css),
  // "terminal" (terminal.css), "glass" (raw template styles in style.css).
  // Layouts branch on it via `is-terminal` / `is-glass` (lib/components.typ).
  family: "glass",
  paper: glass,
  card: glass,
  ink: rgb("#DFF0FF"),
  muted: rgb(223, 240, 255, 133), // rgba(223,240,255,0.52)
  edge: glass-border,
  primary: rgb("#0EA5E9"),
  primary-bg: rgb(14, 165, 233, 23), // rgba(14,165,233,0.09)
  primary-border: rgb(14, 165, 233, 61), // rgba(14,165,233,0.24)
  // `--primary-light`.
  primary-strong: rgb("#38BDF8"),
  // `.spotlight-media`: rgba(255,255,255,0.03) (hardcoded in the template).
  media-bg: rgb(255, 255, 255, 8),
  stripe: none,
  sheet-tint: glass,
  sheet: glass,
  outline: none,
  section-num-stroke: rgb(14, 165, 233, 36), // rgba(14,165,233,0.14)
  // Unused (deco.tape: false); kept so every theme dictionary has the same keys.
  tape: rgb(14, 165, 233, 23),
  tape-edge: rgb(14, 165, 233, 61),
  // `.section-intro__lead`: same hardcoded tints as dark-yellow (the template's
  // success tint and white rules are theme-independent).
  lead: (
    fill: gradient.linear(
      (rgb(16, 185, 129, 26), 0%), // color-mix(success 10%, transparent)
      (rgb(16, 185, 129, 0), 55%),
      (rgb(16, 185, 129, 0), 100%),
      angle: 70deg, // CSS 160deg
    ),
    border: glass-border,
    ink: rgb("#DFF0FF"),
    soft: rgb(255, 255, 255, 107), // `.section-intro__meta-label` rgba(255,255,255,0.42)
    rule: rgb(255, 255, 255, 20), // rgba(255,255,255,0.08)
    note-rule: rgb(52, 211, 153, 89), // rgba(52,211,153,0.35)
    badge-bg: rgb(16, 185, 129, 23),
    badge-border: rgb(16, 185, 129, 61),
    badge-ink: rgb("#34d399"),
  ),
  // Unused on this theme (deco.ruled: false); kept for parity with light-notes.
  rule: rgb(14, 165, 233, 13),
  margin-fill: glass,
  margin-line: glass-border,
  radius: (card: 12, sheet: 12, pill: 999),
  tracking: (title: -0.02em, label: 0.05em),
  deco: (
    ruled: false,
    tape: false,
    double-frame: false,
    certificate: false,
    stamp: "single",
    section-num: "outline",
  ),
  // toneVars(): info aliases the brand in this theme; warning is the amber
  // family (unlike dark-yellow, where warning aliases the brand).
  tones: (
    neutral: tone(rgb("#0EA5E9"), rgb("#38BDF8"), rgb(14, 165, 233, 23), rgb(14, 165, 233, 61), rgb("#f0f9ff")),
    blue: tone(rgb("#0EA5E9"), rgb("#38BDF8"), rgb(14, 165, 233, 23), rgb(14, 165, 233, 61), rgb("#f0f9ff")),
    green: tone(rgb("#10b981"), rgb("#34d399"), rgb(16, 185, 129, 23), rgb(16, 185, 129, 61), rgb("#f0fdfb")),
    amber: tone(rgb("#F59E0B"), rgb("#FCD34D"), rgb(245, 158, 11, 26), rgb(245, 158, 11, 71), rgb("#fffdf5")),
    red: tone(rgb("#F43F5E"), rgb("#FB7185"), rgb(244, 63, 94, 23), rgb(244, 63, 94, 61), rgb("#fff5f7")),
  ),
)

// See dark-yellow.typ: a blurred CSS glow blob as a soft radial patch.
#let glow(cx, cy, r, color) = place(top + left, dx: cx - r, dy: cy - r, rect(
  width: 2 * r,
  height: 2 * r,
  fill: gradient.radial((color, 0%), (color.transparentize(100%), 100%)),
))

// Page background: `--bg` sheet, the two `--bg-gradient` sky-blue glows, the
// `.slide-shell` primary corner radials (info aliases primary here), a static
// approximation of the ambient light blobs (preset 0), and the
// `.slide-shell__frame` hairline.
#let background(w, h) = {
  place(rect(width: w * px, height: h * px, fill: rgb("#020810")))
  // `--bg-gradient`: rgba(14,165,233,0.22) at 88% 12% fading at 42% (≈543px
  // of the farthest-corner distance), rgba(6,78,159,0.14) at 10% 85% fading at
  // 38% (≈496px).
  glow(0.88 * w * px, 0.12 * h * px, 543 * px, rgb(14, 165, 233, 56))
  glow(0.1 * w * px, 0.85 * h * px, 496 * px, rgb(6, 78, 159, 36))
  // `.slide-shell` radials (see dark-yellow for the geometry).
  glow(0.12 * w * px, 0.18 * h * px, 381 * px, rgb(14, 165, 233, 26))
  glow(0.84 * w * px, 0.12 * h * px, 324 * px, rgb(14, 165, 233, 26))
  // `.slide-shell__ambient` (preset 0), animation dropped; the fade radius
  // covers each blob's half-size plus its 42px blur.
  glow(52 * px + 130 * px, 38 * px + 110 * px, 170 * px, rgb(14, 165, 233, 16))
  glow(w * px - 74 * px - 90 * px, 56 * px + 90 * px, 130 * px, rgb(14, 165, 233, 9))
  glow(w * px - 138 * px - 110 * px, h * px - 52 * px - 70 * px, 150 * px, rgb(56, 189, 248, 6))
  place(dx: 14 * px, dy: 12 * px, rect(
    width: (w - 28) * px,
    height: (h - 24) * px,
    radius: 20 * px,
    stroke: 1 * px + rgb(14, 165, 233, 26), // `--shell-frame-border` rgba(14,165,233,0.1)
  ))
}
