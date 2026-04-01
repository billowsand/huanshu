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
    if trimmed.len() <= max_len {
        return trimmed.to_string();
    }
    format!(
        "{}... [truncated, {} chars total]",
        &trimmed[..max_len],
        trimmed.len()
    )
}

pub fn parse_blueprint_with_repair(
    raw: &str,
    expected_kind: &SlideKind,
) -> Result<BlueprintWrapper> {
    let json_text = extract_json_object(raw).unwrap_or_else(|| raw.to_string());
    let sanitized = sanitize_json_text(&json_text);
    let mut value: Value = match serde_json::from_str(&sanitized) {
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

pub fn parse_layout_plan_response(
    raw: &str,
    page_plans: &[PagePlan],
) -> Result<LayoutPlanResponse> {
    let json_text = extract_json_object(raw).unwrap_or_else(|| raw.to_string());
    let sanitized = sanitize_json_text(&json_text);
    let mut value: Value = match serde_json::from_str(&sanitized) {
        Ok(v) => v,
        Err(e) => {
            anyhow::bail!(
                "layout plan JSON parse failed:\n\
                 ~~~ raw preview ~~~\n{}\n\
                 ~~~ extracted & sanitized preview ~~~\n{}\n\
                 ~~~ serde error ~~~\n{}",
                truncate_for_display(raw, 300),
                truncate_for_display(&sanitized, 300),
                e
            )
        }
    };
    normalize_layout_plan_value(&mut value, page_plans);
    validate_layout_plan_value(&value)?;
    serde_json::from_value(value).context("model did not return a valid layout plan")
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
        .replace("<br>", "；")
        .replace("<br/>", "；")
        .replace("<br />", "；")
        .replace('\n', " ")
        .trim()
        .to_string();
    strip_english_only_clauses(&cleaned)
}

pub fn strip_english_only_clauses(text: &str) -> String {
    if !contains_cjk(text) {
        return text.to_string();
    }
    let mut kept = Vec::new();
    for segment in text.split_inclusive(['。', '！', '？', '；', ';', '.']) {
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
        kept.push(trimmed.to_string());
    }
    if kept.is_empty() {
        text.to_string()
    } else {
        kept.join(" ")
    }
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
        SlideKind::StepFlow
        | SlideKind::Process
        | SlideKind::Compare
        | SlideKind::Cover
        | SlideKind::Closing
        | SlideKind::Overview => {}
    }
}

pub fn normalize_layout_plan_value(value: &mut Value, page_plans: &[PagePlan]) {
    if value.get("pages").is_none() && value.is_array() {
        *value = Value::Object(Map::from_iter([("pages".to_string(), value.take())]));
    }

    let Some(pages) = value.get_mut("pages").and_then(Value::as_array_mut) else {
        return;
    };

    for page in pages {
        let Some(obj) = page.as_object_mut() else {
            continue;
        };
        promote_string_field(obj, "component_kind", "kind");
        promote_string_field(obj, "layout_kind", "kind");
        promote_string_field(obj, "reasoning", "reason");

        if let Some(kind) = obj.get("kind").and_then(Value::as_str) {
            obj.insert("kind".to_string(), Value::String(normalize_kind_name(kind)));
        } else {
            obj.insert(
                "kind".to_string(),
                Value::String("feature_grid".to_string()),
            );
        }

        let page_id = get_string(obj, &["page_id"]);
        let matching_plan = page_id
            .as_deref()
            .and_then(|id| resolve_page_plan(page_plans, id));

        if let Some(plan) = matching_plan {
            obj.insert("page_id".to_string(), Value::String(plan.page_id.clone()));
            obj.entry("title".to_string())
                .or_insert_with(|| Value::String(clean_model_text(&plan.page_title)));
            if let Some(subtitle) = plan
                .takeaway
                .as_deref()
                .filter(|text| !text.trim().is_empty())
                .map(clean_model_text)
                .or_else(|| {
                    plan.subsection_title
                        .as_deref()
                        .filter(|text| !text.trim().is_empty())
                        .map(clean_model_text)
                })
            {
                obj.entry("subtitle".to_string())
                    .or_insert_with(|| Value::String(subtitle));
            }
            obj.entry("section_label".to_string()).or_insert_with(|| {
                Value::String(clean_model_text(
                    plan.subsection_title
                        .as_deref()
                        .unwrap_or(&plan.section_title),
                ))
            });
        } else {
            // No matching page plan — use page_id as title fallback so validation passes
            let fallback_title = obj
                .get("page_id")
                .and_then(Value::as_str)
                .unwrap_or("Slide")
                .replace('_', " ");
            obj.entry("title".to_string())
                .or_insert_with(|| Value::String(fallback_title));
        }

        obj.entry("reason".to_string())
            .or_insert_with(|| Value::String("Auto-normalized from model output.".to_string()));
        remove_keys(obj, &["component_kind", "layout_kind", "reasoning"]);
    }
}

fn resolve_page_plan<'a>(page_plans: &'a [PagePlan], page_id: &str) -> Option<&'a PagePlan> {
    page_plans
        .iter()
        .find(|plan| plan.page_id.eq_ignore_ascii_case(page_id))
}

