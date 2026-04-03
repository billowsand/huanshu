use crate::config::GenerationConfig;
use crate::icon::IconIndex;
use crate::input::ParsedDocument;
use crate::lmstudio::LmStudioClient;
use crate::types::*;
use crate::validate::validate_blueprints;
use anyhow::{Result, bail};
use std::collections::HashSet;
use std::path::Path;

use crate::generator::utils::{
    clean_model_text, format_issue_block, infer_asset_from_text, write_debug,
};

pub async fn repair_until_valid(
    client: &LmStudioClient,
    config: &GenerationConfig,
    doc: &ParsedDocument,
    page_plans: &[PagePlan],
    layout_plans: &[LayoutPlan],
    mut slides: Vec<SlideBlueprint>,
    icon_index: &IconIndex,
    asset_paths: &HashSet<String>,
    debug_dir: &Path,
) -> Result<Vec<SlideBlueprint>> {
    use crate::generator::utils::blueprint_schema_hint;

    // Always attempt at least 3 rounds; use config.repair_rounds if higher.
    let max_rounds = config.repair_rounds.max(3);

    for round in 0..max_rounds {
        let issues = validate_blueprints(&slides, icon_index, asset_paths);
        if issues.is_empty() {
            return Ok(slides);
        }

        // Parse failing slide indices (1-based in messages, convert to 0-based).
        let failing_indices: Vec<usize> = {
            let mut seen = std::collections::BTreeSet::new();
            for issue in &issues {
                if let Some(rest) = issue.message.strip_prefix("slide ") {
                    if let Some(idx_str) = rest.split(':').next() {
                        if let Ok(n) = idx_str.trim().parse::<usize>() {
                            seen.insert(n - 1);
                        }
                    }
                }
            }
            seen.into_iter().collect()
        };

        if failing_indices.is_empty() {
            // Cannot determine which slides are failing; skip to fallback.
            break;
        }

        let is_last_round = round == max_rounds - 1;

        if is_last_round {
            // Final LLM round: actually switch the component kind for failing slides.
            // The exclusion hint is useless if we keep the same layout kind (the prompt
            // and normaliser both enforce slide.kind == layout.kind). Instead we pick a
            // new kind deterministically and regenerate from scratch with that new layout.
            use crate::generator::planning::pick_different_kind;

            let failing_with_context: Vec<(usize, String, Vec<String>)> = failing_indices
                .iter()
                .map(|&idx| {
                    let prev_kind = slides
                        .get(idx)
                        .map(|s| format!("{:?}", s.kind))
                        .unwrap_or_default();
                    let errs: Vec<String> = issues
                        .iter()
                        .filter(|iss| {
                            iss.message
                                .starts_with(&format!("slide {}:", idx + 1))
                        })
                        .map(|iss| iss.message.clone())
                        .collect();
                    (idx, prev_kind, errs)
                })
                .collect();

            // Build a modified layout plan list where each failing slide gets a new kind.
            let mut modified_layouts = layout_plans.to_vec();
            for (idx, _prev_kind, _) in &failing_with_context {
                if let (Some(lp), Some(page)) =
                    (modified_layouts.get_mut(*idx), page_plans.get(*idx))
                {
                    let current_kind = slides.get(*idx).map(|s| &s.kind).unwrap_or(&lp.kind);
                    let new_kind = pick_different_kind(page, current_kind);
                    eprintln!(
                        "repair_until_valid: slide {} switching kind {:?} → {:?} for final regen",
                        idx + 1, current_kind, new_kind
                    );
                    lp.reason =
                        format!("re-kind from {:?} after validation failures", current_kind);
                    lp.kind = new_kind;
                }
            }

            let debug_prefix = format!("04-repair-round{}-regen", round + 1);
            write_debug(
                debug_dir,
                &format!("{debug_prefix}.context.json"),
                &serde_json::to_string_pretty(&failing_with_context
                    .iter()
                    .map(|(i, k, e)| serde_json::json!({"idx": i+1, "prev_kind": k, "new_kind": modified_layouts.get(*i).map(|l| format!("{:?}", l.kind)), "errors": e}))
                    .collect::<Vec<_>>())
                    .unwrap_or_default(),
            )?;

            crate::generator::slides::regenerate_slides_at(
                client,
                config,
                doc,
                page_plans,
                &modified_layouts,
                &mut slides,
                &failing_with_context,
                icon_index,
                asset_paths,
                debug_dir,
                round,
            )
            .await?;
        } else {
            // Standard LLM repair round: patch the existing blueprints.
            let failing_items: Vec<serde_json::Value> = failing_indices
                .iter()
                .filter_map(|&idx| {
                    let slide = slides.get(idx)?;
                    let schema = blueprint_schema_hint(&slide.kind);
                    let page = page_plans.get(idx);
                    let layout = layout_plans.get(idx);
                    Some(serde_json::json!({
                        "slide_index": idx + 1,
                        "page_plan": page,
                        "layout_plan": layout,
                        "page_id": page.map(|p| &p.page_id),
                        "current_slide": slide,
                        "component_schema": schema,
                    }))
                })
                .collect();

            let system = "You repair invalid Slidev slide blueprints. Fix only what is necessary to satisfy validation while preserving the content intent. Return strict JSON. IMPORTANT: all icon fields must use the i-carbon: prefix (e.g. \"i-carbon:chart-line\"). Never use i-mdi:, i-fa:, or any other prefix.";
            let user = format!(
                "Document title: {}\n\nFailing slides with their plans and schemas:\n{}\n\nValidation issues:\n{}\n\nReturn ONLY the corrected failing slides as JSON: {{\"slides\": [...]}} in the same order as listed above.",
                doc.title,
                serde_json::to_string_pretty(&failing_items).unwrap_or_default(),
                format_issue_block("issues", &issues),
            );
            let debug_prefix = format!("04-repair-round{}", round + 1);
            write_debug(debug_dir, &format!("{debug_prefix}.system.txt"), system)?;
            write_debug(debug_dir, &format!("{debug_prefix}.user.txt"), &user)?;
            let raw = client.generate_text(&config.model, system, &user).await?;
            write_debug(debug_dir, &format!("{debug_prefix}.raw.txt"), &raw)?;
            let resp: RepairResponse = crate::generator::utils::parse_json_with_extraction(&raw)?;
            write_debug(
                debug_dir,
                &format!("{debug_prefix}.parsed.json"),
                &serde_json::to_string_pretty(&resp)?,
            )?;

            let mut repaired_iter = resp.slides.into_iter();
            for &idx in &failing_indices {
                if let Some(repaired) = repaired_iter.next() {
                    if let Some(slot) = slides.get_mut(idx) {
                        *slot = repaired;
                    }
                }
            }
        }

        normalize_blueprints(
            slides.as_mut_slice(),
            client,
            &config.embedding_model,
            icon_index,
            asset_paths,
            config.aspect_ratio,
        )
        .await?;
    }

    // All rounds exhausted. Apply deterministic fallback for any remaining failures.
    let issues = validate_blueprints(&slides, icon_index, asset_paths);
    if !issues.is_empty() {
        let still_failing: Vec<usize> = {
            let mut seen = std::collections::BTreeSet::new();
            for issue in &issues {
                if let Some(rest) = issue.message.strip_prefix("slide ") {
                    if let Some(idx_str) = rest.split(':').next() {
                        if let Ok(n) = idx_str.trim().parse::<usize>() {
                            seen.insert(n - 1);
                        }
                    }
                }
            }
            seen.into_iter().collect()
        };

        if still_failing.is_empty() {
            // Index parsing failed; nothing we can do.
            bail!(format_issue_block("slides remain invalid after repair", &issues));
        }

        eprintln!(
            "repair_until_valid: {} slide(s) still invalid after {} rounds; applying deterministic fallback",
            still_failing.len(),
            max_rounds
        );

        for idx in still_failing {
            if let Some(page) = page_plans.get(idx) {
                let layout = layout_plans.get(idx);
                let fallback = make_fallback_slide(page, layout, config.aspect_ratio);
                if let Some(slot) = slides.get_mut(idx) {
                    *slot = fallback;
                }
            }
        }

        // Fallback slides: apply component-specific defaults first (fills required
        // fields for every kind), then normalize lengths and tones.
        for slide in slides.iter_mut() {
            apply_component_defaults(slide);
            normalize_lengths(slide);
            normalize_tones(slide);
        }

        // Final check after fallback — should always pass.
        let final_issues = validate_blueprints(&slides, icon_index, asset_paths);
        if !final_issues.is_empty() {
            bail!(format_issue_block(
                "slides remain invalid even after fallback",
                &final_issues
            ));
        }
    }

    Ok(slides)
}

