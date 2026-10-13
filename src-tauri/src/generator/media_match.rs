//! Place media-library images into the deck by reading the article with them.
//!
//! A caption generated at upload time only says *what* an image shows; it cannot say
//! *where* it belongs. This pass hands the multimodal model the whole page outline as
//! context next to the picture and asks for a target page, a purpose, and a caption that
//! matches the article.
//!
//! Results are advisory: they seed `preferred_assets` for that page and ride along in the
//! per-page prompts, but the model may still choose a layout that shows no image.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Deserialize;

use crate::assets::MediaPlacement;
use crate::db::MediaAsset;
use crate::generator::utils::parse_json_with_extraction;
use crate::lmstudio::LmStudioClient;
use crate::types::PagePlan;

/// Cap on images matched per run, so a large library cannot stall generation. Images past
/// the cap still appear in the asset catalogue; they just keep the caption-only behaviour.
const MAX_MATCHED_IMAGES: usize = 12;
/// Characters of each page's source excerpt kept in the prompt.
const MAX_EXCERPT_CHARS: usize = 120;
/// Key points listed per page.
const MAX_POINTS_PER_PAGE: usize = 4;

/// Where one image belongs, as answered by the model.
#[derive(Debug, Deserialize, Default)]
pub(crate) struct PlacementResponse {
    #[serde(default)]
    page_id: Option<String>,
    #[serde(default)]
    purpose: Option<String>,
    #[serde(default)]
    caption: Option<String>,
}

/// Match every image it can: one vision call per image, bounded and concurrent.
///
/// Never fails. An image whose file is missing, or whose reply cannot be parsed, is simply
/// left out — which degrades to the caption-only prompt behaviour.
pub async fn match_media_to_pages(
    client: &LmStudioClient,
    model: &str,
    media: &[MediaAsset],
    media_dir: &Path,
    doc_title: &str,
    pages: &[PagePlan],
    concurrency: usize,
) -> Vec<MediaPlacement> {
    let images: Vec<&MediaAsset> = media
        .iter()
        .filter(|asset| asset.media_type == "image")
        .take(MAX_MATCHED_IMAGES)
        .collect();
    if images.is_empty() || pages.is_empty() {
        return Vec::new();
    }

    let page_ids: Vec<String> = pages.iter().map(|page| page.page_id.clone()).collect();
    let outline = page_outline(pages);
    let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrency.max(1)));

    let mut handles = Vec::with_capacity(images.len());
    for asset in images {
        let client = client.clone();
        let model = model.to_string();
        let image_path: PathBuf = media_dir.join(&asset.storage_path);
        let prompt = build_placement_prompt(doc_title, &outline, asset);
        let page_ids = page_ids.clone();
        let semaphore = Arc::clone(&semaphore);
        let media_id = asset.id;
        let reference = format!("media:{media_id}");
        handles.push(tokio::spawn(async move {
            let _permit = semaphore.acquire().await.ok()?;
            if !image_path.is_file() {
                return None;
            }
            let raw = client
                .generate_vision(&model, &image_path.to_string_lossy(), &prompt)
                .await
                .ok()?;
            let parsed = parse_placement(&raw, &page_ids)?;
            Some(MediaPlacement {
                media_id,
                reference,
                page_id: parsed.page_id,
                purpose: parsed.purpose.unwrap_or_default(),
                caption: parsed.caption,
            })
        }));
    }

    let mut placements = Vec::with_capacity(handles.len());
    for handle in handles {
        if let Ok(Some(placement)) = handle.await {
            placements.push(placement);
        }
    }

    // Deck order first (unmatched images last), then media id, so the debug file and the
    // generation log always read the same way.
    placements.sort_by(|a, b| {
        let rank = |placement: &MediaPlacement| {
            placement
                .page_id
                .as_ref()
                .and_then(|id| page_ids.iter().position(|candidate| candidate == id))
                .unwrap_or(usize::MAX)
        };
        rank(a).cmp(&rank(b)).then(a.media_id.cmp(&b.media_id))
    });
    placements
}

