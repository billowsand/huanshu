//! Lenient parsing of the layout-candidate LLM response.
//!
//! Deserializing straight into `Vec<LayoutCandidate>` failed the *whole* response on a single
//! bad entry (a made-up kind like `two_column`, a score of `"8"` or `8.5`, a missing field),
//! discarding the valid candidates next to it and dropping the page to deterministic fallback.
//! Here each entry is converted on its own; unusable entries are dropped with a note.
//!
//! This layer only handles *syntax*. Whether a parsed kind may be used on the page is decided
//! afterwards by `planning::sanitize_candidates` via the `layout_rules` eligibility table.

use serde_json::{Map, Value};

use crate::types::{LayoutCandidate, SlideKind};

pub struct ParsedCandidates {
    pub candidates: Vec<LayoutCandidate>,
    /// Human-readable notes about entries that could not be used, e.g. `#2: unknown kind "x"`.
    pub dropped: Vec<String>,
}

/// Keys under which models put the candidate list, in lookup order.
const LIST_KEYS: &[&str] = &["candidates", "layouts", "options", "kinds", "results"];
const KIND_KEYS: &[&str] = &["kind", "layout", "type", "component", "name"];
const SCORE_KEYS: &[&str] = &["score", "fitness", "confidence", "rating"];
const REASON_KEYS: &[&str] = &["reason", "rationale", "why", "explanation"];

/// Parse a raw model reply into layout candidates, tolerating:
/// - shape drift: `{"candidates": [...]}`, `{"layouts": [...]}`, a top-level array, a single
///   candidate object, bare kind strings, or the list nested one wrapper deeper;
/// - kind spelling: `FeatureGrid`, `feature-grid`, `Feature Grid`, `KeynoteFeatureGridSlide`;
/// - score format: `8`, `8.6`, `"8"`, `"8/10"`, `85` (percent), `0.85` (fraction); a missing
///   score is derived from the entry's rank.
///
/// If the reply is not valid JSON at all, `kind` / `score` pairs are salvaged by pattern.
/// Fails only when nothing candidate-like can be found.
pub fn parse_layout_candidates(raw: &str) -> anyhow::Result<ParsedCandidates> {
    let mut candidates = Vec::new();
    let mut dropped = Vec::new();
    let entries = match crate::generator::utils::parse_json_with_extraction::<Value>(raw) {
        Ok(value) => candidate_entries(value),
        Err(err) => {
            let salvaged = salvage_entries(raw);
            if salvaged.is_empty() {
                return Err(err);
            }
            dropped.push(format!(
                "reply was not valid JSON; salvaged {} entries by pattern",
                salvaged.len()
            ));
            salvaged
        }
    };
    for (rank, entry) in entries.iter().enumerate() {
        match parse_entry(entry, rank) {
            Ok(candidate) => candidates.push(candidate),
            Err(why) => dropped.push(format!("#{}: {why}", rank + 1)),
        }
    }
    Ok(ParsedCandidates {
        candidates,
        dropped,
    })
}

fn candidate_entries(value: Value) -> Vec<Value> {
    match value {
        Value::Array(items) => items,
        Value::String(_) => vec![value],
        Value::Object(mut map) => {
            for key in LIST_KEYS {
                match map.remove(*key) {
                    Some(Value::Array(items)) => return items,
                    Some(single @ Value::Object(_)) => return vec![single],
                    _ => {}
                }
            }
            if KIND_KEYS.iter().any(|key| map.contains_key(*key)) {
                return vec![Value::Object(map)];
            }
            // One wrapper deeper, e.g. {"result": {"candidates": [...]}}.
            map.into_iter()
                .filter_map(|(_, inner)| match inner {
                    Value::Object(_) => Some(candidate_entries(inner)),
                    _ => None,
                })
                .find(|entries| !entries.is_empty())
                .unwrap_or_default()
        }
        _ => Vec::new(),
    }
}

