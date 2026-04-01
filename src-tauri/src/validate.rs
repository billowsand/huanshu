use crate::icon::IconIndex;
use crate::types::{SlideBlueprint, SlideKind};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ValidationIssue {
    pub message: String,
}

pub fn validate_blueprints(
    blueprints: &[SlideBlueprint],
    icon_index: &IconIndex,
    asset_paths: &HashSet<String>,
) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();
    for (idx, slide) in blueprints.iter().enumerate() {
        if slide.title.trim().is_empty() {
            issues.push(issue(idx, "title is empty"));
        }
        if slide.kind == SlideKind::Spotlight && slide.panels.is_empty() {
            issues.push(issue(idx, "spotlight slide requires at least one panel"));
        }
        if slide.kind == SlideKind::Overview && slide.overview_items.is_empty() {
            issues.push(issue(idx, "overview slide requires overview_items"));
        }
        if slide.kind == SlideKind::SectionIntro && slide.cards.is_empty() {
            issues.push(issue(idx, "section intro requires cards"));
        }
        if slide.kind == SlideKind::FeatureGrid && slide.cards.is_empty() {
            issues.push(issue(idx, "feature grid requires cards"));
        }
        if slide.kind == SlideKind::IssueStack && slide.cards.is_empty() {
            issues.push(issue(idx, "issue stack requires cards"));
        }
        if slide.kind == SlideKind::SectionIntro {
            for (card_idx, card) in slide.cards.iter().enumerate() {
                let has_content = card.body.as_ref().is_some_and(|v| !v.trim().is_empty())
                    || !card.items.is_empty()
                    || card
                        .conclusion
                        .as_ref()
                        .is_some_and(|v| !v.trim().is_empty());
                if !has_content {
                    issues.push(issue(
                        idx,
                        format!(
                            "section intro card {} has no body/items/conclusion",
                            card_idx + 1
                        ),
                    ));
                }
            }
        }
        if slide.kind == SlideKind::FeatureGrid {
            for (card_idx, card) in slide.cards.iter().enumerate() {
                let has_content = card.body.as_ref().is_some_and(|v| !v.trim().is_empty())
                    || !card.items.is_empty()
                    || card
                        .conclusion
                        .as_ref()
                        .is_some_and(|v| !v.trim().is_empty());
                if !has_content {
                    issues.push(issue(
                        idx,
                        format!(
                            "feature grid card {} has no body/items/conclusion",
                            card_idx + 1
                        ),
                    ));
                }
            }
        }
        for icon in slide.iter_icons() {
            if !icon_index.contains(icon) {
                issues.push(issue(idx, format!("invalid icon: {icon}")));
            }
        }
        for image in slide.iter_images() {
            if !asset_exists(image, asset_paths) {
                issues.push(issue(idx, format!("missing image asset: {image}")));
            }
        }
        if slide.kind == SlideKind::SplitLayers && slide.layers.is_empty() {
            issues.push(issue(idx, "split layers slide requires at least one layer"));
        }
        if slide.kind == SlideKind::SplitLayers && slide.left_items.is_empty() {
            issues.push(issue(idx, "split layers slide requires left_items"));
        }
        if slide.kind == SlideKind::FocusExample && slide.points.is_empty() {
            issues.push(issue(
                idx,
                "focus example slide requires at least one point",
            ));
        }
        if slide.kind == SlideKind::OutcomeGrid && slide.cards.is_empty() {
            issues.push(issue(idx, "outcome grid requires cards"));
        }
        if slide.kind == SlideKind::OutcomeGrid {
            for (card_idx, card) in slide.cards.iter().enumerate() {
                if !has_text(card.body.as_deref()) {
                    issues.push(issue(
                        idx,
                        format!("outcome grid card {} requires body", card_idx + 1),
                    ));
                }
                if !has_text(card.icon.as_deref()) {
                    issues.push(issue(
                        idx,
                        format!("outcome grid card {} requires icon", card_idx + 1),
                    ));
                }
                if !has_text(card.tag.as_deref()) {
                    issues.push(issue(
                        idx,
                        format!("outcome grid card {} requires tag", card_idx + 1),
                    ));
                }
                if !has_text(card.top_bar_class.as_deref()) {
                    issues.push(issue(
                        idx,
                        format!("outcome grid card {} requires top_bar_class", card_idx + 1),
                    ));
                }
            }
        }
        if slide.kind == SlideKind::SectionList && slide.list_items.is_empty() {
            issues.push(issue(idx, "section list requires list_items"));
        }
        if slide.kind == SlideKind::FeatureGrid && slide.cards.len() > 4 {
            issues.push(issue(
                idx,
                "feature grid supports at most 4 cards for this theme",
            ));
        }
        if slide.kind == SlideKind::IssueStack && slide.cards.len() > 4 {
            issues.push(issue(idx, "issue stack supports at most 4 cards"));
        }
        if slide.kind == SlideKind::SectionIntro && slide.cards.len() > 4 {
            issues.push(issue(idx, "section intro supports at most 4 cards"));
        }
        if slide.kind == SlideKind::SectionList && slide.list_items.len() > 4 {
            issues.push(issue(
                idx,
                "section list supports at most 4 items for readability",
            ));
        }
        if slide.kind == SlideKind::CenterGrid && slide.center_items.len() > 4 {
            issues.push(issue(idx, "center grid supports at most 4 items"));
        }
        if slide.kind == SlideKind::CenterGrid && slide.center_items.is_empty() {
            issues.push(issue(idx, "center grid requires center_items"));
        }
        if slide.kind == SlideKind::Timeline && slide.timeline_events.is_empty() {
            issues.push(issue(idx, "timeline slide requires timeline_events"));
        }
        if slide.kind == SlideKind::StepFlow && slide.steps.len() < 2 {
            issues.push(issue(idx, "step flow requires at least 2 steps"));
        }
        if slide.kind == SlideKind::StepFlow && slide.steps.len() > 5 {
            issues.push(issue(idx, "step flow supports at most 5 steps"));
        }
        if slide.kind == SlideKind::Process && slide.phases.is_empty() {
            issues.push(issue(idx, "process slide requires phases"));
        }
        if slide.kind == SlideKind::Process && slide.phases.len() > 4 {
            issues.push(issue(idx, "process slide supports at most 4 phases"));
        }
        if slide.kind == SlideKind::Process {
            for (phase_idx, phase) in slide.phases.iter().enumerate() {
                if phase.steps.is_empty() {
                    issues.push(issue(
                        idx,
                        format!("process phase {} requires steps", phase_idx + 1),
                    ));
                }
            }
        }
        if slide.kind == SlideKind::Spotlight && slide.panels.len() > 3 {
            issues.push(issue(idx, "spotlight slide supports at most 3 panels"));
        }
        if slide.kind == SlideKind::Timeline
            && !slide.timeline_events.is_empty()
            && slide.timeline_events.len() < 3
        {
            issues.push(issue(idx, "timeline slide requires at least 3 events"));
        }
        if slide.kind == SlideKind::Compare {
            let Some(compare) = slide.compare_data.as_ref() else {
                issues.push(issue(idx, "compare slide requires compare_data"));
                continue;
            };
            if compare.left.items.is_empty() {
                issues.push(issue(idx, "compare slide left side requires items"));
            }
            if compare.right.items.is_empty() {
                issues.push(issue(idx, "compare slide right side requires items"));
            }
            if compare.left.items.len() < 2 {
                issues.push(issue(
                    idx,
                    "compare slide left side should have at least 2 items",
                ));
            }
            if compare.right.items.len() < 2 {
                issues.push(issue(
                    idx,
                    "compare slide right side should have at least 2 items",
                ));
            }
        }
        if slide.kind == SlideKind::Swot {
            let Some(swot) = slide.swot_data.as_ref() else {
                issues.push(issue(idx, "swot slide requires swot_data"));
                continue;
            };
            if swot.quadrants.len() != 4 {
                issues.push(issue(idx, "swot slide requires exactly 4 quadrants"));
            }
            const VALID_SWOT_KEYS: &[&str] =
                &["strengths", "weaknesses", "opportunities", "threats"];
            for (quad_idx, quadrant) in swot.quadrants.iter().enumerate() {
                if quadrant.items.is_empty() {
                    issues.push(issue(
                        idx,
                        format!("swot quadrant {} requires at least one item", quad_idx + 1),
                    ));
                }
                if !VALID_SWOT_KEYS.contains(&quadrant.key.as_str()) {
                    issues.push(issue(
                        idx,
                        format!(
                            "swot quadrant {} has invalid key '{}'; must be one of strengths, weaknesses, opportunities, threats",
                            quad_idx + 1,
                            quadrant.key
                        ),
                    ));
                }
            }
        }
        // Validate tone values across all tone-bearing fields
        for (field, tone) in collect_tones(slide) {
            if !matches!(tone.as_str(), "amber" | "blue" | "green" | "red") {
                issues.push(issue(
                    idx,
                    format!("invalid tone '{tone}' in {field}; must be amber, blue, green, or red"),
                ));
            }
        }
    }
    issues
}