/// Build a minimal valid slide deterministically from a PagePlan.
/// Used as a last-resort fallback when all LLM repair rounds fail.
///
/// Uses `safe_fallback_kind` for full 12-kind coverage. Required fields for each
/// kind are guaranteed by the `apply_component_defaults` call in the caller; this
/// function only pre-fills the five kinds it can meaningfully populate from
/// key_points (SectionIntro, FeatureGrid, OutcomeGrid, SectionList, StepFlow).
pub fn make_fallback_slide(page: &PagePlan, layout: Option<&LayoutPlan>, aspect_ratio: AspectRatio) -> SlideBlueprint {
    use crate::generator::planning::safe_fallback_kind;
    // Deliberately ignore the layout plan's kind — it's the one that failed.
    let _ = layout;
    let kind = safe_fallback_kind(page);

    // Build minimal cards / list_items from key_points
    let key_points = &page.key_points;

    let cards: Vec<crate::types::GridCard> = key_points
        .iter()
        .take(4)
        .map(|kp| crate::types::GridCard {
            title: kp.clone(),
            tone: None,
            icon: None,
            subtitle: None,
            body: Some(kp.clone()),
            items: Vec::new(),
            conclusion: None,
            footer_tag: None,
            footer_tone: None,
            top_bar_class: None,
            risk: None,
            tag: None,
        })
        .collect();

    let list_items: Vec<crate::types::ListItem> = key_points
        .iter()
        .enumerate()
        .take(4)
        .map(|(i, kp)| crate::types::ListItem {
            step: Some(format!("{:02}", i + 1)),
            icon: None,
            title: kp.clone(),
            body: kp.clone(),
        })
        .collect();

    SlideBlueprint {
        kind: kind.clone(),
        aspect_ratio: Some(aspect_ratio),
        section: Some(page.page_id.clone()),
        title: page.page_title.clone(),
        subtitle: None,
        badge: None,
        accent: None,
        note: None,
        label: None,
        label_tone: None,
        image: None,
        images: Vec::new(),
        placeholder: None,
        side_width: None,
        badges: Vec::new(),
        overview_items: Vec::new(),
        cards: match kind {
            SlideKind::SectionIntro
            | SlideKind::FeatureGrid
            | SlideKind::OutcomeGrid
            | SlideKind::IssueStack => cards,
            _ => Vec::new(),
        },
        panels: Vec::new(),
        left_items: Vec::new(),
        layers: Vec::new(),
        list_items: match kind {
            SlideKind::SectionList => list_items,
            _ => Vec::new(),
        },
        points: Vec::new(),
        ranking: Vec::new(),
        center_items: Vec::new(),
        footer: None,
        example_title: None,
        example_body: None,
        timeline_events: Vec::new(),
        steps: match kind {
            SlideKind::StepFlow => key_points
                .iter()
                .take(4)
                .map(|kp| crate::types::StepItem {
                    title: kp.clone(),
                    body: Some(kp.clone()),
                    icon: None,
                    tone: None,
                })
                .collect(),
            _ => Vec::new(),
        },
        phases: Vec::new(),
        direction: None,
        compare_data: None,
        swot_data: None,
        infographic_syntax: None,
    }
}

