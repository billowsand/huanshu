use crate::config::GenerationConfig;
use crate::icon::IconIndex;
use crate::input::ParsedDocument;
use crate::lmstudio::LmStudioClient;
use crate::types::*;
use anyhow::{Context, Result};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Semaphore;

use crate::generator::icons::{collect_icon_candidates, precompute_semantic_candidates};
use crate::generator::utils::{
    blueprint_schema_hint, parse_page_display_index, sanitize_filename, sorted_assets, write_debug,
};

/// Generate content slides and emit per-slide progress via a callback as each
/// slide completes (not after all slides finish).
pub async fn generate_content_slides_with_progress<F>(
    llm_client: &LmStudioClient,
    embedding_client: &LmStudioClient,
    config: &GenerationConfig,
    doc: &ParsedDocument,
    page_plans: &[PagePlan],
    layout_plans: &[LayoutPlan],
    icon_index: &IconIndex,
    asset_paths: &HashSet<String>,
    debug_dir: &Path,
    concurrency: usize,
    on_slide_ready: F,
) -> Result<Vec<SlideBlueprint>>
where
    F: Fn(usize, SlideBlueprint) + Send + 'static,
{
    let layout_by_id = layout_plans
        .iter()
        .map(|layout| (layout.page_id.clone(), layout))
        .collect::<BTreeMap<_, _>>();

    // Pre-compute semantically ranked icon candidates for all slides in one
    // batch embedding call, then fall back to lexical if the index is empty.
    let semantic_candidates: Vec<Vec<String>> = precompute_semantic_candidates(
        embedding_client,
        &config.embedding.model,
        icon_index,
        page_plans,
        layout_plans,
    )
    .await
    .unwrap_or_else(|_| vec![Vec::new(); page_plans.len()]);

    let sem = Arc::new(Semaphore::new(concurrency.max(1)));

    enum SlideSource {
        Cached(SlideBlueprint),
        Pending(tokio::task::JoinHandle<Result<SlideBlueprint>>),
    }

    let mut sources: Vec<SlideSource> = Vec::with_capacity(page_plans.len());

    for (idx, page) in page_plans.iter().enumerate() {
        let layout = layout_by_id
            .get(&page.page_id)
            .ok_or_else(|| anyhow::anyhow!("missing layout plan for page {}", page.page_id))?;
        let display_index = parse_page_display_index(&page.page_id).unwrap_or(idx + 1);
        let prefix = format!(
            "03-slide-{:02}-{}",
            display_index,
            sanitize_filename(&page.page_id)
        );
        let parsed_path = debug_dir.join(format!("{prefix}.parsed.json"));
        if parsed_path.is_file() {
            if let Ok(cached) =
                serde_json::from_str::<BlueprintWrapper>(&fs::read_to_string(&parsed_path)?)
            {
                sources.push(SlideSource::Cached(cached.slide));
                continue;
            }
        }

        // Use semantic candidates if available, otherwise fall back to lexical
        let candidates = {
            let sem_cands = semantic_candidates.get(idx).cloned().unwrap_or_default();
            if sem_cands.is_empty() {
                collect_icon_candidates(icon_index, page, layout)
            } else {
                sem_cands
            }
        };

        let client_owned = llm_client.clone();
        let model = config.llm.model.clone();
        let doc_title = doc.title.clone();
        let page_owned = page.clone();
        let layout_owned = (*layout).clone();
        let asset_list = sorted_assets(asset_paths);
        let schema_hint = blueprint_schema_hint(&layout.kind);
        let debug_dir_owned = debug_dir.to_path_buf();
        let permit = sem.clone().acquire_owned().await?;
        let handle = tokio::task::spawn(async move {
            let _permit = permit;
            generate_one_slide(
                client_owned, model, doc_title, page_owned, layout_owned,
                asset_list, candidates, schema_hint, debug_dir_owned, prefix, idx,
            )
            .await
        });
        sources.push(SlideSource::Pending(handle));
    }

    let mut slides = Vec::with_capacity(sources.len());
    for (idx, source) in sources.into_iter().enumerate() {
        let slide = match source {
            SlideSource::Cached(s) => s,
            SlideSource::Pending(handle) => handle.await??,
        };
        on_slide_ready(idx, slide.clone());
        slides.push(slide);
    }
    Ok(slides)
}

