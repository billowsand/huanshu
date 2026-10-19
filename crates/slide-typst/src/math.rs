//! Rewrites LaTeX math spans (`$...$` / `$$...$$`) in deck strings into
//! sentinel-wrapped Typst math source, so `rich()` in `typst/lib/base.typ`
//! can `eval` them as equations. Conversion runs through the `mitex` crate;
//! spans that fail conversion keep their original text and render literally.
//!
//! The scanning rules mirror `splitMathSegments` in `src/utils/richText.ts`
//! and the protection in `crates/huanshu-core/src/generator/utils.rs`:
//! - `$$...$$` is display math and may span lines;
//! - `$...$` is inline math, cannot cross a newline, and its content must not
//!   start or end with whitespace (pandoc-style guard against `$5 and $10`);
//! - a closing `$` immediately followed by a digit does not count;
//! - `\$` is a literal dollar sign.
//!
//! The sentinel characters are stripped from every input string first: text
//! that already contained `\u{1}I…\u{2}` would otherwise reach `eval` as raw
//! Typst code.

use std::borrow::Cow;
use std::collections::HashSet;

use serde::Serialize;

/// Starts a converted math span; the byte after it is `I` (inline) or `D` (display).
pub const SPAN_OPEN: char = '\u{1}';
/// Ends a converted math span.
pub const SPAN_CLOSE: char = '\u{2}';

/// A converted math span as `rich()` will eval it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Span {
    /// Typst math source produced by mitex.
    pub source: String,
    pub display: bool,
}

/// Rewrites math spans in every string of the slide tree, in place, and
/// returns the spans it emitted. Spans whose converted source is in
/// `rejected` keep their original LaTeX text.
pub fn rewrite_deck(slides: &mut [serde_json::Value], rejected: &HashSet<String>) -> Vec<Span> {
    let mut spans = Vec::new();
    for slide in slides {
        rewrite_value(slide, rejected, &mut spans);
    }
    spans
}

fn rewrite_value(value: &mut serde_json::Value, rejected: &HashSet<String>, spans: &mut Vec<Span>) {
    match value {
        serde_json::Value::String(s) => *s = rewrite_str(s, rejected, spans),
        serde_json::Value::Array(items) => items
            .iter_mut()
            .for_each(|item| rewrite_value(item, rejected, spans)),
        serde_json::Value::Object(map) => map
            .values_mut()
            .for_each(|item| rewrite_value(item, rejected, spans)),
        _ => {}
    }
}

/// `input` without the sentinel characters, borrowed when there are none.
///
/// Every string that is scanned or converted goes through this first: text that
/// already contained `\u{1}I…\u{2}` would otherwise reach `eval` as raw Typst
/// code. [`convert_formula`] needs the very same rule, so it lives here.
pub(crate) fn strip_sentinels(input: &str) -> Cow<'_, str> {
    if input.contains([SPAN_OPEN, SPAN_CLOSE]) {
        Cow::Owned(input.replace([SPAN_OPEN, SPAN_CLOSE], ""))
    } else {
        Cow::Borrowed(input)
    }
}

/// Typst math source for one LaTeX formula with no `$` delimiters, or mitex's
/// error message when the LaTeX cannot be converted. The result is meant to be
/// `eval`ed as `$…$`, exactly like a converted span of [`rewrite_deck`];
/// whether Typst accepts it is only known at that point.
///
/// Sentinel characters are stripped first, so the caller's text can never
/// introduce markup ([`Renderer::render_formula`] uses this).
pub(crate) fn convert_formula(latex: &str) -> Result<String, String> {
    mitex::convert_math(&strip_sentinels(latex), None)
}

fn rewrite_str(input: &str, rejected: &HashSet<String>, spans: &mut Vec<Span>) -> String {
    let stripped = strip_sentinels(input);
    let input = stripped.as_ref();
    if !input.contains('$') {
        return input.to_owned();
    }
    let b = input.as_bytes();
    let n = b.len();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    let mut text_start = 0;

    while i < n {
        if b[i] == b'\\' && i + 1 < n && b[i + 1] == b'$' {
            i += 2;
            continue;
        }
        if b[i] != b'$' {
            i += 1;
            continue;
        }
        let display = i + 1 < n && b[i + 1] == b'$';
        let open_end = i + if display { 2 } else { 1 };
        let close = if display {
            find_display_close(b, open_end)
        } else {
            find_inline_close(b, open_end)
        };
        let Some(close) = close else {
            i = open_end;
            continue;
        };
        let raw = &input[open_end..close];
        let tex = if display { raw.trim() } else { raw };
        let valid = !tex.is_empty()
            && (display
                || (!tex.starts_with(char::is_whitespace) && !tex.ends_with(char::is_whitespace)));
        if !valid {
            i = open_end;
            continue;
        }
        let end = close + if display { 2 } else { 1 };
        match mitex::convert_math(tex, None) {
            Ok(source) if !rejected.contains(&source) => {
                push_text(&mut out, input, text_start, i);
                out.push(SPAN_OPEN);
                out.push(if display { 'D' } else { 'I' });
                out.push_str(&source);
                out.push(SPAN_CLOSE);
                spans.push(Span { source, display });
            }
            // LaTeX that mitex cannot convert, or whose conversion Typst rejects,
            // stays literal text instead of failing the export.
            _ => push_text(&mut out, input, text_start, end),
        }
        i = end;
        text_start = i;
    }
    push_text(&mut out, input, text_start, n);
    out
}

