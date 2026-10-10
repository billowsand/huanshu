//! Deterministic layout preference scoring for pages the LLM gives no usable layout for,
//! and for repair-time kind switches.
//!
//! Replaces the old `safe_fallback_kind` if-chain, whose rule order let generic branches
//! (e.g. "summary + 2-4 points → outcome_grid") swallow specific signals such as timeline,
//! workflow or problem pages. Here every row in [`SCORE_RULES`] adds its weight to one kind
//! when its signal matches, and all kinds are ranked by total score.
//!
//! Weight guide: specific content signals (content_shape, warning, problem wording) 3-4,
//! supporting signals 1.5-2, generic hints (key-point count, density) 0.5-1. [`PRIORS`]
//! break ties toward general-purpose layouts.
//!
//! This only expresses *preference*. Whether a kind may be used at all is decided by
//! `layout_rules` (callers filter the ranking through `is_eligible`).

use crate::generator::layout_rules::PROBLEM_WORDS;
use crate::types::{PagePlan, SlideKind};

/// Page signals the score rules read, extracted once per page.
pub struct Signals<'a> {
    content_shape: &'a str,
    argument_mode: &'a str,
    density: &'a str,
    visual_need: &'a str,
    object_count: &'a str,
    key_count: usize,
    title: &'a str,
}

impl<'a> Signals<'a> {
    fn from_page(page: &'a PagePlan) -> Self {
        Self {
            content_shape: page.content_shape.as_deref().unwrap_or(""),
            argument_mode: page.argument_mode.as_deref().unwrap_or(""),
            density: page.density.as_deref().unwrap_or(""),
            visual_need: page.visual_need.as_deref().unwrap_or(""),
            object_count: page.object_count.as_deref().unwrap_or(""),
            key_count: page.key_points.len(),
            title: page.page_title.as_str(),
        }
    }

    fn title_has(&self, words: &[&str]) -> bool {
        words.iter().any(|word| self.title.contains(word))
    }
}

struct ScoreRule {
    kind: SlideKind,
    weight: f32,
    /// Short label of the matched signal, surfaced in fallback reasons and debug output.
    signal: &'static str,
    matches: fn(&Signals) -> bool,
}

/// Every content kind with its tie-breaking prior. Also defines the stable order for ties.
static PRIORS: &[(SlideKind, f32)] = &[
    (SlideKind::FeatureGrid, 1.0),
    (SlideKind::SectionList, 0.8),
    (SlideKind::OutcomeGrid, 0.8),
    (SlideKind::CenterGrid, 0.5),
    (SlideKind::FocusExample, 0.5),
    (SlideKind::SplitLayers, 0.3),
    (SlideKind::StepFlow, 0.3),
    (SlideKind::Timeline, 0.2),
    (SlideKind::Process, 0.2),
    (SlideKind::Compare, 0.1),
    (SlideKind::IssueStack, 0.1),
    (SlideKind::Swot, 0.1),
    (SlideKind::Spotlight, 0.1),
];

const GOAL_WORDS: &[&str] = &["目标", "思路", "转变", "方向", "愿景"];
const COMPARE_WORDS: &[&str] = &["对比", "比较", "优劣", "vs", "VS"];
const ARCHITECTURE_WORDS: &[&str] = &["架构", "分层", "层级", "体系"];
const PROCESS_WORDS: &[&str] = &["流程", "步骤", "路径", "方法"];
const HISTORY_WORDS: &[&str] = &["历程", "发展", "里程碑", "历史", "演进"];

