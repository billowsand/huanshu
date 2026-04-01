use crate::generator::utils::cosine_similarity;
use crate::icon::{IconIndex, IconRecord};
use crate::lmstudio::LmStudioClient;
use crate::types::{LayoutPlan, PagePlan, SlideBlueprint};
use anyhow::{Result, bail};
use std::collections::HashMap;

const PROMPT_ICON_LIMIT: usize = 48;
const LEXICAL_RECALL: usize = 128; // wider lexical net before semantic re-ranking
const RECALL_PER_QUERY: usize = 48;

// ---------------------------------------------------------------------------
// Pre-generation: semantic candidate pre-selection
// ---------------------------------------------------------------------------
// One batch embedding call covers all slides. Returns a per-slide list of
// icon name strings (e.g. "i-carbon:chart-line") to feed into the LLM prompt.

pub async fn precompute_semantic_candidates(
    client: &LmStudioClient,
    embedding_model: &str,
    icon_index: &IconIndex,
    page_plans: &[PagePlan],
    layout_plans: &[LayoutPlan],
) -> Result<Vec<Vec<String>>> {
    if icon_index.is_empty() {
        return Ok(vec![Vec::new(); page_plans.len()]);
    }

    let layout_by_id: HashMap<&str, &LayoutPlan> = layout_plans
        .iter()
        .map(|l| (l.page_id.as_str(), l))
        .collect();

    // Build one combined query per slide
    let slide_queries: Vec<String> = page_plans
        .iter()
        .map(|page| {
            let mut parts = vec![
                page.page_title.clone(),
                page.section_title.clone(),
                page.objective.clone(),
            ];
            if let Some(sub) = &page.subsection_title { parts.push(sub.clone()); }
            if let Some(shape) = &page.content_shape  { parts.push(shape.clone()); }
            parts.extend(page.key_points.iter().cloned());
            if let Some(layout) = layout_by_id.get(page.page_id.as_str()) {
                parts.push(layout.title.clone());
                parts.push(layout.reason.clone());
            }
            parts.join(" ")
        })
        .collect();

    // Lexical pre-filter: collect unique candidates across all slides
    let mut unique_candidates: Vec<IconRecord> = Vec::new();
    let mut seen_names = std::collections::HashSet::new();
    for query in &slide_queries {
        for cand in icon_index.top_candidates(query, LEXICAL_RECALL) {
            if seen_names.insert(cand.full_name.clone()) {
                unique_candidates.push(cand);
            }
        }
    }

    if unique_candidates.is_empty() {
        return Ok(slide_queries.iter().map(|_| Vec::new()).collect());
    }

    // Single embedding call: [slide_queries... , candidate_texts...]
    let mut all_texts: Vec<String> = slide_queries.clone();
    let cand_offset = all_texts.len();
    all_texts.extend(unique_candidates.iter().map(|c| c.search_text.clone()));

    let embeddings = client.embed(embedding_model, &all_texts).await?;
    if embeddings.len() != all_texts.len() {
        bail!("embedding size mismatch in precompute_semantic_candidates");
    }

    let cand_vecs: Vec<&Vec<f32>> = embeddings[cand_offset..].iter().collect();

    // For each slide, rank candidates by cosine similarity to slide query
    let result = slide_queries
        .iter()
        .enumerate()
        .map(|(sidx, _)| {
            let q_vec = &embeddings[sidx];
            let mut scored: Vec<(f32, usize)> = cand_vecs
                .iter()
                .enumerate()
                .map(|(cidx, cv)| (cosine_similarity(q_vec, cv), cidx))
                .collect();
            scored.sort_by(|a, b| b.0.total_cmp(&a.0));
            scored
                .into_iter()
                .take(PROMPT_ICON_LIMIT)
                .map(|(_, cidx)| unique_candidates[cidx].full_name.clone())
                .collect()
        })
        .collect();

    Ok(result)
}

// ---------------------------------------------------------------------------
// Post-generation: targeted fix for only invalid icon names
// ---------------------------------------------------------------------------
// Checks each icon slot against icon_index. Only slots with an invalid or
// missing icon name get re-resolved using pre-computed icon embeddings.
// Valid icons are left untouched.

