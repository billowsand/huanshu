use crate::types::*;
use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::HashSet;
use std::fs;
use std::path::Path;

pub fn parse_json_with_extraction<T: for<'de> Deserialize<'de>>(raw: &str) -> Result<T> {
    let sanitized_raw = sanitize_json_text(raw);
    match serde_json::from_str(&sanitized_raw) {
        Ok(value) => Ok(value),
        Err(e) => {
            let extracted = extract_json_object(raw);
            let extracted_text = extracted.as_deref();
            match extracted_text {
                Some(extracted) if extracted != sanitized_raw => {
                    let sanitized_extracted = sanitize_json_text(extracted);
                    match serde_json::from_str(&sanitized_extracted) {
                        Ok(value) => Ok(value),
                        Err(e2) => {
                            let preview = truncate_for_display(raw, 300);
                            anyhow::bail!(
                                "model JSON parse failed (after extraction attempt):\n\
                                 ~~~ raw preview ~~~\n{}\n\
                                 ~~~ sanitized (extracted) ~~~\n{}\n\
                                 ~~~ serde error on extracted ~~~\n{}",
                                preview,
                                truncate_for_display(&sanitized_extracted, 300),
                                e2
                            )
                        }
                    }
                }
                _ => {
                    let preview = truncate_for_display(raw, 300);
                    anyhow::bail!(
                        "model JSON parse failed (no valid JSON object found):\n\
                         ~~~ raw preview ~~~\n{}\n\
                         ~~~ sanitized ~~~\n{}\n\
                         ~~~ serde error ~~~\n{}",
                        preview,
                        truncate_for_display(&sanitized_raw, 300),
                        e
                    )
                }
            }
        }
    }
}

fn truncate_for_display(s: &str, max_len: usize) -> String {
    let trimmed = s.trim();
    let total_chars = trimmed.chars().count();
    if total_chars <= max_len {
        return trimmed.to_string();
    }

    let truncated: String = trimmed.chars().take(max_len).collect();
    format!("{}... [truncated, {} chars total]", truncated, total_chars)
}

pub fn parse_blueprint_with_repair(
    raw: &str,
    expected_kind: &SlideKind,
) -> Result<BlueprintWrapper> {
    let json_text = extract_json_object(raw).unwrap_or_else(|| raw.to_string());
    let sanitized = sanitize_json_text(&json_text);
    // Only on a parse failure: repairs are guesses and must never touch valid JSON.
    let parsed = serde_json::from_str::<Value>(&sanitized).or_else(|err| {
        // A dropped `{` also unbalances the brace-matched extraction, which then stops one `}`
        // early: repair the span from the first `{` to the last `}` instead.
        let span = raw
            .find('{')
            .zip(raw.rfind('}'))
            .filter(|(start, end)| start < end)
            .map_or(json_text.as_str(), |(start, end)| &raw[start..=end]);
        repair_bare_step_items(&sanitize_json_text(span))
            .and_then(|repaired| serde_json::from_str(&repaired).ok())
            .ok_or(err)
    });
    let mut value: Value = match parsed {
        Ok(v) => v,
        Err(e) => {
            anyhow::bail!(
                "blueprint JSON parse failed (expected {:?}):\n\
                 ~~~ raw preview ~~~\n{}\n\
                 ~~~ extracted & sanitized preview ~~~\n{}\n\
                 ~~~ serde error ~~~\n{}",
                expected_kind,
                truncate_for_display(raw, 300),
                truncate_for_display(&sanitized, 300),
                e
            )
        }
    };
    if value.get("slide").is_none() && value.is_object() {
        value = Value::Object(Map::from_iter([("slide".to_string(), value)]));
    }
    normalize_blueprint_value(&mut value, expected_kind);
    serde_json::from_value(value).context("model did not return a valid slide blueprint")
}

/// Repairs list items whose opening `{` the model dropped, writing the item's step number as a
/// bare array element: `[ {…}, "02", "title": "…", "body": "…" }, … ]` (seen repeatedly from
/// local models on `points` / `list_items`). An array element string followed by `, "key":` can
/// only be that, so it becomes `{"step": "02", "title": …}`. `None` when nothing matched.
fn repair_bare_step_items(text: &str) -> Option<String> {
    fn string_end(chars: &[char], open: usize) -> Option<usize> {
        let mut escape = false;
        for (offset, &c) in chars.get(open + 1..)?.iter().enumerate() {
            match c {
                _ if escape => escape = false,
                '\\' => escape = true,
                '"' => return Some(open + 1 + offset),
                _ => {}
            }
        }
        None
    }
    fn skip_ws(chars: &[char], mut i: usize) -> usize {
        while chars.get(i).is_some_and(|c| c.is_whitespace()) {
            i += 1;
        }
        i
    }
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len() + 32);
    let mut stack: Vec<char> = Vec::new();
    let mut changed = false;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '"' => {
                let end = string_end(&chars, i)?;
                if stack.last() == Some(&'[') {
                    let comma = skip_ws(&chars, end + 1);
                    let key = skip_ws(&chars, comma + 1);
                    let key_is_key = chars.get(comma) == Some(&',')
                        && chars.get(key) == Some(&'"')
                        && string_end(&chars, key)
                            .is_some_and(|key_end| chars.get(skip_ws(&chars, key_end + 1)) == Some(&':'));
                    if key_is_key {
                        out.push_str("{\"step\": ");
                        stack.push('{');
                        changed = true;
                    }
                }
                out.extend(&chars[i..=end]);
                i = end + 1;
                continue;
            }
            '[' | '{' => stack.push(c),
            ']' | '}' => {
                stack.pop();
            }
            _ => {}
        }
        out.push(c);
        i += 1;
    }
    changed.then_some(out)
}

/// Value-level counterpart of [`repair_bare_step_items`] for output that is valid JSON:
/// `[ {…}, "02", { "step": "02", … } ]`. A bare string becomes the `step` of the object right
/// after it when that object has none; every other non-object element is dropped, since list
/// fields deserialize into structs and one stray string fails the whole slide.
fn absorb_bare_list_items(items: &mut Vec<Value>) {
    let mut out = Vec::with_capacity(items.len());
    let mut pending_step: Option<Value> = None;
    for item in items.drain(..) {
        match item {
            Value::Object(mut obj) => {
                if let Some(step) = pending_step.take() {
                    obj.entry("step".to_string()).or_insert(step);
                }
                out.push(Value::Object(obj));
            }
            Value::String(s) if !s.trim().is_empty() => pending_step = Some(Value::String(s)),
            _ => pending_step = None,
        }
    }
    *items = out;
}

