//! Single source of truth for when each slide layout may be used.
//!
//! The same table drives:
//! - the layout-candidate prompt (only eligible kinds are offered, with their `use_when` text,
//!   and ineligible ones are listed with the unmet `requirement`),
//! - filtering of LLM candidates before deck-level assignment,
//! - every deterministic fallback (`planning::fallback_candidates`, `pick_different_kind`,
//!   `normalize::make_fallback_slide`), so no code path can pick a kind the page cannot use.
//!
//! Hard requirements here should stay lenient (OR of several signals): a page whose signals
//! are slightly off loses the kind entirely, so only encode rules that are clearly wrong to break.

use crate::types::{AspectRatio, PagePlan, SlideKind};

use crate::generator::utils::slide_kind_name;

pub struct LayoutRule {
    pub kind: SlideKind,
    /// Prompt guidance: what content this layout suits.
    pub use_when: &'static str,
    /// Hard requirement, enforced by `check`. Shown in the prompt and logs when unmet.
    pub requirement: Option<&'static str>,
    /// Aspect ratios where the fixed layout breaks down (extreme whitespace).
    pub blocked_aspects: &'static [AspectRatio],
    check: fn(&PagePlan) -> bool,
}

const WIDE: &[AspectRatio] = &[AspectRatio::Ratio32x9, AspectRatio::Ratio48x9];
const ULTRA_WIDE: &[AspectRatio] = &[AspectRatio::Ratio48x9];

/// Ordered from most general to most specialized; prompt rules follow this order.
pub static LAYOUT_RULES: &[LayoutRule] = &[
    LayoutRule {
        kind: SlideKind::SectionIntro,
        use_when: "chapter-opening preview of the upcoming sub-topics",
        requirement: Some("only for section_summary pages"),
        blocked_aspects: &[],
        check: is_section_summary,
    },
    LayoutRule {
        kind: SlideKind::FeatureGrid,
        use_when: "3+ parallel items of equal weight",
        requirement: None,
        blocked_aspects: &[],
        check: always,
    },
    LayoutRule {
        kind: SlideKind::SectionList,
        use_when: "strict order-dependent steps",
        requirement: None,
        blocked_aspects: &[],
        check: always,
    },
    LayoutRule {
        kind: SlideKind::OutcomeGrid,
        use_when: "2-4 parallel deliverables/results",
        requirement: None,
        blocked_aspects: &[],
        check: always,
    },
    LayoutRule {
        kind: SlideKind::CenterGrid,
        use_when: "compact closing summary or goal",
        requirement: None,
        blocked_aspects: &[],
        check: always,
    },
    LayoutRule {
        kind: SlideKind::FocusExample,
        use_when: "central thesis + example",
        requirement: None,
        blocked_aspects: ULTRA_WIDE,
        check: always,
    },
    LayoutRule {
        kind: SlideKind::SplitLayers,
        use_when: "layered/architectural content",
        requirement: None,
        blocked_aspects: &[],
        check: always,
    },
    LayoutRule {
        kind: SlideKind::Timeline,
        use_when: "chronological events with dates",
        requirement: None,
        blocked_aspects: &[],
        check: always,
    },
    LayoutRule {
        kind: SlideKind::StepFlow,
        use_when: "2-4 step linear process",
        requirement: Some("key_points <= 4 (use process for 5+)"),
        blocked_aspects: &[],
        check: |page| page.key_points.len() <= 4,
    },
    LayoutRule {
        kind: SlideKind::Process,
        use_when: "5+ steps grouped into phases",
        requirement: Some("key_points >= 5 (use step_flow for <= 4)"),
        blocked_aspects: &[],
        check: |page| page.key_points.len() >= 5,
    },
    LayoutRule {
        kind: SlideKind::Compare,
        use_when: "EXACTLY two alternatives side by side",
        requirement: Some(
            "exactly two alternatives (object_count=pair or content_shape=comparison)",
        ),
        blocked_aspects: ULTRA_WIDE,
        check: is_pairwise,
    },
    LayoutRule {
        kind: SlideKind::IssueStack,
        use_when: "2-4 stacked problems, challenges, or risks",
        requirement: Some(
            "problem/challenge/risk content (argument_mode=warning/causal or problem wording)",
        ),
        blocked_aspects: &[],
        check: is_problem_content,
    },
    LayoutRule {
        kind: SlideKind::Swot,
        use_when: "four-quadrant matrix",
        requirement: Some("four-quadrant matrix content (content_shape=matrix or SWOT wording)"),
        blocked_aspects: &[],
        check: is_matrix,
    },
    LayoutRule {
        kind: SlideKind::ChartFocus,
        // Never offered to the LLM: a page with a chart is forced to chart_focus
        // (`planning::forced_kind`), a page without one cannot use it. Not blocked on any
        // aspect: both renderers split the width proportionally, and blocking would leave
        // ultra-wide decks with no way to show the user's chart.
        use_when:
            "page has a ```chart data chart to visualize (left key points + right chart panel)",
        requirement: Some("an attached ```chart block"),
        blocked_aspects: &[],
        check: |page| page.chart.is_some(),
    },
    LayoutRule {
        kind: SlideKind::Spotlight,
        use_when: "single-object focus with image",
        requirement: Some("a relevant image asset (preferred_assets non-empty)"),
        blocked_aspects: WIDE,
        check: has_image,
    },
];

