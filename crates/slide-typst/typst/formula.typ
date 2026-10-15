// One converted LaTeX formula on a transparent page sized to fit it (see
// `Renderer::render_formula`, which is the editor preview's formula image).
// `/deck.json` here is `{ source, display, fill, size_pt, fonts, math_fonts }`:
// the Typst math source mitex produced, whether it is block-level, the text
// color as `#rrggbb`, the size in points and the font fallback lists.
//
// The formula is `eval`ed exactly as `rich()` in lib/base.typ does it, so the
// preview and the slides typeset the same way. An eval error cannot be caught:
// a formula Typst rejects fails this compile, and the caller falls back to the
// LaTeX source (never to a broken export).
#import "/vendor/mitex/specs/mod.typ": mitex-scope

#let spec = json("/deck.json")

// `auto` page and `fill: none`: the page hugs the formula (plus the 1pt margin
// around it, which keeps tall accents and limits from being clipped) and the
// raster comes out with a transparent background.
#set page(width: auto, height: auto, margin: 1pt, fill: none)
// Math scales with the text size, so `size_pt` is the formula's font size.
#set text(font: spec.fonts, size: spec.size_pt * 1pt, fill: rgb(spec.fill), lang: "zh", region: "cn")
// Same rule as main.typ: equations need an OpenType MATH font, and Rust passes
// only the installed families (missing ones in a fallback list each warn).
#show math.equation: it => if spec.math_fonts.len() > 0 {
  set text(font: spec.math_fonts)
  it
} else { it }

// A block equation is centered in the free width by default, and an inline one
// keeps its natural width — both are exactly the page's content under `auto`.
#math.equation(
  block: spec.display,
  eval("$" + spec.source + "$", scope: mitex-scope),
)