pub fn collect_assets(project_root: &Path) -> HashSet<String> {
    let mut set = HashSet::new();
    for base in ["figure", "public"] {
        let root = project_root.join(base);
        collect_dir(&root, &root, &mut set);
    }
    set
}

fn collect_dir(root: &Path, current: &Path, set: &mut HashSet<String>) {
    let Ok(entries) = std::fs::read_dir(current) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_dir(root, &path, set);
            continue;
        }
        if let Ok(rel) = path.strip_prefix(root) {
            let rel = rel.to_string_lossy().replace('\\', "/");
            let prefix = root.file_name().unwrap_or_default().to_string_lossy();
            set.insert(format!("{prefix}/{rel}"));
            set.insert(format!("/{prefix}/{rel}"));
            if prefix == "figure" {
                set.insert(rel.clone());
            }
        }
    }
}

fn asset_exists(image: &str, paths: &HashSet<String>) -> bool {
    if paths.contains(image) {
        return true;
    }

    if parse_media_ref(image).is_some() {
        return true;
    }

    let path = Path::new(image);
    path.is_absolute() && path.exists()
}

fn parse_media_ref(value: &str) -> Option<i64> {
    value.strip_prefix("media:")?.parse::<i64>().ok()
}

fn issue(slide_idx: usize, message: impl Into<String>) -> ValidationIssue {
    ValidationIssue {
        message: format!("slide {}: {}", slide_idx + 1, message.into()),
    }
}