/// Kinds that every content page may use regardless of signals or aspect ratio. Fallback
/// paths rely on these to always find an eligible kind; a test guards the invariant.
pub const UNIVERSAL_CONTENT_KINDS: &[SlideKind] = &[SlideKind::FeatureGrid, SlideKind::SectionList];

pub fn layout_rule(kind: &SlideKind) -> Option<&'static LayoutRule> {
    LAYOUT_RULES.iter().find(|rule| &rule.kind == kind)
}

/// Why `kind` cannot be used for this page, or `None` if it can.
pub fn ineligibility(kind: &SlideKind, page: &PagePlan, aspect: AspectRatio) -> Option<String> {
    let Some(rule) = layout_rule(kind) else {
        return Some("not a content layout".to_string());
    };
    // section_summary pages are reserved for SectionIntro, and vice versa.
    if is_section_summary(page) && rule.kind != SlideKind::SectionIntro {
        return Some("section_summary pages always use section_intro".to_string());
    }
    if rule.blocked_aspects.contains(&aspect) {
        return Some(format!("unsuitable for {} format", aspect_label(aspect)));
    }
    if !(rule.check)(page) {
        return Some(format!(
            "needs {}",
            rule.requirement.unwrap_or("different content")
        ));
    }
    None
}

pub fn is_eligible(kind: &SlideKind, page: &PagePlan, aspect: AspectRatio) -> bool {
    ineligibility(kind, page, aspect).is_none()
}

/// Prompt section for the layout-candidate call: the whitelist of eligible kinds with
/// guidance, plus the kinds ruled out for this page and why.
pub struct LayoutPromptRules {
    pub allowed_names: Vec<&'static str>,
    pub rules_text: String,
}

pub fn prompt_rules(page: &PagePlan, aspect: AspectRatio) -> LayoutPromptRules {
    let mut allowed_names = Vec::new();
    let mut allowed_lines = Vec::new();
    let mut blocked_lines = Vec::new();
    for rule in LAYOUT_RULES {
        let name = slide_kind_name(&rule.kind);
        match ineligibility(&rule.kind, page, aspect) {
            None => {
                allowed_names.push(name);
                allowed_lines.push(format!("- {name}: {}.", rule.use_when));
            }
            // Don't clutter content-page prompts with the structural section_intro rule.
            Some(_) if rule.kind == SlideKind::SectionIntro => {}
            Some(reason) => blocked_lines.push(format!("- {name}: {reason}")),
        }
    }
    let mut rules_text = format!("Allowed kinds for this page:\n{}", allowed_lines.join("\n"));
    if !blocked_lines.is_empty() {
        rules_text.push_str(&format!(
            "\n\nNot available for this page (never return these):\n{}",
            blocked_lines.join("\n")
        ));
    }
    if let Some(guidance) = aspect_guidance(aspect) {
        rules_text.push_str("\n\n");
        rules_text.push_str(guidance);
    }
    LayoutPromptRules {
        allowed_names,
        rules_text,
    }
}