static SCORE_RULES: &[ScoreRule] = &[
    // --- content_shape: the most specific structural signal ---
    ScoreRule { kind: SlideKind::StepFlow, weight: 3.0, signal: "content_shape=workflow", matches: |s| s.content_shape == "workflow" },
    ScoreRule { kind: SlideKind::Process, weight: 3.0, signal: "content_shape=workflow", matches: |s| s.content_shape == "workflow" },
    ScoreRule { kind: SlideKind::SectionList, weight: 1.5, signal: "content_shape=workflow", matches: |s| s.content_shape == "workflow" },
    ScoreRule { kind: SlideKind::Timeline, weight: 4.0, signal: "content_shape=timeline", matches: |s| s.content_shape == "timeline" },
    ScoreRule { kind: SlideKind::StepFlow, weight: 1.0, signal: "content_shape=timeline", matches: |s| s.content_shape == "timeline" },
    ScoreRule { kind: SlideKind::SplitLayers, weight: 4.0, signal: "content_shape=architecture", matches: |s| s.content_shape == "architecture" },
    ScoreRule { kind: SlideKind::Compare, weight: 4.0, signal: "content_shape=comparison", matches: |s| s.content_shape == "comparison" },
    ScoreRule { kind: SlideKind::Swot, weight: 3.5, signal: "content_shape=matrix", matches: |s| s.content_shape == "matrix" },
    ScoreRule { kind: SlideKind::Swot, weight: 1.0, signal: "matrix with 4 key points", matches: |s| s.content_shape == "matrix" && s.key_count == 4 },
    ScoreRule { kind: SlideKind::FeatureGrid, weight: 0.5, signal: "content_shape=matrix", matches: |s| s.content_shape == "matrix" },
    ScoreRule { kind: SlideKind::FeatureGrid, weight: 1.5, signal: "content_shape=overview", matches: |s| s.content_shape == "overview" },
    ScoreRule { kind: SlideKind::OutcomeGrid, weight: 1.0, signal: "content_shape=overview", matches: |s| s.content_shape == "overview" },
    ScoreRule { kind: SlideKind::CenterGrid, weight: 0.5, signal: "content_shape=summary", matches: |s| s.content_shape == "summary" },
    ScoreRule { kind: SlideKind::OutcomeGrid, weight: 0.5, signal: "content_shape=summary", matches: |s| s.content_shape == "summary" },
    // --- argument_mode ---
    ScoreRule { kind: SlideKind::FeatureGrid, weight: 2.0, signal: "argument_mode=parallel", matches: |s| s.argument_mode == "parallel" },
    ScoreRule { kind: SlideKind::OutcomeGrid, weight: 1.5, signal: "argument_mode=parallel", matches: |s| s.argument_mode == "parallel" },
    ScoreRule { kind: SlideKind::StepFlow, weight: 2.0, signal: "argument_mode=sequential", matches: |s| s.argument_mode == "sequential" },
    ScoreRule { kind: SlideKind::Process, weight: 2.0, signal: "argument_mode=sequential", matches: |s| s.argument_mode == "sequential" },
    ScoreRule { kind: SlideKind::SectionList, weight: 1.5, signal: "argument_mode=sequential", matches: |s| s.argument_mode == "sequential" },
    ScoreRule { kind: SlideKind::SplitLayers, weight: 3.0, signal: "argument_mode=layered", matches: |s| s.argument_mode == "layered" },
    ScoreRule { kind: SlideKind::CenterGrid, weight: 1.0, signal: "argument_mode=summary", matches: |s| s.argument_mode == "summary" },
    ScoreRule { kind: SlideKind::OutcomeGrid, weight: 1.0, signal: "argument_mode=summary with 2-4 points", matches: |s| s.argument_mode == "summary" && (2..=4).contains(&s.key_count) },
    ScoreRule { kind: SlideKind::OutcomeGrid, weight: 2.0, signal: "argument_mode=evidence", matches: |s| s.argument_mode == "evidence" },
    ScoreRule { kind: SlideKind::FocusExample, weight: 1.5, signal: "argument_mode=evidence", matches: |s| s.argument_mode == "evidence" },
    ScoreRule { kind: SlideKind::IssueStack, weight: 1.5, signal: "argument_mode=causal", matches: |s| s.argument_mode == "causal" },
    ScoreRule { kind: SlideKind::FocusExample, weight: 1.0, signal: "argument_mode=causal", matches: |s| s.argument_mode == "causal" },
    ScoreRule { kind: SlideKind::IssueStack, weight: 3.0, signal: "argument_mode=warning", matches: |s| s.argument_mode == "warning" },
    // --- title wording ---
    ScoreRule { kind: SlideKind::IssueStack, weight: 3.5, signal: "problem wording in title", matches: |s| s.title_has(PROBLEM_WORDS) },
    ScoreRule { kind: SlideKind::CenterGrid, weight: 2.0, signal: "goal/vision title, light content", matches: |s| s.title_has(GOAL_WORDS) && s.density != "high" },
    ScoreRule { kind: SlideKind::FocusExample, weight: 2.0, signal: "goal/vision title, dense content", matches: |s| s.title_has(GOAL_WORDS) && s.density == "high" },
    ScoreRule { kind: SlideKind::Compare, weight: 2.0, signal: "comparison wording in title", matches: |s| s.title_has(COMPARE_WORDS) },
    ScoreRule { kind: SlideKind::SplitLayers, weight: 1.5, signal: "architecture wording in title", matches: |s| s.title_has(ARCHITECTURE_WORDS) },
    ScoreRule { kind: SlideKind::StepFlow, weight: 1.5, signal: "process wording in title", matches: |s| s.title_has(PROCESS_WORDS) },
    ScoreRule { kind: SlideKind::Process, weight: 1.5, signal: "process wording in title", matches: |s| s.title_has(PROCESS_WORDS) },
    ScoreRule { kind: SlideKind::Timeline, weight: 2.0, signal: "history wording in title", matches: |s| s.title_has(HISTORY_WORDS) },
    // --- visuals ---
    ScoreRule { kind: SlideKind::Spotlight, weight: 3.0, signal: "image wanted for a single object", matches: |s| s.visual_need.starts_with("image_") && s.object_count == "single" },
    ScoreRule { kind: SlideKind::Spotlight, weight: 1.0, signal: "visual_need=image_required", matches: |s| s.visual_need == "image_required" },
    // --- object count & amount of content ---
    ScoreRule { kind: SlideKind::Compare, weight: 2.0, signal: "object_count=pair", matches: |s| s.object_count == "pair" },
    ScoreRule { kind: SlideKind::FeatureGrid, weight: 0.5, signal: "object_count=multi", matches: |s| s.object_count == "multi" },
    ScoreRule { kind: SlideKind::OutcomeGrid, weight: 0.5, signal: "2-4 key points", matches: |s| (2..=4).contains(&s.key_count) },
    ScoreRule { kind: SlideKind::IssueStack, weight: 0.5, signal: "2-4 key points", matches: |s| (2..=4).contains(&s.key_count) },
    ScoreRule { kind: SlideKind::Process, weight: 1.0, signal: "5+ key points", matches: |s| s.key_count >= 5 },
    ScoreRule { kind: SlideKind::SectionList, weight: 0.5, signal: "5+ key points", matches: |s| s.key_count >= 5 },
    ScoreRule { kind: SlideKind::CenterGrid, weight: 1.0, signal: "0-1 key points", matches: |s| s.key_count <= 1 },
    ScoreRule { kind: SlideKind::FocusExample, weight: 1.0, signal: "0-1 key points", matches: |s| s.key_count <= 1 },
    // --- density ---
    ScoreRule { kind: SlideKind::FocusExample, weight: 0.5, signal: "density=high", matches: |s| s.density == "high" },
    ScoreRule { kind: SlideKind::SectionList, weight: 0.5, signal: "density=high", matches: |s| s.density == "high" },
    ScoreRule { kind: SlideKind::CenterGrid, weight: 0.5, signal: "density=low", matches: |s| s.density == "low" },
];