pub async fn generate_one_slide(
    client: LmStudioClient,
    model: String,
    doc_title: String,
    page: PagePlan,
    layout: LayoutPlan,
    asset_list: Vec<String>,
    candidates: Vec<String>,
    schema_hint: &'static str,
    debug_dir: PathBuf,
    prefix: String,
    slide_idx: usize,
) -> Result<SlideBlueprint> {
    generate_one_slide_inner(
        client, model, doc_title, page, layout, asset_list, candidates,
        schema_hint, debug_dir, prefix, slide_idx, String::new(),
    )
    .await
}

async fn generate_one_slide_inner(
    client: LmStudioClient,
    model: String,
    doc_title: String,
    page: PagePlan,
    layout: LayoutPlan,
    asset_list: Vec<String>,
    candidates: Vec<String>,
    schema_hint: &'static str,
    debug_dir: PathBuf,
    prefix: String,
    slide_idx: usize,
    extra_context: String,
) -> Result<SlideBlueprint> {
    let system = "You generate a single Slidev slide blueprint as strict JSON. Follow the chosen component kind exactly. Keep text compact and presentation-ready. Never invent image paths or icon names. Faithfully express the content using the assigned component kind; do not substitute or simplify it into a different layout.";
    let user = format!(
        "Document title: {}\n\nPage plan:\n{}\n\nLayout plan:\n{}\n\nAvailable assets:\n{}\n\nIcon candidates:\n{}\n\nSchema:\nReturn {{\"slide\": SlideBlueprint}}.\nComponent-specific schema:\n{}\nRules:\n- `slide.kind` must equal the chosen layout kind.\n- Follow the schema exactly; do not add extra top-level objects copied from page plan or layout plan.\n- Use only fields shown in the component-specific schema for this kind.\n- Never use unsupported fields such as `layout`, `content_shape`, `layout_intent`, `visual_need`, `object_count`, `argument_mode`, `density`, `section_label`, `description`, `content`, or `color`.\n- Use the same language as the source page. This document is Chinese, so all user-facing text must be Chinese.\n- Labels, panel titles, notes, and placeholders must also be Chinese when present.\n- section should be a short label like 01, 02, 建议 if useful.\n- Use only tones amber, blue, green, red.\n- Use only icon names from the candidates list.\n- preferred_assets should be used when relevant.\n- If an image is provided, placeholder may be omitted; do not output an empty placeholder string.\n- Fill all required fields for the assigned component kind as shown in the schema.\n- For section_intro: produce 2-4 concise preview cards for the upcoming sub-topics. This is a chapter-opening overview, not a closing summary.\n- For feature_grid: produce 2-4 balanced cards, each with its own title and concise body/items.\n- For section_list: produce 2-4 ordered list_items with step numbers.\n- For focus_example: produce 2-4 points plus a meaningful example_title and example_body.\n- For outcome_grid: produce 2-4 result cards with tag and top_bar_class.\n- For split_layers: produce both left_items and layers.\n- For split_layers: left_items must explain responsibilities, value, constraints, interfaces, or operating principles. Do not make them mere repeats of the layer names.\n- For split_layers: layers must describe the actual architecture stack or module structure on the right side, using wording that complements rather than duplicates left_items.\n- For split_layers: when the structure is clear enough, also fill `layers_infographic_syntax` with valid @antv/infographic syntax for a hierarchy, flow, or relationship diagram in Chinese. Even then, still provide `layers` as the semantic source of truth.\n- For step_flow: produce 2-5 steps, each with title, body, icon, and tone. Steps should represent a clear linear process.\n- For process: produce 2-4 phases, each with phase label, title, icon, tone, and 2-4 steps. Include a highlight conclusion for each phase.\n- For compare: produce left and right panels, each with title, tone, icon, items, and conclusion. Items should have label, desc, and highlight flag. Set mode to 'side-by-side', 'pros-cons', or 'versus'.\n- For issue_stack: produce 2-4 stacked problem cards. Each card needs title, tone, icon, one concise body sentence, and 2-4 short items capturing pains, obstacles, or failure modes.\n- For swot: produce exactly 4 quadrants in this order: strengths, weaknesses, opportunities, threats.\n- For swot: quadrant titles must be short Chinese labels suitable for in-card flags, for example '优势', '劣势', '机会', '威胁'. Do not include English in quadrant titles.\n- For swot: each quadrant needs tone, icon, 2-4 short items, and a one-line summary. Also provide a short strategy sentence that synthesizes the matrix.\n- For infographic: you MUST fill the infographic_syntax field with valid @antv/infographic custom syntax. The syntax starts with 'infographic <template-name>' followed by a 'data' section using YAML-like indentation. Choose an appropriate built-in template such as: list-grid-3-col, list-row-simple-horizontal-arrow, sequence-steps-badge-card, sequence-timeline-vertical, compare-binary-horizontal-simple-vs, hierarchy-tree-lr-tech-style-capsule-item, chart-column-simple, chart-pie-simple, or chart-word-cloud. All text in the syntax must be Chinese. The syntax must be a single string value (use \\n for newlines). Example: \"infographic list-row-simple-horizontal-arrow\\ndata\\n  lists\\n    - label 要点1\\n      desc 描述1\\n    - label 要点2\\n      desc 描述2\".\n- Do not collapse structured content into a plain list.\nReturn only JSON.",
        doc_title,
        serde_json::to_string_pretty(&page).unwrap_or_default(),
        serde_json::to_string_pretty(&layout).unwrap_or_default(),
        serde_json::to_string_pretty(&asset_list).unwrap_or_default(),
        serde_json::to_string_pretty(&candidates).unwrap_or_default(),
        schema_hint,
    );
    let user = if extra_context.is_empty() {
        user
    } else {
        format!("{user}\n\nIMPORTANT CONTEXT FOR THIS REGENERATION:\n{extra_context}")
    };
    write_debug(&debug_dir, &format!("{prefix}.system.txt"), system)?;
    write_debug(&debug_dir, &format!("{prefix}.user.txt"), &user)?;
    let raw = client
        .generate_text(&model, system, &user)
        .await
        .with_context(|| format!("failed generating slide {}", slide_idx + 1))?;
    write_debug(&debug_dir, &format!("{prefix}.raw.txt"), &raw)?;
    let resp = crate::generator::utils::parse_blueprint_with_repair(&raw, &layout.kind)
        .with_context(|| {
            format!(
                "failed to parse blueprint for page {} (kind={:?}); check {prefix}.raw.txt for the raw model output",
                page.page_id, layout.kind
            )
        })?;
    write_debug(
        &debug_dir,
        &format!("{prefix}.parsed.json"),
        &serde_json::to_string_pretty(&resp)?,
    )?;
    Ok(resp.slide)
}