/// Last resort for replies that are not valid JSON (seen in practice: a single-quoted
/// `reason` string). Pulls `kind` / `score` pairs out by pattern, in reply order; each
/// kind's score is searched only up to the next kind.
fn salvage_entries(raw: &str) -> Vec<Value> {
    use regex::Regex;
    use std::sync::OnceLock;

    static KIND: OnceLock<Regex> = OnceLock::new();
    static SCORE: OnceLock<Regex> = OnceLock::new();
    let kind_re = KIND.get_or_init(|| {
        Regex::new(r#"["']?(?:kind|layout|type)["']?\s*:\s*["']([A-Za-z][A-Za-z_\- ]*)["']"#)
            .unwrap()
    });
    let score_re = SCORE.get_or_init(|| {
        Regex::new(r#"["']?(?:score|fitness|confidence|rating)["']?\s*:\s*["']?([0-9]+(?:\.[0-9]+)?)"#)
            .unwrap()
    });

    let kinds: Vec<(usize, &str)> = kind_re
        .captures_iter(raw)
        .map(|caps| (caps.get(0).unwrap().end(), caps.get(1).unwrap().as_str()))
        .collect();
    kinds
        .iter()
        .enumerate()
        .map(|(i, (after_kind, kind))| {
            let segment_end = kinds
                .get(i + 1)
                .map_or(raw.len(), |(next_end, next_kind)| next_end - next_kind.len());
            let segment = &raw[*after_kind..segment_end.max(*after_kind)];
            let mut entry = Map::new();
            entry.insert("kind".to_string(), Value::String(kind.to_string()));
            if let Some(caps) = score_re.captures(segment) {
                entry.insert("score".to_string(), Value::String(caps[1].to_string()));
            }
            entry.insert(
                "reason".to_string(),
                Value::String("(salvaged from malformed JSON)".to_string()),
            );
            Value::Object(entry)
        })
        .collect()
}

fn parse_entry(entry: &Value, rank: usize) -> Result<LayoutCandidate, String> {
    let (kind_raw, fields) = match entry {
        Value::String(name) => (name.as_str(), None),
        Value::Object(map) => (
            first_str(map, KIND_KEYS).ok_or_else(|| "missing kind".to_string())?,
            Some(map),
        ),
        other => return Err(format!("unexpected entry {other}")),
    };
    let kind = parse_kind_name(kind_raw).ok_or_else(|| format!("unknown kind {kind_raw:?}"))?;
    let score = fields
        .and_then(|map| SCORE_KEYS.iter().find_map(|key| map.get(*key)))
        .and_then(parse_score)
        .unwrap_or_else(|| rank_default_score(rank));
    let reason = fields
        .and_then(|map| first_str(map, REASON_KEYS))
        .unwrap_or("")
        .trim()
        .to_string();
    Ok(LayoutCandidate {
        kind,
        score,
        reason,
    })
}

fn first_str<'a>(map: &'a Map<String, Value>, keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|key| map.get(*key).and_then(Value::as_str))
}

/// Map a model-written kind name onto `SlideKind`, accepting snake_case, kebab-case,
/// spaced words, PascalCase/camelCase and the `Keynote…Slide` component names.
pub fn parse_kind_name(raw: &str) -> Option<SlideKind> {
    let trimmed = raw
        .trim()
        .trim_matches(|c: char| c == '`' || c == '"' || c == '\'');
    let mut snake = String::with_capacity(trimmed.len() + 4);
    let mut after_lower_or_digit = false;
    for ch in trimmed.chars() {
        if ch.is_ascii_uppercase() {
            if after_lower_or_digit {
                snake.push('_');
            }
            snake.push(ch.to_ascii_lowercase());
            after_lower_or_digit = false;
        } else if matches!(ch, '-' | ' ' | '_') {
            if !snake.is_empty() && !snake.ends_with('_') {
                snake.push('_');
            }
            after_lower_or_digit = false;
        } else {
            snake.push(ch);
            after_lower_or_digit = ch.is_ascii_lowercase() || ch.is_ascii_digit();
        }
    }
    let name = snake.trim_matches('_');
    let name = name.strip_prefix("keynote_").unwrap_or(name);
    let name = name.strip_suffix("_slide").unwrap_or(name);
    serde_json::from_value(Value::String(name.to_string())).ok()
}

/// Normalize a score onto 1..=10. Percentages (10 < s <= 100) and fractions (0 < s < 1)
/// are rescaled; anything else non-numeric or out of range is rejected.
fn parse_score(value: &Value) -> Option<u8> {
    let number = match value {
        Value::Number(n) => n.as_f64()?,
        Value::String(text) => text.trim().split('/').next()?.trim().parse::<f64>().ok()?,
        _ => return None,
    };
    if !number.is_finite() || number <= 0.0 || number > 100.0 {
        return None;
    }
    let scaled = if number > 10.0 {
        number / 10.0
    } else if number < 1.0 {
        number * 10.0
    } else {
        number
    };
    Some(scaled.round().clamp(1.0, 10.0) as u8)
}