/// The article as the model sees it: one line per page, in deck order.
pub(crate) fn page_outline(pages: &[PagePlan]) -> String {
    pages
        .iter()
        .map(|page| {
            let mut line = format!("- page_id={} | 页标题：{}", page.page_id, page.page_title);
            if !page.section_title.trim().is_empty() {
                line.push_str(&format!(" | 章节：{}", page.section_title));
            }
            if let Some(subsection) = page
                .subsection_title
                .as_deref()
                .filter(|title| !title.trim().is_empty())
            {
                line.push_str(&format!(" | 小节：{subsection}"));
            }
            let points: Vec<&str> = page
                .key_points
                .iter()
                .map(String::as_str)
                .take(MAX_POINTS_PER_PAGE)
                .collect();
            if !points.is_empty() {
                line.push_str(&format!(" | 要点：{}", points.join("；")));
            }
            if let Some(excerpt) = page
                .source_excerpt
                .as_deref()
                .map(str::trim)
                .filter(|excerpt| !excerpt.is_empty())
            {
                line.push_str(&format!(
                    " | 原文：{}",
                    truncate_chars(excerpt, MAX_EXCERPT_CHARS)
                ));
            }
            line
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Prompt for one image: the article outline, the image, and what we want back from it.
pub(crate) fn build_placement_prompt(doc_title: &str, outline: &str, asset: &MediaAsset) -> String {
    let caption = asset
        .caption
        .as_deref()
        .map(str::trim)
        .filter(|caption| !caption.is_empty())
        .unwrap_or("（暂无描述）");
    let rules = "- page_id 必须原样取自上面的分页大纲；如果这张图与任何一页都不相关（例如纯装饰、与主题无关），page_id 返回空字符串。\n- purpose 用一句中文说明这张图在该页承担什么（例如“佐证三层架构的说法”）。\n- caption 用中文，不超过 30 字，要结合文稿语境，不要泛泛描述画面。\n- 不要臆造文稿里没有的内容。";
    format!(
        "你是一名演示文稿编辑。下面是整篇文稿的分页大纲，以及用户上传的其中一张图片。\n\n文稿标题：{doc_title}\n\n分页大纲：\n{outline}\n\n这张图片在上传时的自动描述是：{caption}\n文件名：{filename}\n\n任务：结合文稿内容判断这张图片最适合放在哪一页，并说明它在该页的作用，同时给出适合该页的图注。\n\n规则：\n{rules}\n\n只返回 JSON：{{\"page_id\": \"...\", \"purpose\": \"...\", \"caption\": \"...\"}}",
        filename = asset.original_name,
    )
}

/// Parse one placement reply, keeping only a page id that exists in the deck.
///
/// A reply that names no page is still useful: its caption can be written back. Only a
/// reply we cannot read at all — or one that says nothing at all — is dropped.
pub(crate) fn parse_placement(raw: &str, page_ids: &[String]) -> Option<PlacementResponse> {
    let response: PlacementResponse = parse_json_with_extraction(raw).ok()?;
    let page_id = response
        .page_id
        .as_deref()
        .map(str::trim)
        .filter(|id| page_ids.iter().any(|candidate| candidate == id))
        .map(str::to_string);
    let purpose = clean(response.purpose);
    let caption = clean(response.caption);
    if page_id.is_none() && purpose.is_none() && caption.is_none() {
        return None;
    }
    Some(PlacementResponse {
        page_id,
        purpose,
        caption,
    })
}

fn clean(value: Option<String>) -> Option<String> {
    value
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

fn truncate_chars(text: &str, limit: usize) -> String {
    let mut out: String = text.chars().take(limit).collect();
    if text.chars().count() > limit {
        out.push('…');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset(id: i64, caption: Option<&str>) -> MediaAsset {
        MediaAsset {
            id,
            project_id: 1,
            slide_index: None,
            filename: format!("{id}.png"),
            original_name: format!("园区{id}.png"),
            media_type: "image".to_string(),
            mime_type: Some("image/png".to_string()),
            storage_path: format!("projects/1/images/{id}.png"),
            caption: caption.map(str::to_string),
            description: None,
            file_size: None,
            width: Some(1920),
            height: Some(1080),
            created_at: 0,
        }
    }

    fn page(id: &str, title: &str, points: &[&str]) -> PagePlan {
        PagePlan {
            page_id: id.to_string(),
            section_title: "总体设计".to_string(),
            subsection_title: None,
            page_title: title.to_string(),
            objective: String::new(),
            key_points: points.iter().map(|point| point.to_string()).collect(),
            takeaway: None,
            content_shape: Some("architecture".to_string()),
            layout_intent: None,
            visual_need: Some("image_optional".to_string()),
            object_count: Some("single".to_string()),
            argument_mode: Some("layered".to_string()),
            density: Some("medium".to_string()),
            source_excerpt: None,
            preferred_assets: Vec::new(),
            page_role: Some("content".to_string()),
            section_number: Some("01".to_string()),
            chart: None,
        }
    }

    #[test]
    fn the_prompt_carries_the_article_outline_and_the_image() {
        let pages = vec![
            page("2", "总体架构", &["三层解耦", "统一入口"]),
            page("3", "实施路径", &[]),
        ];
        let prompt = build_placement_prompt(
            "智慧园区方案",
            &page_outline(&pages),
            &asset(7, Some("一张架构图")),
        );

        assert!(prompt.contains("智慧园区方案"), "{prompt}");
        assert!(
            prompt.contains("- page_id=2 | 页标题：总体架构"),
            "{prompt}"
        );
        assert!(prompt.contains("章节：总体设计"), "{prompt}");
        assert!(prompt.contains("要点：三层解耦；统一入口"), "{prompt}");
        assert!(prompt.contains("一张架构图"), "{prompt}");
        assert!(prompt.contains("园区7.png"), "{prompt}");
    }

    #[test]
    fn a_page_id_outside_the_outline_is_dropped_but_the_caption_survives() {
        let ids = vec!["2".to_string(), "3".to_string()];

        let invented = parse_placement(
            r#"{"page_id": "9", "purpose": "佐证实施顺序", "caption": "三步实施路线"}"#,
            &ids,
        )
        .expect("parsed");
        assert!(invented.page_id.is_none());
        assert_eq!(invented.caption.as_deref(), Some("三步实施路线"));

        let kept = parse_placement(
            r#"{"page_id": "3", "purpose": "佐证实施顺序", "caption": "三步实施路线"}"#,
            &ids,
        )
        .expect("parsed");
        assert_eq!(kept.page_id.as_deref(), Some("3"));
    }

    #[test]
    fn fenced_json_and_unmatched_images_are_tolerated() {
        let ids = vec!["2".to_string()];

        let fenced = parse_placement(
            "```json\n{\"page_id\": \"2\", \"purpose\": \"x\", \"caption\": \"y\"}\n```",
            &ids,
        )
        .expect("parsed");
        assert_eq!(fenced.page_id.as_deref(), Some("2"));

        // An image that fits nowhere still yields a caption worth writing back.
        let decorative = parse_placement(
            r#"{"page_id": "", "purpose": "", "caption": "园区鸟瞰图"}"#,
            &ids,
        )
        .expect("parsed");
        assert!(decorative.page_id.is_none());
        assert_eq!(decorative.caption.as_deref(), Some("园区鸟瞰图"));

        assert!(parse_placement("这张图挺好的，但我不确定放哪", &ids).is_none());
    }

    #[test]
    fn long_excerpts_are_truncated() {
        let mut long = page("2", "总体架构", &[]);
        long.source_excerpt = Some("甲".repeat(MAX_EXCERPT_CHARS + 50));
        let outline = page_outline(&[long]);
        let excerpt = outline.split("原文：").nth(1).expect("excerpt");
        assert_eq!(excerpt.chars().count(), MAX_EXCERPT_CHARS + 1);
        assert!(excerpt.ends_with('…'), "{outline}");
    }
}