pub async fn normalize_blueprints(
    slides: &mut [SlideBlueprint],
    client: &LmStudioClient,
    embedding_model: &str,
    icon_index: &IconIndex,
    asset_paths: &HashSet<String>,
    aspect_ratio: AspectRatio,
) -> Result<()> {
    // CPU-only passes first
    for slide in slides.iter_mut() {
        slide.aspect_ratio = Some(aspect_ratio);
        normalize_lengths(slide);
        normalize_tones(slide);
        repair_assets(slide, asset_paths);
        apply_component_defaults(slide);
    }
    // Fix only icon slots that are invalid/missing — valid icons kept as-is
    crate::generator::icons::fix_invalid_icons(slides, client, embedding_model, icon_index).await?;
    Ok(())
}

pub fn normalize_lengths(slide: &mut SlideBlueprint) {
    use crate::generator::utils::summarize;

    slide.title = summarize(&clean_model_text(&slide.title), 28);
    slide.subtitle = slide
        .subtitle
        .as_ref()
        .map(|v| summarize(&clean_model_text(v), 36));
    slide.note = slide
        .note
        .as_ref()
        .map(|v| summarize(&clean_model_text(v), 110));
    slide.label = slide
        .label
        .as_ref()
        .map(|v| summarize(&clean_model_text(v), 20));
    slide.placeholder = slide
        .placeholder
        .as_ref()
        .map(|v| summarize(&clean_model_text(v), 28))
        .filter(|v| !v.trim().is_empty());
    slide.footer = slide
        .footer
        .as_ref()
        .map(|v| summarize(&clean_model_text(v), 80));
    slide.example_title = slide
        .example_title
        .as_ref()
        .map(|v| summarize(&clean_model_text(v), 20));
    slide.example_body = slide
        .example_body
        .as_ref()
        .map(|v| summarize(&clean_model_text(v), 80));
    if let Some(swot) = &mut slide.swot_data {
        swot.strategy = swot
            .strategy
            .as_ref()
            .map(|v| summarize(&clean_model_text(v), 64));
        for quadrant in &mut swot.quadrants {
            quadrant.title = summarize(&clean_model_text(&quadrant.title), 20);
            quadrant.summary = quadrant
                .summary
                .as_ref()
                .map(|v| summarize(&clean_model_text(v), 32));
            for item in &mut quadrant.items {
                *item = summarize(&clean_model_text(item), 22);
            }
        }
    }

    for card in &mut slide.cards {
        card.title = summarize(&clean_model_text(&card.title), 18);
        card.subtitle = card
            .subtitle
            .as_ref()
            .map(|v| summarize(&clean_model_text(v), 24));
        card.body = card
            .body
            .as_ref()
            .map(|v| summarize(&clean_model_text(v), 72));
        card.conclusion = card
            .conclusion
            .as_ref()
            .map(|v| summarize(&clean_model_text(v), 20));
        for item in &mut card.items {
            *item = summarize(&clean_model_text(item), 24);
        }
    }
    for panel in &mut slide.panels {
        panel.title = summarize(&clean_model_text(&panel.title), 16);
        panel.body = panel
            .body
            .as_ref()
            .map(|v| summarize(&clean_model_text(v), 88));
        for item in &mut panel.items {
            *item = summarize(&clean_model_text(item), 22);
        }
        for step in &mut panel.steps {
            *step = summarize(&clean_model_text(step), 22);
        }
    }
    for item in slide
        .list_items
        .iter_mut()
        .chain(slide.left_items.iter_mut())
        .chain(slide.points.iter_mut())
    {
        item.title = summarize(&clean_model_text(&item.title), 18);
        item.body = summarize(&clean_model_text(&item.body), 70);
    }
    for item in &mut slide.center_items {
        item.title = summarize(&clean_model_text(&item.title), 16);
        item.desc = summarize(&clean_model_text(&item.desc), 24);
    }
    for layer in &mut slide.layers {
        layer.title = summarize(&clean_model_text(&layer.title), 16);
        layer.meta = summarize(&clean_model_text(&layer.meta), 36);
    }
}