/// Emits `input[from..to]` with `\$` unescaped to `$`.
fn push_text(out: &mut String, input: &str, from: usize, to: usize) {
    let slice = &input[from..to];
    if slice.contains("\\$") {
        out.push_str(&slice.replace("\\$", "$"));
    } else {
        out.push_str(slice);
    }
}

/// Closing `$$` for display math; may span lines. `\$` is not a delimiter.
fn find_display_close(b: &[u8], from: usize) -> Option<usize> {
    let mut j = from;
    while j + 1 < b.len() {
        match (b[j], b[j + 1]) {
            (b'\\', b'$') => j += 2,
            (b'$', b'$') => return Some(j),
            _ => j += 1,
        }
    }
    None
}

/// Closing `$` for inline math: same line, and not immediately followed by a
/// digit (pandoc's currency guard). A `$$` pair never closes inline math.
fn find_inline_close(b: &[u8], from: usize) -> Option<usize> {
    let mut j = from;
    while j < b.len() {
        match b[j] {
            b'\n' => return None,
            b'\\' if j + 1 < b.len() && b[j + 1] == b'$' => j += 1,
            b'$' => {
                let next = b.get(j + 1);
                if next == Some(&b'$') {
                    j += 1;
                } else if next.is_some_and(|c| c.is_ascii_digit()) {
                    j += 1;
                } else {
                    return Some(j);
                }
            }
            _ => {}
        }
        j += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn converted(input: &str) -> String {
        rewrite_str(input, &HashSet::new(), &mut Vec::new())
    }

    #[test]
    fn inline_math_becomes_span() {
        let out = converted("速度 $v = \\frac{s}{t}$ 很快");
        assert!(out.contains(SPAN_OPEN), "{out:?}");
        assert!(out.contains('I'));
        assert!(!out.contains('$'), "{out:?}");
    }

    #[test]
    fn display_math_becomes_display_span() {
        let out = converted("$$E = mc^2$$");
        let start = out.find(SPAN_OPEN).unwrap();
        assert_eq!(
            out[start + SPAN_OPEN.len_utf8()..].chars().next(),
            Some('D')
        );
    }

    #[test]
    fn currency_stays_literal() {
        assert_eq!(converted("价格 $5 和 $10"), "价格 $5 和 $10");
        assert_eq!(converted("a $5 和$10 b"), "a $5 和$10 b");
    }

    #[test]
    fn unmatched_dollar_stays_literal() {
        assert_eq!(converted("只剩 $一个"), "只剩 $一个");
    }

    #[test]
    fn inline_math_cannot_span_lines() {
        assert_eq!(converted("$x\ny$"), "$x\ny$");
    }

    #[test]
    fn escaped_dollar_unescapes() {
        assert_eq!(converted("价格 \\$5 不变"), "价格 $5 不变");
    }

    #[test]
    fn invalid_latex_keeps_original_text() {
        let input = "公式 $\\badcmd{$ 保留";
        assert_eq!(converted(input), input);
    }

    #[test]
    fn rewrites_nested_strings() {
        let mut slides = vec![serde_json::json!({
            "title": "勾股定理 $a^2 + b^2 = c^2$",
            "cards": [{ "body": "$$x_{1,2} = \\frac{-b \\pm \\sqrt{b^2-4ac}}{2a}$$" }],
        })];
        let spans = rewrite_deck(&mut slides, &HashSet::new());
        assert_eq!(spans.len(), 2);
        let title = slides[0]["title"].as_str().unwrap();
        let body = slides[0]["cards"][0]["body"].as_str().unwrap();
        assert!(title.contains(SPAN_OPEN));
        assert!(body.contains('D'));
    }

    #[test]
    fn sentinel_chars_in_input_never_form_a_span() {
        let out = converted("\u{1}I#panic(\"x\")\u{2} 文本 $x$");
        assert_eq!(out.matches(SPAN_OPEN).count(), 1, "{out:?}");
        assert!(out.starts_with("I#panic"), "{out:?}");
    }

    #[test]
    fn convert_formula_strips_sentinels_like_a_span() {
        // `Renderer::render_formula` converts through here, so a formula the
        // caller wrapped in sentinels converts as its stripped self.
        assert_eq!(
            convert_formula("\u{1}\u{2}x^2").unwrap(),
            convert_formula("x^2").unwrap()
        );
        assert!(convert_formula("\\badcmd{").is_err());
    }

    #[test]
    fn rejected_span_stays_literal() {
        let mut spans = Vec::new();
        rewrite_str("a $x^2$ b", &HashSet::new(), &mut spans);
        let rejected: HashSet<String> = spans.into_iter().map(|s| s.source).collect();
        assert_eq!(
            rewrite_str("a $x^2$ b", &rejected, &mut Vec::new()),
            "a $x^2$ b"
        );
    }

    #[test]
    fn escaped_dollar_does_not_close_display_math() {
        let out = converted(r"$$x = \$$$");
        assert_eq!(out.matches(SPAN_OPEN).count(), 1, "{out:?}");
        assert!(!out.contains('$'), "{out:?}");
    }
}
