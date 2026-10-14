//! Typst-rendered slide thumbnails.
//!
//! The Vue templates stay the renderer for the editor preview and the presentation stage; the
//! generation grid and the presentation overview show many pages at once, so they display a
//! rasterized PNG per slide instead of mounting a full slide component for every card.
//!
//! The deck is compiled and rasterized through the same renderer, options and asset mapping as
//! the PDF export (`super::pdf`), so a thumbnail can never disagree with the exported page about
//! which image or theme a reference resolves to.

use std::sync::Mutex;
use std::time::Instant;

use base64::Engine;
use serde::Serialize;
use slide_typst::ExportOptions;
use tauri::State;

use crate::types::SlideBlueprint;
use crate::AppState;

use super::pdf::{prepare_export, renderer};

/// One rasterized page, addressed by its index in the `slides` array of the request.
#[derive(Debug, Serialize)]
pub struct Thumbnail {
    pub index: usize,
    /// `data:image/png;base64,...`, ready for an `<img src>`.
    pub data_url: String,
    /// The slide's kind has no Typst layout, so this page is a placeholder: the view must
    /// keep its slide component for it instead of showing the image.
    pub placeholder: bool,
    /// The page is clipped: its content did not fit even at the smallest fit factor, so the
    /// overflowing part is cut off (`overflows` of the compile behind this page). The editor
    /// warns about it; the image itself shows the clipped page, exactly like the PDF.
    pub overflow: bool,
}

#[derive(Debug, Serialize)]
pub struct ThumbnailBatch {
    pub pages: Vec<Thumbnail>,
    /// The requested theme id when it has no Typst layout and the render fell back to
    /// light-notes; `None` when the requested theme rendered as-is.
    pub theme_fallback: Option<String>,
    pub elapsed_ms: u128,
}

/// Rasterizes `slides` (the deck currently on screen) into PNG thumbnails.
///
/// `only` limits the rasterized pages; positions refer to `slides`.
#[tauri::command]
pub async fn render_slide_thumbnails(
    project_id: Option<i64>,
    slides: Vec<SlideBlueprint>,
    theme: Option<String>,
    width_px: u32,
    only: Option<Vec<usize>>,
    state: State<'_, Mutex<AppState>>,
) -> Result<ThumbnailBatch, String> {
    if slides.is_empty() {
        return Ok(ThumbnailBatch {
            pages: Vec::new(),
            theme_fallback: None,
            elapsed_ms: 0,
        });
    }
    let (slides, options, theme_fallback) =
        prepare_export(project_id, Some(slides), theme, &state)?;
    tauri::async_runtime::spawn_blocking(move || {
        render_thumbnails(&slides, &options, width_px, only.as_deref(), theme_fallback)
    })
    .await
    .map_err(|e| format!("缩略图渲染任务异常结束：{e}"))?
}

fn render_thumbnails(
    slides: &[serde_json::Value],
    options: &ExportOptions,
    width_px: u32,
    only: Option<&[usize]>,
    theme_fallback: Option<String>,
) -> Result<ThumbnailBatch, String> {
    let started = Instant::now();
    let rendered = renderer()
        .render_png(slides, options, width_px, only)
        .map_err(|e| e.to_string())?;
    // `render_png` compiles the whole deck, so it reports the clipped pages of the whole deck
    // even when only a few were rasterized; the caller maps this onto its own page keys.
    let clipped: Vec<usize> = rendered
        .overflows
        .iter()
        .filter_map(|overflow| overflow.slide)
        .collect();
    let thumbnails = rendered
        .pages
        .into_iter()
        .map(|page| Thumbnail {
            index: page.index,
            data_url: format!(
                "data:image/png;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(&page.png)
            ),
            placeholder: slides
                .get(page.index)
                .and_then(|slide| slide.get("kind"))
                .and_then(|kind| kind.as_str())
                .is_none_or(|kind| !slide_typst::SUPPORTED_KINDS.contains(&kind)),
            overflow: clipped.contains(&page.index),
        })
        .collect();
    Ok(ThumbnailBatch {
        pages: thumbnails,
        theme_fallback,
        elapsed_ms: started.elapsed().as_millis(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_selects_pages_and_unsupported_kinds_are_placeholders() {
        let slides = vec![
            serde_json::json!({ "kind": "cover", "title": "封面" }),
            serde_json::json!({ "kind": "future_kind", "title": "未移植" }),
            serde_json::json!({ "kind": "closing", "title": "结束" }),
        ];
        let batch = render_thumbnails(
            &slides,
            &ExportOptions::default(),
            320,
            Some(&[0, 2]),
            Some("no-such-theme".to_string()),
        )
        .unwrap();

        assert_eq!(
            batch.pages.iter().map(|p| p.index).collect::<Vec<_>>(),
            vec![0, 2]
        );
        for page in &batch.pages {
            assert!(
                page.data_url.starts_with("data:image/png;base64,iVBOR"),
                "slide {}: {}",
                page.index,
                &page.data_url[..40.min(page.data_url.len())]
            );
            assert!(
                !page.placeholder,
                "slide {} is a supported kind",
                page.index
            );
        }
        assert_eq!(batch.theme_fallback.as_deref(), Some("no-such-theme"));

        // The unsupported kind renders a placeholder page and must be flagged, so the view
        // keeps its slide component for that card.
        let batch =
            render_thumbnails(&slides, &ExportOptions::default(), 320, Some(&[1]), None).unwrap();
        assert_eq!(batch.pages.len(), 1);
        assert_eq!(batch.pages[0].index, 1);
        assert!(batch.pages[0].placeholder);
    }

    /// A page whose content does not fit even at `fit-min` is clipped and must say so, because
    /// the editor warns the user about it; a page that only fits after scaling must not.
    #[test]
    fn clipped_pages_are_flagged_and_scaled_pages_are_not() {
        // Long enough to overflow a `feature_grid` card at `fit-min` even after `normalize`
        // truncates the body: 90 copies still fit, 150 already clip, so this leaves margin.
        let long = "这是一段刻意写得很长的说明文字，用来把卡片撑到放不下。".repeat(200);
        let slides = vec![
            serde_json::json!({
                "kind": "feature_grid",
                "title": "内容过多",
                "cards": [
                    { "title": "卡片一", "body": long, "icon": "i-carbon:square" },
                    { "title": "卡片二", "body": long, "icon": "i-carbon:square" },
                ],
            }),
            serde_json::json!({
                "kind": "feature_grid",
                "title": "内容正常",
                "cards": [{ "title": "卡片", "body": "短文本", "icon": "i-carbon:square" }],
            }),
        ];
        let batch = render_thumbnails(&slides, &ExportOptions::default(), 320, None, None).unwrap();
        assert_eq!(batch.pages.len(), 2);
        assert!(
            batch.pages[0].overflow,
            "the oversized page is clipped at fit-min"
        );
        assert!(
            !batch.pages[1].overflow,
            "the short page fits without clipping"
        );
        assert!(!batch.pages[0].placeholder && !batch.pages[1].placeholder);

        // `only` limits the rasterized pages but the compile behind them covers the whole deck,
        // so a clipped page is still reported when a different page was asked for.
        let batch =
            render_thumbnails(&slides, &ExportOptions::default(), 320, Some(&[1]), None).unwrap();
        assert_eq!(batch.pages.len(), 1);
        assert_eq!(batch.pages[0].index, 1);
        assert!(!batch.pages[0].overflow);
    }
}
