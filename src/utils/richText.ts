import katex from 'katex'

// Shared rich-text renderer for blueprint string fields: escapes HTML, keeps
// the small tag subset that Typst's `rich()` understands (<br>, <strong|b|em|i|mark>),
// and renders $...$ / $$...$$ as KaTeX. Output is injected via v-html.

// Attributes are matched but never emitted: `<b onmouseover=...>` must not
// reach v-html, only the bare tag does.
const ALLOWED_TAG_RE = /<br\s*\/?>|<(\/?)(strong|b|em|i|mark)(?:\s[^>]*)?>/gi

const ESCAPED_DOLLAR = '\x0bDOLLAR\x0b'

type Segment =
  | { kind: 'text', text: string }
  | { kind: 'math', tex: string, display: boolean }

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
}

function renderTextSegment(s: string): string {
  const tags: string[] = []
  const protectedText = s.replace(ALLOWED_TAG_RE, (tag, slash?: string, name?: string) => {
    tags.push(name ? `<${slash}${name.toLowerCase()}>` : '<br>')
    return `\x0bTAG${tags.length - 1}\x0b`
  })
  return escapeHtml(protectedText).replace(/\x0bTAG(\d+)\x0b/g, (_, i) => tags[Number(i)])
}

function renderMathSegment(tex: string, display: boolean, raw: string): string {
  try {
    return katex.renderToString(tex, { displayMode: display, throwOnError: true, output: 'html' })
  } catch {
    return escapeHtml(raw)
  }
}

// Splits on $$...$$ first, then $...$ with pandoc-ish guards: an opening $
// must not be followed by whitespace, a closing $ must not be preceded by
// whitespace, and inline math must stay on one line. \$ is a literal dollar.
export function splitMathSegments(input: string): Segment[] {
  const s = input.replace(/\\\$/g, ESCAPED_DOLLAR)
  const segments: Segment[] = []
  let i = 0
  let textStart = 0

  const pushText = (end: number) => {
    if (end > textStart)
      segments.push({ kind: 'text', text: s.slice(textStart, end) })
  }

  while (i < s.length) {
    if (s[i] !== '$') {
      i++
      continue
    }
    const display = s[i + 1] === '$'
    const openEnd = i + (display ? 2 : 1)
    const close = display ? s.indexOf('$$', openEnd) : findInlineClose(s, openEnd)
    if (close === -1) {
      i = openEnd
      continue
    }
    let tex = s.slice(openEnd, close)
    if (display) {
      tex = tex.trim()
      if (!tex) {
        i = openEnd
        continue
      }
    } else if (!tex || /^\s|\s$/.test(tex)) {
      i = openEnd
      continue
    }
    pushText(i)
    segments.push({ kind: 'math', tex, display })
    i = close + (display ? 2 : 1)
    textStart = i
  }
  pushText(s.length)

  // In text `\$` is a literal dollar; inside math it stays the TeX command `\$`.
  return segments.map(seg => seg.kind === 'text'
    ? { ...seg, text: seg.text.split(ESCAPED_DOLLAR).join('$') }
    : { ...seg, tex: seg.tex.split(ESCAPED_DOLLAR).join('\\$') })
}

function findInlineClose(s: string, from: number): number {
  for (let j = from; j < s.length; j++) {
    if (s[j] === '\n') return -1
    if (s[j] !== '$') continue
    const next = s[j + 1]
    // `$$` never closes inline math, and a digit right after the closing $
    // means currency (pandoc's guard), keep scanning in both cases.
    if (next !== '$' && !(next !== undefined && next >= '0' && next <= '9')) return j
    j++
  }
  return -1
}

export function renderRichText(value: unknown): string {
  if (value === null || value === undefined) return ''
  const s = String(value)
  if (!s) return ''
  return splitMathSegments(s)
    .map((seg) => {
      if (seg.kind === 'text') return renderTextSegment(seg.text)
      const raw = seg.display ? `$$${seg.tex}$$` : `$${seg.tex}$`
      return renderMathSegment(seg.tex, seg.display, raw)
    })
    .join('')
}
