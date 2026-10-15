//! 文稿工具：原版 `Step1Prepare.vue` / `MarkdownWorkbench.vue` 里的一批纯文本规则。
//!
//! **改这里必须改原版**（`src/views/steps/Step1Prepare.vue`、`src/components/MarkdownWorkbench.vue`），
//! 反过来也一样——两边用 `tests/markdown_tools.rs` 对着
//! `tests/fixtures/markdown-tools.json` 快照逐条比对（快照由 `target/gen-markdown-tools.ts`
//! 从原版 TypeScript **逐字拷贝**的函数生成）。
//!
//! 移植注意（JS 与 Rust 正则的语义差，快照已覆盖）：
//! - JS 的 `\d` 永远是 ASCII 数字，Rust 的 `\d` 默认匹配 Unicode 数字——这里一律写 `[0-9]`；
//! - JS 的 `.` 不匹配 `\r`，Rust 的 `.` 只排除 `\n`——跨平台换行的捕获用 `[^\r\n]`；
//! - `extract_h1` 的 JS 正则带 `m`，`$` 能停在 `\r` 前，Rust 的 `(?m)$` 只停在 `\n` 前，要补 `\r?`。

use regex::Regex;

/// 分页粒度（原版 `'auto' | 'h2' | 'h3'`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Granularity {
    Auto,
    H2,
    H3,
}

impl Granularity {
    /// 解析原版存进 `localStorage` / 项目附加信息里的字符串；非法值返回 `None`。
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "auto" => Some(Self::Auto),
            "h2" => Some(Self::H2),
            "h3" => Some(Self::H3),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::H2 => "h2",
            Self::H3 => "h3",
        }
    }
}

/// 编号样式（原版 `HeadingNumberStyle`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeadingNumberStyle {
    /// `一、二、`
    CnSection,
    /// `（一）（二）`
    CnSubsection,
    /// `1. 2.`
    Decimal,
}

fn re(pattern: &str) -> Regex {
    Regex::new(pattern).expect("markdown_tools: static regex")
}

/// 与 JS `s.split(/\r?\n/)` 完全一致的行切分：结尾有换行时会多出一个空串元素
/// （`"".split` 得到 `[""]`），函数输出的末尾换行因此与原版逐字一致。
fn split_lines(s: &str) -> Vec<&str> {
    if s.is_empty() {
        return vec![""];
    }
    let mut lines: Vec<&str> = s.lines().collect();
    if s.ends_with('\n') {
        lines.push("");
    }
    lines
}

/// 原版 `countExactHeadings`：只数「正好是这一级」的标题（行首 `#` 数精确、后面非 `#`）。
pub fn count_exact_headings(raw: &str, level: u8) -> usize {
    let pattern = match level {
        2 => r"^##\s+[^#]",
        3 => r"^###\s+[^#]",
        other => panic!("count_exact_headings: level must be 2 or 3, got {other}"),
    };
    let re = re(pattern);
    split_lines(raw)
        .iter()
        .filter(|line| {
            let trimmed = line.trim();
            // 行尾 \r 会被 trim 掉，与原版先 trim 再 test 一致。
            re.is_match(trimmed)
        })
        .count()
}

/// 原版 `parseSections` 的一节。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ParsedSection {
    pub paragraphs: Vec<String>,
    pub subsections: Vec<ParsedSubsection>,
}

/// 原版 `parseSections` 的小节（H3）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ParsedSubsection {
    pub paragraphs: Vec<String>,
}