/// Soft preferences for wide formats; the hard exclusions live in `blocked_aspects`.
fn aspect_guidance(aspect: AspectRatio) -> Option<&'static str> {
    match aspect {
        AspectRatio::Ratio16x9 => None,
        AspectRatio::Ratio32x9 => Some(
            "WIDE FORMAT (32:9): Prefer step_flow/timeline/process/feature_grid/outcome_grid. \
             Use focus_example/compare only when content truly demands a two-panel layout.",
        ),
        AspectRatio::Ratio48x9 => Some(
            "ULTRA-WIDE FORMAT (48:9): Strongly prefer step_flow/timeline/process/feature_grid/outcome_grid; \
             wide multi-column layouts fill the panoramic width best.",
        ),
    }
}

fn aspect_label(aspect: AspectRatio) -> &'static str {
    match aspect {
        AspectRatio::Ratio16x9 => "16:9",
        AspectRatio::Ratio32x9 => "32:9",
        AspectRatio::Ratio48x9 => "48:9",
    }
}

fn always(_: &PagePlan) -> bool {
    true
}

fn is_section_summary(page: &PagePlan) -> bool {
    page.page_role.as_deref() == Some("section_summary")
}

fn is_pairwise(page: &PagePlan) -> bool {
    page.object_count.as_deref() == Some("pair")
        || page.content_shape.as_deref() == Some("comparison")
}

/// Problem/risk wording; shared with `layout_scoring`.
pub(crate) const PROBLEM_WORDS: &[&str] = &[
    "问题", "挑战", "难点", "障碍", "风险", "痛点", "瓶颈", "不足", "隐患", "困境",
];

fn is_problem_content(page: &PagePlan) -> bool {
    matches!(page.argument_mode.as_deref(), Some("warning" | "causal"))
        || PROBLEM_WORDS.iter().any(|word| {
            page.page_title.contains(word) || page.key_points.iter().any(|kp| kp.contains(word))
        })
}

fn is_matrix(page: &PagePlan) -> bool {
    page.content_shape.as_deref() == Some("matrix")
        || page.page_title.to_ascii_lowercase().contains("swot")
        || page.page_title.contains("优劣势")
}