pub fn normalize_tones(slide: &mut SlideBlueprint) {
    for tone in slide
        .cards
        .iter_mut()
        .filter_map(|x| x.tone.as_mut())
        .chain(slide.panels.iter_mut().filter_map(|x| x.tone.as_mut()))
        .chain(slide.layers.iter_mut().filter_map(|x| x.tone.as_mut()))
        .chain(
            slide
                .center_items
                .iter_mut()
                .filter_map(|x| x.tone.as_mut()),
        )
        .chain(
            slide
                .swot_data
                .iter_mut()
                .flat_map(|data| data.quadrants.iter_mut())
                .filter_map(|x| x.tone.as_mut()),
        )
        .chain(slide.steps.iter_mut().filter_map(|x| x.tone.as_mut()))
        .chain(slide.phases.iter_mut().filter_map(|x| x.tone.as_mut()))
        .chain(
            slide
                .timeline_events
                .iter_mut()
                .filter_map(|x| x.tone.as_mut()),
        )
        .chain(
            slide
                .compare_data
                .iter_mut()
                .flat_map(|c| [c.left.tone.as_mut(), c.right.tone.as_mut()])
                .flatten(),
        )
    {
        if !matches!(tone.as_str(), "amber" | "blue" | "green" | "red") {
            *tone = "amber".to_string();
        }
    }
}

pub fn repair_assets(slide: &mut SlideBlueprint, asset_paths: &HashSet<String>) {
    if let Some(image) = slide.image.as_ref() {
        let image_path = std::path::Path::new(image);
        let is_media_ref = image
            .strip_prefix("media:")
            .and_then(|id| id.parse::<i64>().ok())
            .is_some();
        if !asset_paths.contains(image)
            && !is_media_ref
            && !(image_path.is_absolute() && image_path.exists())
        {
            slide.image = infer_asset_from_text(
                &slide.title,
                slide.note.as_deref().unwrap_or(""),
                asset_paths,
            );
        }
    }
    if slide.image.is_none() && !slide.images.is_empty() {
        slide.images.retain(|img| {
            let path = std::path::Path::new(img);
            let is_media_ref = img
                .strip_prefix("media:")
                .and_then(|id| id.parse::<i64>().ok())
                .is_some();
            asset_paths.contains(img) || is_media_ref || (path.is_absolute() && path.exists())
        });
    }
    if slide.kind == SlideKind::Spotlight && slide.image.is_none() && slide.images.is_empty() {
        slide
            .placeholder
            .get_or_insert_with(|| "图片待补充".to_string());
    }
}