pub fn sanitize_json_text(text: &str) -> String {
    let trimmed = text.trim();
    let without_fence = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .unwrap_or(trimmed)
        .trim();
    let without_fence = without_fence
        .strip_suffix("```")
        .unwrap_or(without_fence)
        .trim();
    let fixed = fix_unescaped_control_chars_in_strings(without_fence);
    remove_trailing_commas(&fixed)
}

pub fn fix_unescaped_control_chars_in_strings(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_string = false;
    let mut escape = false;
    for ch in text.chars() {
        if in_string {
            if escape {
                escape = false;
                out.push(ch);
                continue;
            }
            match ch {
                '\\' => {
                    escape = true;
                    out.push(ch);
                }
                '"' => {
                    in_string = false;
                    out.push(ch);
                }
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                _ => out.push(ch),
            }
        } else {
            if ch == '"' {
                in_string = true;
            }
            out.push(ch);
        }
    }
    out
}

pub fn remove_trailing_commas(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut in_string = false;
    let mut escape = false;

    while let Some(ch) = chars.next() {
        if in_string {
            out.push(ch);
            if escape {
                escape = false;
            } else {
                match ch {
                    '\\' => escape = true,
                    '"' => in_string = false,
                    _ => {}
                }
            }
            continue;
        }

        match ch {
            '"' => {
                in_string = true;
                out.push(ch);
            }
            ',' => {
                let mut lookahead = chars.clone();
                while let Some(next) = lookahead.peek() {
                    if next.is_whitespace() {
                        lookahead.next();
                    } else {
                        break;
                    }
                }
                match lookahead.peek() {
                    Some('}') | Some(']') => {}
                    _ => out.push(ch),
                }
            }
            _ => out.push(ch),
        }
    }

    out
}