fn has_text(value: Option<&str>) -> bool {
    value.is_some_and(|v| !v.trim().is_empty())
}

/// Collect all (field_description, tone_value) pairs from a slide for tone validation.
fn collect_tones(slide: &SlideBlueprint) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (i, c) in slide.cards.iter().enumerate() {
        if let Some(t) = &c.tone { out.push((format!("card[{}].tone", i), t.clone())); }
    }
    for (i, p) in slide.panels.iter().enumerate() {
        if let Some(t) = &p.tone { out.push((format!("panel[{}].tone", i), t.clone())); }
    }
    for (i, l) in slide.layers.iter().enumerate() {
        if let Some(t) = &l.tone { out.push((format!("layer[{}].tone", i), t.clone())); }
    }
    for (i, ci) in slide.center_items.iter().enumerate() {
        if let Some(t) = &ci.tone { out.push((format!("center_item[{}].tone", i), t.clone())); }
    }
    for (i, s) in slide.steps.iter().enumerate() {
        if let Some(t) = &s.tone { out.push((format!("step[{}].tone", i), t.clone())); }
    }
    for (i, ph) in slide.phases.iter().enumerate() {
        if let Some(t) = &ph.tone { out.push((format!("phase[{}].tone", i), t.clone())); }
    }
    for (i, ev) in slide.timeline_events.iter().enumerate() {
        if let Some(t) = &ev.tone { out.push((format!("timeline_event[{}].tone", i), t.clone())); }
    }
    if let Some(cd) = &slide.compare_data {
        if let Some(t) = &cd.left.tone { out.push(("compare_data.left.tone".into(), t.clone())); }
        if let Some(t) = &cd.right.tone { out.push(("compare_data.right.tone".into(), t.clone())); }
    }
    if let Some(sd) = &slide.swot_data {
        for (i, q) in sd.quadrants.iter().enumerate() {
            if let Some(t) = &q.tone { out.push((format!("swot.quadrant[{}].tone", i), t.clone())); }
        }
    }
    out
}

#[allow(dead_code)]
pub fn deck_output_path(project_root: &Path, output: &Path) -> PathBuf {
    if output.is_absolute() {
        output.to_path_buf()
    } else {
        project_root.join(output)
    }
}