pub fn apply_component_defaults(slide: &mut SlideBlueprint) {
    let cols_multiplier = match slide.aspect_ratio {
        Some(AspectRatio::Ratio16x9) => 1,
        Some(AspectRatio::Ratio32x9) => 2,  // 4 * 2 = 8 cols max
        Some(AspectRatio::Ratio48x9) => 3,  // 4 * 3 = 12 cols max
        None => 1,
    };

    match slide.kind {
        SlideKind::SectionIntro => {
            slide.cards.truncate(4 * cols_multiplier);
            while slide.cards.len() < 2 {
                let idx = slide.cards.len() + 1;
                slide.cards.push(GridCard {
                    title: format!("子主题{}", idx),
                    tone: Some(match idx {
                        1 => "green".to_string(),
                        2 => "blue".to_string(),
                        3 => "amber".to_string(),
                        _ => "red".to_string(),
                    }),
                    icon: Some(match idx {
                        1 => "i-carbon:roadmap".to_string(),
                        2 => "i-carbon:idea".to_string(),
                        3 => "i-carbon:workflow-automation".to_string(),
                        _ => "i-carbon:arrow-right".to_string(),
                    }),
                    subtitle: None,
                    body: Some("待补充".to_string()),
                    items: Vec::new(),
                    conclusion: None,
                    footer_tag: None,
                    footer_tone: None,
                    top_bar_class: None,
                    risk: None,
                    tag: None,
                });
            }
            if slide
                .badge
                .as_ref()
                .is_none_or(|badge| badge.trim().is_empty())
            {
                slide.badge = Some("章节导览".to_string());
            }
            for (idx, card) in slide.cards.iter_mut().enumerate() {
                if card.body.as_ref().is_none_or(|body| body.trim().is_empty())
                    && card.items.is_empty()
                {
                    card.body = Some("待补充".to_string());
                }
                card.tone.get_or_insert_with(|| match idx {
                    0 => "green".to_string(),
                    1 => "blue".to_string(),
                    2 => "amber".to_string(),
                    _ => "red".to_string(),
                });
                card.icon.get_or_insert_with(|| match idx {
                    0 => "i-carbon:roadmap".to_string(),
                    1 => "i-carbon:idea".to_string(),
                    2 => "i-carbon:workflow-automation".to_string(),
                    _ => "i-carbon:arrow-right".to_string(),
                });
            }
        }
        SlideKind::FeatureGrid => {
            slide.cards.truncate(4 * cols_multiplier);
            if slide.cards.len() < 2 {
                slide.cards.push(GridCard {
                    title: "补充要点".to_string(),
                    tone: Some("blue".to_string()),
                    icon: Some("i-carbon:idea".to_string()),
                    subtitle: None,
                    body: None,
                    items: vec!["待补充".to_string()],
                    conclusion: None,
                    footer_tag: None,
                    footer_tone: Some("blue".to_string()),
                    top_bar_class: None,
                    risk: None,
                    tag: None,
                });
            }
        }
        SlideKind::IssueStack => {
            slide.cards.truncate(4);
            while slide.cards.len() < 2 {
                let idx = slide.cards.len() + 1;
                slide.cards.push(GridCard {
                    title: format!("问题{}", idx),
                    tone: Some(match idx {
                        1 => "blue".to_string(),
                        2 => "green".to_string(),
                        3 => "amber".to_string(),
                        _ => "red".to_string(),
                    }),
                    icon: Some(match idx {
                        1 => "i-carbon:warning-alt".to_string(),
                        2 => "i-carbon:warning-other".to_string(),
                        3 => "i-carbon:data-error".to_string(),
                        _ => "i-carbon:warning".to_string(),
                    }),
                    subtitle: None,
                    body: Some("待补充".to_string()),
                    items: vec!["待补充".to_string()],
                    conclusion: None,
                    footer_tag: None,
                    footer_tone: None,
                    top_bar_class: None,
                    risk: None,
                    tag: None,
                });
            }
            for (idx, card) in slide.cards.iter_mut().enumerate() {
                card.tone.get_or_insert_with(|| match idx {
                    0 => "blue".to_string(),
                    1 => "green".to_string(),
                    2 => "amber".to_string(),
                    _ => "red".to_string(),
                });
                card.icon.get_or_insert_with(|| match idx {
                    0 => "i-carbon:warning-alt".to_string(),
                    1 => "i-carbon:warning-other".to_string(),
                    2 => "i-carbon:data-error".to_string(),
                    _ => "i-carbon:warning".to_string(),
                });
                if card.body.as_ref().is_none_or(|body| body.trim().is_empty()) {
                    card.body = Some("待补充".to_string());
                }
                card.items.retain(|item| !item.trim().is_empty());
                if card.items.is_empty() {
                    card.items.push("待补充".to_string());
                }
                card.items.truncate(4);
            }
        }
        SlideKind::Spotlight => {
            slide.panels.truncate(3);
            if slide.panels.is_empty() {
                slide.panels.push(SpotlightPanel {
                    title: "核心说明".to_string(),
                    kind: None,
                    icon: Some("i-carbon:idea".to_string()),
                    tone: Some("blue".to_string()),
                    body: Some("待补充".to_string()),
                    items: Vec::new(),
                    steps: Vec::new(),
                    highlight: None,
                });
            }
        }
        SlideKind::SectionList => {
            slide.list_items.truncate(4);
            fill_empty_section_list_bodies(&mut slide.list_items);
            strengthen_section_list_items(&mut slide.list_items);
            if slide.list_items.len() < 2 {
                slide.list_items.push(ListItem {
                    step: Some("2".to_string()),
                    icon: Some("i-carbon:list-checked".to_string()),
                    title: "补充项".to_string(),
                    body: "待补充".to_string(),
                });
            }
        }
        SlideKind::SplitLayers => {
            strengthen_split_layers(slide);
        }
        SlideKind::FocusExample => {
            if slide.points.is_empty() {
                slide.points.push(ListItem {
                    step: None,
                    icon: Some("i-carbon:idea".to_string()),
                    title: "核心关注点".to_string(),
                    body: "待补充".to_string(),
                });
            }
            if slide
                .example_title
                .as_ref()
                .is_none_or(|title| title.trim().is_empty())
            {
                slide.example_title = Some("示例".to_string());
            }
            if slide
                .example_body
                .as_ref()
                .is_none_or(|body| body.trim().is_empty())
            {
                slide.example_body = Some("待补充".to_string());
            }
            if slide.ranking.is_empty() {
                slide.ranking.push(RankingItem {
                    index: "1.".to_string(),
                    label: "优先处理".to_string(),
                    meta: "待补充".to_string(),
                    muted: false,
                });
            }
        }
        SlideKind::CenterGrid => {
            slide.center_items.truncate(4 * cols_multiplier);
            if slide.center_items.is_empty() {
                slide.center_items.push(CenterItem {
                    title: "关键结论".to_string(),
                    desc: "待补充".to_string(),
                    icon: Some("i-carbon:idea".to_string()),
                    tone: Some("blue".to_string()),
                });
            }
        }
        SlideKind::OutcomeGrid => {
            slide.cards.truncate(4 * cols_multiplier);
            if slide.cards.is_empty() {
                slide.cards.push(GridCard {
                    title: "补充成果".to_string(),
                    tone: Some("blue".to_string()),
                    icon: Some("i-carbon:checkmark-filled".to_string()),
                    subtitle: None,
                    body: Some("待补充".to_string()),
                    items: Vec::new(),
                    conclusion: None,
                    footer_tag: None,
                    footer_tone: None,
                    top_bar_class: Some("bg-gradient-to-r from-blue-500 to-blue-700".to_string()),
                    risk: None,
                    tag: Some("成果".to_string()),
                });
            }
            for (idx, card) in slide.cards.iter_mut().enumerate() {
                card.body.get_or_insert_with(|| "待补充".to_string());
                card.icon
                    .get_or_insert_with(|| "i-carbon:checkmark-filled".to_string());
                card.tag.get_or_insert_with(|| "成果".to_string());
                card.top_bar_class.get_or_insert_with(|| match idx {
                    0 => "bg-gradient-to-r from-amber-500 to-amber-700".to_string(),
                    1 => "bg-gradient-to-r from-blue-500 to-blue-700".to_string(),
                    _ => "bg-gradient-to-r from-green-500 to-green-700".to_string(),
                });
            }
        }
        SlideKind::Timeline => {
            slide.timeline_events.truncate(6);
            let tones = ["amber", "blue", "green", "red", "amber", "blue"];
            for (idx, event) in slide.timeline_events.iter_mut().enumerate() {
                event
                    .tone
                    .get_or_insert_with(|| tones[idx % tones.len()].to_string());
                event
                    .icon
                    .get_or_insert_with(|| "i-carbon:time".to_string());
            }
            if slide.timeline_events.len() < 3 {
                slide.timeline_events.push(TimelineEvent {
                    date: "待补充".to_string(),
                    title: "补充事件".to_string(),
                    body: "待补充".to_string(),
                    tone: Some("blue".to_string()),
                    icon: Some("i-carbon:time".to_string()),
                });
            }
        }
        SlideKind::Swot => {
            let defaults = [
                ("strengths", "优势", "green", "i-carbon:thumbs-up"),
                ("weaknesses", "劣势", "red", "i-carbon:warning-alt"),
                ("opportunities", "机会", "blue", "i-carbon:growth"),
                ("threats", "威胁", "amber", "i-carbon:security"),
            ];
            let swot = slide.swot_data.get_or_insert_with(|| SwotData {
                quadrants: Vec::new(),
                strategy: Some("利用优势抓住机会，同时补强短板并对冲外部威胁。".to_string()),
            });
            swot.quadrants.truncate(4);
            while swot.quadrants.len() < 4 {
                let (key, title, tone, icon) = defaults[swot.quadrants.len()];
                swot.quadrants.push(SwotQuadrant {
                    key: key.to_string(),
                    title: title.to_string(),
                    tone: Some(tone.to_string()),
                    icon: Some(icon.to_string()),
                    items: vec!["待补充".to_string()],
                    summary: Some("待补充".to_string()),
                });
            }
            for (idx, quadrant) in swot.quadrants.iter_mut().enumerate() {
                let (key, title, tone, icon) = defaults[idx];
                if quadrant.key.trim().is_empty() {
                    quadrant.key = key.to_string();
                }
                if quadrant.title.trim().is_empty() {
                    quadrant.title = title.to_string();
                }
                quadrant.tone.get_or_insert_with(|| tone.to_string());
                quadrant.icon.get_or_insert_with(|| icon.to_string());
                quadrant.items.retain(|item| !item.trim().is_empty());
                if quadrant.items.is_empty() {
                    quadrant.items.push("待补充".to_string());
                }
                quadrant.items.truncate(4);
                if quadrant
                    .summary
                    .as_ref()
                    .is_none_or(|s| s.trim().is_empty())
                {
                    quadrant.summary = Some("待补充".to_string());
                }
            }
            if swot.strategy.as_ref().is_none_or(|s| s.trim().is_empty()) {
                swot.strategy = Some("利用优势抓住机会，同时补强短板并对冲外部威胁。".to_string());
            }
        }
        SlideKind::StepFlow => {
            slide.steps.truncate(5);
            while slide.steps.len() < 2 {
                let idx = slide.steps.len() + 1;
                slide.steps.push(StepItem {
                    title: format!("步骤{}", idx),
                    body: Some("待补充".to_string()),
                    icon: Some("i-carbon:arrow-right".to_string()),
                    tone: Some(if idx % 2 == 0 { "green" } else { "blue" }.to_string()),
                });
            }
            for step in &mut slide.steps {
                step.icon
                    .get_or_insert_with(|| "i-carbon:arrow-right".to_string());
                step.tone.get_or_insert_with(|| "blue".to_string());
            }
        }
        SlideKind::Process => {
            slide.phases.truncate(4);
            if slide.phases.is_empty() {
                slide.phases.push(PhaseItem {
                    phase: "阶段1".to_string(),
                    title: "补充流程".to_string(),
                    icon: Some("i-carbon:idea".to_string()),
                    tone: Some("blue".to_string()),
                    steps: vec![PhaseStep {
                        label: "补充动作".to_string(),
                        desc: Some("待补充".to_string()),
                    }],
                    highlight: Some("待补充".to_string()),
                });
            }
            for (idx, phase) in slide.phases.iter_mut().enumerate() {
                if phase.phase.trim().is_empty() {
                    phase.phase = format!("阶段{}", idx + 1);
                }
                phase.icon
                    .get_or_insert_with(|| "i-carbon:idea".to_string());
                phase.tone.get_or_insert_with(|| match idx {
                    0 => "amber".to_string(),
                    1 => "blue".to_string(),
                    2 => "green".to_string(),
                    _ => "red".to_string(),
                });
                if phase.steps.is_empty() {
                    phase.steps.push(PhaseStep {
                        label: "补充动作".to_string(),
                        desc: Some("待补充".to_string()),
                    });
                }
            }
        }
        SlideKind::Compare => {
            let compare = slide.compare_data.get_or_insert_with(|| CompareData {
                mode: Some("side-by-side".to_string()),
                left: CompareSide {
                    title: "左侧".to_string(),
                    tone: Some("blue".to_string()),
                    icon: Some("i-carbon:checkmark".to_string()),
                    items: vec![CompareItem {
                        label: "待补充".to_string(),
                        desc: Some("待补充".to_string()),
                        highlight: false,
                    }],
                    conclusion: None,
                },
                right: CompareSide {
                    title: "右侧".to_string(),
                    tone: Some("amber".to_string()),
                    icon: Some("i-carbon:close".to_string()),
                    items: vec![CompareItem {
                        label: "待补充".to_string(),
                        desc: Some("待补充".to_string()),
                        highlight: false,
                    }],
                    conclusion: None,
                },
            });
            compare
                .mode
                .get_or_insert_with(|| "side-by-side".to_string());
            compare.left.tone.get_or_insert_with(|| "blue".to_string());
            compare.right.tone.get_or_insert_with(|| "amber".to_string());
            if compare.left.items.is_empty() {
                compare.left.items.push(CompareItem {
                    label: "待补充".to_string(),
                    desc: Some("待补充".to_string()),
                    highlight: false,
                });
            }
            if compare.right.items.is_empty() {
                compare.right.items.push(CompareItem {
                    label: "待补充".to_string(),
                    desc: Some("待补充".to_string()),
                    highlight: false,
                });
            }
        }
        SlideKind::Infographic => {
            slide.infographic_syntax.get_or_insert_with(|| {
                "infographic list-grid-3-col\ndata\n  lists\n    - label 要点1\n      desc 描述1\n    - label 要点2\n      desc 描述2\n    - label 要点3\n      desc 描述3".to_string()
            });
        }
        _ => {}
    }
}