pub async fn fix_invalid_icons(
    slides: &mut [SlideBlueprint],
    client: &LmStudioClient,
    embedding_model: &str,
    icon_index: &IconIndex,
) -> Result<()> {
    if icon_index.is_empty() {
        for slide in slides.iter_mut() {
            fill_default_icons(slide);
        }
        return Ok(());
    }

    // Collect only invalid slots
    struct InvalidSlot {
        slide_idx: usize,
        field_key: String,
        query: String,
    }

    let mut invalid_slots: Vec<InvalidSlot> = Vec::new();

    for (sidx, slide) in slides.iter().enumerate() {
        let title = &slide.title;
        let mut check = |icon: Option<&str>, field_title: &str, key: String| {
            let is_valid = icon
                .filter(|n| !n.trim().is_empty())
                .map(|n| icon_index.contains(n))
                .unwrap_or(false);
            if !is_valid {
                let q = format!("{title} {field_title}");
                invalid_slots.push(InvalidSlot { slide_idx: sidx, field_key: key, query: q });
            }
        };

        for (i, c) in slide.cards.iter().enumerate() {
            check(c.icon.as_deref(), &c.title, format!("card:{i}"));
        }
        for (i, p) in slide.panels.iter().enumerate() {
            check(p.icon.as_deref(), &p.title, format!("panel:{i}"));
        }
        for (i, it) in slide.left_items.iter().enumerate() {
            check(it.icon.as_deref(), &it.title, format!("left:{i}"));
        }
        for (i, it) in slide.list_items.iter().enumerate() {
            check(it.icon.as_deref(), &it.title, format!("list:{i}"));
        }
        for (i, it) in slide.points.iter().enumerate() {
            check(it.icon.as_deref(), &it.title, format!("point:{i}"));
        }
        for (i, it) in slide.center_items.iter().enumerate() {
            check(it.icon.as_deref(), &it.title, format!("center:{i}"));
        }
        for (i, ev) in slide.timeline_events.iter().enumerate() {
            check(ev.icon.as_deref(), &ev.title, format!("timeline:{i}"));
        }
        if let Some(swot) = &slide.swot_data {
            for (i, q) in swot.quadrants.iter().enumerate() {
                check(q.icon.as_deref(), &q.title, format!("swot:{i}"));
            }
        }
    }

    if invalid_slots.is_empty() {
        return Ok(());
    }

    // Fast path: icon_index has pre-computed embeddings — use direct semantic search
    if icon_index.is_embedded() {
        let query_texts: Vec<String> = invalid_slots.iter().map(|s| s.query.clone()).collect();
        let embeddings = client.embed(embedding_model, &query_texts).await?;

        let resolved: Vec<String> = embeddings
            .iter()
            .map(|q_emb| {
                let results = icon_index.semantic_search_with_emb(q_emb, 1);
                results
                    .into_iter()
                    .next()
                    .map(|(_, r)| r.full_name.clone())
                    .unwrap_or_else(|| "i-carbon:circle-dash".to_string())
            })
            .collect();

        for (slot, icon) in invalid_slots.iter().zip(resolved.iter()) {
            write_icon_back(slides, slot.slide_idx, &slot.field_key, icon.clone());
        }
        return Ok(());
    }

    // Slow path: no pre-computed embeddings, fall back to lexical召回 + per-query embedding
    for slot in invalid_slots {
        let cands = icon_index.top_candidates(&format!("carbon {}", slot.query), RECALL_PER_QUERY);
        let chosen = if cands.is_empty() {
            "i-carbon:circle-dash".to_string()
        } else {
            let mut inputs = vec![slot.query.clone()];
            inputs.extend(cands.iter().map(|c| c.search_text.clone()));
            let embs = client.embed(embedding_model, &inputs).await?;
            if embs.len() != inputs.len() {
                "i-carbon:circle-dash".to_string()
            } else {
                let q_vec = &embs[0];
                let mut best: Option<(f32, &IconRecord)> = None;
                for (idx, cand) in cands.iter().enumerate() {
                    let score = cosine_similarity(q_vec, &embs[idx + 1]);
                    if best.map_or(true, |(s, _)| score > s) {
                        best = Some((score, cand));
                    }
                }
                best.map(|(_, r)| r.full_name.clone())
                    .unwrap_or_else(|| "i-carbon:circle-dash".to_string())
            }
        };
        write_icon_back(slides, slot.slide_idx, &slot.field_key, chosen);
    }

    Ok(())
}