/// Score for an entry without one: models list candidates best-first, so 8, 7, 6, …
fn rank_default_score(rank: usize) -> u8 {
    8usize.saturating_sub(rank).max(1) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds_and_scores(raw: &str) -> Vec<(SlideKind, u8)> {
        parse_layout_candidates(raw)
            .unwrap()
            .candidates
            .into_iter()
            .map(|c| (c.kind, c.score))
            .collect()
    }

    #[test]
    fn one_bad_entry_does_not_discard_the_others() {
        let parsed = parse_layout_candidates(
            r#"{"candidates": [
                {"kind": "feature_grid", "score": 9, "reason": "parallel"},
                {"kind": "two_column", "score": 8},
                {"kind": "timeline", "score": 7}
            ]}"#,
        )
        .unwrap();
        let kinds: Vec<SlideKind> = parsed.candidates.iter().map(|c| c.kind.clone()).collect();
        assert_eq!(kinds, vec![SlideKind::FeatureGrid, SlideKind::Timeline]);
        assert_eq!(parsed.candidates[0].reason, "parallel");
        assert_eq!(parsed.dropped, vec![r#"#2: unknown kind "two_column""#.to_string()]);
    }

    #[test]
    fn kind_names_tolerate_spelling_drift() {
        for name in [
            "feature_grid",
            "FeatureGrid",
            "featureGrid",
            "feature-grid",
            "Feature Grid",
            " `feature_grid` ",
            "KeynoteFeatureGridSlide",
            "FEATURE_GRID",
        ] {
            assert_eq!(parse_kind_name(name), Some(SlideKind::FeatureGrid), "{name}");
        }
        assert_eq!(parse_kind_name("SWOT"), Some(SlideKind::Swot));
        assert_eq!(parse_kind_name("IssueStack"), Some(SlideKind::IssueStack));
        assert_eq!(parse_kind_name("two_column"), None);
        assert_eq!(parse_kind_name("特性网格"), None);
    }

    #[test]
    fn scores_tolerate_format_drift() {
        let raw = r#"{"candidates": [
            {"kind": "feature_grid", "score": "8"},
            {"kind": "outcome_grid", "score": 6.6},
            {"kind": "timeline", "score": "7/10"},
            {"kind": "step_flow", "score": 85},
            {"kind": "center_grid", "confidence": 0.42},
            {"kind": "section_list", "score": -3}
        ]}"#;
        assert_eq!(
            kinds_and_scores(raw),
            vec![
                (SlideKind::FeatureGrid, 8),
                (SlideKind::OutcomeGrid, 7),
                (SlideKind::Timeline, 7),
                (SlideKind::StepFlow, 9),
                (SlideKind::CenterGrid, 4),
                // invalid score -> rank default (6th entry: 8 - 5)
                (SlideKind::SectionList, 3),
            ]
        );
    }

    #[test]
    fn missing_scores_follow_rank_order() {
        assert_eq!(
            kinds_and_scores(r#"["process", "step_flow", "section_list"]"#),
            vec![
                (SlideKind::Process, 8),
                (SlideKind::StepFlow, 7),
                (SlideKind::SectionList, 6),
            ]
        );
    }

    #[test]
    fn response_shape_drift_is_tolerated() {
        let expected = vec![(SlideKind::Compare, 9)];
        for raw in [
            r#"{"candidates": [{"kind": "compare", "score": 9}]}"#,
            r#"{"layouts": [{"layout": "compare", "fitness": 9}]}"#,
            r#"[{"type": "compare", "score": 9}]"#,
            r#"{"kind": "compare", "score": 9}"#,
            r#"{"candidates": {"kind": "compare", "score": 9}}"#,
            r#"{"result": {"candidates": [{"kind": "compare", "score": 9}]}}"#,
            "Sure! Here is the ranking:\n```json\n{\"candidates\": [{\"kind\": \"compare\", \"score\": 9}]}\n```",
        ] {
            assert_eq!(kinds_and_scores(raw), expected, "{raw}");
        }
    }

    #[test]
    fn non_content_kinds_still_parse_for_later_eligibility_filtering() {
        // Parsing is syntax only; planning::sanitize_candidates rejects cover etc.
        assert_eq!(
            kinds_and_scores(r#"[{"kind": "cover", "score": 9}]"#),
            vec![(SlideKind::Cover, 9)]
        );
    }

    #[test]
    fn entries_without_a_kind_are_reported() {
        let parsed =
            parse_layout_candidates(r#"{"candidates": [{"score": 9}, 42, {"kind": "swot"}]}"#)
                .unwrap();
        assert_eq!(parsed.candidates.len(), 1);
        assert_eq!(parsed.candidates[0].kind, SlideKind::Swot);
        assert_eq!(parsed.dropped.len(), 2);
        assert!(parsed.dropped[0].contains("missing kind"));
        assert!(parsed.dropped[1].contains("unexpected entry 42"));
    }

    #[test]
    fn no_json_is_an_error() {
        assert!(parse_layout_candidates("I think feature grid fits best.").is_err());
    }

    #[test]
    fn malformed_json_is_salvaged_by_pattern() {
        // Real qwen3.5-9b reply: the second reason uses single quotes, so the whole reply
        // is invalid JSON.
        let raw = r#"{
  "candidates": [
    {
      "kind": "feature_grid",
      "score": 9.5,
      "reason": "four parallel key points of equal weight"
    },
    {
      "kind": "outcome_grid",
      "score": 8.0,
      "reason": 'The content describes four distinct deliverables'
    },
    {
      "kind": "center_grid",
      "score": 6.5,
      "reason": "clear central theme ('Overall Objectives')"
    }
  ]
}"#;
        let parsed = parse_layout_candidates(raw).unwrap();
        let got: Vec<(SlideKind, u8)> =
            parsed.candidates.iter().map(|c| (c.kind.clone(), c.score)).collect();
        assert_eq!(
            got,
            vec![
                (SlideKind::FeatureGrid, 10),
                (SlideKind::OutcomeGrid, 8),
                (SlideKind::CenterGrid, 7),
            ]
        );
        assert!(parsed.dropped[0].contains("salvaged 3 entries"));
    }
}