#[derive(Debug, Clone)]
pub struct ScoredKind {
    pub kind: SlideKind,
    pub score: f32,
    /// Labels of the matched signals, strongest first.
    pub signals: Vec<&'static str>,
}

/// Rank every content kind for this page, best first (ties keep [`PRIORS`] order).
/// Covers all content kinds, so filtering by eligibility never empties the list for a
/// content page. Ignores `page_role`: callers handle section_summary pages separately.
pub fn score_kinds(page: &PagePlan) -> Vec<ScoredKind> {
    let signals = Signals::from_page(page);
    let mut scored: Vec<ScoredKind> = PRIORS
        .iter()
        .map(|(kind, prior)| {
            let mut matched: Vec<(&'static str, f32)> = SCORE_RULES
                .iter()
                .filter(|rule| &rule.kind == kind && (rule.matches)(&signals))
                .map(|rule| (rule.signal, rule.weight))
                .collect();
            matched.sort_by(|a, b| b.1.total_cmp(&a.1));
            ScoredKind {
                kind: kind.clone(),
                score: prior + matched.iter().map(|(_, weight)| weight).sum::<f32>(),
                signals: matched.into_iter().map(|(signal, _)| signal).collect(),
            }
        })
        .collect();
    scored.sort_by(|a, b| b.score.total_cmp(&a.score));
    scored
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(title: &str, cs: &str, am: &str, key_count: usize) -> PagePlan {
        PagePlan {
            page_id: "1".to_string(),
            section_title: "章节".to_string(),
            subsection_title: None,
            page_title: title.to_string(),
            objective: String::new(),
            key_points: (0..key_count).map(|i| format!("要点{i}")).collect(),
            takeaway: None,
            content_shape: Some(cs.to_string()),
            layout_intent: None,
            visual_need: Some("text_only".to_string()),
            object_count: Some(
                match key_count {
                    0 | 1 => "single",
                    2 => "pair",
                    _ => "multi",
                }
                .to_string(),
            ),
            argument_mode: Some(am.to_string()),
            density: Some("medium".to_string()),
            source_excerpt: None,
            preferred_assets: Vec::new(),
            page_role: Some("content".to_string()),
        }
    }

    fn top(page: &PagePlan) -> SlideKind {
        score_kinds(page)[0].kind.clone()
    }

    #[test]
    fn priors_cover_every_content_kind_once_and_rules_only_score_those() {
        for (i, (kind, _)) in PRIORS.iter().enumerate() {
            assert!(
                PRIORS[i + 1..].iter().all(|(other, _)| other != kind),
                "{kind:?} listed twice"
            );
        }
        assert_eq!(PRIORS.len(), 13);
        for rule in SCORE_RULES {
            assert!(
                PRIORS.iter().any(|(kind, _)| kind == &rule.kind),
                "{:?} has rules but no prior",
                rule.kind
            );
        }
    }

    #[test]
    fn specific_shape_beats_generic_summary_signals() {
        // Old if-chain: "summary + 2-4 points → outcome_grid" fired before the workflow and
        // timeline branches, so these pages all became outcome_grid.
        assert_eq!(top(&page("实施安排", "workflow", "summary", 3)), SlideKind::StepFlow);
        assert_eq!(top(&page("实施安排", "workflow", "summary", 5)), SlideKind::Process);
        assert_eq!(top(&page("发展脉络", "timeline", "summary", 3)), SlideKind::Timeline);
        assert_eq!(top(&page("总体设计", "architecture", "summary", 3)), SlideKind::SplitLayers);
    }

    #[test]
    fn problem_wording_beats_generic_summary_signals() {
        // Old if-chain returned outcome_grid here (summary branch came first).
        assert_eq!(top(&page("主要风险", "summary", "summary", 3)), SlideKind::IssueStack);
        assert_eq!(top(&page("现状", "summary", "warning", 3)), SlideKind::IssueStack);
    }

    #[test]
    fn generic_pages_keep_previous_defaults() {
        assert_eq!(top(&page("背景", "summary", "summary", 1)), SlideKind::CenterGrid);
        assert_eq!(top(&page("核心能力", "summary", "parallel", 3)), SlideKind::FeatureGrid);
        assert_eq!(top(&page("建设成效", "summary", "evidence", 3)), SlideKind::OutcomeGrid);
        assert_eq!(top(&page("总体目标", "summary", "summary", 1)), SlideKind::CenterGrid);
    }

    #[test]
    fn pair_and_matrix_signals_pick_their_layouts() {
        assert_eq!(top(&page("方案选择", "comparison", "parallel", 2)), SlideKind::Compare);
        assert_eq!(top(&page("态势分析", "matrix", "parallel", 4)), SlideKind::Swot);
    }

    #[test]
    fn single_object_with_image_prefers_spotlight() {
        let mut p = page("产品介绍", "summary", "parallel", 1);
        p.visual_need = Some("image_required".to_string());
        assert_eq!(top(&p), SlideKind::Spotlight);
    }

    #[test]
    fn scores_explain_themselves() {
        let scored = score_kinds(&page("主要风险", "summary", "warning", 3));
        let issue = scored.iter().find(|s| s.kind == SlideKind::IssueStack).unwrap();
        assert_eq!(issue.signals[0], "problem wording in title");
        assert!(issue.signals.contains(&"argument_mode=warning"));
        assert!(scored.windows(2).all(|w| w[0].score >= w[1].score));
    }
}