fn has_image(page: &PagePlan) -> bool {
    !page.preferred_assets.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page() -> PagePlan {
        PagePlan {
            page_id: "1".to_string(),
            section_title: "章节".to_string(),
            subsection_title: None,
            page_title: "核心能力".to_string(),
            objective: String::new(),
            key_points: vec!["一".to_string(), "二".to_string(), "三".to_string()],
            takeaway: None,
            content_shape: Some("summary".to_string()),
            layout_intent: None,
            visual_need: Some("text_only".to_string()),
            object_count: Some("multi".to_string()),
            argument_mode: Some("parallel".to_string()),
            density: Some("medium".to_string()),
            source_excerpt: None,
            preferred_assets: Vec::new(),
            page_role: Some("content".to_string()),
            section_number: Some("01".to_string()),
            chart: None,
        }
    }

    const ALL_ASPECTS: [AspectRatio; 3] = [
        AspectRatio::Ratio16x9,
        AspectRatio::Ratio32x9,
        AspectRatio::Ratio48x9,
    ];

    #[test]
    fn every_assignable_kind_has_exactly_one_rule() {
        let kinds = [
            SlideKind::SectionIntro,
            SlideKind::FeatureGrid,
            SlideKind::Spotlight,
            SlideKind::SplitLayers,
            SlideKind::SectionList,
            SlideKind::FocusExample,
            SlideKind::OutcomeGrid,
            SlideKind::CenterGrid,
            SlideKind::Timeline,
            SlideKind::StepFlow,
            SlideKind::Process,
            SlideKind::Compare,
            SlideKind::IssueStack,
            SlideKind::Swot,
            SlideKind::ChartFocus,
        ];
        for kind in &kinds {
            let count = LAYOUT_RULES
                .iter()
                .filter(|rule| &rule.kind == kind)
                .count();
            assert_eq!(count, 1, "{kind:?}");
        }
        assert_eq!(LAYOUT_RULES.len(), kinds.len());
        for kind in [SlideKind::Cover, SlideKind::Overview, SlideKind::Closing] {
            assert!(!is_eligible(&kind, &page(), AspectRatio::Ratio16x9));
        }
    }

    #[test]
    fn universal_kinds_are_eligible_for_any_content_page() {
        let mut sparse = page();
        sparse.key_points.clear();
        sparse.content_shape = None;
        sparse.object_count = None;
        sparse.argument_mode = None;
        let mut dense = page();
        dense.key_points = (0..6).map(|i| i.to_string()).collect();
        for page in [page(), sparse, dense] {
            for aspect in ALL_ASPECTS {
                for kind in UNIVERSAL_CONTENT_KINDS {
                    assert!(is_eligible(kind, &page, aspect), "{kind:?} {aspect:?}");
                }
            }
        }
    }

    #[test]
    fn section_summary_pages_only_allow_section_intro() {
        let mut summary = page();
        summary.page_role = Some("section_summary".to_string());
        assert!(is_eligible(
            &SlideKind::SectionIntro,
            &summary,
            AspectRatio::Ratio16x9
        ));
        assert!(!is_eligible(
            &SlideKind::FeatureGrid,
            &summary,
            AspectRatio::Ratio16x9
        ));
        assert!(!is_eligible(
            &SlideKind::SectionIntro,
            &page(),
            AspectRatio::Ratio16x9
        ));
    }

    #[test]
    fn step_flow_and_process_split_on_key_point_count() {
        let mut four = page();
        four.key_points = (0..4).map(|i| i.to_string()).collect();
        let mut five = page();
        five.key_points = (0..5).map(|i| i.to_string()).collect();
        let aspect = AspectRatio::Ratio16x9;
        assert!(is_eligible(&SlideKind::StepFlow, &four, aspect));
        assert!(!is_eligible(&SlideKind::Process, &four, aspect));
        assert!(!is_eligible(&SlideKind::StepFlow, &five, aspect));
        assert!(is_eligible(&SlideKind::Process, &five, aspect));
    }

    #[test]
    fn content_signals_gate_specialized_kinds() {
        let aspect = AspectRatio::Ratio16x9;
        let base = page();
        for kind in [
            SlideKind::Compare,
            SlideKind::IssueStack,
            SlideKind::Swot,
            SlideKind::Spotlight,
        ] {
            assert!(!is_eligible(&kind, &base, aspect), "{kind:?}");
        }

        let mut pair = page();
        pair.object_count = Some("pair".to_string());
        assert!(is_eligible(&SlideKind::Compare, &pair, aspect));

        let mut risky = page();
        risky.key_points.push("数据孤岛是主要瓶颈".to_string());
        assert!(is_eligible(&SlideKind::IssueStack, &risky, aspect));

        let mut swot = page();
        swot.page_title = "SWOT 分析".to_string();
        assert!(is_eligible(&SlideKind::Swot, &swot, aspect));

        let mut with_image = page();
        with_image.preferred_assets = vec!["images/arch.png".to_string()];
        assert!(is_eligible(&SlideKind::Spotlight, &with_image, aspect));
    }

    #[test]
    fn wide_formats_block_panel_and_image_layouts() {
        let mut with_image = page();
        with_image.preferred_assets = vec!["images/arch.png".to_string()];
        with_image.object_count = Some("pair".to_string());
        assert!(!is_eligible(
            &SlideKind::Spotlight,
            &with_image,
            AspectRatio::Ratio32x9
        ));
        assert!(is_eligible(
            &SlideKind::Compare,
            &with_image,
            AspectRatio::Ratio32x9
        ));
        assert!(!is_eligible(
            &SlideKind::Compare,
            &with_image,
            AspectRatio::Ratio48x9
        ));
        assert!(!is_eligible(
            &SlideKind::FocusExample,
            &with_image,
            AspectRatio::Ratio48x9
        ));
    }

    #[test]
    fn prompt_lists_only_eligible_kinds_and_explains_the_rest() {
        let rules = prompt_rules(&page(), AspectRatio::Ratio48x9);
        assert!(rules.allowed_names.contains(&"feature_grid"));
        assert!(!rules.allowed_names.contains(&"compare"));
        assert!(!rules.allowed_names.contains(&"section_intro"));
        assert!(rules
            .rules_text
            .contains("- feature_grid: 3+ parallel items"));
        assert!(rules
            .rules_text
            .contains("- compare: unsuitable for 48:9 format"));
        assert!(rules
            .rules_text
            .contains("- process: needs key_points >= 5"));
        assert!(!rules.rules_text.contains("section_intro"));
        assert!(rules.rules_text.contains("ULTRA-WIDE FORMAT"));
    }
}
