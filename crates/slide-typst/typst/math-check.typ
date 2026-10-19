// Compile check for converted LaTeX math spans (see `Renderer::failing_math`).
// `/deck.json` here is an array of `{ source, display }`; each span is
// evaluated and laid out exactly as `rich()` in lib/base.typ does, so a span
// that passes here cannot fail the real export.
#import "vendor/mitex/specs/mod.typ": mitex-scope

#for span in json("deck.json") {
  math.equation(block: span.display, eval("$" + span.source + "$", scope: mitex-scope))
  parbreak()
}
