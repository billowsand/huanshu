// Compile-time interface check for the theme modules. Not part of main.typ;
// compiled on its own by `Renderer::check_theme_interface` (see src/lib.rs),
// which the `theme_dictionaries_share_one_interface` test calls. Every theme
// dictionary must expose the same keys so layouts can read any of them without
// a fallback.

#import "notes.typ" as notes
#import "light-terminal.typ" as light-terminal
#import "light-editorial.typ" as light-editorial
#import "dark-terminal.typ" as dark-terminal
#import "dark-yellow.typ" as dark-yellow
#import "dark-blue.typ" as dark-blue

#let modules = (notes, light-terminal, light-editorial, dark-terminal, dark-yellow, dark-blue)
#let keyset(d) = d.keys().sorted()

#let theme-keys = keyset(notes.theme)
#for m in modules {
  assert.eq(keyset(m.theme), theme-keys, message: m.theme.name + " 的 theme 键集合与 light-notes 不一致")
  assert(
    m.theme.family in ("notes", "terminal", "glass", "editorial"),
    message: m.theme.name + " 的 family 必须是 notes / terminal / glass / editorial 之一",
  )
  // Each theme module also provides the page background painter.
  assert(type(m.background) == function, message: m.theme.name + " 缺少 background(w, h)")
}

#for sub in ("lead", "radius", "tracking", "deco", "tones", "chart") {
  let sub-keys = keyset(notes.theme.at(sub))
  for m in modules {
    assert.eq(keyset(m.theme.at(sub)), sub-keys, message: m.theme.name + "." + sub + " 的键集合不一致")
  }
}

// The chart palette is indexed by series / sector, so every theme must define
// exactly six colours: `palette(i)` in lib/chart.typ wraps around, and a short
// list would silently repeat colours inside one chart.
#for m in modules {
  assert.eq(
    m.theme.chart.palette.len(),
    6,
    message: m.theme.name + ".chart.palette 必须恰好 6 色，实际 " + str(m.theme.chart.palette.len()),
  )
  for c in m.theme.chart.palette {
    assert(type(c) == color, message: m.theme.name + ".chart.palette 里有非颜色值")
  }
}

#let tone-keys = keyset(notes.theme.tones.neutral)
#for m in modules {
  for (name, tone) in m.theme.tones {
    assert.eq(keyset(tone), tone-keys, message: m.theme.name + ".tones." + name + " 的键集合不一致")
  }
}
