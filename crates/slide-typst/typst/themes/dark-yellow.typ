// 暗·琥珀 (dark-yellow). Token values are the `[data-theme="dark-yellow"]`
// variables in src/style.css (L455-514), confirmed against the preview page
// with measure()/getComputedStyle; keep the two in sync when the CSS changes.
//
// Unlike light-notes and the terminal themes there is no per-layout CSS for
// this theme: every layout falls back to the raw Vue template styles — flat
// `var(--glass)` panels (GlassCard sets the fill inline), 12px card radius,
// 999px pills, and primary→transparent gradient rules.

#import "../lib/base.typ": px

#let tone(solid, text, bg, border, contrast) = (
  solid: solid,
  text: text, // `--*-light` — toneVars().text resolves to the light variant
  bg: bg,
  border: border,
  contrast: contrast, // `--*-contrast`
)

#let glass = rgb(255, 255, 255, 9) // `--glass` rgba(255,255,255,0.035)
#let glass-border = rgb(255, 255, 255, 19) // `--glass-border` rgba(255,255,255,0.075)

#let theme = (
  name: "dark-yellow",
  // Style family: which CSS layer styles the slides — "notes" (notes-layouts.css),
  // "terminal" (terminal.css), "glass" (raw template styles in style.css).
  // Layouts branch on it via `is-terminal` / `is-glass` (lib/components.typ).
  family: "glass",
  // `.glass-card` under this theme is a flat `var(--glass)` fill (the GlassCard
  // component's inline style overrides the class gradient).
  paper: glass,
  card: glass,
  ink: rgb("#f5edda"),
  muted: rgb(245, 237, 218, 153), // rgba(245,237,218,0.6)
  edge: glass-border,
  primary: rgb("#f59e0b"),
  primary-bg: rgb(245, 158, 11, 31), // rgba(245,158,11,0.12)
  primary-border: rgb(245, 158, 11, 71), // rgba(245,158,11,0.28)
  // `--primary-light`.
  primary-strong: rgb("#fcd34d"),
  // `.spotlight-media`: rgba(255,255,255,0.03) behind rgba(255,255,255,0.08).
  media-bg: rgb(255, 255, 255, 8),
  // No row striping: non-highlighted `.cp-item` rows are transparent.
  stripe: none,
  // Overview note / annotation sheets are plain glass panels in this theme.
  sheet-tint: glass,
  sheet: glass,
  // No certificate outline (deco.certificate: false).
  outline: none,
  section-num-stroke: rgb(245, 158, 11, 31), // rgba(245,158,11,0.12)
  // Unused (deco.tape: false); kept so every theme dictionary has the same keys.
  tape: rgb(245, 158, 11, 31),
  tape-edge: rgb(245, 158, 11, 71),
  // `.section-intro__lead`: glass panel with a faint success tint; the badge is
  // TagBadge color=green, the meta divider is rgba(255,255,255,0.08) and the
  // note rule rgba(52,211,153,0.35) (all raw template styles).
  lead: (
    fill: gradient.linear(
      (rgb(16, 185, 129, 26), 0%), // color-mix(success 10%, transparent)
      (rgb(16, 185, 129, 0), 55%),
      (rgb(16, 185, 129, 0), 100%),
      angle: 70deg, // CSS 160deg
    ),
    border: glass-border,
    ink: rgb("#f5edda"),
    soft: rgb(255, 255, 255, 107), // `.section-intro__meta-label` rgba(255,255,255,0.42)
    rule: rgb(255, 255, 255, 20), // rgba(255,255,255,0.08)
    note-rule: rgb(52, 211, 153, 89), // rgba(52,211,153,0.35)
    badge-bg: rgb(16, 185, 129, 26),
    badge-border: rgb(16, 185, 129, 64),
    badge-ink: rgb("#34d399"),
  ),
  // Unused on this theme (deco.ruled: false); kept for parity with light-notes.
  rule: rgb(255, 255, 255, 13),
  margin-fill: glass,
  margin-line: glass-border,
  // `.glass-card` / annotation sheets / `.tag-pill`.
  radius: (card: 12, sheet: 12, pill: 999),
  // `.slide-title` / `.tag-pill` letter spacing.
  tracking: (title: -0.02em, label: 0.05em),
  deco: (
    ruled: false,
    tape: false,
    double-frame: false,
    certificate: false,
    stamp: "single",
    section-num: "outline",
  ),
  // data-tone resolves through toneVars() to the --info/--success/--danger/
  // --warning variables; warning aliases the brand (intentional, see the CSS).
  // TagBadge without a color uses the primary family, which is `neutral` here.
  tones: (
    neutral: tone(rgb("#f59e0b"), rgb("#fcd34d"), rgb(245, 158, 11, 31), rgb(245, 158, 11, 71), rgb("#fffaf0")),
    blue: tone(rgb("#818cf8"), rgb("#a5b4fc"), rgb(129, 140, 248, 26), rgb(129, 140, 248, 64), rgb("#f8faff")),
    green: tone(rgb("#10b981"), rgb("#34d399"), rgb(16, 185, 129, 26), rgb(16, 185, 129, 64), rgb("#f8fffc")),
    amber: tone(rgb("#f59e0b"), rgb("#fcd34d"), rgb(245, 158, 11, 31), rgb(245, 158, 11, 71), rgb("#fffaf0")),
    red: tone(rgb("#f43f5e"), rgb("#fb7185"), rgb(244, 63, 94, 26), rgb(244, 63, 94, 64), rgb("#fff5f7")),
  ),
)

