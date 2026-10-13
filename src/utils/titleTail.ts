// Two-tone headline split for the editorial theme (light-editorial): the title
// breaks after its first full-width comma / colon / bar or em dash, and the
// theme CSS colors the tail and starts it on a new line. Other themes render
// the tail span unstyled. The Typst port (`title-tail` in
// crates/slide-typst/typst/lib/components.typ) uses the same rule.
// ASCII `:` / `|` only count before whitespace, so `10:30` never splits; the
// pattern has no lookahead because the Typst side uses Rust's regex crate.
const TITLE_BREAK_RE = /[，：｜]|——|[:|]\s/

/** UTF-16 offset where the tail starts, or null when the title has no split. */
export function titleTailOffset(title: string): number | null {
  // Math titles render as one atomic span (see SlideTitle.vue).
  if (title.includes('$')) return null
  const match = TITLE_BREAK_RE.exec(title)
  if (!match) return null
  // The tail starts after any whitespace that follows the punctuation, so it
  // never begins its own line with a space (the title is `pre-wrap`).
  let cut = match.index + match[0].length
  while (cut < title.length && /\s/.test(title[cut])) cut++
  if (!title.slice(0, match.index).trim() || cut >= title.length) return null
  return cut
}