pub fn extract_json_object(text: &str) -> Option<String> {
    let start = text.find('{')?;
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escape = false;
    for (offset, ch) in text[start..].char_indices() {
        if in_string {
            if escape {
                escape = false;
                continue;
            }
            match ch {
                '\\' => escape = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(text[start..start + offset + ch.len_utf8()].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

pub fn summarize(text: &str, max_chars: usize) -> String {
    let cleaned = text.replace('\n', " ").trim().to_string();
    if cleaned.chars().count() <= max_chars {
        cleaned
    } else {
        let mut result: String = cleaned.chars().take(max_chars.saturating_sub(1)).collect();
        result.push('…');
        result
    }
}

pub fn clean_model_text(text: &str) -> String {
    let cleaned = text
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n");
    let cleaned = strip_markdown(&cleaned);
    strip_english_only_clauses(&cleaned)
}

/// 数学段（`$...$` / `$$...$$`）抽出为控制字符占位符，避免公式里的 `*`、`_`、`` ` `` 等
/// 被当成 Markdown 清洗掉；清洗完成后用 [`restore_math_segments`] 还原。识别规则与前端
/// `src/utils/richText.ts` 的 splitMathSegments 对齐：`$$...$$` 优先且可跨行（内容 trim
/// 后不能为空）；行内 `$...$` 不跨行，要求 `$` 后不跟空白、闭合 `$` 前不是空白、内容非空，
/// 并按 pandoc 规则要求闭合 `$` 后不紧跟数字（否则「价格 $5 和 $10」会被误判为公式）；
/// `\$` 是转义的字面美元符，不作定界符。
fn extract_math_segments(text: &str) -> (String, Vec<String>) {
    let chars: Vec<char> = text.chars().collect();
    let mut segments: Vec<String> = Vec::new();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\\' && chars.get(i + 1) == Some(&'$') {
            out.push('\\');
            out.push('$');
            i += 2;
            continue;
        }
        if chars[i] != '$' {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        let display = chars.get(i + 1) == Some(&'$');
        let open_end = i + if display { 2 } else { 1 };
        let close = if display {
            find_display_math_close(&chars, open_end)
        } else {
            find_inline_math_close(&chars, open_end)
        };
        let Some(close) = close else {
            // 找不到合法闭合：`$` 按普通字符处理
            out.push('$');
            i += 1;
            continue;
        };
        let close_end = close + if display { 2 } else { 1 };
        segments.push(chars[i..close_end].iter().collect());
        // \u{1}...\u{2} 不会被下面的清洗正则误伤，正常文本也不会出现控制字符
        out.push_str(&format!("\u{1}MATH{}\u{2}", segments.len() - 1));
        i = close_end;
    }
    (out, segments)
}

/// `$$...$$` 的闭合位置：可跨行，内容 trim 后不能为空。
fn find_display_math_close(chars: &[char], from: usize) -> Option<usize> {
    let mut j = from;
    while j + 1 < chars.len() {
        if chars[j] == '\\' && chars[j + 1] == '$' {
            j += 2;
            continue;
        }
        if chars[j] == '$' && chars[j + 1] == '$' {
            let content: String = chars[from..j].iter().collect();
            return (!content.trim().is_empty()).then_some(j);
        }
        j += 1;
    }
    None
}

/// `$...$` 的闭合位置：不跨行，开 `$` 后不跟空白、闭合 `$` 前不是空白（内容因此非空），
/// 闭合 `$` 后不紧跟数字。
fn find_inline_math_close(chars: &[char], from: usize) -> Option<usize> {
    if chars.get(from).is_none_or(|c| c.is_whitespace()) {
        return None;
    }
    let mut j = from;
    while j < chars.len() {
        if chars[j] == '\\' && chars.get(j + 1) == Some(&'$') {
            j += 2;
            continue;
        }
        match chars[j] {
            '\n' => return None,
            '$' if chars.get(j + 1) != Some(&'$') => {
                if chars[j - 1].is_whitespace() {
                    return None;
                }
                if chars.get(j + 1).is_some_and(|c| c.is_ascii_digit()) {
                    return None;
                }
                return Some(j);
            }
            _ => {}
        }
        j += 1;
    }
    None
}

fn restore_math_segments(text: &str, segments: &[String]) -> String {
    let mut out = text.to_string();
    for (idx, segment) in segments.iter().enumerate() {
        out = out.replace(&format!("\u{1}MATH{idx}\u{2}"), segment);
    }
    out
}

/// 幻灯片字段都是纯文本（多数模板用 `{{ }}` 插值，少数用 `v-html`），LLM 常把源
/// Markdown 的 `**加粗**`、`- 列表`、`1. 编号`、`# 标题`、`` `代码` ``、`[链接](url)`
/// 原样抄进字段，渲染时就成了裸符号。这里把它们还原成纯文本：行首块级标记去掉，
/// 多行（通常是被塞进一个字段的列表）用「；」合并成一句，行内强调/代码/链接只留文字。
/// 数学段（`$...$` / `$$...$$`）先抽成占位符、行合并完成后在末尾还原，全程原样保留。
pub fn strip_markdown(text: &str) -> String {
    use regex::Regex;
    use std::sync::OnceLock;

    let (text, math_segments) = extract_math_segments(text);
    let text = text.as_str();

    static BLOCK_PREFIX: OnceLock<Regex> = OnceLock::new();
    static HRULE: OnceLock<Regex> = OnceLock::new();
    static IMAGE: OnceLock<Regex> = OnceLock::new();
    static LINK: OnceLock<Regex> = OnceLock::new();
    static STAR_EM: OnceLock<Regex> = OnceLock::new();
    static UNDERSCORE_STRONG: OnceLock<Regex> = OnceLock::new();
    static HTML_EMPHASIS: OnceLock<Regex> = OnceLock::new();

    // 标题 / 引用 / 无序列表 / 有序列表 / 任务列表，可叠加（如 `> - [x] 1. ...`）
    let block_prefix = BLOCK_PREFIX.get_or_init(|| {
        Regex::new(r"^(?:#{1,6}\s+|>\s?|[-*+]\s+|[•·▪●]\s*|\d{1,2}[.)、]\s+|\[[ xX]\]\s+)+")
            .unwrap()
    });
    let hrule = HRULE.get_or_init(|| Regex::new(r"^\s*(?:[-*_]\s*){3,}$").unwrap());
    let image = IMAGE.get_or_init(|| Regex::new(r"!\[([^\]]*)\]\([^)]*\)").unwrap());
    let link = LINK.get_or_init(|| Regex::new(r"\[([^\]]+)\]\([^)]*\)").unwrap());
    let star_em = STAR_EM.get_or_init(|| Regex::new(r"\*([^*\s](?:[^*]*[^*\s])?)\*").unwrap());
    let underscore_strong = UNDERSCORE_STRONG.get_or_init(|| Regex::new(r"__([^_]+)__").unwrap());
    let html_emphasis =
        HTML_EMPHASIS.get_or_init(|| Regex::new(r"(?i)</?(?:strong|b|em|i|u|mark|code)>").unwrap());

    let mut merged = String::new();
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with("```") || hrule.is_match(line) {
            continue;
        }
        let line = block_prefix.replace(line, "");
        let line = image.replace_all(&line, "$1");
        let line = link.replace_all(&line, "$1");
        let line = line.replace("**", "").replace("~~", "").replace('`', "");
        let line = underscore_strong.replace_all(&line, "$1");
        let line = star_em.replace_all(&line, "$1");
        let line = html_emphasis.replace_all(&line, "");
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(prev) = merged.chars().last() {
            let cjk_context = is_cjk(prev) || line.chars().next().is_some_and(is_cjk);
            let ends_with_punct = "。！？；;，,：:、".contains(prev);
            match (ends_with_punct, cjk_context) {
                (true, true) => {}
                (true, false) => merged.push(' '),
                (false, true) => merged.push('；'),
                (false, false) => merged.push_str("; "),
            }
        }
        merged.push_str(line);
    }
    restore_math_segments(&merged, &math_segments)
}

pub fn strip_english_only_clauses(text: &str) -> String {
    if !contains_cjk(text) {
        return text.to_string();
    }
    // 数学段同样抽出保护：纯公式的分句没有 CJK，直接判断会被误判为英文残句丢掉
    let (protected, math_segments) = extract_math_segments(text);
    let mut kept = Vec::new();
    for segment in protected.split_inclusive(['。', '！', '？', '；', ';', '.']) {
        let trimmed = segment.trim();
        if trimmed.is_empty() {
            continue;
        }
        let cjk = trimmed.chars().filter(|ch| is_cjk(*ch)).count();
        let ascii_alpha = trimmed
            .chars()
            .filter(|ch| ch.is_ascii_alphabetic())
            .count();
        if cjk == 0 && ascii_alpha >= 12 {
            continue;
        }
        // 保留原始片段（含分隔符与空白）直接拼接：按 `.` 切分会把 `3.5` 拆开，
        // 若用空格 join 就会变成 `3. 5`，「；」后也会多出空格。
        kept.push(segment);
    }
    let merged = if kept.is_empty() {
        protected
    } else {
        kept.concat().trim().to_string()
    };
    restore_math_segments(&merged, &math_segments)
}

pub fn contains_cjk(text: &str) -> bool {
    text.chars().any(is_cjk)
}

pub fn is_cjk(ch: char) -> bool {
    matches!(
        ch as u32,
        0x4E00..=0x9FFF | 0x3400..=0x4DBF | 0x20000..=0x2A6DF | 0x2A700..=0x2B73F
            | 0x2B740..=0x2B81F | 0x2B820..=0x2CEAF | 0xF900..=0xFAFF
    )
}

pub fn infer_tone_from_class(class_name: &str) -> Option<&'static str> {
    if class_name.contains("amber") {
        Some("amber")
    } else if class_name.contains("blue") {
        Some("blue")
    } else if class_name.contains("green") {
        Some("green")
    } else if class_name.contains("red") {
        Some("red")
    } else {
        None
    }
}

pub fn promote_string_field(obj: &mut Map<String, Value>, from: &str, to: &str) {
    if obj.get(to).is_some() {
        return;
    }
    let Some(value) = obj.get(from) else {
        return;
    };
    if let Some(text) = value.as_str() {
        obj.insert(to.to_string(), Value::String(clean_model_text(text)));
    }
}

pub fn normalize_string_array(items: &mut [Value]) {
    for item in items {
        if let Some(text) = item.as_str() {
            *item = Value::String(clean_model_text(text));
        }
    }
}

pub fn get_string(obj: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| obj.get(*key))
        .and_then(|value| match value {
            Value::String(s) if !s.trim().is_empty() => Some(clean_model_text(s)),
            _ => None,
        })
}

pub fn to_list_item(obj: &Map<String, Value>, idx: usize) -> Value {
    let title = get_string(obj, &["title"]).unwrap_or_else(|| format!("要点 {}", idx + 1));
    let body = get_string(obj, &["body", "description", "content", "desc"])
        .unwrap_or_else(|| "待补充".to_string());
    let icon = get_string(obj, &["icon"]);
    let step = get_string(obj, &["step"]).unwrap_or_else(|| (idx + 1).to_string());
    let mut out = Map::new();
    out.insert("step".to_string(), Value::String(step));
    if let Some(icon) = icon {
        out.insert("icon".to_string(), Value::String(icon));
    }
    out.insert("title".to_string(), Value::String(title));
    out.insert("body".to_string(), Value::String(clean_model_text(&body)));
    Value::Object(out)
}

pub fn to_spotlight_panel(obj: &Map<String, Value>) -> Value {
    let title = get_string(obj, &["title"]).unwrap_or_else(|| "核心说明".to_string());
    let body = get_string(obj, &["body", "description", "content", "desc"])
        .unwrap_or_else(|| "待补充".to_string());
    let icon = get_string(obj, &["icon"]);
    let mut out = Map::new();
    out.insert("title".to_string(), Value::String(title));
    if let Some(icon) = icon {
        out.insert("icon".to_string(), Value::String(icon));
    }
    out.insert("body".to_string(), Value::String(clean_model_text(&body)));
    Value::Object(out)
}

pub fn to_center_item(obj: &Map<String, Value>) -> Value {
    let title = get_string(obj, &["title"]).unwrap_or_else(|| "要点".to_string());
    let desc = get_string(obj, &["desc", "body", "description", "content"])
        .unwrap_or_else(|| "待补充".to_string());
    let icon = get_string(obj, &["icon"]);
    let mut out = Map::new();
    out.insert("title".to_string(), Value::String(title));
    out.insert("desc".to_string(), Value::String(clean_model_text(&desc)));
    if let Some(icon) = icon {
        out.insert("icon".to_string(), Value::String(icon));
    }
    Value::Object(out)
}

pub fn normalize_blueprint_value(value: &mut Value, expected_kind: &SlideKind) {
    let Some(slide) = value.get_mut("slide").and_then(Value::as_object_mut) else {
        return;
    };
    // Log when the model returned a different kind than expected — this is often a
    // sign that the model is confused about the component or hallucinating kind names.
    let incoming = slide
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let expected_name = slide_kind_name(expected_kind);
    if !incoming.is_empty() && incoming != expected_name {
        eprintln!(
            "[warn] normalize_blueprint_value: model returned kind '{}', expected '{}' — overwriting",
            incoming, expected_name
        );
    }
    slide.insert("kind".to_string(), Value::String(expected_name.to_string()));

    match expected_kind {
        SlideKind::SectionIntro | SlideKind::FeatureGrid | SlideKind::OutcomeGrid => {
            normalize_feature_grid_like_slide(slide)
        }
        SlideKind::IssueStack => normalize_feature_grid_like_slide(slide),
        SlideKind::SectionList => normalize_section_list_slide(slide),
        SlideKind::Spotlight => normalize_spotlight_slide(slide),
        SlideKind::SplitLayers => normalize_split_layers_slide(slide),
        SlideKind::FocusExample => normalize_focus_example_slide(slide),
        SlideKind::CenterGrid => normalize_center_grid_slide(slide),
        SlideKind::Timeline => normalize_timeline_slide(slide),
        SlideKind::Swot => normalize_swot_slide(slide),
        SlideKind::ChartFocus => normalize_chart_focus_slide(slide),
        SlideKind::StepFlow
        | SlideKind::Process
        | SlideKind::Compare
        | SlideKind::Cover
        | SlideKind::Closing
        | SlideKind::Overview => {}
    }
}

fn normalize_feature_grid_like_slide(slide: &mut Map<String, Value>) {
    let Some(cards) = slide.get_mut("cards").and_then(Value::as_array_mut) else {
        return;
    };
    let tones = ["amber", "blue", "green", "red"];
    for (idx, card) in cards.iter_mut().enumerate() {
        let Some(card_obj) = card.as_object_mut() else {
            continue;
        };
        promote_string_field(card_obj, "description", "body");
        promote_string_field(card_obj, "content", "body");
        if card_obj.get("body").is_none() {
            if let Some(items) = card_obj.get_mut("items").and_then(Value::as_array_mut) {
                normalize_string_array(items);
            }
        }
        if card_obj.get("body").is_some() && card_obj.get("items").is_none() {
            card_obj.insert("items".to_string(), Value::Array(Vec::new()));
        }
        if card_obj.get("tone").is_none() {
            let inferred = card_obj
                .get("top_bar_class")
                .and_then(Value::as_str)
                .and_then(infer_tone_from_class)
                .unwrap_or(tones[idx % tones.len()]);
            card_obj.insert("tone".to_string(), Value::String(inferred.to_string()));
        }
        remove_keys(card_obj, &["description", "content", "color"]);
    }
}

fn normalize_section_list_slide(slide: &mut Map<String, Value>) {
    if slide.get("list_items").is_none() {
        if let Some(items) = slide
            .get("section")
            .and_then(Value::as_object)
            .and_then(|section| section.get("items"))
            .and_then(Value::as_array)
        {
            let list_items = items
                .iter()
                .enumerate()
                .filter_map(|(idx, item)| item.as_object().map(|obj| to_list_item(obj, idx)))
                .collect::<Vec<_>>();
            if !list_items.is_empty() {
                slide.insert("list_items".to_string(), Value::Array(list_items));
            }
        } else if let Some(cards) = slide.get("cards").and_then(Value::as_array) {
            let list_items = cards
                .iter()
                .enumerate()
                .filter_map(|(idx, item)| item.as_object().map(|obj| to_list_item(obj, idx)))
                .collect::<Vec<_>>();
            if !list_items.is_empty() {
                slide.insert("list_items".to_string(), Value::Array(list_items));
            }
        }
    }
    if let Some(items) = slide.get_mut("list_items").and_then(Value::as_array_mut) {
        for (idx, item) in items.iter_mut().enumerate() {
            let Some(item_obj) = item.as_object_mut() else {
                continue;
            };
            promote_string_field(item_obj, "description", "body");
            promote_string_field(item_obj, "content", "body");
            if item_obj.get("step").is_none() {
                item_obj.insert("step".to_string(), Value::String((idx + 1).to_string()));
            }
            item_obj
                .entry("body".to_string())
                .or_insert_with(|| Value::String("待补充".to_string()));
            remove_keys(item_obj, &["description", "content"]);
        }
    }
    remove_keys(
        slide,
        &[
            "section",
            "spotlight",
            "content_shape",
            "layout_intent",
            "visual_need",
            "object_count",
            "argument_mode",
            "density",
            "source_excerpt",
            "takeaway",
            "section_label",
        ],
    );
}

fn normalize_spotlight_slide(slide: &mut Map<String, Value>) {
    if slide.get("image").is_none() {
        if let Some(image) = slide
            .get("spotlight")
            .and_then(Value::as_object)
            .and_then(|x| x.get("image"))
            .cloned()
        {
            slide.insert("image".to_string(), image);
        }
    }
    if slide.get("label").is_none() {
        if let Some(label) = slide.get("section_label").and_then(Value::as_str) {
            slide.insert("label".to_string(), Value::String(label.to_string()));
        }
    }
    slide
        .entry("label_tone".to_string())
        .or_insert_with(|| Value::String("blue".to_string()));

    if slide.get("panels").is_none() {
        if let Some(items) = slide
            .get("section")
            .and_then(Value::as_object)
            .and_then(|section| section.get("items"))
            .and_then(Value::as_array)
        {
            let panels = items
                .iter()
                .filter_map(|item| item.as_object().map(to_spotlight_panel))
                .collect::<Vec<_>>();
            if !panels.is_empty() {
                slide.insert("panels".to_string(), Value::Array(panels));
            }
        }
    }

    if let Some(panels) = slide.get_mut("panels").and_then(Value::as_array_mut) {
        for panel in panels {
            let Some(panel_obj) = panel.as_object_mut() else {
                continue;
            };
            promote_string_field(panel_obj, "description", "body");
            promote_string_field(panel_obj, "content", "body");
            remove_keys(panel_obj, &["description", "content"]);
        }
    }
    if slide
        .get("placeholder")
        .and_then(Value::as_str)
        .is_some_and(|value| value.trim().is_empty())
    {
        slide.remove("placeholder");
    }
    remove_keys(
        slide,
        &[
            "section",
            "spotlight",
            "content_shape",
            "layout_intent",
            "visual_need",
            "object_count",
            "argument_mode",
            "density",
            "source_excerpt",
            "takeaway",
            "section_label",
        ],
    );
}

fn normalize_split_layers_slide(slide: &mut Map<String, Value>) {
    if slide.get("left_items").is_none() {
        if let Some(items) = slide
            .get("section")
            .and_then(Value::as_object)
            .and_then(|section| section.get("items"))
            .and_then(Value::as_array)
        {
            let left_items = items
                .iter()
                .enumerate()
                .filter_map(|(idx, item)| item.as_object().map(|obj| to_list_item(obj, idx)))
                .collect::<Vec<_>>();
            if !left_items.is_empty() {
                slide.insert("left_items".to_string(), Value::Array(left_items));
            }
        }
    }
    if let Some(items) = slide.get_mut("left_items").and_then(Value::as_array_mut) {
        for (idx, item) in items.iter_mut().enumerate() {
            let Some(item_obj) = item.as_object_mut() else {
                continue;
            };
            promote_string_field(item_obj, "description", "body");
            if item_obj.get("step").is_none() {
                item_obj.insert("step".to_string(), Value::String((idx + 1).to_string()));
            }
            item_obj
                .entry("body".to_string())
                .or_insert_with(|| Value::String("待补充".to_string()));
            remove_keys(item_obj, &["description", "content"]);
        }
    }
    remove_keys(
        slide,
        &[
            "section",
            "spotlight",
            "content_shape",
            "layout_intent",
            "visual_need",
            "object_count",
            "argument_mode",
            "density",
            "source_excerpt",
            "takeaway",
            "section_label",
        ],
    );
}

fn normalize_focus_example_slide(slide: &mut Map<String, Value>) {
    if slide.get("points").is_none() {
        if let Some(items) = slide.get("list_items").and_then(Value::as_array) {
            slide.insert("points".to_string(), Value::Array(items.clone()));
        }
    }
    if let Some(points) = slide.get_mut("points").and_then(Value::as_array_mut) {
        for point in points {
            let Some(point_obj) = point.as_object_mut() else {
                continue;
            };
            promote_string_field(point_obj, "description", "body");
            promote_string_field(point_obj, "content", "body");
            point_obj
                .entry("body".to_string())
                .or_insert_with(|| Value::String("待补充".to_string()));
            remove_keys(point_obj, &["description", "content"]);
        }
    }
}

fn normalize_chart_focus_slide(slide: &mut Map<String, Value>) {
    // points 字段修复：去掉混进数组的裸字符串（"02"），兼容 description/content → body，缺失 body 补占位
    if let Some(points) = slide.get_mut("points").and_then(Value::as_array_mut) {
        absorb_bare_list_items(points);
        for point in points {
            let Some(point_obj) = point.as_object_mut() else {
                continue;
            };
            promote_string_field(point_obj, "description", "body");
            promote_string_field(point_obj, "content", "body");
            point_obj
                .entry("body".to_string())
                .or_insert_with(|| Value::String(String::new()));
            remove_keys(point_obj, &["description", "content"]);
        }
    }
    // chart.title 为空时退回 slide.title（图表渲染与布局都依赖它）
    let fallback_title = slide.get("title").and_then(Value::as_str).map(str::to_string);
    if let Some(chart) = slide.get_mut("chart").and_then(Value::as_object_mut) {
        let title_empty = chart
            .get("title")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or("")
            .is_empty();
        if title_empty {
            if let Some(title) = fallback_title {
                chart.insert("title".to_string(), Value::String(title));
            }
        }
    }
}

fn normalize_center_grid_slide(slide: &mut Map<String, Value>) {
    if slide.get("center_items").is_none() {
        if let Some(items) = slide.get("cards").and_then(Value::as_array) {
            let center_items = items
                .iter()
                .filter_map(|item| item.as_object().map(to_center_item))
                .collect::<Vec<_>>();
            if !center_items.is_empty() {
                slide.insert("center_items".to_string(), Value::Array(center_items));
            }
        }
    }
}

fn normalize_timeline_slide(slide: &mut Map<String, Value>) {
    if slide.get("timeline_events").is_none() {
        if let Some(events) = slide.remove("events") {
            slide.insert("timeline_events".to_string(), events);
        }
    }
    let tones = ["amber", "blue", "green", "red", "amber", "blue"];
    if let Some(events) = slide
        .get_mut("timeline_events")
        .and_then(Value::as_array_mut)
    {
        for (idx, event) in events.iter_mut().enumerate() {
            let Some(obj) = event.as_object_mut() else {
                continue;
            };
            obj.entry("date".to_string())
                .or_insert_with(|| Value::String("待补充".to_string()));
            obj.entry("title".to_string())
                .or_insert_with(|| Value::String(format!("事件 {}", idx + 1)));
            obj.entry("body".to_string())
                .or_insert_with(|| Value::String("待补充".to_string()));
            obj.entry("tone".to_string())
                .or_insert_with(|| Value::String(tones[idx % tones.len()].to_string()));
            obj.entry("icon".to_string())
                .or_insert_with(|| Value::String("i-carbon:time".to_string()));
        }
    }
}

fn normalize_swot_slide(slide: &mut Map<String, Value>) {
    if slide.get("swot_data").is_none() {
        let mut swot_data = Map::new();
        if let Some(quadrants) = slide.remove("quadrants") {
            swot_data.insert("quadrants".to_string(), quadrants);
        }
        if let Some(strategy) = slide.remove("strategy") {
            swot_data.insert("strategy".to_string(), strategy);
        }
        if !swot_data.is_empty() {
            slide.insert("swot_data".to_string(), Value::Object(swot_data));
        }
    }

    let Some(swot_data) = slide.get_mut("swot_data").and_then(Value::as_object_mut) else {
        return;
    };
    if let Some(quadrants) = swot_data.get_mut("quadrants").and_then(Value::as_array_mut) {
        let defaults = [
            ("strengths", "优势", "green", "i-carbon:thumbs-up"),
            ("weaknesses", "劣势", "red", "i-carbon:warning-alt"),
            ("opportunities", "机会", "blue", "i-carbon:growth"),
            ("threats", "威胁", "amber", "i-carbon:security"),
        ];
        for (idx, quadrant) in quadrants.iter_mut().enumerate() {
            let Some(obj) = quadrant.as_object_mut() else {
                continue;
            };
            let (key, title, tone, icon) = defaults.get(idx).copied().unwrap_or(defaults[0]);
            obj.entry("key".to_string())
                .or_insert_with(|| Value::String(key.to_string()));
            obj.entry("title".to_string())
                .or_insert_with(|| Value::String(title.to_string()));
            obj.entry("tone".to_string())
                .or_insert_with(|| Value::String(tone.to_string()));
            obj.entry("icon".to_string())
                .or_insert_with(|| Value::String(icon.to_string()));
            if obj.get("items").is_none() {
                obj.insert(
                    "items".to_string(),
                    Value::Array(vec![Value::String("待补充".to_string())]),
                );
            }
            if let Some(items) = obj.get_mut("items").and_then(Value::as_array_mut) {
                normalize_string_array(items);
            }
            promote_string_field(obj, "conclusion", "summary");
            promote_string_field(obj, "body", "summary");
        }
    }
}

pub fn remove_keys(obj: &mut Map<String, Value>, keys: &[&str]) {
    for key in keys {
        obj.remove(*key);
    }
}

pub fn slide_kind_name(kind: &SlideKind) -> &'static str {
    match kind {
        SlideKind::Cover => "cover",
        SlideKind::Closing => "closing",
        SlideKind::Overview => "overview",
        SlideKind::SectionIntro => "section_intro",
        SlideKind::FeatureGrid => "feature_grid",
        SlideKind::Spotlight => "spotlight",
        SlideKind::SplitLayers => "split_layers",
        SlideKind::SectionList => "section_list",
        SlideKind::FocusExample => "focus_example",
        SlideKind::OutcomeGrid => "outcome_grid",
        SlideKind::CenterGrid => "center_grid",
        SlideKind::Timeline => "timeline",
        SlideKind::StepFlow => "step_flow",
        SlideKind::Process => "process",
        SlideKind::Compare => "compare",
        SlideKind::IssueStack => "issue_stack",
        SlideKind::Swot => "swot",
        SlideKind::ChartFocus => "chart_focus",
    }
}