// A blurred CSS glow blob approximated by a soft radial patch: a square rect
// (side 2R) centered on (cx, cy) with a circular gradient that reaches full
// transparency exactly at R, so the square's corners are always clear and the
// fade distance does not depend on how Typst resolves relative gradient stops.
#let glow(cx, cy, r, color) = place(top + left, dx: cx - r, dy: cy - r, rect(
  width: 2 * r,
  height: 2 * r,
  fill: gradient.radial((color, 0%), (color.transparentize(100%), 100%)),
))

// Page background: `--bg` sheet, the `--bg-gradient` warm glow at 20% 30%, the
// `.slide-shell` primary/info corner radials, a static approximation of the
// ambient light blobs (preset 0 positions; the screen animates them per slide
// with blur(42px), which PDF cannot reproduce), and the `.slide-shell__frame`
// hairline (shell padding 12px top/bottom, 14px left/right, radius 20px).
// The `--shell-frame` top sheen (≈1% white) is below visibility and omitted.
#let background(w, h) = {
  place(rect(width: w * px, height: h * px, fill: rgb("#09090f")))
  // `--bg-gradient` (ellipse ≈ circle on 16:9; the 60% stop sits at 685px from
  // the focal point — 60% of the farthest-corner distance).
  glow(0.2 * w * px, 0.3 * h * px, 685 * px, rgb(125, 85, 10, 46))
  // `.slide-shell` radials: primary 10% at 12% 18% (fade 30% ≈ 381px),
  // info 10% at 84% 12% (fade 26% ≈ 324px).
  glow(0.12 * w * px, 0.18 * h * px, 381 * px, rgb(245, 158, 11, 26))
  glow(0.84 * w * px, 0.12 * h * px, 324 * px, rgb(129, 140, 248, 26))
  // `.slide-shell__ambient` (preset 0): blurred blobs at 16%/14%/12% tint with
  // per-preset opacity; the drift animation is dropped. The fade radius covers
  // the blob's half-size plus its 42px blur.
  glow(52 * px + 130 * px, 38 * px + 110 * px, 170 * px, rgb(245, 158, 11, 16))
  glow(w * px - 74 * px - 90 * px, 56 * px + 90 * px, 130 * px, rgb(129, 140, 248, 9))
  glow(w * px - 138 * px - 110 * px, h * px - 52 * px - 70 * px, 150 * px, rgb(252, 211, 77, 6))
  place(dx: 14 * px, dy: 12 * px, rect(
    width: (w - 28) * px,
    height: (h - 24) * px,
    radius: 20 * px,
    stroke: 1 * px + rgb(255, 255, 255, 15), // `--shell-frame-border` rgba(255,255,255,0.06)
  ))
}
