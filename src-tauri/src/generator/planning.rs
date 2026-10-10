use crate::config::GenerationConfig;
use crate::input::{HeadingLevel, ParsedDocument, Section};
use crate::lmstudio::LmStudioClient;
use crate::types::*;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

use crate::generator::layout_rules::{
    ineligibility, is_eligible, prompt_rules, UNIVERSAL_CONTENT_KINDS,
};
use crate::generator::utils::{slide_kind_name, sorted_assets, write_debug};

// ---------------------------------------------------------------------------
// Public stage entry points (called by Tauri commands)
// ---------------------------------------------------------------------------

pub async fn ensure_models_ready(
    llm_client: &LmStudioClient,
    llm_model: &str,
    embedding_client: &LmStudioClient,
    embedding_model: &str,
) -> Result<()> {
    // Health check LLM model by attempting a simple generation
    let test_prompt = "简短回复 ok 即可";
    let result = llm_client
        .generate_text(llm_model, "你是一个简洁的助手", test_prompt)
        .await
        .with_context(|| format!("LLM model is not usable: {llm_model}"))?;
    if result.trim().is_empty() {
        bail!("LLM model returned empty response: {llm_model}");
    }

    // Health check embedding model with embed call
    let probe = vec!["icon embedding healthcheck".to_string()];
    let embeddings = embedding_client
        .embed(embedding_model, &probe)
        .await
        .with_context(|| {
            format!("embedding model is not usable via LM Studio: {embedding_model}")
        })?;
    if embeddings.len() != 1 || embeddings[0].is_empty() {
        bail!("embedding model returned invalid healthcheck output: {embedding_model}");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Internal generation functions
// ---------------------------------------------------------------------------

/// Deterministic page derivation from document structure.
/// No LLM call — page count is fully determined by heading hierarchy.
pub fn derive_page_plans(doc: &ParsedDocument, granularity: HeadingLevel) -> Vec<PagePlan> {
    let mut pages = Vec::new();
    let mut global_idx: usize = 1;

    match granularity {
        HeadingLevel::H2 => {
            for sec in &doc.sections {
                pages.push(build_content_page(
                    global_idx,
                    &sec.title,
                    None,
                    &sec.title,
                    &sec.paragraphs,
                ));
                global_idx += 1;
            }
        }
        HeadingLevel::H3 => {
            for sec in &doc.sections {
                if sec.subsections.is_empty() {
                    // H2 with no children → single content slide
                    pages.push(build_content_page(
                        global_idx,
                        &sec.title,
                        None,
                        &sec.title,
                        &sec.paragraphs,
                    ));
                    global_idx += 1;
                } else {
                    // H2 with H3 children: check whether the H2 itself has substantive intro text.
                    // If it does, emit an intro content slide (not a summary/TOC page).
                    // If it doesn't, emit a section_summary overview of the H3 children.
                    let intro_chars: usize = sec.paragraphs.iter().map(|p| p.chars().count()).sum();
                    if intro_chars > 50 {
                        pages.push(build_content_page(
                            global_idx,
                            &sec.title,
                            None,
                            &sec.title,
                            &sec.paragraphs,
                        ));
                    } else {
                        pages.push(build_summary_page(global_idx, sec));
                    }
                    global_idx += 1;
                    for sub in &sec.subsections {
                        pages.push(build_content_page(
                            global_idx,
                            &sec.title,
                            Some(&sub.title),
                            &sub.title,
                            &sub.paragraphs,
                        ));
                        global_idx += 1;
                    }
                }
            }
        }
    }

    pages
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PageSignalResponse {
    pages: Vec<PageSignalPatch>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PageSignalPatch {
    #[serde(default)]
    page_id: Option<String>,
    #[serde(default)]
    objective: Option<String>,
    #[serde(default)]
    key_points: Option<Vec<String>>,
    #[serde(default)]
    takeaway: Option<String>,
    #[serde(default)]
    content_shape: Option<String>,
    #[serde(default)]
    layout_intent: Option<String>,
    #[serde(default)]
    visual_need: Option<String>,
    #[serde(default)]
    object_count: Option<String>,
    #[serde(default)]
    argument_mode: Option<String>,
    #[serde(default)]
    density: Option<String>,
    #[serde(default)]
    preferred_assets: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OverviewSummaryResponse {
    items: Vec<OverviewSummaryItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OverviewSummaryItem {
    desc: String,
}

fn merge_page_signal_patch(
    base: &PagePlan,
    patch: &PageSignalPatch,
    asset_paths: &HashSet<String>,
) -> PagePlan {
    let patch_page_id = patch
        .page_id
        .as_deref()
        .unwrap_or(base.page_id.as_str())
        .trim();
    if patch_page_id != base.page_id {
        return base.clone();
    }

    let mut merged = base.clone();

    if let Some(objective) = non_empty_text(patch.objective.as_deref()) {
        merged.objective = objective.to_string();
    }

    if let Some(key_points) = normalize_key_points(patch.key_points.as_deref()) {
        merged.key_points = key_points;
    }

    merged.takeaway = non_empty_text(patch.takeaway.as_deref()).map(str::to_string);
    merged.content_shape = normalized_enum(
        patch.content_shape.as_deref(),
        &[
            "overview",
            "summary",
            "comparison",
            "architecture",
            "timeline",
            "workflow",
            "matrix",
        ],
    )
    .map(str::to_string)
    .or_else(|| merged.content_shape.clone());
    merged.layout_intent = non_empty_text(patch.layout_intent.as_deref()).map(str::to_string);
    merged.visual_need = normalized_enum(
        patch.visual_need.as_deref(),
        &["text_only", "image_optional", "image_required"],
    )
    .map(str::to_string)
    .or_else(|| merged.visual_need.clone());
    merged.object_count =
        normalized_enum(patch.object_count.as_deref(), &["single", "pair", "multi"])
            .map(str::to_string)
            .or_else(|| merged.object_count.clone());
    merged.argument_mode = normalized_enum(
        patch.argument_mode.as_deref(),
        &[
            "parallel",
            "sequential",
            "layered",
            "summary",
            "evidence",
            "causal",
            "warning",
        ],
    )
    .map(str::to_string)
    .or_else(|| merged.argument_mode.clone());
    merged.density = normalized_enum(patch.density.as_deref(), &["low", "medium", "high"])
        .map(str::to_string)
        .or_else(|| merged.density.clone());
    merged.preferred_assets =
        normalize_preferred_assets(patch.preferred_assets.as_deref(), asset_paths);

    if merged.key_points.is_empty() {
        merged.key_points = base.key_points.clone();
    }
    if merged.objective.trim().is_empty() {
        merged.objective = base.objective.clone();
    }

    merged
}

fn non_empty_text(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|v| !v.is_empty())
}

fn normalized_enum<'a>(value: Option<&str>, allowed: &'a [&'a str]) -> Option<&'a str> {
    let normalized = value.map(str::trim).filter(|v| !v.is_empty())?;
    allowed
        .iter()
        .copied()
        .find(|candidate| normalized.eq_ignore_ascii_case(candidate))
}

fn normalize_key_points(value: Option<&[String]>) -> Option<Vec<String>> {
    let points = value?
        .iter()
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .take(6)
        .map(str::to_string)
        .collect::<Vec<_>>();
    if points.is_empty() {
        None
    } else {
        Some(points)
    }
}

fn normalize_preferred_assets(
    value: Option<&[String]>,
    asset_paths: &HashSet<String>,
) -> Vec<String> {
    value
        .into_iter()
        .flat_map(|items| items.iter())
        .map(|item| item.trim())
        .filter(|item| !item.is_empty() && asset_paths.contains(*item))
        .take(2)
        .map(str::to_string)
        .collect()
}

/// Build a section_summary PagePlan for an H2 that has H3 children.
fn build_summary_page(idx: usize, sec: &Section) -> PagePlan {
    let subsection_titles: Vec<String> = sec.subsections.iter().map(|s| s.title.clone()).collect();
    let n = subsection_titles.len();
    PagePlan {
        page_id: idx.to_string(),
        section_title: sec.title.clone(),
        subsection_title: None,
        page_title: sec.title.clone(),
        objective: format!("概述「{}」的 {} 个子主题", sec.title, n),
        key_points: subsection_titles.clone(),
        takeaway: None,
        content_shape: Some("overview".to_string()),
        layout_intent: Some(
            "section overview showing each sub-topic as a card or list item".to_string(),
        ),
        visual_need: Some("text_only".to_string()),
        object_count: Some(object_count_label(n)),
        argument_mode: Some("parallel".to_string()),
        density: Some("low".to_string()),
        source_excerpt: if sec.paragraphs.is_empty() {
            None
        } else {
            Some(sec.paragraphs.join("\n\n"))
        },
        preferred_assets: Vec::new(),
        page_role: Some("section_summary".to_string()),
    }
}

/// Build a content PagePlan for a leaf section (H2 without children, or H3).
fn build_content_page(
    idx: usize,
    section_title: &str,
    subsection_title: Option<&str>,
    page_title: &str,
    paragraphs: &[String],
) -> PagePlan {
    let text = paragraphs.join("\n\n");
    let (content_shape, argument_mode, density, object_count) = infer_signals(paragraphs);
    PagePlan {
        page_id: idx.to_string(),
        section_title: section_title.to_string(),
        subsection_title: subsection_title.map(str::to_string),
        page_title: page_title.to_string(),
        objective: format!("阐述「{}」的核心内容", page_title),
        key_points: extract_key_points(paragraphs),
        takeaway: None,
        content_shape: Some(content_shape),
        layout_intent: None,
        visual_need: Some("text_only".to_string()),
        object_count: Some(object_count),
        argument_mode: Some(argument_mode),
        density: Some(density),
        source_excerpt: if text.is_empty() { None } else { Some(text) },
        preferred_assets: Vec::new(),
        page_role: Some("content".to_string()),
    }
}

/// Infer content_shape, argument_mode, density, object_count from paragraph text.
fn infer_signals(paragraphs: &[String]) -> (String, String, String, String) {
    let text = paragraphs.join("\n").to_lowercase();
    let word_count = text.split_whitespace().count();

    let content_shape = if text.contains("流程")
        || text.contains("步骤")
        || text.contains("阶段")
        || text.contains("workflow")
        || text.contains("process")
    {
        "workflow"
    } else if text.contains("对比")
        || text.contains("比较")
        || text.contains("vs ")
        || text.contains("versus")
        || text.contains("优劣")
    {
        "comparison"
    } else if text.contains("架构")
        || text.contains("层")
        || text.contains("architecture")
        || text.contains("layer")
    {
        "architecture"
    } else if text.contains("时间")
        || text.contains("历史")
        || text.contains("timeline")
        || text.contains("年代")
    {
        "timeline"
    } else {
        "summary"
    };

    let bullet_count = paragraphs
        .iter()
        .flat_map(|p| p.lines())
        .filter(|l| {
            let t = l.trim_start();
            t.starts_with("- ")
                || t.starts_with("* ")
                || t.starts_with("• ")
                || (t.len() > 2
                    && t.chars().next().map_or(false, |c| c.is_ascii_digit())
                    && t.chars().nth(1) == Some('.'))
        })
        .count();

    let argument_mode = if bullet_count >= 3 {
        "parallel"
    } else if content_shape == "workflow" || content_shape == "timeline" {
        "sequential"
    } else if content_shape == "architecture" {
        "layered"
    } else {
        "summary"
    };

    let density = if word_count > 200 {
        "high"
    } else if word_count > 60 {
        "medium"
    } else {
        "low"
    };

    let object_count = object_count_label(bullet_count.max(1));

    (
        content_shape.to_string(),
        argument_mode.to_string(),
        density.to_string(),
        object_count,
    )
}

/// Extract bullet items from paragraphs as key_points.
fn extract_key_points(paragraphs: &[String]) -> Vec<String> {
    let mut points: Vec<String> = paragraphs
        .iter()
        .flat_map(|p| p.lines())
        .filter_map(|line| {
            let t = line.trim_start();
            if t.starts_with("- ") || t.starts_with("* ") || t.starts_with("• ") {
                Some(t[2..].trim().to_string())
            } else if t.len() > 3
                && t.chars().next().map_or(false, |c| c.is_ascii_digit())
                && t.chars().nth(1) == Some('.')
            {
                Some(t[2..].trim().to_string())
            } else {
                None
            }
        })
        .filter(|s| !s.is_empty())
        .take(6)
        .collect();
    // If no bullet points, create one key_point from the first sentence of the first paragraph
    if points.is_empty() {
        if let Some(first_para) = paragraphs.first() {
            let sentence = first_para
                .split(['。', '.', '！', '!', '\n'])
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            if !sentence.is_empty() {
                points.push(sentence);
            }
        }
    }
    points
}

fn object_count_label(n: usize) -> String {
    match n {
        0 | 1 => "single",
        2 => "pair",
        _ => "multi",
    }
    .to_string()
}

pub fn safe_fallback_kind(page: &PagePlan) -> SlideKind {
    if page.page_role.as_deref() == Some("section_summary") {
        return SlideKind::SectionIntro;
    }
    let am = page.argument_mode.as_deref().unwrap_or("");
    let cs = page.content_shape.as_deref().unwrap_or("");
    let vn = page.visual_need.as_deref().unwrap_or("");
    let density = page.density.as_deref().unwrap_or("");
    let key_count = page.key_points.len();
    let title = page.page_title.as_str();

    // Text-only central thesis: specific Chinese page archetypes
    if vn == "text_only"
        && (title.contains("目标")
            || title.contains("思路")
            || title.contains("转变")
            || title.contains("方向")
            || title.contains("愿景"))
    {
        return if density == "low" || density == "medium" {
            SlideKind::CenterGrid
        } else {
            SlideKind::FocusExample
        };
    }

    if (am == "summary" || am == "evidence") && (2..=4).contains(&key_count) {
        return SlideKind::OutcomeGrid;
    }
    if (title.contains("问题")
        || title.contains("挑战")
        || title.contains("难点")
        || title.contains("障碍")
        || title.contains("风险"))
        && (2..=4).contains(&key_count)
    {
        return SlideKind::IssueStack;
    }
    if cs == "matrix" && key_count == 4 {
        return SlideKind::Swot;
    }
    if vn == "image_required" || vn == "image_optional" {
        if page.object_count.as_deref() == Some("single") {
            return SlideKind::Spotlight;
        }
    }
    if am == "summary" && density != "high" {
        return SlideKind::CenterGrid;
    }
    if cs == "timeline" || (am == "sequential" && cs == "timeline") {
        return SlideKind::Timeline;
    }
    // workflow content: use process/step_flow regardless of argument_mode
    if cs == "workflow" {
        return if key_count >= 5 {
            SlideKind::Process
        } else {
            SlideKind::StepFlow
        };
    }
    if am == "sequential" && cs == "architecture" {
        return SlideKind::SplitLayers;
    }
    if am == "sequential" {
        return if key_count <= 4 {
            SlideKind::StepFlow
        } else {
            SlideKind::SectionList
        };
    }
    SlideKind::FeatureGrid
}

/// Pick a non-FeatureGrid fallback for deck variety violations.
/// When the deck has too many FeatureGrids, we need guaranteed alternatives.
pub fn variety_fallback_kind(page: &PagePlan) -> SlideKind {
    let kind = safe_fallback_kind(page);
    if kind != SlideKind::FeatureGrid {
        return kind;
    }
    // safe_fallback_kind still returned FeatureGrid — force an alternative
    // based on whatever signals we have.
    let am = page.argument_mode.as_deref().unwrap_or("");
    let key_count = page.key_points.len();
    let density = page.density.as_deref().unwrap_or("");
    let is_summary_page = page.page_role.as_deref() == Some("section_summary");
    let title = page.page_title.as_str();

    if is_summary_page && (2..=4).contains(&key_count) {
        return SlideKind::OutcomeGrid;
    }
    if am == "parallel" && (2..=4).contains(&key_count) {
        return SlideKind::OutcomeGrid;
    }
    // Content-shape signals that safe_fallback_kind may have missed due to argument_mode mismatch.
    let cs = page.content_shape.as_deref().unwrap_or("");
    if cs == "comparison" {
        return SlideKind::Compare;
    }
    if title.contains("问题")
        || title.contains("挑战")
        || title.contains("难点")
        || title.contains("障碍")
        || title.contains("风险")
    {
        return SlideKind::IssueStack;
    }
    if cs == "architecture" {
        return SlideKind::SplitLayers;
    }
    if density == "low" || density == "medium" {
        return SlideKind::FocusExample;
    }
    SlideKind::SectionList
}

/// Signal-based preference order for deterministic fallbacks: the heuristic picks first,
/// then every content kind (universal kinds early) so an eligible kind is always reachable.
fn fallback_kind_order(page: &PagePlan) -> Vec<SlideKind> {
    let mut order = vec![safe_fallback_kind(page), variety_fallback_kind(page)];
    order.extend([
        SlideKind::SectionList,
        SlideKind::OutcomeGrid,
        SlideKind::FeatureGrid,
        SlideKind::StepFlow,
        SlideKind::CenterGrid,
        SlideKind::FocusExample,
        SlideKind::Spotlight,
        SlideKind::SplitLayers,
        SlideKind::Timeline,
        SlideKind::Process,
        SlideKind::Compare,
        SlideKind::IssueStack,
        SlideKind::Swot,
    ]);
    let mut deduped: Vec<SlideKind> = Vec::with_capacity(order.len());
    for kind in order {
        if !deduped.contains(&kind) {
            deduped.push(kind);
        }
    }
    deduped
}

/// Kinds this page may use, in fallback preference order. Never empty: section_summary
/// pages get `SectionIntro`, content pages always include the universal kinds.
pub fn eligible_fallback_kinds(page: &PagePlan, aspect: AspectRatio) -> Vec<SlideKind> {
    if let Some(kind) = forced_kind(page) {
        return vec![kind];
    }
    fallback_kind_order(page)
        .into_iter()
        .filter(|kind| is_eligible(kind, page, aspect))
        .collect()
}

/// Best deterministic kind for this page (used for fallback slides).
pub fn fallback_kind(page: &PagePlan, aspect: AspectRatio) -> SlideKind {
    eligible_fallback_kinds(page, aspect)
        .into_iter()
        .next()
        .unwrap_or(SlideKind::FeatureGrid)
}

/// Pick an eligible kind that differs from `current_kind`.
/// Used in the last-resort repair round to actually switch the component.
pub fn pick_different_kind(
    page: &PagePlan,
    current_kind: &SlideKind,
    aspect: AspectRatio,
) -> SlideKind {
    if let Some(kind) = forced_kind(page) {
        return kind;
    }
    eligible_fallback_kinds(page, aspect)
        .into_iter()
        .find(|kind| kind != current_kind)
        .unwrap_or_else(|| {
            UNIVERSAL_CONTENT_KINDS
                .iter()
                .find(|kind| *kind != current_kind)
                .cloned()
                .unwrap_or(SlideKind::FeatureGrid)
        })
}

pub async fn assemble_slides(
    client: &LmStudioClient,
    config: &GenerationConfig,
    doc: &ParsedDocument,
    page_plans: &[PagePlan],
    mut content_slides: Vec<SlideBlueprint>,
    model: &str,
) -> Vec<SlideBlueprint> {
    let mut slides = Vec::with_capacity(content_slides.len() + 3);
    slides.push(SlideBlueprint {
        kind: SlideKind::Cover,
        aspect_ratio: Some(config.aspect_ratio),
        section: None,
        title: doc.title.clone(),
        subtitle: Some(format!("由 {} 分阶段生成", model)),
        badge: None,
        accent: None,
        note: None,
        label: None,
        label_tone: None,
        image: None,
        images: Vec::new(),
        placeholder: None,
        side_width: None,
        badges: vec![model.to_string(), "幻述".to_string()],
        overview_items: Vec::new(),
        cards: Vec::new(),
        panels: Vec::new(),
        left_items: Vec::new(),
        layers: Vec::new(),
        list_items: Vec::new(),
        points: Vec::new(),
        ranking: Vec::new(),
        center_items: Vec::new(),
        footer: None,
        example_title: None,
        example_body: None,
        timeline_events: Vec::new(),
        steps: Vec::new(),
        phases: Vec::new(),
        direction: None,
        compare_data: None,
        swot_data: None,
    });
    slides.push(make_overview_slide(client, config, doc, page_plans).await);
    slides.append(&mut content_slides);
    slides.push(make_closing_slide(model, config.aspect_ratio));
    slides
}

pub async fn make_overview_slide(
    client: &LmStudioClient,
    config: &GenerationConfig,
    doc: &ParsedDocument,
    page_plans: &[PagePlan],
) -> SlideBlueprint {
    use crate::generator::utils::{clean_model_text, summarize};

    let section_descs = generate_overview_section_descs(client, config, doc, page_plans)
        .await
        .unwrap_or_else(|err| {
            eprintln!("overview summary generation failed, using fallback: {err}");
            doc.sections
                .iter()
                .map(|section| fallback_overview_section_desc(section, page_plans))
                .collect()
        });

    let overview_items = doc
        .sections
        .iter()
        .enumerate()
        .map(|(idx, section)| OverviewItem {
            number: format!("{:02}", idx + 1),
            title: summarize(&section.title, 18),
            desc: summarize(
                &clean_model_text(
                    section_descs
                        .get(idx)
                        .map(String::as_str)
                        .unwrap_or("待补充"),
                ),
                16,
            ),
        })
        .collect::<Vec<_>>();

    SlideBlueprint {
        kind: SlideKind::Overview,
        aspect_ratio: Some(config.aspect_ratio),
        section: Some("00".to_string()),
        title: "提纲总览".to_string(),
        subtitle: None,
        badge: None,
        accent: None,
        note: Some("先由模型拆成逐页内容，再由模型选择组件并生成最终幻灯片。".to_string()),
        label: None,
        label_tone: None,
        image: None,
        images: Vec::new(),
        placeholder: None,
        side_width: None,
        badges: Vec::new(),
        overview_items,
        cards: Vec::new(),
        panels: Vec::new(),
        left_items: Vec::new(),
        layers: Vec::new(),
        list_items: Vec::new(),
        points: Vec::new(),
        ranking: Vec::new(),
        center_items: Vec::new(),
        footer: None,
        example_title: None,
        example_body: None,
        timeline_events: Vec::new(),
        steps: Vec::new(),
        phases: Vec::new(),
        direction: None,
        compare_data: None,
        swot_data: None,
    }
}

pub fn make_closing_slide(model: &str, aspect_ratio: AspectRatio) -> SlideBlueprint {
    SlideBlueprint {
        kind: SlideKind::Closing,
        aspect_ratio: Some(aspect_ratio),
        section: None,
        title: "感谢聆听".to_string(),
        subtitle: Some("敬请批评指正".to_string()),
        badge: None,
        accent: None,
        note: Some("欢迎交流讨论，也可继续提出修改建议。".to_string()),
        label: None,
        label_tone: None,
        image: None,
        images: Vec::new(),
        placeholder: None,
        side_width: None,
        badges: vec!["Q&A".to_string(), model.to_string()],
        overview_items: Vec::new(),
        cards: Vec::new(),
        panels: Vec::new(),
        left_items: Vec::new(),
        layers: Vec::new(),
        list_items: Vec::new(),
        points: Vec::new(),
        ranking: Vec::new(),
        center_items: Vec::new(),
        footer: None,
        example_title: None,
        example_body: None,
        timeline_events: Vec::new(),
        steps: Vec::new(),
        phases: Vec::new(),
        direction: None,
        compare_data: None,
        swot_data: None,
    }
}

async fn generate_overview_section_descs(
    client: &LmStudioClient,
    config: &GenerationConfig,
    doc: &ParsedDocument,
    page_plans: &[PagePlan],
) -> Result<Vec<String>> {
    let section_payload = doc
        .sections
        .iter()
        .enumerate()
        .map(|(idx, section)| {
            let related_pages = page_plans
                .iter()
                .filter(|page| page.section_title == section.title)
                .map(|page| {
                    serde_json::json!({
                        "page_title": page.page_title,
                        "subsection_title": page.subsection_title,
                        "objective": page.objective,
                        "key_points": page.key_points,
                        "source_excerpt": page.source_excerpt,
                    })
                })
                .collect::<Vec<_>>();

            serde_json::json!({
                "number": format!("{:02}", idx + 1),
                "section_title": section.title,
                "section_paragraphs": section.paragraphs,
                "subsection_titles": section.subsections.iter().map(|sub| sub.title.clone()).collect::<Vec<_>>(),
                "pages": related_pages,
            })
        })
        .collect::<Vec<_>>();

    let system = "You write concise Chinese section summaries for an overview slide. Return strict JSON only.";
    let user = format!(
        "Document title: {}\n\nSections:\n{}\n\nTask:\nFor each section, write one short Chinese summary for the overview slide `desc` field.\n\nRules:\n- Return JSON only: {{\"items\": [{{\"desc\": \"...\"}}]}}.\n- Keep the output order exactly the same as the input sections.\n- Each desc should usually be 6-16 Chinese characters.\n- Prefer compact noun phrases or topic cues, not complete explanatory sentences.\n- Summaries must be specific to the section content, not generic filler.\n- Prefer summarizing the chapter's core topic, method, or takeaway.\n- Do not mention page count, slide count, or numbering.\n- Do not repeat the section title verbatim unless necessary.\n- Avoid leading words like '介绍' '概述' '本章' unless truly needed.\n- Do not use bullet markers, quotes, or full stops unless needed.\n- Use the same language as the source document.\n",
        doc.title,
        serde_json::to_string_pretty(&section_payload).unwrap_or_default(),
    );

    write_debug(&config.debug_dir, "02a-overview-summary.system.txt", system)?;
    write_debug(&config.debug_dir, "02a-overview-summary.user.txt", &user)?;
    let raw = client
        .generate_text(&config.llm.model, system, &user)
        .await?;
    write_debug(&config.debug_dir, "02a-overview-summary.raw.txt", &raw)?;
    let resp: OverviewSummaryResponse = crate::generator::utils::parse_json_with_extraction(&raw)
        .context("failed to parse overview summary response")?;
    write_debug(
        &config.debug_dir,
        "02a-overview-summary.parsed.json",
        &serde_json::to_string_pretty(&resp)?,
    )?;

    if resp.items.len() != doc.sections.len() {
        bail!(
            "overview summary count mismatch: expected {}, got {}",
            doc.sections.len(),
            resp.items.len()
        );
    }

    Ok(resp
        .items
        .into_iter()
        .map(|item| item.desc)
        .collect::<Vec<_>>())
}

// ---------------------------------------------------------------------------
// Per-page planning functions (for the concurrent per-page pipeline)
// ---------------------------------------------------------------------------

/// Enrich a single page plan via LLM. Falls back to the base plan on failure.
pub async fn enrich_one_page_plan(
    client: &LmStudioClient,
    config: &GenerationConfig,
    doc: &ParsedDocument,
    base_plan: &PagePlan,
    asset_paths: &HashSet<String>,
    debug_dir: &Path,
) -> PagePlan {
    let idx = base_plan.page_id.parse::<usize>().unwrap_or(0);
    let prefix = format!("01-page-{:02}-enrich", idx);

    let system = "You enrich a deterministic slide page plan for a Chinese Slidev deck. Return strict JSON only.";
    let user = format!(
        "Document title: {}\n\nPage plan to enrich:\n{}\n\nAvailable image assets:\n{}\n\nEnrich only these fields:\n- objective: one concise Chinese sentence describing the communication goal.\n- key_points: 1-6 short Chinese bullet-like points capturing the page's actual content.\n- takeaway: optional one-line takeaway in Chinese.\n- content_shape: one of overview, summary, comparison, architecture, timeline, workflow, matrix.\n- layout_intent: short English phrase describing the ideal visual arrangement.\n- visual_need: one of text_only, image_optional, image_required.\n- object_count: one of single, pair, multi.\n- argument_mode: one of parallel, sequential, layered, summary, evidence, causal, warning.\n- density: one of low, medium, high.\n- preferred_assets: zero to two items chosen only from the available asset list.\n\nRules:\n- Preserve page_id exactly.\n- Use the page's source_excerpt and headings as the primary evidence.\n- key_points must be faithful to the source and not generic filler.\n- If no asset is truly relevant, return an empty preferred_assets array.\n- Do not invent asset paths.\n\nReturn JSON: {{\"pages\": [{{...}}]}}",
        doc.title,
        serde_json::to_string_pretty(base_plan).unwrap_or_default(),
        serde_json::to_string_pretty(&sorted_assets(asset_paths)).unwrap_or_default(),
    );

    let _ = write_debug(debug_dir, &format!("{prefix}.system.txt"), system);
    let _ = write_debug(debug_dir, &format!("{prefix}.user.txt"), &user);

    match client.generate_text(&config.llm.model, system, &user).await {
        Ok(raw) => {
            let _ = write_debug(debug_dir, &format!("{prefix}.raw.txt"), &raw);
            match crate::generator::utils::parse_json_with_extraction::<PageSignalResponse>(&raw) {
                Ok(resp) => {
                    let _ = write_debug(
                        debug_dir,
                        &format!("{prefix}.parsed.json"),
                        &serde_json::to_string_pretty(&resp).unwrap_or_default(),
                    );
                    if let Some(patch) = resp.pages.into_iter().next() {
                        return merge_page_signal_patch(base_plan, &patch, asset_paths);
                    }
                }
                Err(e) => eprintln!("parse error enriching page {}: {e}", base_plan.page_id),
            }
        }
        Err(e) => eprintln!("LLM error enriching page {}: {e}", base_plan.page_id),
    }

    base_plan.clone()
}

/// Ask the LLM to rank layout candidates for a single page, judged on per-page fit only.
///
/// Deck-level variety is decided afterwards by [`assign_layouts`], which sees every page's
/// candidates at once. Never fails: on LLM/parse failure, or when every returned candidate is
/// unusable, deterministic fallback candidates are returned instead.
pub async fn layout_candidates_for_page(
    client: &LmStudioClient,
    config: &GenerationConfig,
    page_plan: &PagePlan,
    debug_dir: &Path,
) -> Vec<LayoutCandidate> {
    let aspect = config.aspect_ratio;
    if forced_kind(page_plan).is_some() {
        return fallback_candidates(page_plan, aspect);
    }

    let idx = page_plan.page_id.parse::<usize>().unwrap_or(0);
    let prefix = format!("02-layout-{:02}", idx);

    // Whitelist and per-kind guidance come from the shared eligibility table, so the LLM is
    // only offered kinds that `sanitize_candidates` would accept anyway.
    let rules = prompt_rules(page_plan, aspect);
    let system = format!(
        "You choose the best Slidev component kinds for a single slide page. Use only: {}. Return strict JSON.",
        rules.allowed_names.join(", ")
    );
    let user = format!(
        "Page plan:\n{}\n\nRank up to 3 distinct suitable component kinds for this page, ordered by fitness, choosing only from the allowed kinds.\nScore each kind only by how well it fits THIS page. Deck-level variety is handled separately, so do not lower a good fit's score just to vary layouts, and give honest scores to the 2nd and 3rd choices.\n\n{}\n\nReturn JSON:\n{{\"candidates\": [{{\"kind\": \"...\", \"score\": 1-10, \"reason\": \"...\"}}, ...]}}",
        serde_json::to_string_pretty(page_plan).unwrap_or_default(),
        rules.rules_text,
    );

    let _ = write_debug(debug_dir, &format!("{prefix}.system.txt"), &system);
    let _ = write_debug(debug_dir, &format!("{prefix}.user.txt"), &user);

    match client.generate_text(&config.llm.model, &system, &user).await {
        Ok(raw) => {
            let _ = write_debug(debug_dir, &format!("{prefix}.raw.txt"), &raw);
            match crate::generator::utils::parse_json_with_extraction::<LayoutCandidateResponse>(
                &raw,
            ) {
                Ok(resp) => {
                    let _ = write_debug(
                        debug_dir,
                        &format!("{prefix}.parsed.json"),
                        &serde_json::to_string_pretty(&resp).unwrap_or_default(),
                    );
                    let candidates = sanitize_candidates(page_plan, aspect, resp.candidates);
                    if !candidates.is_empty() {
                        return candidates;
                    }
                    eprintln!(
                        "no usable layout candidates for page {}, using fallback",
                        page_plan.page_id
                    );
                }
                Err(e) => eprintln!(
                    "parse error layout candidates page {}: {e}",
                    page_plan.page_id
                ),
            }
        }
        Err(e) => eprintln!("LLM error layout page {}: {e}", page_plan.page_id),
    }

    fallback_candidates(page_plan, aspect)
}

/// Kinds fully determined by the page plan, needing no LLM call.
/// `section_summary` pages always render as `SectionIntro`.
fn forced_kind(page_plan: &PagePlan) -> Option<SlideKind> {
    (page_plan.page_role.as_deref() == Some("section_summary")).then_some(SlideKind::SectionIntro)
}

/// Drop kinds the eligibility table rules out, collapse duplicate kinds (keeping the higher
/// score), clamp scores into 1..=10 and order by score (stable, so LLM order breaks ties).
fn sanitize_candidates(
    page_plan: &PagePlan,
    aspect: AspectRatio,
    raw: Vec<LayoutCandidate>,
) -> Vec<LayoutCandidate> {
    let mut out: Vec<LayoutCandidate> = Vec::new();
    for mut candidate in raw {
        if let Some(reason) = ineligibility(&candidate.kind, page_plan, aspect) {
            eprintln!(
                "page {}: dropping layout candidate {}: {reason}",
                page_plan.page_id,
                slide_kind_name(&candidate.kind)
            );
            continue;
        }
        candidate.score = candidate.score.clamp(1, 10);
        match out.iter_mut().find(|c| c.kind == candidate.kind) {
            Some(existing) if candidate.score > existing.score => *existing = candidate,
            Some(_) => {}
            None => out.push(candidate),
        }
    }
    out.sort_by(|a, b| b.score.cmp(&a.score));
    out
}

/// Deterministic candidates for pages the LLM is not asked about (section_summary) or gave
/// nothing usable for: the best eligible signal-based pick plus one different eligible
/// alternative so the deck-level assigner still has room to avoid repeats.
fn fallback_candidates(page_plan: &PagePlan, aspect: AspectRatio) -> Vec<LayoutCandidate> {
    if let Some(kind) = forced_kind(page_plan) {
        return vec![LayoutCandidate {
            kind,
            score: 10,
            reason: "section_summary page always uses section_intro".to_string(),
        }];
    }
    eligible_fallback_kinds(page_plan, aspect)
        .into_iter()
        .take(2)
        .zip([
            (6, "deterministic fallback (LLM layout failed)"),
            (4, "deterministic fallback alternative"),
        ])
        .map(|(kind, (score, reason))| LayoutCandidate {
            kind,
            score,
            reason: reason.to_string(),
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Deck-level layout assignment
// ---------------------------------------------------------------------------

/// Penalty when a page repeats the previous page's kind.
const ADJACENT_SAME_PENALTY: f32 = 3.0;
/// Penalty when a page repeats the kind from two pages back (A-B-A rhythm).
const GAP2_SAME_PENALTY: f32 = 1.0;
/// Penalty per use of a kind beyond the deck-wide cap.
const OVERUSE_PENALTY: f32 = 2.0;
/// Share of content pages a single kind may take before the overuse penalty applies.
const MAX_KIND_SHARE: f32 = 0.3;
/// Upper bound on local-search sweeps (it converges in a handful in practice).
const MAX_LOCAL_SEARCH_SWEEPS: usize = 32;

/// Choose one layout per page for the whole deck at once.
///
/// `candidates[i]` is the ranked candidate list for `page_plans[i]`, in slide order. The
/// assignment maximizes
///
/// ```text
///   Σ score − ADJACENT·[same kind as page i−1] − GAP2·[same kind as page i−2]
///           − OVERUSE·Σ_kind max(0, count − cap)
/// ```
///
/// An exact Viterbi pass optimizes score + adjacency, then a deterministic local search
/// (single-page swaps) handles the remaining terms. The result depends only on the
/// candidates, never on generation concurrency or completion order.
pub fn assign_layouts(
    page_plans: &[PagePlan],
    candidates: &[Vec<LayoutCandidate>],
    aspect: AspectRatio,
) -> Vec<LayoutPlan> {
    let cands: Vec<Vec<LayoutCandidate>> = page_plans
        .iter()
        .enumerate()
        .map(|(i, page)| match candidates.get(i) {
            Some(list) if !list.is_empty() => list.clone(),
            _ => fallback_candidates(page, aspect),
        })
        .collect();

    // SectionIntro pages are structural, so the cap is based on the remaining pages.
    let content_pages = page_plans
        .iter()
        .filter(|page| forced_kind(page).is_none())
        .count();
    let cap = overuse_cap(content_pages);

    let mut choice = viterbi_choice(&cands);
    let mut current = deck_objective(&cands, &choice, cap);
    for _ in 0..MAX_LOCAL_SEARCH_SWEEPS {
        let mut improved = false;
        for i in 0..cands.len() {
            for j in 0..cands[i].len() {
                if j == choice[i] {
                    continue;
                }
                let previous = choice[i];
                choice[i] = j;
                let value = deck_objective(&cands, &choice, cap);
                if value > current + 1e-4 {
                    current = value;
                    improved = true;
                } else {
                    choice[i] = previous;
                }
            }
        }
        if !improved {
            break;
        }
    }

    page_plans
        .iter()
        .zip(&cands)
        .zip(&choice)
        .map(|((page, list), &j)| {
            let chosen = &list[j];
            let reason = if j == 0 {
                chosen.reason.clone()
            } else {
                format!(
                    "{} [deck variety: candidate #{} of {}, over {}]",
                    chosen.reason,
                    j + 1,
                    list.len(),
                    slide_kind_name(&list[0].kind)
                )
            };
            LayoutPlan {
                page_id: page.page_id.clone(),
                kind: chosen.kind.clone(),
                title: page.page_title.clone(),
                subtitle: None,
                section_label: None,
                reason,
            }
        })
        .collect()
}

fn overuse_cap(content_pages: usize) -> usize {
    ((content_pages as f32 * MAX_KIND_SHARE).ceil() as usize).max(2)
}

/// Exact maximization of Σ score − ADJACENT·[same kind as previous page].
/// Ties resolve to the earlier (higher-ranked) candidate, keeping the result deterministic.
fn viterbi_choice(cands: &[Vec<LayoutCandidate>]) -> Vec<usize> {
    let n = cands.len();
    if n == 0 {
        return Vec::new();
    }
    let mut best: Vec<Vec<f32>> = Vec::with_capacity(n);
    let mut back: Vec<Vec<usize>> = Vec::with_capacity(n);
    best.push(cands[0].iter().map(|c| c.score as f32).collect());
    back.push(vec![0; cands[0].len()]);

    for i in 1..n {
        let mut row = Vec::with_capacity(cands[i].len());
        let mut back_row = Vec::with_capacity(cands[i].len());
        for candidate in &cands[i] {
            let mut top = f32::NEG_INFINITY;
            let mut arg = 0;
            for (k, prev) in cands[i - 1].iter().enumerate() {
                let penalty = if prev.kind == candidate.kind {
                    ADJACENT_SAME_PENALTY
                } else {
                    0.0
                };
                let value = best[i - 1][k] - penalty;
                if value > top {
                    top = value;
                    arg = k;
                }
            }
            row.push(top + candidate.score as f32);
            back_row.push(arg);
        }
        best.push(row);
        back.push(back_row);
    }

    let last = &best[n - 1];
    let mut j = 0;
    for (k, &value) in last.iter().enumerate() {
        if value > last[j] {
            j = k;
        }
    }
    let mut choice = vec![0; n];
    for i in (0..n).rev() {
        choice[i] = j;
        j = back[i][j];
    }
    choice
}

/// Full deck objective (higher is better); see [`assign_layouts`].
fn deck_objective(cands: &[Vec<LayoutCandidate>], choice: &[usize], cap: usize) -> f32 {
    let kind = |i: usize| &cands[i][choice[i]].kind;
    let mut total = 0.0;
    let mut counts: std::collections::HashMap<&'static str, usize> =
        std::collections::HashMap::new();
    for i in 0..choice.len() {
        total += cands[i][choice[i]].score as f32;
        if i >= 1 && kind(i) == kind(i - 1) {
            total -= ADJACENT_SAME_PENALTY;
        }
        if i >= 2 && kind(i) == kind(i - 2) {
            total -= GAP2_SAME_PENALTY;
        }
        if *kind(i) != SlideKind::SectionIntro {
            *counts.entry(slide_kind_name(kind(i))).or_insert(0) += 1;
        }
    }
    for &count in counts.values() {
        if count > cap {
            total -= OVERUSE_PENALTY * (count - cap) as f32;
        }
    }
    total
}

fn fallback_overview_section_desc(section: &Section, page_plans: &[PagePlan]) -> String {
    use crate::generator::utils::{clean_model_text, summarize};

    let related_pages = page_plans
        .iter()
        .filter(|page| page.section_title == section.title)
        .collect::<Vec<_>>();

    let candidates = section
        .subsections
        .iter()
        .map(|sub| sub.title.as_str())
        .chain(
            related_pages
                .iter()
                .flat_map(|page| page.key_points.iter().map(String::as_str)),
        )
        .chain(related_pages.iter().map(|page| page.objective.as_str()))
        .chain(section.paragraphs.iter().map(String::as_str))
        .filter(|text| !text.trim().is_empty())
        .collect::<Vec<_>>();

    for candidate in candidates {
        let cleaned = clean_model_text(candidate);
        if cleaned != section.title && cleaned.chars().count() >= 6 {
            return summarize(&cleaned, 16);
        }
    }

    summarize(&format!("核心议题：{}", section.title), 16)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::Subsection;

    fn cand(kind: SlideKind, score: u8) -> LayoutCandidate {
        LayoutCandidate {
            kind,
            score,
            reason: format!("fit {score}"),
        }
    }

    fn content_page(idx: usize) -> PagePlan {
        build_content_page(idx, "章节", None, &format!("页面{idx}"), &[])
    }

    fn summary_page(idx: usize) -> PagePlan {
        let section = Section {
            title: "实施路径".to_string(),
            paragraphs: Vec::new(),
            subsections: ["阶段一", "阶段二"]
                .iter()
                .map(|title| Subsection {
                    title: title.to_string(),
                    paragraphs: Vec::new(),
                })
                .collect(),
        };
        build_summary_page(idx, &section)
    }

    fn kinds(plans: &[LayoutPlan]) -> Vec<SlideKind> {
        plans.iter().map(|plan| plan.kind.clone()).collect()
    }

    fn assert_no_adjacent_repeats(kinds: &[SlideKind]) {
        for pair in kinds.windows(2) {
            assert_ne!(pair[0], pair[1], "adjacent repeat in {kinds:?}");
        }
    }

    #[test]
    fn sanitize_drops_unassignable_dedups_clamps_and_sorts() {
        let page = content_page(1);
        let raw = vec![
            cand(SlideKind::Cover, 10),
            cand(SlideKind::FeatureGrid, 6),
            cand(SlideKind::SectionIntro, 9),
            cand(SlideKind::Timeline, 0),
            cand(SlideKind::FeatureGrid, 8),
            cand(SlideKind::OutcomeGrid, 7),
            cand(SlideKind::Closing, 10),
        ];
        let out = sanitize_candidates(&page, AspectRatio::Ratio16x9, raw);
        let summary: Vec<(SlideKind, u8)> = out.iter().map(|c| (c.kind.clone(), c.score)).collect();
        assert_eq!(
            summary,
            vec![
                (SlideKind::FeatureGrid, 8),
                (SlideKind::OutcomeGrid, 7),
                (SlideKind::Timeline, 1),
            ]
        );
    }

    #[test]
    fn sanitize_keeps_section_intro_only_for_summary_pages() {
        let out = sanitize_candidates(&summary_page(1), AspectRatio::Ratio16x9, vec![cand(SlideKind::SectionIntro, 9)]);
        assert_eq!(out.len(), 1);
        assert!(sanitize_candidates(&content_page(1), AspectRatio::Ratio16x9, vec![cand(SlideKind::SectionIntro, 9)])
            .is_empty());
    }

    #[test]
    fn fallback_candidates_offer_two_distinct_kinds_for_content_pages() {
        let out = fallback_candidates(&content_page(1), AspectRatio::Ratio16x9);
        assert_eq!(out.len(), 2);
        assert_ne!(out[0].kind, out[1].kind);
        assert!(out[0].score > out[1].score);

        let forced = fallback_candidates(&summary_page(1), AspectRatio::Ratio16x9);
        assert_eq!(forced.len(), 1);
        assert_eq!(forced[0].kind, SlideKind::SectionIntro);
    }

    #[test]
    fn assign_alternates_when_alternatives_are_close() {
        let pages: Vec<PagePlan> = (1..=4).map(content_page).collect();
        let candidates = vec![
            vec![cand(SlideKind::FeatureGrid, 9), cand(SlideKind::OutcomeGrid, 8)];
            4
        ];
        let plans = assign_layouts(&pages, &candidates, AspectRatio::Ratio16x9);
        assert_no_adjacent_repeats(&kinds(&plans));
    }

    #[test]
    fn assign_keeps_a_strong_fit_over_a_poor_alternative() {
        let pages: Vec<PagePlan> = (1..=2).map(content_page).collect();
        let candidates = vec![
            vec![cand(SlideKind::FeatureGrid, 9), cand(SlideKind::Compare, 2)];
            2
        ];
        let plans = assign_layouts(&pages, &candidates, AspectRatio::Ratio16x9);
        assert_eq!(
            kinds(&plans),
            vec![SlideKind::FeatureGrid, SlideKind::FeatureGrid]
        );
    }

    #[test]
    fn assign_respects_deck_wide_overuse_cap() {
        // Every page prefers FeatureGrid. Plain alternation would still use it 3 times;
        // with 6 content pages the cap is 2, so the assigner must spread it out further.
        let alternatives = [
            SlideKind::OutcomeGrid,
            SlideKind::Timeline,
            SlideKind::StepFlow,
            SlideKind::CenterGrid,
            SlideKind::SectionList,
            SlideKind::SplitLayers,
        ];
        let pages: Vec<PagePlan> = (1..=6).map(content_page).collect();
        let candidates: Vec<Vec<LayoutCandidate>> = alternatives
            .iter()
            .map(|alt| vec![cand(SlideKind::FeatureGrid, 9), cand(alt.clone(), 8)])
            .collect();
        let chosen = kinds(&assign_layouts(&pages, &candidates, AspectRatio::Ratio16x9));
        let feature_grids = chosen
            .iter()
            .filter(|kind| **kind == SlideKind::FeatureGrid)
            .count();
        assert!(feature_grids <= overuse_cap(6), "{chosen:?}");
        assert_no_adjacent_repeats(&chosen);
    }

    #[test]
    fn assign_is_independent_of_how_many_pages_ran_concurrently() {
        // Same candidates must always produce the same deck (no hidden ordering state).
        let pages: Vec<PagePlan> = (1..=5).map(content_page).collect();
        let candidates = vec![
            vec![
                cand(SlideKind::FeatureGrid, 9),
                cand(SlideKind::OutcomeGrid, 8),
                cand(SlideKind::StepFlow, 7),
            ];
            5
        ];
        let first = kinds(&assign_layouts(&pages, &candidates, AspectRatio::Ratio16x9));
        for _ in 0..5 {
            assert_eq!(kinds(&assign_layouts(&pages, &candidates, AspectRatio::Ratio16x9)), first);
        }
    }

    #[test]
    fn assign_forces_section_intro_and_fills_missing_candidates() {
        let pages = vec![summary_page(1), content_page(2), content_page(3)];
        let candidates = vec![
            Vec::new(),
            vec![cand(SlideKind::Timeline, 9)],
            Vec::new(),
        ];
        let plans = assign_layouts(&pages, &candidates, AspectRatio::Ratio16x9);
        assert_eq!(plans[0].kind, SlideKind::SectionIntro);
        assert_eq!(plans[1].kind, SlideKind::Timeline);
        let fallback: Vec<SlideKind> = fallback_candidates(&pages[2], AspectRatio::Ratio16x9)
            .into_iter()
            .map(|c| c.kind)
            .collect();
        assert!(fallback.contains(&plans[2].kind));
        assert_eq!(
            plans.iter().map(|p| p.page_id.as_str()).collect::<Vec<_>>(),
            vec!["1", "2", "3"]
        );
    }

    #[test]
    fn assign_annotates_reason_when_not_taking_the_top_candidate() {
        let pages: Vec<PagePlan> = (1..=2).map(content_page).collect();
        let candidates = vec![
            vec![cand(SlideKind::FeatureGrid, 9), cand(SlideKind::OutcomeGrid, 8)];
            2
        ];
        let plans = assign_layouts(&pages, &candidates, AspectRatio::Ratio16x9);
        let switched = plans
            .iter()
            .find(|plan| plan.kind == SlideKind::OutcomeGrid)
            .expect("one page should trade down for variety");
        assert!(switched.reason.contains("deck variety"), "{}", switched.reason);
        assert!(switched.reason.contains("feature_grid"), "{}", switched.reason);
    }

    #[test]
    fn sanitize_drops_candidates_the_eligibility_table_rules_out() {
        // A plain content page: no pair, no problem wording, no image, 0 key points.
        let page = content_page(1);
        let raw = vec![
            cand(SlideKind::Compare, 10),
            cand(SlideKind::Spotlight, 9),
            cand(SlideKind::Process, 9),
            cand(SlideKind::StepFlow, 7),
        ];
        let kept: Vec<SlideKind> = sanitize_candidates(&page, AspectRatio::Ratio16x9, raw)
            .into_iter()
            .map(|c| c.kind)
            .collect();
        assert_eq!(kept, vec![SlideKind::StepFlow]);
    }

    fn image_hungry_page() -> PagePlan {
        // Signals that make safe_fallback_kind return Spotlight.
        let mut page = content_page(1);
        page.argument_mode = Some("parallel".to_string());
        page.visual_need = Some("image_required".to_string());
        page.object_count = Some("single".to_string());
        page
    }

    #[test]
    fn fallback_kind_skips_ineligible_heuristic_picks() {
        let page = image_hungry_page();
        assert_eq!(safe_fallback_kind(&page), SlideKind::Spotlight);
        // No image asset -> Spotlight is not allowed.
        assert_ne!(fallback_kind(&page, AspectRatio::Ratio16x9), SlideKind::Spotlight);

        let mut with_image = page.clone();
        with_image.preferred_assets = vec!["images/a.png".to_string()];
        assert_eq!(fallback_kind(&with_image, AspectRatio::Ratio16x9), SlideKind::Spotlight);
        // Spotlight is blocked on wide formats even with an image.
        assert_ne!(fallback_kind(&with_image, AspectRatio::Ratio32x9), SlideKind::Spotlight);
    }

    #[test]
    fn every_fallback_path_returns_eligible_kinds() {
        let mut pair = content_page(2);
        pair.object_count = Some("pair".to_string());
        pair.content_shape = Some("comparison".to_string());
        let pages = vec![content_page(1), image_hungry_page(), pair, summary_page(4)];
        for page in &pages {
            for aspect in [
                AspectRatio::Ratio16x9,
                AspectRatio::Ratio32x9,
                AspectRatio::Ratio48x9,
            ] {
                let kinds = eligible_fallback_kinds(page, aspect);
                assert!(!kinds.is_empty());
                assert!(kinds.iter().all(|k| is_eligible(k, page, aspect)), "{kinds:?}");
                for c in fallback_candidates(page, aspect) {
                    assert!(is_eligible(&c.kind, page, aspect), "{:?}", c.kind);
                }
                let current = fallback_kind(page, aspect);
                assert!(is_eligible(&current, page, aspect));
                let switched = pick_different_kind(page, &current, aspect);
                assert!(is_eligible(&switched, page, aspect), "{switched:?}");
                if forced_kind(page).is_none() {
                    assert_ne!(switched, current);
                }
            }
        }
    }
}