/// Repair a single slide given free-form user feedback.
///
/// `icon_index` and `asset_paths` are optional — pass `None` when not available
/// (e.g. in tests) and the prompt will simply omit those sections.
pub async fn repair_single_slide(
    config: &GenerationConfig,
    blueprint: &SlideBlueprint,
    feedback: &str,
    icon_index: Option<&IconIndex>,
    asset_paths: Option<&HashSet<String>>,
) -> Result<SlideBlueprint> {
    use crate::generator::icons::collect_icon_candidates_from_slide;
    use crate::generator::utils::blueprint_schema_hint;

    let client = config.llm_client();
    let schema_hint = blueprint_schema_hint(&blueprint.kind);

    let icon_section = icon_index
        .map(|idx| {
            let candidates = collect_icon_candidates_from_slide(idx, blueprint);
            format!(
                "\n\nAvailable icon candidates (use ONLY these, prefix i-carbon:):\n{}",
                serde_json::to_string_pretty(&candidates).unwrap_or_default()
            )
        })
        .unwrap_or_default();

    let asset_section = asset_paths
        .map(|paths| {
            let mut sorted: Vec<&String> = paths.iter().collect();
            sorted.sort();
            format!(
                "\n\nAvailable image assets:\n{}",
                serde_json::to_string_pretty(&sorted).unwrap_or_default()
            )
        })
        .unwrap_or_default();

    let system = "You repair a single Slidev slide blueprint based on user feedback. Return strictly the corrected JSON object with a 'slide' wrapper. Preserve the slide kind and all valid content. Use only icon names from the provided candidates list (i-carbon: prefix only).";
    let user = format!(
        "Current slide blueprint:\n{}\n\nUser feedback:\n{}\n\nComponent schema:\n{}{}{}\n\nReturn {{\"slide\": <corrected blueprint>}}. Keep the same kind. Apply the feedback while preserving valid content. Return only JSON.",
        serde_json::to_string_pretty(blueprint).unwrap_or_default(),
        feedback,
        schema_hint,
        icon_section,
        asset_section,
    );

    let raw = client
        .generate_text(&config.llm.model, system, &user)
        .await
        .context("failed to repair slide")?;

    let resp = crate::generator::utils::parse_blueprint_with_repair(&raw, &blueprint.kind)
        .context("failed to parse repaired blueprint")?;

    Ok(resp.slide)
}

