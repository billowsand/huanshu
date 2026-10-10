use crate::config::GenerationConfig;
use crate::input::{HeadingLevel, ParsedDocument, Section};
use crate::lmstudio::LmStudioClient;
use crate::types::*;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

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

/// Pick a valid kind that differs from `current_kind`.
/// Used in the last-resort repair round to actually switch the component.
pub fn pick_different_kind(page: &PagePlan, current_kind: &SlideKind) -> SlideKind {
    if page.page_role.as_deref() == Some("section_summary") {
        return SlideKind::SectionIntro;
    }
    // Try the signal-based fallback first; if it happens to equal the current kind, escalate.
    let candidate = safe_fallback_kind(page);
    if &candidate != current_kind {
        return candidate;
    }
    let variety = variety_fallback_kind(page);
    if &variety != current_kind {
        return variety;
    }
    // Brute-force: cycle through all content kinds in priority order until we find one
    // that differs from current_kind. Cover all 12 content kinds so even "hard" kinds
    // like Swot/Compare/Process are reachable when both signal-based paths failed.
    for kind in &[
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
    ] {
        if kind != current_kind {
            return kind.clone();
        }
    }
    // Unreachable: the list covers all content kinds.
    SlideKind::SectionList
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

/// Choose a layout kind for a single page via LLM, with deterministic fallback.
/// `used_layouts` contains (slide_index, kind_label) of already-generated pages
/// to help the LLM maintain deck variety.
pub async fn layout_one_page(
    client: &LmStudioClient,
    config: &GenerationConfig,
    page_plan: &PagePlan,
    used_layouts: &[(usize, String)],
    debug_dir: &Path,
) -> LayoutPlan {
    if let Some(forced) = forced_layout(page_plan) {
        return forced;
    }

    let idx = page_plan.page_id.parse::<usize>().unwrap_or(0);
    let prefix = format!("02-layout-{:02}", idx);

    let diversity_hint = build_diversity_hint(used_layouts);

    let aspect_constraint = match config.aspect_ratio {
        crate::types::AspectRatio::Ratio16x9 => "",
        crate::types::AspectRatio::Ratio32x9 => {
            "\n\nWIDE FORMAT (32:9): Prefer step_flow/timeline/process/feature_grid/outcome_grid. \
             Avoid spotlight. Use focus_example/compare only when content truly demands two-panel layout."
        }
        crate::types::AspectRatio::Ratio48x9 => {
            "\n\nULTRA-WIDE FORMAT (48:9): Strongly prefer step_flow/timeline/process/feature_grid/outcome_grid/section_intro. \
             AVOID focus_example, compare, spotlight — these fixed two-panel or single-image layouts create extreme whitespace at panoramic width."
        }
    };
    let system = "You choose the best Slidev component kinds for a single slide page. Use only: section_intro, feature_grid, spotlight, split_layers, section_list, focus_example, outcome_grid, center_grid, timeline, step_flow, process, compare, issue_stack, swot. Return strict JSON.";
    let user = format!(
        "Page plan:\n{}{}{}\n\nRank 2-3 suitable component kinds for this page, ordered by fitness.\n\nRules:\n- section_intro: only for section_summary pages that preview sub-topics.\n- feature_grid: 3+ parallel items of equal weight.\n- spotlight: single-object focus with image.\n- split_layers: layered/architectural content.\n- section_list: strict order-dependent steps.\n- focus_example: central thesis + example.\n- outcome_grid: 2-4 parallel deliverables/results.\n- center_grid: compact closing summary or goal.\n- timeline: chronological events with dates.\n- step_flow: 2-4 step linear process.\n- process: 5+ steps grouped into phases.\n- compare: EXACTLY two alternatives.\n- issue_stack: 2-4 stacked problems, challenges, or risks; argument_mode=warning or causal, or page content discusses failures/obstacles.\n- swot: four-quadrant matrix.\n\nHard constraints:\n- step_flow when key_points <= 4; process when >= 5.\n- compare ONLY for exactly two alternatives.\n- issue_stack ONLY for problem/challenge/risk content with 2-4 items.\n- swot ONLY for four-quadrant matrices.\n\n\nReturn JSON:\n{{\"candidates\": [{{\"kind\": \"...\", \"score\": 1-10, \"reason\": \"...\"}}, ...]}}",
        serde_json::to_string_pretty(page_plan).unwrap_or_default(),
        diversity_hint,
        aspect_constraint,
    );

    let _ = write_debug(debug_dir, &format!("{prefix}.system.txt"), system);
    let _ = write_debug(debug_dir, &format!("{prefix}.user.txt"), &user);

    match client.generate_text(&config.llm.model, system, &user).await {
        Ok(raw) => {
            let _ = write_debug(debug_dir, &format!("{prefix}.raw.txt"), &raw);
            match crate::generator::utils::parse_json_with_extraction::<
                crate::types::LayoutCandidateResponse,
            >(&raw)
            {
                Ok(resp) => {
                    let _ = write_debug(
                        debug_dir,
                        &format!("{prefix}.parsed.json"),
                        &serde_json::to_string_pretty(&resp).unwrap_or_default(),
                    );
                    if !resp.candidates.is_empty() {
                        let chosen = select_layout_with_diversity(&resp.candidates, used_layouts);
                        return LayoutPlan {
                            page_id: page_plan.page_id.clone(),
                            kind: chosen.kind.clone(),
                            title: page_plan.page_title.clone(),
                            subtitle: None,
                            section_label: None,
                            reason: chosen.reason.clone(),
                        };
                    }
                }
                Err(e) => eprintln!(
                    "parse error layout candidates page {}: {e}",
                    page_plan.page_id
                ),
            }
        }
        Err(e) => eprintln!("LLM error layout page {}: {e}", page_plan.page_id),
    }

    // Deterministic fallback
    let kind = safe_fallback_kind(page_plan);
    LayoutPlan {
        page_id: page_plan.page_id.clone(),
        kind,
        title: page_plan.page_title.clone(),
        subtitle: None,
        section_label: None,
        reason: "deterministic fallback (LLM layout failed)".to_string(),
    }
}

/// Layouts that are fully determined by the page plan and need no LLM call.
/// `section_summary` pages always render as `SectionIntro`.
fn forced_layout(page_plan: &PagePlan) -> Option<LayoutPlan> {
    if page_plan.page_role.as_deref() != Some("section_summary") {
        return None;
    }
    Some(LayoutPlan {
        page_id: page_plan.page_id.clone(),
        kind: SlideKind::SectionIntro,
        title: page_plan.page_title.clone(),
        subtitle: None,
        section_label: None,
        reason: "section_summary page always uses section_intro".to_string(),
    })
}

/// Record (or overwrite) the layout kind used by `slide_index` in the shared
/// diversity tracker. Called once after layout selection and again after repair,
/// since the repair stage may switch the slide to a different kind.
pub fn record_used_layout(
    used_layouts: &mut Vec<(usize, String)>,
    slide_index: usize,
    kind: &SlideKind,
) {
    let name = slide_kind_name(kind).to_string();
    match used_layouts.iter_mut().find(|(i, _)| *i == slide_index) {
        Some(entry) => entry.1 = name,
        None => used_layouts.push((slide_index, name)),
    }
}

/// Build a diversity hint string describing already-used layouts and overuse warnings.
fn build_diversity_hint(used_layouts: &[(usize, String)]) -> String {
    if used_layouts.is_empty() {
        return String::new();
    }
    let kind_counts: std::collections::BTreeMap<&str, usize> = {
        let mut m = std::collections::BTreeMap::new();
        for (_, k) in used_layouts {
            *m.entry(k.as_str()).or_insert(0) += 1;
        }
        m
    };
    let total = used_layouts.len();
    let overused: Vec<String> = kind_counts
        .iter()
        .filter(|(_, &count)| count as f32 / total as f32 > 0.5)
        .map(|(kind, count)| format!("{} ({}/{})", kind, count, total))
        .collect();
    if overused.is_empty() {
        format!(
            "\n\nPreviously used layouts ({} pages generated so far): {}",
            total,
            used_layouts
                .iter()
                .map(|(i, k)| format!("#{}: {}", i + 1, k))
                .collect::<Vec<_>>()
                .join(", ")
        )
    } else {
        format!(
            "\n\nIMPORTANT diversity constraint — previously used layouts: {}\nThese layouts are over-represented (>{:.0}%). Prefer NOT using: {}.\nRank alternative layout types higher to ensure visual variety.",
            used_layouts
                .iter()
                .map(|(i, k)| format!("#{}: {}", i + 1, k))
                .collect::<Vec<_>>()
                .join(", "),
            50.0,
            overused.join(", ")
        )
    }
}

/// Select the best layout candidate from an LLM-ordered list, trading off score
/// against already-used layout diversity.
///
/// Logic:
/// 1. Compute usage ratio for each candidate's kind from `used_layouts`.
/// 2. Adjust the candidate's effective score: subtract a penalty proportional to
///    how over-used that kind already is.
/// 3. If the top candidate is over-used, trade down to the highest-ranked
///    alternative that is a *different* kind, is *less* used, and whose raw
///    score is within 2 points (diversity trade-off).
/// 4. Otherwise pick the highest effective score.
fn select_layout_with_diversity<'a>(
    candidates: &'a [crate::types::LayoutCandidate],
    used_layouts: &[(usize, String)],
) -> &'a crate::types::LayoutCandidate {
    if candidates.len() == 1 || used_layouts.is_empty() {
        return candidates.first().expect("candidates must be non-empty");
    }

    let total = used_layouts.len() as f32;
    let kind_count = |kind: &SlideKind| -> f32 {
        let name = slide_kind_name(kind);
        used_layouts.iter().filter(|(_, k)| k == name).count() as f32
    };

    // Sort candidates by (raw_score - diversity_penalty), descending.
    // penalty = 3 * (usage_ratio). So a kind used 50%+ gets -1.5 penalty.
    let mut ranked: Vec<(usize, f32)> = candidates
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let raw = c.score as f32;
            let ratio = kind_count(&c.kind) / total;
            let penalty = 3.0 * ratio;
            (i, raw - penalty)
        })
        .collect();
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // If the top choice has a high usage ratio, trade down to a close alternative
    // that is genuinely a different and less-used kind. Never switch to the same
    // kind (duplicate candidates) or to a kind that is used even more.
    let best = &candidates[ranked[0].0];
    let best_ratio = kind_count(&best.kind) / total;
    if best_ratio > 0.3 {
        let best_raw = best.score as f32;
        let alternative = ranked[1..].iter().map(|&(i, _)| &candidates[i]).find(|c| {
            c.kind != best.kind
                && kind_count(&c.kind) / total < best_ratio
                && best_raw - c.score as f32 <= 2.0
        });
        if let Some(alt) = alternative {
            return alt;
        }
    }

    best
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
    use crate::types::LayoutCandidate;

    fn cand(kind: SlideKind, score: u8) -> LayoutCandidate {
        LayoutCandidate {
            kind,
            score,
            reason: String::new(),
        }
    }

    fn used(kinds: &[SlideKind]) -> Vec<(usize, String)> {
        let mut out = Vec::new();
        for (i, kind) in kinds.iter().enumerate() {
            record_used_layout(&mut out, i, kind);
        }
        out
    }

    #[test]
    fn select_returns_top_when_nothing_used() {
        let cands = [cand(SlideKind::FeatureGrid, 9), cand(SlideKind::OutcomeGrid, 8)];
        assert_eq!(
            select_layout_with_diversity(&cands, &[]).kind,
            SlideKind::FeatureGrid
        );
    }

    #[test]
    fn select_trades_overused_top_for_close_less_used_alternative() {
        let history = used(&[
            SlideKind::FeatureGrid,
            SlideKind::FeatureGrid,
            SlideKind::Timeline,
        ]);
        // FeatureGrid: 9 - 3*(2/3) = 7.0; StepFlow: 7 - 0 = 7.0 -> FeatureGrid ranks first
        // (stable sort), is overused (0.67 > 0.3), StepFlow is within 2 points and unused.
        let cands = [cand(SlideKind::FeatureGrid, 9), cand(SlideKind::StepFlow, 7)];
        assert_eq!(
            select_layout_with_diversity(&cands, &history).kind,
            SlideKind::StepFlow
        );
    }

    #[test]
    fn select_never_trades_down_to_a_more_used_kind() {
        // Regression: OutcomeGrid (ratio 0.4) ranks first; the old logic switched to
        // FeatureGrid (ratio 0.6) just because it was "second and close".
        let history = used(&[
            SlideKind::FeatureGrid,
            SlideKind::FeatureGrid,
            SlideKind::FeatureGrid,
            SlideKind::OutcomeGrid,
            SlideKind::OutcomeGrid,
        ]);
        let cands = [cand(SlideKind::OutcomeGrid, 9), cand(SlideKind::FeatureGrid, 9)];
        assert_eq!(
            select_layout_with_diversity(&cands, &history).kind,
            SlideKind::OutcomeGrid
        );
    }

    #[test]
    fn select_ignores_duplicate_kind_as_alternative() {
        let history = used(&[SlideKind::FeatureGrid, SlideKind::FeatureGrid]);
        let cands = [
            cand(SlideKind::FeatureGrid, 9),
            cand(SlideKind::FeatureGrid, 8),
            cand(SlideKind::Compare, 3),
        ];
        // Duplicate FeatureGrid is not a real alternative; Compare is too far behind.
        assert_eq!(
            select_layout_with_diversity(&cands, &history).kind,
            SlideKind::FeatureGrid
        );
    }

    #[test]
    fn select_skips_unsuitable_second_and_uses_third() {
        let history = used(&[SlideKind::FeatureGrid, SlideKind::OutcomeGrid]);
        // FeatureGrid 10-1.5=8.5, OutcomeGrid 9-1.5=7.5, StepFlow 8-0=8.0.
        // Ranked: FeatureGrid, StepFlow, OutcomeGrid -> StepFlow is the less-used pick.
        let cands = [
            cand(SlideKind::FeatureGrid, 10),
            cand(SlideKind::OutcomeGrid, 9),
            cand(SlideKind::StepFlow, 8),
        ];
        assert_eq!(
            select_layout_with_diversity(&cands, &history).kind,
            SlideKind::StepFlow
        );
    }

    #[test]
    fn select_keeps_top_when_alternative_score_gap_too_large() {
        let history = used(&[SlideKind::FeatureGrid]);
        let cands = [cand(SlideKind::FeatureGrid, 10), cand(SlideKind::Compare, 4)];
        assert_eq!(
            select_layout_with_diversity(&cands, &history).kind,
            SlideKind::FeatureGrid
        );
    }

    #[test]
    fn record_used_layout_uses_snake_case_and_overwrites_same_index() {
        let mut layouts = Vec::new();
        record_used_layout(&mut layouts, 3, &SlideKind::FeatureGrid);
        record_used_layout(&mut layouts, 5, &SlideKind::Timeline);
        assert_eq!(
            layouts,
            vec![(3, "feature_grid".to_string()), (5, "timeline".to_string())]
        );

        // Repair switched slide 3 to another kind: overwrite, don't append.
        record_used_layout(&mut layouts, 3, &SlideKind::SectionList);
        assert_eq!(
            layouts,
            vec![(3, "section_list".to_string()), (5, "timeline".to_string())]
        );
    }

    #[test]
    fn forced_layout_only_applies_to_section_summary_pages() {
        let section = Section {
            title: "实施路径".to_string(),
            paragraphs: Vec::new(),
            subsections: vec![
                Subsection {
                    title: "阶段一".to_string(),
                    paragraphs: Vec::new(),
                },
                Subsection {
                    title: "阶段二".to_string(),
                    paragraphs: Vec::new(),
                },
            ],
        };
        let summary = build_summary_page(1, &section);
        let forced = forced_layout(&summary).expect("section_summary must be forced");
        assert_eq!(forced.kind, SlideKind::SectionIntro);
        assert_eq!(forced.page_id, "1");

        let content = build_content_page(2, "实施路径", None, "实施路径", &[]);
        assert!(forced_layout(&content).is_none());
    }
}