/// 原版 `parseSections`：H1 / `---` 是边界，H2 开节、H3 开小节，空行分段。
pub fn parse_sections(raw: &str) -> Vec<ParsedSection> {
    struct Parser<'a> {
        sections: Vec<ParsedSection>,
        current_section: Option<ParsedSection>,
        current_subsection: Option<ParsedSubsection>,
        paragraph_buffer: Vec<&'a str>,
    }

    impl<'a> Parser<'a> {
        fn flush_paragraph(&mut self) {
            if self.paragraph_buffer.is_empty() {
                return;
            }
            let chunk = self.paragraph_buffer.join("\n").trim().to_string();
            self.paragraph_buffer.clear();
            if chunk.is_empty() {
                return;
            }
            if let Some(sub) = self.current_subsection.as_mut() {
                sub.paragraphs.push(chunk);
            } else if let Some(sec) = self.current_section.as_mut() {
                sec.paragraphs.push(chunk);
            }
        }

        fn flush_subsection(&mut self) {
            let (Some(_), Some(sub)) = (&self.current_section, self.current_subsection.take())
            else {
                return;
            };
            self.current_section
                .as_mut()
                .expect("checked above")
                .subsections
                .push(sub);
        }

        fn flush_section(&mut self) {
            if let Some(sec) = self.current_section.take() {
                self.sections.push(sec);
            }
        }
    }

    let h2_re = re(r"^##\s+[^#]");
    let h3_re = re(r"^###\s+[^#]");

    let mut parser = Parser {
        sections: Vec::new(),
        current_section: None,
        current_subsection: None,
        paragraph_buffer: Vec::new(),
    };

    for raw_line in split_lines(raw).iter() {
        // 原版的 split(/\r?\n/)：Rust 的 lines() 同样吃掉 \r\n 的 \r，等价。
        let line = raw_line.trim();
        if line == "---" {
            parser.flush_paragraph();
            continue;
        }
        if line.starts_with("# ") && !line.starts_with("## ") {
            parser.flush_paragraph();
            parser.flush_subsection();
            parser.flush_section();
            continue;
        }
        if h2_re.is_match(line) {
            parser.flush_paragraph();
            parser.flush_subsection();
            parser.flush_section();
            parser.current_section = Some(ParsedSection {
                paragraphs: Vec::new(),
                subsections: Vec::new(),
            });
            continue;
        }
        if h3_re.is_match(line) {
            parser.flush_paragraph();
            parser.flush_subsection();
            // 原版在没有节时也开一个小节，它只是挂不进去、最终被丢弃——一致。
            parser.current_subsection = Some(ParsedSubsection {
                paragraphs: Vec::new(),
            });
            continue;
        }
        if line.is_empty() {
            parser.flush_paragraph();
            continue;
        }
        parser.paragraph_buffer.push(line);
    }

    parser.flush_paragraph();
    parser.flush_subsection();
    parser.flush_section();
    parser.sections
}

/// 原版 `estimateSlides`：`auto` 按「二级标题数 > 三级标题数」取 h2，否则 h3。
pub fn estimate_slides(raw: &str, mode: Granularity) -> usize {
    let sections = parse_sections(raw);
    if sections.is_empty() {
        return 0;
    }
    let resolved = match mode {
        Granularity::Auto => detect_granularity(raw),
        Granularity::H2 => Granularity::H2,
        Granularity::H3 => Granularity::H3,
    };
    if resolved == Granularity::H2 {
        sections.len()
    } else {
        sections
            .iter()
            .map(|section| 1 + section.subsections.len())
            .sum()
    }
}

/// 原版 `detectedGranularityLocal`：二级标题数 > 三级标题数 → h2，否则 h3。
pub fn detect_granularity(raw: &str) -> Granularity {
    if count_exact_headings(raw, 2) > count_exact_headings(raw, 3) {
        Granularity::H2
    } else {
        Granularity::H3
    }
}

/// 原版 `MarkdownWorkbench.vue` 的 `splitSlides`：按粒度切成预览页，`$$` 公式块里的
/// `##` 不切页；`$$...$$` 单行会开关两次。
pub fn split_preview_pages(raw: &str, granularity: Granularity) -> Vec<String> {
    let h2_re = re(r"^##\s+[^#]");
    let h3_re = re(r"^###\s+[^#]");

    let mut slides: Vec<String> = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    let mut in_display_math = false;

    let flush = |current: &mut Vec<&str>, slides: &mut Vec<String>| {
        let chunk = current.join("\n").trim().to_string();
        if !chunk.is_empty() {
            slides.push(chunk);
        }
        current.clear();
    };

    for line in split_lines(raw).iter() {
        let trimmed = line.trim();
        if trimmed.starts_with("$$") {
            let fences = line.split("$$").count() - 1;
            if fences % 2 == 1 {
                in_display_math = !in_display_math;
            }
        }
        let is_h2 = h2_re.is_match(trimmed);
        let is_h3 = h3_re.is_match(trimmed);
        let should_split = !in_display_math
            && match granularity {
                Granularity::H2 => is_h2,
                _ => is_h2 || is_h3,
            };

        if should_split && !current.join("\n").trim().is_empty() {
            flush(&mut current, &mut slides);
        }

        current.push(line);
    }

    flush(&mut current, &mut slides);
    slides
}