fn normalize_kind_name(kind: &str) -> String {
    let base = kind
        .trim()
        .to_ascii_lowercase()
        .replace('-', "_")
        .replace(' ', "_");
    // Map model hallucinations / content_shape values to valid SlideKind names
    match base.as_str() {
        "cover" => "cover",
        "overview" => "overview",
        "section_intro" | "section_overview" | "chapter_intro" | "chapter_overview" => {
            "section_intro"
        }
        "feature_grid" => "feature_grid",
        "spotlight" => "spotlight",
        "split_layers" => "split_layers",
        "section_list" | "list" | "bullet_list" | "bullets" => "section_list",
        "focus_example" | "focus" | "example" => "focus_example",
        "outcome_grid" | "outcomes" => "outcome_grid",
        "center_grid" | "grid" => "center_grid",
        "timeline" => "timeline",
        "step_flow" | "steps" | "workflow" => "step_flow",
        "process" => "process",
        "compare" | "comparison" | "versus" | "vs" => "compare",
        "issue_stack" | "issues" | "problems" | "challenge_stack" | "pain_points" => {
            "issue_stack"
        }
        "swot" => "swot",
        // Common hallucinated kinds → nearest valid equivalent
        "checklist" | "check_list" => "section_list",
        "matrix" => "center_grid",
        "summary" | "conclusion" => "center_grid",
        "architecture" | "diagram" => "split_layers",
        "table" | "data_table" => "compare",
        "quote" | "statement" | "callout" => "spotlight",
        "metrics" | "stats" | "numbers" => "outcome_grid",
        // Unknown / hallucinated kind — log so it is visible in debug output
        _ => {
            eprintln!(
                "[warn] normalize_kind_name: unrecognized kind '{}' → feature_grid",
                kind
            );
            "feature_grid"
        }
    }
    .to_string()
}

fn validate_layout_plan_value(value: &Value) -> Result<()> {
    let Some(pages) = value.get("pages").and_then(Value::as_array) else {
        anyhow::bail!("layout plan response is missing a `pages` array");
    };

    for (idx, page) in pages.iter().enumerate() {
        let Some(obj) = page.as_object() else {
            anyhow::bail!("layout plan entry {} is not a JSON object", idx + 1);
        };

        let page_label = obj
            .get("page_id")
            .and_then(Value::as_str)
            .filter(|text| !text.trim().is_empty())
            .map(|id| format!("page_id={id}"))
            .unwrap_or_else(|| format!("index={}", idx + 1));

        let mut missing = Vec::new();
        for field in ["page_id", "kind", "title", "reason"] {
            let present = obj
                .get(field)
                .map(|value| match value {
                    Value::String(text) => !text.trim().is_empty(),
                    Value::Null => false,
                    _ => true,
                })
                .unwrap_or(false);
            if !present {
                missing.push(field);
            }
        }

        if !missing.is_empty() {
            anyhow::bail!(
                "layout plan {} is missing required field(s): {}",
                page_label,
                missing.join(", ")
            );
        }
    }

    Ok(())
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
    let tones = ["amber", "blue", "green", "red", "teal", "indigo"];
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
    }
}

pub fn blueprint_schema_hint(kind: &SlideKind) -> &'static str {
    match kind {
        SlideKind::SectionIntro => {
            r#"{
  "kind": "section_intro",
  "section": "01",
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
  "section": "01",
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
  "section": "02",
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
  "section": "03",
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
  "section": "04",
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
  "section": "05",
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
  "section": "06",
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
  "section": "07",
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
  "section": "08",
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
  "section": "09",
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
  "section": "10",
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
  "section": "11",
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
  "section": "11",
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
    let path = root.join(filename);
    fs::write(&path, content)
        .with_context(|| format!("failed to write debug file: {}", path.display()))
}

pub fn load_cached_layout_plans(
    debug_dir: &Path,
    page_plans: &[PagePlan],
) -> Result<Option<Vec<LayoutPlan>>> {
    use crate::generator::audit::audit_layout_plans;

    let candidates = [
        debug_dir.join("02-layout-plan.final.parsed.json"),
        debug_dir.join("02-layout-plan.parsed.json"),
    ];
    let mut best: Option<(std::time::SystemTime, Vec<LayoutPlan>)> = None;
    for path in candidates {
        if !path.is_file() {
            continue;
        }
        let cached = serde_json::from_str::<LayoutPlanResponse>(&fs::read_to_string(&path)?)
            .with_context(|| format!("failed to read cached layout plan: {}", path.display()))?;
        let issues = audit_layout_plans(page_plans, &cached.pages);
        if issues.is_empty() {
            let modified = fs::metadata(&path)
                .and_then(|meta| meta.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            match &best {
                Some((best_time, _)) if modified <= *best_time => {}
                _ => best = Some((modified, cached.pages)),
            }
        }
    }
    Ok(best.map(|(_, pages)| pages))
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

pub fn compute_doc_hash(doc: &crate::input::ParsedDocument) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    doc.title.hash(&mut hasher);
    for sec in &doc.sections {
        sec.title.hash(&mut hasher);
        for para in &sec.paragraphs {
            para.hash(&mut hasher);
        }
        for sub in &sec.subsections {
            sub.title.hash(&mut hasher);
            for para in &sub.paragraphs {
                para.hash(&mut hasher);
            }
        }
    }
    for para in &doc.intro {
        para.hash(&mut hasher);
    }
    format!("{:016x}", hasher.finish())
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

pub fn sorted_assets(asset_paths: &HashSet<String>) -> Vec<String> {
    let mut assets = asset_paths.iter().cloned().collect::<Vec<_>>();
    assets.sort();
    assets
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
