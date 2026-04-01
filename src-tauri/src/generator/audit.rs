use crate::types::{LayoutPlan, PagePlan, SlideKind};
use std::collections::BTreeMap;

pub fn audit_layout_plans(page_plans: &[PagePlan], layouts: &[LayoutPlan]) -> Vec<String> {
    let mut issues = Vec::new();
    let by_id = layouts
        .iter()
        .map(|layout| (layout.page_id.as_str(), layout))
        .collect::<BTreeMap<_, _>>();

    for page in page_plans {
        let Some(layout) = by_id.get(page.page_id.as_str()) else {
            issues.push(format!("missing layout for page_id={}", page.page_id));
            continue;
        };

        let object_count = page.object_count.as_deref().unwrap_or("");
        let argument_mode = page.argument_mode.as_deref().unwrap_or("");
        let content_shape = page.content_shape.as_deref().unwrap_or("");
        let visual_need = page.visual_need.as_deref().unwrap_or("");
        let density = page.density.as_deref().unwrap_or("");
        let key_count = page.key_points.len();
        let is_summary = page.page_role.as_deref() == Some("section_summary");
        let is_text_thesis = is_text_only_central_thesis(page);

        if is_summary {
            if layout.kind != SlideKind::SectionIntro {
                issues.push(format!(
                    "page {} is a section_summary and must use section_intro",
                    page.page_id
                ));
            }
            continue;
        }

        match layout.kind {
            SlideKind::SectionIntro => {
                issues.push(format!(
                    "page {} uses section_intro but is not a pure section_summary page",
                    page.page_id
                ));
            }
            SlideKind::Spotlight => {
                if object_count == "multi" {
                    issues.push(format!(
                        "page {} uses spotlight but object_count=multi; spotlight should focus on one object",
                        page.page_id
                    ));
                }
                if visual_need == "text_only" {
                    issues.push(format!(
                        "page {} uses spotlight but visual_need=text_only",
                        page.page_id
                    ));
                }
            }
            SlideKind::SplitLayers => {
                if argument_mode != "layered" && content_shape != "architecture" {
                    issues.push(format!(
                        "page {} uses split_layers but is not layered/architectural",
                        page.page_id
                    ));
                }
            }
            SlideKind::CenterGrid => {
                if !((argument_mode == "summary" && density != "high")
                    || (is_text_thesis && density != "high"))
                {
                    issues.push(format!(
                        "page {} uses center_grid but is not a compact summary page",
                        page.page_id
                    ));
                }
            }
            SlideKind::OutcomeGrid => {
                let mode_ok = argument_mode == "summary" || argument_mode == "evidence";
                if !mode_ok {
                    issues.push(format!(
                        "page {} uses outcome_grid but argument_mode is not summary/evidence",
                        page.page_id
                    ));
                }
                if !(2..=4).contains(&key_count) {
                    issues.push(format!(
                        "page {} uses outcome_grid but key_points count is {}",
                        page.page_id, key_count
                    ));
                }
            }
            SlideKind::SectionList => {
                if argument_mode == "parallel" && content_shape == "comparison" {
                    issues.push(format!(
                        "page {} uses section_list but looks like parallel comparison",
                        page.page_id
                    ));
                }
                let title = page.page_title.as_str();
                if (title.contains("成果") || title.contains("积累") || title.contains("沉淀"))
                    && key_count >= 2
                    && key_count <= 4
                    && (argument_mode == "summary" || argument_mode == "evidence")
                {
                    issues.push(format!(
                        "page {} uses section_list but looks like parallel outcomes/results",
                        page.page_id
                    ));
                }
            }
            SlideKind::FeatureGrid => {
                if object_count == "single" && visual_need == "image_required" {
                    issues.push(format!(
                        "page {} uses feature_grid but looks like a single showcased object with image",
                        page.page_id
                    ));
                }
                let title = page.page_title.as_str();
                if (title.contains("建议") || title.contains("机制") || title.contains("规范"))
                    && (argument_mode == "sequential" || argument_mode == "warning")
                {
                    issues.push(format!(
                        "page {} uses feature_grid but looks like action recommendations",
                        page.page_id
                    ));
                }
                if (title.contains("成果") || title.contains("积累") || title.contains("沉淀"))
                    && key_count >= 2
                    && key_count <= 4
                    && (argument_mode == "summary" || argument_mode == "evidence")
                {
                    issues.push(format!(
                        "page {} uses feature_grid but should likely be outcome_grid",
                        page.page_id
                    ));
                }
            }
            SlideKind::FocusExample => {
                if argument_mode == "parallel"
                    && content_shape != "comparison"
                    && object_count != "pair"
                {
                    issues.push(format!(
                        "page {} uses focus_example but looks like pure parallel items without a central thesis; consider feature_grid or outcome_grid",
                        page.page_id
                    ));
                }
            }
            SlideKind::Timeline => {
                if !(3..=6).contains(&key_count) {
                    issues.push(format!(
                        "page {} uses timeline but key_points count {} is outside the 3-6 range",
                        page.page_id, key_count
                    ));
                }
            }
            SlideKind::StepFlow => {
                if !(2..=5).contains(&key_count) {
                    issues.push(format!(
                        "page {} uses step_flow but key_points count {} is outside the 2-5 range",
                        page.page_id, key_count
                    ));
                }
            }
            SlideKind::Process => {
                if argument_mode != "sequential" && argument_mode != "causal" {
                    issues.push(format!(
                        "page {} uses process but argument_mode is not sequential/causal",
                        page.page_id
                    ));
                }
            }
            SlideKind::Compare => {
                if content_shape != "comparison" && argument_mode != "parallel" {
                    issues.push(format!(
                        "page {} uses compare but content_shape/argument_mode does not suggest comparison",
                        page.page_id
                    ));
                }
            }
            SlideKind::IssueStack => {
                let title = page.page_title.as_str();
                let looks_like_problem_page = title.contains("问题")
                    || title.contains("挑战")
                    || title.contains("难点")
                    || title.contains("障碍")
                    || title.contains("风险");
                if !looks_like_problem_page || !(2..=4).contains(&key_count) {
                    issues.push(format!(
                        "page {} uses issue_stack but does not look like a 2-4 item challenge/problem page",
                        page.page_id
                    ));
                }
            }
            SlideKind::Swot => {
                if content_shape != "matrix" || key_count != 4 {
                    issues.push(format!(
                        "page {} uses swot but is not a 4-item matrix page",
                        page.page_id
                    ));
                }
            }
            SlideKind::Cover | SlideKind::Closing | SlideKind::Overview => {}
        }

        if is_text_thesis
            && !matches!(
                layout.kind,
                SlideKind::CenterGrid | SlideKind::SectionList | SlideKind::FocusExample
            )
        {
            issues.push(format!(
                "page {} is a text-only central thesis page and should not use {:?}",
                page.page_id, layout.kind
            ));
        }
    }

    let total = page_plans.len();
    if total >= 5 {
        let fg_count = layouts
            .iter()
            .filter(|l| l.kind == SlideKind::FeatureGrid)
            .count();
        if fg_count * 100 / total > 65 {
            let target = (total * 65 / 100).max(1);
            let excess = fg_count.saturating_sub(target);
            let mut reassigned = 0usize;
            for page in page_plans {
                if reassigned >= excess {
                    break;
                }
                let Some(layout) = by_id.get(page.page_id.as_str()) else {
                    continue;
                };
                if layout.kind != SlideKind::FeatureGrid {
                    continue;
                }
                let am = page.argument_mode.as_deref().unwrap_or("");
                let cs = page.content_shape.as_deref().unwrap_or("");
                let vn = page.visual_need.as_deref().unwrap_or("");
                let density = page.density.as_deref().unwrap_or("");
                let has_alternative = am == "sequential"
                    || am == "layered"
                    || (am == "summary" && density != "high")
                    || cs == "workflow"
                    || cs == "architecture"
                    || cs == "spotlight"
                    || vn == "image_required"
                    || page.key_points.len() == 2;
                if has_alternative {
                    issues.push(format!(
                        "page {} uses feature_grid but deck variety requires a different kind \
                        (argument_mode={am}, content_shape={cs}, visual_need={vn}); \
                        consider spotlight, focus_example, section_list, outcome_grid, or center_grid",
                        page.page_id
                    ));
                    reassigned += 1;
                }
            }
        }
    }

    issues
}

fn is_text_only_central_thesis(page: &PagePlan) -> bool {
    let visual_need = page.visual_need.as_deref().unwrap_or("");
    let title = page.page_title.as_str();
    visual_need == "text_only"
        && (title.contains("目标")
            || title.contains("思路")
            || title.contains("转变")
            || title.contains("方向")
            || title.contains("愿景"))
}