/// 原版 `stripHeadingNumberPrefix` 的那条正则（逐字翻译）。
/// 注意 `\d` 在 JS 里是 ASCII-only，这里写 `[0-9]`。
pub fn strip_heading_number_prefix(title: &str) -> String {
    let re = re(
        r"^(?:第[一二三四五六七八九十百千万零〇两0-9]+\s*[章节部分篇讲]|[一二三四五六七八九十百千万零〇两]+[、.．:：]|（[一二三四五六七八九十百千万零〇两0-9]+）|\([一二三四五六七八九十百千万零〇两0-9]+\)|[0-9]+[、.．)\]:：-]|[A-Za-z][、.．)\]:：-])\s*",
    );
    re.replace(title.trim(), "").to_string()
}

/// 原版 `toChineseNumber`。
pub fn to_chinese_number(value: i64) -> String {
    const DIGITS: [&str; 10] = ["零", "一", "二", "三", "四", "五", "六", "七", "八", "九"];
    if value <= 10 {
        if value == 10 {
            return "十".to_string();
        }
        return DIGITS[value.max(0) as usize].to_string();
    }
    if value < 20 {
        return format!("十{}", DIGITS[(value % 10) as usize]);
    }
    if value < 100 {
        let tens = value / 10;
        let ones = value % 10;
        return format!(
            "{}十{}",
            DIGITS[tens as usize],
            if ones != 0 { DIGITS[ones as usize] } else { "" }
        );
    }
    value.to_string()
}

/// 原版 `addHeadingNumbers`：给指定级别的标题统一编号（编号计数会被其他级别重置）。
pub fn add_heading_numbers(md: &str, level: u8, style: HeadingNumberStyle) -> String {
    let heading_re = re(r"^(\s*)(#{1,6})\s+(.+)$");
    let mut h2_index = 0i64;
    let mut h3_index = 0i64;
    let mut h4_index = 0i64;

    let next: Vec<String> = split_lines(md)
        .into_iter()
        .map(|line| {
            let Some(caps) = heading_re.captures(line) else {
                return line.to_string();
            };
            let indent = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let hashes = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            let raw_title =
                strip_heading_number_prefix(caps.get(3).map(|m| m.as_str()).unwrap_or(""));
            let current_level = hashes.len() as u8;

            if current_level == 2 {
                h2_index += 1;
                h3_index = 0;
                h4_index = 0;
            } else if current_level == 3 {
                h3_index += 1;
                h4_index = 0;
            } else if current_level == 4 {
                h4_index += 1;
            }

            if current_level != level {
                return line.to_string();
            }

            let index = match level {
                2 => h2_index,
                3 => h3_index,
                _ => h4_index,
            };
            if index <= 0 {
                return format!("{indent}{hashes} {raw_title}")
                    .trim_end()
                    .to_string();
            }

            let marker = match style {
                HeadingNumberStyle::CnSection => format!("{}、", to_chinese_number(index)),
                HeadingNumberStyle::CnSubsection => format!("（{}）", to_chinese_number(index)),
                HeadingNumberStyle::Decimal => format!("{index}."),
            };
            format!("{indent}{hashes} {marker} {raw_title}")
                .trim_end()
                .to_string()
        })
        .collect();
    next.join("\n")
}

/// 原版 `removeHeadingNumbers`：去掉所有标题的编号前缀。
pub fn remove_heading_numbers(md: &str) -> String {
    let heading_re = re(r"^(\s*#{1,6}\s+)(.+)$");
    let next: Vec<String> = split_lines(md)
        .into_iter()
        .map(|line| {
            let Some(caps) = heading_re.captures(line) else {
                return line.to_string();
            };
            let prefix = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let title = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            format!("{prefix}{}", strip_heading_number_prefix(title))
                .trim_end()
                .to_string()
        })
        .collect();
    next.join("\n")
}

/// 原版 `autoDetectHeadings`：把「一、」「（一）」开头的普通行升级成标题。
pub fn auto_detect_headings(md: &str) -> String {
    let h2_re = re(r"^[一二三四五六七八九十百千万零〇两]+[、.．:：]\s*(.+)$");
    let h3_re = re(
        r"^(?:（[一二三四五六七八九十百千万零〇两0-9]+）|\([一二三四五六七八九十百千万零〇两0-9]+\))\s*(.+)$",
    );

    let next: Vec<String> = split_lines(md)
        .into_iter()
        .map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                return line.to_string();
            }
            if let Some(caps) = h2_re.captures(trimmed) {
                let title = caps.get(1).map(|m| m.as_str()).unwrap_or("").trim();
                return format!("## {title}");
            }
            if let Some(caps) = h3_re.captures(trimmed) {
                let title = caps.get(1).map(|m| m.as_str()).unwrap_or("").trim();
                return format!("### {title}");
            }
            line.to_string()
        })
        .collect();
    next.join("\n")
}

