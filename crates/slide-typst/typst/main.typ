// Entry point. The Rust world serves `/deck.json`:
//   { theme, aspect_ratio, fonts: { title: [..], subtitle, body, caption, number, code },
//     assets: { "<image reference>": (path, width, height, vector) }, slides: [SlideBlueprint] }

#import "lib/base.typ": *
#import "lib/components.typ": frame-inset, unsupported-page
#import "themes/notes.typ" as notes
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

#let deck = json("/deck.json")

// Only 便签·稿纸 is ported so far; every theme id resolves to it until the
// terminal themes get their own token files.
#let themes = ("light-notes": notes)
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

#for (index, slide) in deck.slides.enumerate() {
  if index > 0 { pagebreak() }
  let scale = slide-type-scale(slide)
  let ctx = (
    theme: theme-module.theme,
    ty: type-sizes(scale),
    fonts: deck.fonts,
    canvas: canvas,
    index: index,
    // Image reference → (path, width, height, vector); see src/assets.rs.
    assets: deck.at("assets", default: (:)),
  )
  let render = layouts.at(slide.kind, default: none)
  if render == none { unsupported-page(ctx, slide) } else { render(ctx, slide) }
}