/// Re-generate specific slides from scratch using the full content-generation prompt.
/// `failing` is a list of `(slide_idx_0based, previous_kind_label, validation_errors)`.
/// The previous kind and errors are injected into the prompt so the LLM knows what to avoid.
pub async fn regenerate_slides_at(
    client: &LmStudioClient,
    config: &GenerationConfig,
    doc: &ParsedDocument,
    page_plans: &[PagePlan],
    layout_plans: &[LayoutPlan],
    slides: &mut Vec<SlideBlueprint>,
    failing: &[(usize, String, Vec<String>)],
    icon_index: &IconIndex,
    asset_paths: &HashSet<String>,
    debug_dir: &Path,
    round: usize,
) -> Result<()> {
    let layout_by_id: BTreeMap<&str, &LayoutPlan> = layout_plans
        .iter()
        .map(|l| (l.page_id.as_str(), l))
        .collect();

    for (idx, prev_kind, errors) in failing {
        let Some(page) = page_plans.get(*idx) else { continue };
        let Some(layout) = layout_by_id.get(page.page_id.as_str()) else { continue };
        let asset_list = sorted_assets(asset_paths);
        let candidates = collect_icon_candidates(icon_index, page, layout);
        let schema_hint = blueprint_schema_hint(&layout.kind);
        let prefix = format!(
            "03-slide-{:02}-{}-regen-r{}",
            parse_page_display_index(&page.page_id).unwrap_or(*idx + 1),
            sanitize_filename(&page.page_id),
            round + 1,
        );
        let extra_context = if prev_kind.is_empty() && errors.is_empty() {
            String::new()
        } else {
            format!(
                "Previous attempt used component kind '{}' and failed validation:\n{}\nDo NOT use '{}' again. Choose a different component kind that better fits the content.",
                prev_kind,
                errors.join("\n"),
                prev_kind,
            )
        };
        match generate_one_slide_inner(
            client.clone(),
            config.llm.model.clone(),
            doc.title.clone(),
            page.clone(),
            (*layout).clone(),
            asset_list,
            candidates,
            schema_hint,
            debug_dir.to_path_buf(),
            prefix,
            *idx,
            extra_context,
        )
        .await
        {
            Ok(new_slide) => {
                if let Some(slot) = slides.get_mut(*idx) {
                    *slot = new_slide;
                }
            }
            Err(e) => {
                eprintln!("regenerate_slides_at: slide {} failed: {e}", idx + 1);
                // Leave the current (broken) slide in place; it will be caught
                // by the next validation round or replaced by the fallback.
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Single-page pipeline (for the concurrent per-page architecture)
// ---------------------------------------------------------------------------

/// Per-page pipeline stage labels, emitted as events to the frontend.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PageStage {
    Pending,
    Planning,
    Layout,
    Content,
    Normalizing,
    Validating,
    Done,
    Error,
}

/// Run the full pipeline for a single page:
/// enrich → layout → content → normalize → validate + repair
///
/// This is the core function of the per-page concurrent architecture.
/// Each page runs independently; the `used_layouts` parameter carries
/// information about already-completed pages for layout diversity.
pub async fn generate_single_page_pipeline(
    llm_client: &LmStudioClient,
    embedding_client: &LmStudioClient,
    config: &GenerationConfig,
    doc: &ParsedDocument,
    base_page_plan: PagePlan,
    slide_index: usize,
    _total_slides: usize,
    used_layouts_shared: std::sync::Arc<tokio::sync::Mutex<Vec<(usize, String)>>>,
    icon_index: &IconIndex,
    asset_paths: &HashSet<String>,
    debug_dir: &Path,
    on_stage: std::sync::Arc<dyn Fn(usize, PageStage, Option<&str>) + Send + Sync>,
) -> Result<(SlideBlueprint, LayoutPlan)> {
    let page_id = base_page_plan.page_id.clone();

    // Stage 1: Enrich page plan
    on_stage(slide_index, PageStage::Planning, Some("丰富页面规划"));
    let page_plan = crate::generator::planning::enrich_one_page_plan(
        llm_client,
        config,
        doc,
        &base_page_plan,
        asset_paths,
        debug_dir,
    )
    .await;

    // Stage 2: Layout selection
    on_stage(slide_index, PageStage::Layout, Some("选择布局模板"));
    // Snapshot layouts right before selection (not at task spawn time)
    let layouts_snapshot = used_layouts_shared.lock().await.clone();
    let layout_plan = crate::generator::planning::layout_one_page(
        llm_client,
        config,
        &page_plan,
        &layouts_snapshot,
        debug_dir,
    )
    .await;
    // Register the chosen layout immediately for diversity tracking
    {
        let mut layouts = used_layouts_shared.lock().await;
        layouts.push((slide_index, format!("{:?}", layout_plan.kind)));
    }

    // Stage 3: Content generation
    on_stage(
        slide_index,
        PageStage::Content,
        Some("生成页面内容"),
    );

    let display_index =
        crate::generator::utils::parse_page_display_index(&page_id).unwrap_or(slide_index + 1);
    let prefix = format!(
        "03-slide-{:02}-{}",
        display_index,
        crate::generator::utils::sanitize_filename(&page_id)
    );

    let candidates = {
        let sem_cands = crate::generator::icons::precompute_semantic_candidates(
            embedding_client,
            &config.embedding.model,
            icon_index,
            std::slice::from_ref(&page_plan),
            std::slice::from_ref(&layout_plan),
        )
        .await
        .unwrap_or_else(|_| vec![Vec::new()]);
        let sem = sem_cands.into_iter().next().unwrap_or_default();
        if sem.is_empty() {
            crate::generator::icons::collect_icon_candidates(icon_index, &page_plan, &layout_plan)
        } else {
            sem
        }
    };

    let schema_hint = crate::generator::utils::blueprint_schema_hint(&layout_plan.kind);
    let asset_list = crate::generator::utils::sorted_assets(asset_paths);

    let mut slide = generate_one_slide(
        llm_client.clone(),
        config.llm.model.clone(),
        doc.title.clone(),
        page_plan.clone(),
        layout_plan.clone(),
        asset_list,
        candidates,
        schema_hint,
        debug_dir.to_path_buf(),
        prefix,
        slide_index,
    )
    .await?;

    // Stage 4: Normalize
    on_stage(slide_index, PageStage::Normalizing, Some("规范化修复"));
    crate::generator::normalize::normalize_one_blueprint(
        &mut slide,
        embedding_client,
        &config.embedding.model,
        icon_index,
        asset_paths,
        config.aspect_ratio,
    )
    .await?;

    // Stage 5: Validate + repair
    on_stage(slide_index, PageStage::Validating, Some("校验与修复"));
    let slide = crate::generator::normalize::repair_one_slide(
        llm_client,
        embedding_client,
        config,
        doc,
        &page_plan,
        &layout_plan,
        slide,
        icon_index,
        asset_paths,
        debug_dir,
        slide_index,
    )
    .await?;

    on_stage(slide_index, PageStage::Done, None);
    Ok((slide, layout_plan))
}