/// 原版 `clearHeadingBold`：去掉标题行里的 `**加粗**`。
pub fn clear_heading_bold(md: &str) -> String {
    let heading_re = re(r"^(\s*#{1,6}\s+)(.+)$");
    let bold_re = re(r"\*\*(.+?)\*\*");
    let next: Vec<String> = split_lines(md)
        .into_iter()
        .map(|line| {
            let Some(caps) = heading_re.captures(line) else {
                return line.to_string();
            };
            let prefix = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let title = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            let cleaned = bold_re.replace_all(title, "$1").to_string();
            format!("{prefix}{cleaned}").trim_end().to_string()
        })
        .collect();
    next.join("\n")
}

/// 从文稿的 H1 取标题（原版 `text.match(/^#\s+(.+)$/m)`，调用方拿到后自己 `trim`）。
/// JS 的 `.` 不吃 `\r`，`$`（m）能停在 `\r` 前；Rust 的 `(?m)$` 只停在 `\n` 前，补 `\r?`。
pub fn extract_h1(md: &str) -> Option<String> {
    let re = re(r"(?m)^#[^\S\r\n]+([^\r\n]+?)\r?$");
    re.captures(md)
        .map(|caps| caps.get(1).map(|m| m.as_str()).unwrap_or("").to_string())
}

/// 行内数学（`$...$`）的**内容**校验：内容非空、不以空白开头、不以空白结尾。
///
/// 这是三处定界规则的共同部分，规则逐字对齐：
/// - PDF/幻灯片：`slide-typst/src/math.rs` 的 `rewrite_str`（`!tex.is_empty() &&
///   !starts_with(whitespace) && !ends_with(whitespace)`）；
/// - 生成侧：`generator/utils.rs` 的 `find_inline_math_close`；
/// - 屏幕预览：原生界面 Markdown 插件的 `parse`（不认领违反规则的节点，
///   框架把该段回落成普通文字，价格类文本不被吞）。
///
/// 原版对照：`MarkdownWorkbench.vue` 的 `@vscode/markdown-it-katex`
/// （opening `$` 后不跟空白、closing `$` 前不是空白）。
/// 只校验内容本身；「闭合 `$` 后不紧跟数字」见 [`inline_math_close_ok`]。
pub fn inline_math_content_ok(content: &str) -> bool {
    !content.is_empty()
        && !content.starts_with(char::is_whitespace)
        && !content.ends_with(char::is_whitespace)
}

/// 行内数学闭合 `$` 后的下一个字符是否允许：紧跟 ASCII 数字时这段不算公式
/// （pandoc 的货币守卫，「价格 $5 和 $10」不被吞）。`None` = 文档结尾，允许。
pub fn inline_math_close_ok(next: Option<char>) -> bool {
    !next.is_some_and(|c| c.is_ascii_digit())
}

/// 块级数学（`$$...$$`）的内容校验：trim 后非空。对齐 `generator/utils.rs` 的
/// `find_display_math_close` 与 `math.rs` 的 display 分支。
pub fn display_math_content_ok(content: &str) -> bool {
    !content.trim().is_empty()
}

#[cfg(test)]
mod math_delimiter_tests {
    use super::*;

    #[test]
    fn normal_inline_content_passes() {
        assert!(inline_math_content_ok("x^2 + y^2"));
        assert!(inline_math_content_ok("\\frac{1}{2}"));
    }

    #[test]
    fn empty_or_whitespace_padded_content_is_declined() {
        assert!(!inline_math_content_ok(""));
        assert!(!inline_math_content_ok(" x"));
        assert!(!inline_math_content_ok("x "));
        assert!(!inline_math_content_ok(" "));
        assert!(!inline_math_content_ok("\t"));
    }

    #[test]
    fn closing_before_digit_is_declined() {
        // 「价格 $5 和 $10」：闭 $ 后面紧跟数字，整段必须回落成普通文字。
        assert!(!inline_math_close_ok(Some('1')));
        assert!(!inline_math_close_ok(Some('0')));
        assert!(inline_math_close_ok(Some('，')));
        assert!(inline_math_close_ok(Some(' ')));
        assert!(inline_math_close_ok(None));
    }

    #[test]
    fn display_content_must_not_be_blank() {
        assert!(display_math_content_ok("E = mc^2"));
        assert!(display_math_content_ok("  E = mc^2  "));
        assert!(!display_math_content_ok(""));
        assert!(!display_math_content_ok("   "));
        assert!(!display_math_content_ok("\n\n"));
    }
}
