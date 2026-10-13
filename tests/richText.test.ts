import { describe, expect, test } from 'bun:test'
import { renderRichText, splitMathSegments } from '../src/utils/richText'

describe('splitMathSegments', () => {
  test('splits inline and display math', () => {
    expect(splitMathSegments('a $x^2$ b $$E=mc^2$$ c')).toEqual([
      { kind: 'text', text: 'a ' },
      { kind: 'math', tex: 'x^2', display: false },
      { kind: 'text', text: ' b ' },
      { kind: 'math', tex: 'E=mc^2', display: true },
      { kind: 'text', text: ' c' },
    ])
  })

  test('keeps currency-like dollar pairs as text', () => {
    expect(splitMathSegments('价格是 $5 和 $10')).toEqual([
      { kind: 'text', text: '价格是 $5 和 $10' },
    ])
    expect(splitMathSegments('a $5 和$10 b')).toEqual([
      { kind: 'text', text: 'a $5 和$10 b' },
    ])
  })

  test('unmatched dollar stays literal', () => {
    expect(splitMathSegments('只剩 $一个')).toEqual([
      { kind: 'text', text: '只剩 $一个' },
    ])
  })

  test('inline math cannot span lines', () => {
    expect(splitMathSegments('$x\ny$')).toEqual([{ kind: 'text', text: '$x\ny$' }])
  })

  test('display math trims surrounding whitespace and may span lines', () => {
    expect(splitMathSegments('$$\n x^2 + y^2 \n$$')).toEqual([
      { kind: 'math', tex: 'x^2 + y^2', display: true },
    ])
  })

  test('escaped dollar renders as a literal dollar', () => {
    expect(renderRichText('价格 \\$5 不变')).toBe('价格 $5 不变')
  })

  test('escaped dollar inside math stays the TeX command', () => {
    expect(splitMathSegments('$a \\$ b$')).toEqual([
      { kind: 'math', tex: 'a \\$ b', display: false },
    ])
  })
})

describe('renderRichText', () => {
  test('renders inline math as katex html', () => {
    const html = renderRichText('速度 $v = \\frac{s}{t}$ 很快')
    expect(html).toContain('class="katex"')
    expect(html).not.toContain('$')
    expect(html).toContain('速度')
  })

  test('renders display math with display wrapper', () => {
    const html = renderRichText('$$E = mc^2$$')
    expect(html).toContain('katex-display')
  })

  test('falls back to escaped source on invalid latex', () => {
    expect(renderRichText('公式 $\\badcmd{$ 保留')).toBe('公式 $\\badcmd{$ 保留')
  })

  test('escapes html but keeps the allowed tag subset', () => {
    const html = renderRichText('<strong>重点</strong><br><script>alert(1)</script>')
    expect(html).toBe('<strong>重点</strong><br>&lt;script&gt;alert(1)&lt;/script&gt;')
  })

  test('drops attributes from allowed tags', () => {
    expect(renderRichText('<b onmouseover="alert(1)">x</b><BR/>')).toBe('<b>x</b><br>')
    expect(renderRichText('<img src=x onerror=alert(1)>')).toBe('&lt;img src=x onerror=alert(1)&gt;')
  })

  test('escapes ampersand in text', () => {
    expect(renderRichText('A & B')).toBe('A &amp; B')
  })

  test('handles nullish and non-string values', () => {
    expect(renderRichText(null)).toBe('')
    expect(renderRichText(undefined)).toBe('')
    expect(renderRichText(42)).toBe('42')
  })
})