pub fn blueprint_schema_hint(kind: &SlideKind) -> &'static str {
    match kind {
        SlideKind::SectionIntro => {
            r#"{
  "kind": "section_intro",
  "badge": "章节导览",
  "title": "...",
  "subtitle": "...",
  "note": "...",
  "cards": [
    { "title": "...", "tone": "green", "icon": "i-carbon:roadmap", "body": "..." },
    { "title": "...", "tone": "blue", "icon": "i-carbon:idea", "body": "..." },
    { "title": "...", "tone": "amber", "icon": "i-carbon:workflow-automation", "body": "..." }
  ]
}"#
        }
        SlideKind::FeatureGrid => {
            r#"{
  "kind": "feature_grid",
  "title": "...",
  "subtitle": "...",
  "note": "...",
  "cards": [
    { "title": "...", "tone": "amber", "icon": "i-carbon:idea", "body": "...", "items": ["...", "..."], "conclusion": "..." },
    { "title": "...", "tone": "blue", "icon": "i-carbon:data-base", "body": "...", "items": ["...", "..."], "conclusion": "..." },
    { "title": "...", "tone": "green", "icon": "i-carbon:workflow-automation", "body": "...", "items": ["...", "..."], "conclusion": "..." }
  ]
}"#
        }
        SlideKind::Spotlight => {
            r#"{
  "kind": "spotlight",
  "title": "...",
  "label": "...",
  "label_tone": "blue",
  "image": "/figure/example.png",
  "placeholder": "图片待补充",
  "panels": [
    { "title": "...", "icon": "i-carbon:idea", "tone": "blue", "body": "..." }
  ]
}"#
        }
        SlideKind::SplitLayers => {
            r#"{
  "kind": "split_layers",
  "title": "...",
  "subtitle": "...",
  "left_items": [
    { "icon": "i-carbon:api", "title": "...", "body": "..." }
  ],
  "layers": [
    { "title": "...", "meta": "...", "tone": "amber" }
  ],
  "footer": "..."
}"#
        }
        SlideKind::SectionList => {
            r#"{
  "kind": "section_list",
  "title": "...",
  "subtitle": "...",
  "list_items": [
    { "step": "1", "icon": "i-carbon:list-checked", "title": "...", "body": "..." }
  ]
}"#
        }
        SlideKind::FocusExample => {
            r#"{
  "kind": "focus_example",
  "title": "...",
  "subtitle": "...",
  "points": [
    { "icon": "i-carbon:idea", "title": "...", "body": "..." }
  ],
  "example_title": "...",
  "example_body": "...",
  "ranking": [
    { "index": "1.", "label": "...", "meta": "...", "muted": false }
  ]
}"#
        }
        SlideKind::OutcomeGrid => {
            r#"{
  "kind": "outcome_grid",
  "title": "...",
  "subtitle": "...",
  "note": "...",
  "cards": [
    { "title": "...", "tone": "amber", "icon": "i-carbon:list-checked", "body": "...", "tag": "成果", "top_bar_class": "bg-gradient-to-r from-amber-500 to-amber-700" }
  ]
}"#
        }
        SlideKind::CenterGrid => {
            r#"{
  "kind": "center_grid",
  "badge": "总结",
  "title": "主标题",
  "accent": "副标题或补充说明（简短文字）",
  "center_items": [
    { "title": "...", "desc": "...", "icon": "i-carbon:growth", "tone": "amber" }
  ],
  "footer": "..."
}"#
        }
        SlideKind::Timeline => {
            r#"{
  "kind": "timeline",
  "title": "...",
  "subtitle": "...",
  "timeline_events": [
    { "date": "2024年1月", "title": "...", "body": "...", "tone": "amber", "icon": "i-carbon:time" },
    { "date": "2024年6月", "title": "...", "body": "...", "tone": "blue", "icon": "i-carbon:calendar" },
    { "date": "2025年3月", "title": "...", "body": "...", "tone": "red", "icon": "i-carbon:warning" }
  ],
  "footer": "..."
}"#
        }
        SlideKind::StepFlow => {
            r#"{
  "kind": "step_flow",
  "title": "...",
  "subtitle": "...",
  "direction": "horizontal",
  "steps": [
    { "title": "步骤1", "body": "描述", "icon": "i-carbon:idea", "tone": "amber" },
    { "title": "步骤2", "body": "描述", "icon": "i-carbon:chevron-right", "tone": "blue" },
    { "title": "步骤3", "body": "描述", "icon": "i-carbon:checkmark", "tone": "green" }
  ],
  "footer": "..."
}"#
        }
        SlideKind::Process => {
            r#"{
  "kind": "process",
  "title": "...",
  "subtitle": "...",
  "phases": [
    {
      "phase": "阶段1",
      "title": "...",
      "icon": "i-carbon:idea",
      "tone": "amber",
      "steps": [
        { "label": "步骤1.1", "desc": "..." },
        { "label": "步骤1.2", "desc": "..." }
      ],
      "highlight": "关键结论"
    }
  ],
  "footer": "..."
}"#
        }
        SlideKind::Compare => {
            r#"{
  "kind": "compare",
  "title": "...",
  "subtitle": "...",
  "compare_data": {
    "mode": "side-by-side",
    "left": {
      "title": "左侧标题",
      "tone": "blue",
      "icon": "i-carbon:checkmark",
      "items": [
        { "label": "要点1", "desc": "描述", "highlight": false },
        { "label": "要点2", "desc": "描述", "highlight": true }
      ],
      "conclusion": "左侧结论"
    },
    "right": {
      "title": "右侧标题",
      "tone": "amber",
      "icon": "i-carbon:close",
      "items": [
        { "label": "要点1", "desc": "描述", "highlight": false },
        { "label": "要点2", "desc": "描述", "highlight": false }
      ],
      "conclusion": "右侧结论"
    }
  },
  "footer": "..."
}"#
        }
        SlideKind::IssueStack => {
            r#"{
  "kind": "issue_stack",
  "title": "...",
  "subtitle": "...",
  "cards": [
    { "title": "问题一", "tone": "blue", "icon": "i-carbon:warning-alt", "body": "...", "items": ["...", "...", "..."] },
    { "title": "问题二", "tone": "green", "icon": "i-carbon:warning-other", "body": "...", "items": ["...", "..."] },
    { "title": "问题三", "tone": "amber", "icon": "i-carbon:data-error", "body": "...", "items": ["...", "...", "..."] }
  ],
  "footer": "..."
}"#
        }
        SlideKind::Swot => {
            r#"{
  "kind": "swot",
  "title": "...",
  "subtitle": "...",
  "swot_data": {
    "quadrants": [
      {
        "key": "strengths",
        "title": "优势",
        "tone": "green",
        "icon": "i-carbon:thumbs-up",
        "items": ["要点1", "要点2"],
        "summary": "内部正向能力"
      },
      {
        "key": "weaknesses",
        "title": "劣势",
        "tone": "red",
        "icon": "i-carbon:warning-alt",
        "items": ["要点1", "要点2"],
        "summary": "内部约束"
      },
      {
        "key": "opportunities",
        "title": "机会",
        "tone": "blue",
        "icon": "i-carbon:growth",
        "items": ["要点1", "要点2"],
        "summary": "外部增长窗口"
      },
      {
        "key": "threats",
        "title": "威胁",
        "tone": "amber",
        "icon": "i-carbon:security",
        "items": ["要点1", "要点2"],
        "summary": "外部风险压力"
      }
    ],
    "strategy": "利用优势抓住机会，同时针对劣势与威胁建立补强动作。"
  }
}"#
        }
        SlideKind::ChartFocus => {
            r#"{
  "kind": "chart_focus",
  "title": "...",
  "subtitle": "...",
  "points": [
    { "step": "01", "title": "要点标题", "body": "要点描述" },
    { "step": "02", "title": "要点标题", "body": "要点描述" }
  ],
  "footer": "..."
}"#
        }
        SlideKind::Cover | SlideKind::Closing | SlideKind::Overview => {
            r#"{"kind": "feature_grid"}"#
        }
    }
}

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || b.is_empty() || a.len() != b.len() {
        return -1.0;
    }
    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }
    if norm_a == 0.0 || norm_b == 0.0 {
        return -1.0;
    }
    dot / (norm_a.sqrt() * norm_b.sqrt())
}