fn write_icon_back(slides: &mut [SlideBlueprint], slide_idx: usize, field_key: &str, icon: String) {
    let slide = match slides.get_mut(slide_idx) {
        Some(s) => s,
        None => return,
    };
    let (kind, idx) = field_key.split_once(':').unwrap_or((field_key, ""));
    let idx: usize = idx.parse().unwrap_or(0);

    match kind {
        "card" => { if let Some(c) = slide.cards.get_mut(idx) { c.icon = Some(icon); } }
        "panel" => { if let Some(p) = slide.panels.get_mut(idx) { p.icon = Some(icon); } }
        "left" => { if let Some(it) = slide.left_items.get_mut(idx) { it.icon = Some(icon); } }
        "list" => { if let Some(it) = slide.list_items.get_mut(idx) { it.icon = Some(icon); } }
        "point" => { if let Some(it) = slide.points.get_mut(idx) { it.icon = Some(icon); } }
        "center" => { if let Some(it) = slide.center_items.get_mut(idx) { it.icon = Some(icon); } }
        "timeline" => { if let Some(ev) = slide.timeline_events.get_mut(idx) { ev.icon = Some(icon); } }
        "swot" => { if let Some(swot) = &mut slide.swot_data { if let Some(q) = swot.quadrants.get_mut(idx) { q.icon = Some(icon); } } }
        _ => {}
    }
}

/// Fill default icons when no icon index is available.
fn fill_default_icons(slide: &mut SlideBlueprint) {
    let default = "i-carbon:circle-dash";
    for card in &mut slide.cards { card.icon.get_or_insert_with(|| default.to_string()); }
    for panel in &mut slide.panels { panel.icon.get_or_insert_with(|| default.to_string()); }
    for item in slide.left_items.iter_mut().chain(slide.list_items.iter_mut()).chain(slide.points.iter_mut()) {
        item.icon.get_or_insert_with(|| default.to_string());
    }
    for item in &mut slide.center_items { item.icon.get_or_insert_with(|| default.to_string()); }
    if let Some(swot) = &mut slide.swot_data {
        for q in &mut swot.quadrants { q.icon.get_or_insert_with(|| default.to_string()); }
    }
}

/// Lexical fallback for icon candidate collection when embedding fails or is unavailable.
/// Used as fallback in generate_content_slides_with_progress.
pub fn collect_icon_candidates(
    icon_index: &IconIndex,
    page: &PagePlan,
    layout: &LayoutPlan,
) -> Vec<String> {
    let mut queries = Vec::new();
    queries.push(page.page_title.clone());
    queries.push(page.objective.clone());
    queries.push(layout.title.clone());
    queries.push(layout.reason.clone());
    if let Some(subtitle) = &layout.subtitle {
        queries.push(subtitle.clone());
    }
    if let Some(section_title) = &page.subsection_title {
        queries.push(section_title.clone());
    }
    queries.push(page.section_title.clone());
    if let Some(content_shape) = &page.content_shape {
        queries.push(content_shape.clone());
    }
    if let Some(argument_mode) = &page.argument_mode {
        queries.push(argument_mode.clone());
    }
    queries.extend(page.key_points.iter().cloned());

    let mut items = Vec::new();
    for query in queries {
        for icon in icon_index.top_candidates(&query, RECALL_PER_QUERY) {
            items.push(icon.full_name.clone());
        }
    }
    items.sort();
    items.dedup();
    if items.len() > PROMPT_ICON_LIMIT {
        items.truncate(PROMPT_ICON_LIMIT);
    }
    items
}

/// Build icon candidates for a single slide during user-feedback repair.
/// Uses the slide title, note, and card/panel/step titles as lexical queries.
pub fn collect_icon_candidates_from_slide(
    icon_index: &IconIndex,
    slide: &SlideBlueprint,
) -> Vec<String> {
    let mut queries = vec![slide.title.clone()];
    if let Some(note) = &slide.note {
        queries.push(note.clone());
    }
    if let Some(subtitle) = &slide.subtitle {
        queries.push(subtitle.clone());
    }
    for card in &slide.cards {
        queries.push(card.title.clone());
    }
    for panel in &slide.panels {
        queries.push(panel.title.clone());
    }
    for item in slide.list_items.iter().chain(slide.points.iter()).chain(slide.left_items.iter()) {
        queries.push(item.title.clone());
    }
    for step in &slide.steps {
        queries.push(step.title.clone());
    }
    for phase in &slide.phases {
        queries.push(phase.title.clone());
    }

    let mut items = Vec::new();
    for query in queries {
        for icon in icon_index.top_candidates(&query, RECALL_PER_QUERY) {
            items.push(icon.full_name.clone());
        }
    }
    items.sort();
    items.dedup();
    if items.len() > PROMPT_ICON_LIMIT {
        items.truncate(PROMPT_ICON_LIMIT);
    }
    items
}