pub fn strengthen_section_list_items(items: &mut [ListItem]) {
    for item in items {
        let title = item.title.trim();
        let body = item.body.trim();
        let short_body = body.chars().count() <= 8;
        let no_sentence_end = !body.contains('。') && !body.contains('；') && !body.contains('，');
        if short_body && no_sentence_end {
            item.body = format!("围绕{}，完成{}。", title, body);
        } else if !body.ends_with('。') && !body.ends_with('！') && !body.ends_with('？') {
            item.body.push('。');
        }
    }
}

pub fn fill_empty_section_list_bodies(items: &mut [ListItem]) {
    for item in items {
        if item.body.trim().is_empty() {
            item.body = derive_body_from_title(&item.title);
        }
    }
}

pub fn derive_body_from_title(title: &str) -> String {
    let compact = clean_model_text(title);
    if compact.contains('，') {
        let mut parts = compact.splitn(2, '，');
        let head = parts.next().unwrap_or("").trim();
        let tail = parts.next().unwrap_or("").trim();
        if !head.is_empty() && !tail.is_empty() {
            return format!("围绕{}，重点在于{}。", head, tail);
        }
    }
    if compact.contains('、') && compact.chars().count() > 14 {
        return format!("重点覆盖{}。", compact);
    }
    format!("围绕{}展开具体分析。", compact)
}

