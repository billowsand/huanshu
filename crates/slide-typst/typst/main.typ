// Entry point. The Rust world serves `/deck.json`:
//   { theme, aspect_ratio, fonts: { title: [..], subtitle, body, caption, number, code },
//     assets: { "<image reference>": (path, width, height, vector) }, slides: [SlideBlueprint] }

#import "lib/base.typ": *
#import "lib/components.typ": frame-inset, unsupported-page
#import "themes/notes.typ" as notes
#import "themes/light-terminal.typ" as light-terminal
#import "themes/light-editorial.typ" as light-editorial
#import "themes/dark-terminal.typ" as dark-terminal
#import "themes/dark-yellow.typ" as dark-yellow
#import "themes/dark-blue.typ" as dark-blue
#import "layouts/cover.typ" as cover
#import "layouts/feature-grid.typ" as feature-grid
#import "layouts/step-flow.typ" as step-flow
#import "layouts/compare.typ" as compare
#import "layouts/swot.typ" as swot
#import "layouts/spotlight.typ" as spotlight
#import "layouts/section-intro.typ" as section-intro
#import "layouts/issue-stack.typ" as issue-stack
#import "layouts/closing.typ" as closing
#import "layouts/overview.typ" as overview
#import "layouts/timeline.typ" as timeline
#import "layouts/process.typ" as process
#import "layouts/split-layers.typ" as split-layers
#import "layouts/section-list.typ" as section-list
#import "layouts/focus-example.typ" as focus-example
#import "layouts/outcome-grid.typ" as outcome-grid
#import "layouts/center-grid.typ" as center-grid
#import "layouts/chart-focus.typ" as chart-focus

#let deck = json("/deck.json")

// 便签·稿纸, the terminal themes, 报刊·钴蓝 and the dark glass themes are
// ported; every other theme id falls back to light-notes.
#let themes = (
  "light-notes": notes,
  "light-terminal": light-terminal,
  "light-editorial": light-editorial,
  "dark-terminal": dark-terminal,
  "dark-yellow": dark-yellow,
  "dark-blue": dark-blue,
)
#let theme-module = themes.at(deck.at("theme", default: "light-notes"), default: notes)

#let layouts = (
  cover: cover.render,
  feature_grid: feature-grid.render,
  step_flow: step-flow.render,
  compare: compare.render,
  swot: swot.render,
  spotlight: spotlight.render,
  section_intro: section-intro.render,
  issue_stack: issue-stack.render,
  closing: closing.render,
  overview: overview.render,
  timeline: timeline.render,
  process: process.render,
  split_layers: split-layers.render,
  section_list: section-list.render,
  focus_example: focus-example.render,
  outcome_grid: outcome-grid.render,
  center_grid: center-grid.render,
  chart_focus: chart-focus.render,
)

#let canvas = canvas-sizes.at(deck.at("aspect_ratio", default: "ratio_16x9"), default: canvas-sizes.ratio_16x9)

#set document(title: deck.at("title", default: none))
#set page(
  width: canvas.w * px,
  height: canvas.h * px,
  margin: (
    left: frame-inset.left * px,
    right: frame-inset.right * px,
    top: frame-inset.top * px,
    bottom: frame-inset.bottom * px,
  ),
  background: (theme-module.background)(canvas.w, canvas.h),
)
// Every line box is exactly 1em tall around the baseline; `line-box` adds the
// CSS half-leading, so vertical rhythm follows the browser's line-height model.
#set text(
  font: deck.fonts.body,
  lang: "zh",
  region: "cn",
  weight: "regular",
  top-edge: 0.88em,
  bottom-edge: -0.12em,
  fill: theme-module.theme.ink,
)
#set par(spacing: 0pt, justify: false)
#set block(spacing: 0pt)
#set stack(spacing: 0pt)

// Equations need an OpenType MATH font. Rust passes only the installed ones of
// its `MATH_FONTS` list (missing families in a fallback list each warn).
#show math.equation: it => if deck.math_fonts.len() > 0 {
  set text(font: deck.math_fonts + deck.fonts.body)
  it
} else { it }

#for (index, slide) in deck.slides.enumerate() {
  if index > 0 { pagebreak() }
  let scale = slide-type-scale(slide)
  let ctx = (
    theme: theme-module.theme,
    ty: type-sizes(scale),
    fonts: deck.fonts,
    canvas: canvas,
    index: index,
    // `--type-scale` of this slide (chart_focus reports it to the chart SSR).
    scale: scale,
    // Image reference → (path, width, height, vector); see src/assets.rs.
    assets: deck.at("assets", default: (:)),
  )
  let render = layouts.at(slide.kind, default: none)
  // A real page folio, outside the content area, shared by every editorial layout.
  if theme-module.theme.family == "editorial" {
    place(top + left, dy: (canvas.h - frame-inset.top - 32) * px, block(
      width: (canvas.w - frame-inset.left - frame-inset.right) * px,
      stroke: (top: 1 * px + theme-module.theme.edge),
      inset: (top: 8 * px),
      grid(columns: (1fr, auto),
        text(font: deck.fonts.caption, size: 11 * px, fill: theme-module.theme.muted, "幻述 / PRESENTATION"),
        text(font: deck.fonts.number, size: 11 * px, fill: theme-module.theme.muted, if index < 9 { "0" + str(index + 1) } else { str(index + 1) }),
      ),
    ))
  }
  if render == none { unsupported-page(ctx, slide) } else { render(ctx, slide) }
}