pub fn write_debug(root: &Path, filename: &str, content: &str) -> Result<()> {
    fs::create_dir_all(root)
        .with_context(|| format!("failed to create debug directory: {}", root.display()))?;
    let path = root.join(filename);
    fs::write(&path, content)
        .with_context(|| format!("failed to write debug file: {}", path.display()))
}

pub fn sanitize_filename(input: &str) -> String {
    input
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

pub fn parse_page_display_index(page_id: &str) -> Option<usize> {
    page_id.parse::<usize>().ok()
}

pub fn format_issue_block(title: &str, issues: &[crate::validate::ValidationIssue]) -> String {
    let mut out = String::new();
    out.push_str(title);
    for issue in issues {
        out.push_str("\n- ");
        out.push_str(&issue.message);
    }
    out
}

pub fn infer_asset_from_text(
    title: &str,
    body: &str,
    asset_paths: &HashSet<String>,
) -> Option<String> {
    let haystack = format!("{title} {body}").to_lowercase();
    let mut candidates: Vec<&String> = asset_paths
        .iter()
        .filter(|path| {
            matches!(
                Path::new(path).extension().and_then(|e| e.to_str()),
                Some("png" | "jpg" | "jpeg" | "webp" | "svg")
            )
        })
        .collect();
    candidates.sort();
    for path in candidates {
        if let Some(stem) = Path::new(path).file_stem().and_then(|s| s.to_str()) {
            let stem_lower = stem.to_lowercase();
            if stem_lower.len() >= 5 && haystack.contains(&stem_lower) {
                return Some(path.clone());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_write_creates_missing_retry_directory() {
        let root = std::env::temp_dir().join(format!("huanshu-debug-{}", uuid::Uuid::new_v4()));
        let retry = root.join("retry-page-12");
        write_debug(&retry, "slide-12.system.txt", "prompt").unwrap();
        write_debug(&retry, "slide-12.raw.txt", "response").unwrap();
        assert_eq!(
            fs::read_to_string(retry.join("slide-12.system.txt")).unwrap(),
            "prompt"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn strips_inline_markdown() {
        assert_eq!(
            clean_model_text("**核心**能力：`RAG` 与 *检索* 结合，见[文档](https://x.y)"),
            "核心能力：RAG 与 检索 结合，见文档"
        );
        assert_eq!(clean_model_text("__重点__ ~~旧方案~~"), "重点 旧方案");
        assert_eq!(clean_model_text("<strong>加粗</strong>说明"), "加粗说明");
    }

    #[test]
    fn flattens_block_markdown_into_one_sentence() {
        assert_eq!(
            clean_model_text("- 降低成本\n- **提升效率**\n1. 缩短周期"),
            "降低成本；提升效率；缩短周期"
        );
        assert_eq!(
            clean_model_text("## 背景\n> 引用内容。\n---\n结论"),
            "背景；引用内容。结论"
        );
        assert_eq!(clean_model_text("第一点<br>第二点"), "第一点；第二点");
        assert_eq!(clean_model_text("- Fast\n- Cheap"), "Fast; Cheap");
    }

    #[test]
    fn keeps_plain_symbols_intact() {
        assert_eq!(
            clean_model_text("增长 3.5 倍，5 * 3 = 15"),
            "增长 3.5 倍，5 * 3 = 15"
        );
        assert_eq!(clean_model_text("-5% 同比"), "-5% 同比");
        assert_eq!(clean_model_text("snake_case_name"), "snake_case_name");
    }

    #[test]
    fn math_segments_survive_strip_markdown() {
        // 行内公式里的 * _ ` 等符号不能被当 markdown 清洗
        assert_eq!(
            clean_model_text(r"均值为 $\bar{x}_{i}$，整体稳定"),
            r"均值为 $\bar{x}_{i}$，整体稳定"
        );
        assert_eq!(
            clean_model_text(r"计算 $a*b$ 与 `$c_d$` 的结果"),
            r"计算 $a*b$ 与 $c_d$ 的结果"
        );
        // 块级公式（单行与多行）都原样保留
        assert_eq!(
            clean_model_text(r"公式 $$\frac{a}{b}$$ 如上"),
            r"公式 $$\frac{a}{b}$$ 如上"
        );
        assert_eq!(
            clean_model_text("$$\n\\frac{a}{b}\n$$"),
            "$$\n\\frac{a}{b}\n$$"
        );
        // 公式两侧的 markdown 仍正常清洗
        assert_eq!(
            clean_model_text(r"**重点**：$\bar{x}$ 见下"),
            r"重点：$\bar{x}$ 见下"
        );
    }

    #[test]
    fn currency_dollars_are_not_math() {
        assert_eq!(
            clean_model_text("价格在 $5 和 $10 之间波动"),
            "价格在 $5 和 $10 之间波动"
        );
        // pandoc 规则：闭合 $ 后紧跟数字不算公式
        assert_eq!(clean_model_text("从 $5 涨到$10 元"), "从 $5 涨到$10 元");
    }

    #[test]
    fn escaped_dollar_is_a_literal_dollar_sign() {
        assert_eq!(
            clean_model_text(r"单价 \$5 不含税，公式 $x+1$ 保留"),
            r"单价 \$5 不含税，公式 $x+1$ 保留"
        );
        // 转义美元符不参与定界：这里的 $ y $ 前有空格，本就不算公式
        assert_eq!(clean_model_text(r"\$a \$b$"), r"\$a \$b$");
    }

    #[test]
    fn trailing_display_formula_is_not_dropped_as_english_clause() {
        // 纯公式分句没有 CJK，若不做保护会被 strip_english_only_clauses 当英文残句丢掉
        assert_eq!(
            clean_model_text(r"收敛性见下。$$\int_{a}^{b} f(x) \, dx = F(b) - F(a)$$"),
            r"收敛性见下。$$\int_{a}^{b} f(x) \, dx = F(b) - F(a)$$"
        );
    }

    fn chart_points(raw: &str) -> Vec<(Option<String>, String)> {
        let resp = parse_blueprint_with_repair(raw, &SlideKind::ChartFocus).unwrap();
        resp.slide
            .points
            .into_iter()
            .map(|p| (p.step, p.title))
            .collect()
    }

    #[test]
    fn stray_step_string_before_a_point_becomes_its_step() {
        // Real model output (valid JSON): `"02",` sits between two point objects.
        let points = chart_points(include_str!(
            "../../fixtures/model-output/chart-focus-bare-step-valid.txt"
        ));
        let steps: Vec<_> = points.iter().map(|(s, _)| s.as_deref()).collect();
        assert_eq!(steps, [Some("01"), Some("02"), Some("03"), Some("04")]);
    }

    #[test]
    fn point_missing_its_open_brace_is_repaired() {
        // Real model output (invalid JSON): `"02", "title": …, "body": … },` — the `{` is gone
        // and the step number was written bare. Failed the page on every retry.
        let points = chart_points(include_str!(
            "../../fixtures/model-output/chart-focus-bare-step-broken.txt"
        ));
        assert_eq!(
            points,
            vec![
                (Some("01".to_string()), "上半年稳步扩容".to_string()),
                (Some("02".to_string()), "自动化提效显著".to_string()),
                (Some("03".to_string()), "下半年高峰与封网".to_string()),
            ]
        );
    }

    #[test]
    fn bare_step_repair_leaves_other_text_alone() {
        // Strings in arrays followed by another element, and keys inside objects, are not touched.
        assert_eq!(repair_bare_step_items(r#"{"a": ["x", "y"], "b": {"c": "d"}}"#), None);
        // Escaped quotes inside strings do not confuse the scanner.
        let fixed = repair_bare_step_items(r#"{"p": [{"t": "a\"b"}, "2", "t": "c"}]}"#).unwrap();
        assert_eq!(fixed, r#"{"p": [{"t": "a\"b"}, {"step": "2", "t": "c"}]}"#);
        assert!(serde_json::from_str::<Value>(&fixed).is_ok());
    }
}