pub fn strengthen_split_layers(slide: &mut SlideBlueprint) {
    if slide.left_items.len() > 3 {
        slide.left_items.truncate(3);
    }
    if slide.left_items.is_empty() {
        slide.left_items.push(ListItem {
            step: None,
            icon: Some("i-carbon:api".to_string()),
            title: "关键能力".to_string(),
            body: "待补充".to_string(),
        });
    }
    for item in &mut slide.left_items {
        item.icon.get_or_insert_with(|| "i-carbon:api".to_string());
        if item.body.trim().is_empty() {
            item.body = derive_body_from_title(&item.title);
        } else if !item.body.ends_with('。')
            && !item.body.ends_with('！')
            && !item.body.ends_with('？')
        {
            item.body.push('。');
        }
    }

    if slide.layers.len() >= 3 {
        return;
    }

    let inferred = infer_layers_from_split_slide(slide);
    if !inferred.is_empty() {
        slide.layers = inferred;
    }
}

pub fn infer_layers_from_split_slide(slide: &SlideBlueprint) -> Vec<LayerItem> {
    // Derive layer items from left_items when layers are missing.
    // Each left_item becomes one layer; use the item title as layer title
    // and truncate the body to fit the meta field limit (36 chars).
    let tones = ["amber", "blue", "green", "red"];
    slide
        .left_items
        .iter()
        .enumerate()
        .map(|(idx, item)| {
            use crate::generator::utils::summarize;
            LayerItem {
                title: summarize(&item.title, 16),
                meta: summarize(&item.body, 36),
                tone: Some(tones[idx % tones.len()].to_string()),
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Per-page normalize + repair (for the concurrent per-page pipeline)
// ---------------------------------------------------------------------------

/// Normalize a single blueprint (CPU-only passes + icon fix).
pub async fn normalize_one_blueprint(
    slide: &mut SlideBlueprint,
    client: &LmStudioClient,
    embedding_model: &str,
    icon_index: &IconIndex,
    asset_paths: &HashSet<String>,
    aspect_ratio: AspectRatio,
) -> Result<()> {
    slide.aspect_ratio = Some(aspect_ratio);
    normalize_lengths(slide);
    normalize_tones(slide);
    repair_assets(slide, asset_paths);
    apply_component_defaults(slide);
    // Fix invalid icons for this single slide
    crate::generator::icons::fix_invalid_icons(
        std::slice::from_mut(slide),
        client,
        embedding_model,
        icon_index,
    )
    .await?;
    Ok(())
}

/// Validate and repair a single slide until it passes or all rounds are exhausted.
pub async fn repair_one_slide(
    client: &LmStudioClient,
    config: &GenerationConfig,
    doc: &ParsedDocument,
    page_plan: &PagePlan,
    layout_plan: &LayoutPlan,
    mut slide: SlideBlueprint,
    icon_index: &IconIndex,
    asset_paths: &HashSet<String>,
    debug_dir: &Path,
    slide_idx: usize,
) -> Result<SlideBlueprint> {
    use crate::generator::utils::blueprint_schema_hint;

    let max_rounds = config.repair_rounds.max(3);

    for round in 0..max_rounds {
        let issues = validate_blueprints(
            std::slice::from_ref(&slide),
            icon_index,
            asset_paths,
        );
        if issues.is_empty() {
            return Ok(slide);
        }

        let is_last_round = round == max_rounds - 1;

        if is_last_round {
            // Final round: switch kind deterministically and regenerate
            use crate::generator::planning::pick_different_kind;
            let new_kind = pick_different_kind(page_plan, &slide.kind);
            eprintln!(
                "repair_one_slide: slide {} switching {:?} → {:?}",
                slide_idx + 1, slide.kind, new_kind
            );

            let mut new_layout = layout_plan.clone();
            new_layout.kind = new_kind.clone();
            let extra_context = format!(
                "Previous attempt used component kind '{:?}' and failed validation:\n{}\nDo NOT use '{:?}' again.",
                slide.kind,
                issues.iter().map(|i| i.message.as_str()).collect::<Vec<_>>().join("\n"),
                slide.kind,
            );

            let prev_kind_str = format!("{:?}", slide.kind);
            let error_msgs: Vec<String> = issues.iter().map(|i| i.message.clone()).collect();
            let mut slides_vec = vec![slide.clone()];
            match crate::generator::slides::regenerate_slides_at(
                client,
                config,
                doc,
                std::slice::from_ref(page_plan),
                std::slice::from_ref(&new_layout),
                &mut slides_vec,
                &[(0, prev_kind_str, error_msgs)],
                icon_index,
                asset_paths,
                debug_dir,
                round,
            )
            .await
            {
                Ok(()) => {
                    slide = slides_vec.into_iter().next().unwrap_or_else(|| make_fallback_slide(page_plan, Some(layout_plan), config.aspect_ratio));
                }
                Err(e) => eprintln!("repair_one_slide regen failed for slide {}: {e}", slide_idx + 1),
            }
        } else {
            // Standard LLM repair round
            let schema = blueprint_schema_hint(&slide.kind);
            let failing_items = vec![serde_json::json!({
                "slide_index": 1,
                "page_plan": page_plan,
                "layout_plan": layout_plan,
                "page_id": page_plan.page_id,
                "current_slide": slide.clone(),
                "component_schema": schema,
            })];

            let system = "You repair an invalid Slidev slide blueprint. Fix only what is necessary to satisfy validation while preserving the content intent. Return strict JSON. IMPORTANT: all icon fields must use the i-carbon: prefix (e.g. \"i-carbon:chart-line\"). Never use i-mdi:, i-fa:, or any other prefix.";
            let user = format!(
                "Document title: {}\n\nFailing slide with its plan and schema:\n{}\n\nValidation issues:\n{}\n\nReturn ONLY the corrected slide as JSON: {{\"slides\": [<corrected blueprint>]}}.",
                doc.title,
                serde_json::to_string_pretty(&failing_items).unwrap_or_default(),
                format_issue_block("issues", &issues),
            );
            let debug_prefix = format!("04-repair-slide{}-r{}", slide_idx + 1, round + 1);
            let _ = write_debug(debug_dir, &format!("{debug_prefix}.system.txt"), system);
            let _ = write_debug(debug_dir, &format!("{debug_prefix}.user.txt"), &user);

            match client.generate_text(&config.model, system, &user).await {
                Ok(raw) => {
                    let _ = write_debug(debug_dir, &format!("{debug_prefix}.raw.txt"), &raw);
                    if let Ok(resp) = crate::generator::utils::parse_json_with_extraction::<RepairResponse>(&raw) {
                        if let Some(repaired) = resp.slides.into_iter().next() {
                            slide = repaired;
                        }
                    }
                }
                Err(e) => eprintln!("repair_one_slide LLM failed for slide {}: {e}", slide_idx + 1),
            }
        }

        // Re-normalize after repair
        normalize_one_blueprint(&mut slide, client, &config.embedding_model, icon_index, asset_paths, config.aspect_ratio).await?;
    }

    // All rounds exhausted: try deterministic fallback
    let issues = validate_blueprints(std::slice::from_ref(&slide), icon_index, asset_paths);
    if !issues.is_empty() {
        eprintln!(
            "repair_one_slide: slide {} still invalid after {} rounds; using fallback",
            slide_idx + 1, max_rounds
        );
        slide = make_fallback_slide(page_plan, Some(layout_plan), config.aspect_ratio);
        apply_component_defaults(&mut slide);
        normalize_lengths(&mut slide);
        normalize_tones(&mut slide);
    }

    Ok(slide)
}
