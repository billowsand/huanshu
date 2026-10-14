//! PDF 导出与缩略图光栅化（都走 `slide-typst` 的进程内 Typst 编译器）。
//!
//! 缩略图与导出共用 [`prepare_export`]：主题、比例、字体角色与素材解析只有一处，
//! 因此缩略图不可能与导出页在「这张图片/这个主题指向什么」上不一致。
//! 渲染是同步的重活（编译 + 光栅化），调用方（命令层或将来的 GPUI 线程）负责放到后台线程。

use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::Instant;

use base64::Engine;
use serde::Serialize;
use slide_typst::{Assets, ExportOptions, FontRoles, Renderer};

use crate::assets;
use crate::db::{self, MediaAsset};
use crate::state::SharedState;
use crate::types::{AspectRatio, FontSettings, SlideBlueprint};

use super::Result;

/// Scanning system fonts takes ~0.5 s; do it once per process, on first export. Hosts
/// warm it in the background so the first thumbnail does not wait for it.
pub static RENDERER: LazyLock<Renderer> = LazyLock::new(Renderer::with_system_fonts);

pub fn renderer() -> &'static Renderer {
    &RENDERER
}

#[derive(Debug, Serialize)]
pub struct PlaceholderSlide {
    pub index: usize,
    pub kind: String,
}

#[derive(Debug, Serialize)]
pub struct PdfExportReport {
    pub path: String,
    pub pages: usize,
    /// Slides whose template has no PDF layout yet (rendered as placeholder pages).
    pub placeholder_slides: Vec<PlaceholderSlide>,
    /// Slides whose content was clipped because it did not fit even when shrunk.
    pub overflow_slides: Vec<usize>,
    /// Image references shown as placeholders, with the reason.
    pub missing_assets: Vec<slide_typst::MissingAsset>,
    /// The requested theme id when it has no PDF layout; the export fell back
    /// to light-notes. `None` when the requested theme rendered as-is.
    pub theme_fallback: Option<String>,
    pub elapsed_ms: u128,
}

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

/// Resolves the requested theme to one with a Typst layout. Unknown ids fall
/// back to light-notes (the Typst side would fall back too, silently — see
/// `themes` in `typst/main.typ`); the second return value names the requested
/// id so the report can surface the fallback.
fn resolve_theme(theme: Option<String>) -> (String, Option<String>) {
    let requested = theme.unwrap_or_else(|| "light-notes".to_string());
    if slide_typst::SUPPORTED_THEMES.contains(&requested.as_str()) {
        (requested, None)
    } else {
        ("light-notes".to_string(), Some(requested))
    }
}

/// Exports `slides` (the editor's current deck) or, when absent, the project's saved
/// blueprints to a PDF at `path`. Charts are drawn by Typst itself (`lib/chart.typ`).
pub fn export_pdf(
    state: &SharedState,
    project_id: Option<i64>,
    slides: Option<Vec<SlideBlueprint>>,
    path: String,
    theme: Option<String>,
) -> Result<PdfExportReport> {
    let (slides, options, theme_fallback) = prepare_export(state, project_id, slides, theme)?;
    render_to_file(&slides, &options, Path::new(&path), theme_fallback)
}

/// Resolves the deck, theme, fonts, aspect ratio and images of an export. Also reused by
/// [`render_thumbnails`], so a thumbnail and the exported page can never resolve a theme,
/// aspect ratio or image reference differently.
pub fn prepare_export(
    state: &SharedState,
    project_id: Option<i64>,
    slides: Option<Vec<SlideBlueprint>>,
    theme: Option<String>,
) -> Result<(Vec<serde_json::Value>, ExportOptions, Option<String>)> {
    let (slides, project, media_assets, media_dir, project_dir, fonts) = {
        let state = state.lock().unwrap();
        let db = state.db.lock().unwrap();
        let project = match project_id {
            Some(id) => Some(db::get_project(&db, id).map_err(|e| e.to_string())?),
            None => None,
        };
        let slides = match (slides, &project) {
            (Some(slides), _) => slides,
            (None, Some(project)) if !project.blueprints_json.trim().is_empty() => {
                serde_json::from_str(&project.blueprints_json)
                    .map_err(|e| format!("读取幻灯片失败：{e}"))?
            }
            _ => return Err("没有可导出的幻灯片".to_string()),
        };
        let media_assets = match project_id {
            Some(id) => db::get_media_assets_for_project(&db, id).map_err(|e| e.to_string())?,
            None => Vec::new(),
        };
        (
            slides,
            project,
            media_assets,
            PathBuf::from(&state.app_settings.media_dir),
            state.project_dir.clone(),
            state.app_settings.fonts.clone(),
        )
    };
    if slides.is_empty() {
        return Err("没有可导出的幻灯片".to_string());
    }

    let aspect = project
        .as_ref()
        .and_then(|p| p.aspect_ratio)
        .or_else(|| slides.first().and_then(|s| s.aspect_ratio))
        .unwrap_or(AspectRatio::Ratio16x9);
    let (theme, theme_fallback) = resolve_theme(theme);
    let options = ExportOptions {
        theme,
        aspect_ratio: serde_json::to_value(aspect)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_else(|| "ratio_16x9".to_string()),
        fonts: font_roles(&fonts),
        title: project.as_ref().map(|p| p.name.clone()),
        assets: export_assets(&project_dir, &media_dir, &media_assets),
    };
    let slides: Vec<serde_json::Value> = slides
        .iter()
        .map(serde_json::to_value)
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    Ok((slides, options, theme_fallback))
}

fn render_to_file(
    slides: &[serde_json::Value],
    options: &ExportOptions,
    path: &Path,
    theme_fallback: Option<String>,
) -> Result<PdfExportReport> {
    let started = Instant::now();
    let rendered = renderer()
        .render_pdf(slides, options)
        .map_err(|e| e.to_string())?;
    std::fs::write(path, &rendered.pdf)
        .map_err(|e| format!("写入 {} 失败：{e}", path.display()))?;
    let placeholder_slides = slides
        .iter()
        .enumerate()
        .filter_map(|(index, slide)| {
            let kind = slide.get("kind")?.as_str()?;
            (!slide_typst::SUPPORTED_KINDS.contains(&kind)).then(|| PlaceholderSlide {
                index,
                kind: kind.to_string(),
            })
        })
        .collect();
    let mut overflow_slides: Vec<usize> =
        rendered.overflows.iter().filter_map(|o| o.slide).collect();
    overflow_slides.dedup();
    Ok(PdfExportReport {
        path: path.display().to_string(),
        pages: rendered.pages,
        placeholder_slides,
        overflow_slides,
        missing_assets: rendered.missing_assets,
        theme_fallback,
        elapsed_ms: started.elapsed().as_millis(),
    })
}

/// Rasterizes `slides` (the deck currently on screen) into PNG thumbnails.
///
/// `only` limits the rasterized pages; positions refer to `slides`.
pub fn render_slide_thumbnails(
    state: &SharedState,
    project_id: Option<i64>,
    slides: Vec<SlideBlueprint>,
    theme: Option<String>,
    width_px: u32,
    only: Option<Vec<usize>>,
) -> Result<ThumbnailBatch> {
    if slides.is_empty() {
        return Ok(ThumbnailBatch {
            pages: Vec::new(),
            theme_fallback: None,
            elapsed_ms: 0,
        });
    }
    let (slides, options, theme_fallback) = prepare_export(state, project_id, Some(slides), theme)?;
    render_thumbnails(&slides, &options, width_px, only.as_deref(), theme_fallback)
}

fn render_thumbnails(
    slides: &[serde_json::Value],
    options: &ExportOptions,
    width_px: u32,
    only: Option<&[usize]>,
    theme_fallback: Option<String>,
) -> Result<ThumbnailBatch> {
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

/// The user's slide font per role, each followed by CJK fallbacks.
fn font_roles(fonts: &FontSettings) -> FontRoles {
    FontRoles {
        title: FontRoles::with_fallbacks(&fonts.title),
        subtitle: FontRoles::with_fallbacks(&fonts.subtitle),
        body: FontRoles::with_fallbacks(&fonts.body),
        caption: FontRoles::with_fallbacks(&fonts.caption),
        number: FontRoles::with_fallbacks(&fonts.number),
        code: FontRoles::with_fallbacks(&fonts.code),
    }
}

/// Every reference a blueprint may use, mapped to its file: the same spellings the
/// generator validates against (`assets::AssetCatalog`), so an image that passes
/// generation also resolves here.
fn export_assets(project_dir: &Path, media_dir: &Path, media_assets: &[MediaAsset]) -> Assets {
    let mut assets = Assets::new();
    for (reference, path) in assets::project_file_refs(project_dir) {
        assets.insert_path(reference, path);
    }
    for asset in media_assets.iter().filter(|a| a.media_type == "image") {
        assets.insert_path(
            format!("media:{}", asset.id),
            media_dir.join(&asset.storage_path),
        );
    }
    assets
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_theme_falls_back_to_notes_and_is_reported() {
        assert_eq!(resolve_theme(None), ("light-notes".to_string(), None));
        assert_eq!(
            resolve_theme(Some("dark-blue".to_string())),
            ("dark-blue".to_string(), None)
        );
        assert_eq!(
            resolve_theme(Some("no-such-theme".to_string())),
            ("light-notes".to_string(), Some("no-such-theme".to_string()))
        );
    }

    #[test]
    fn export_resolves_project_files_and_media_library_images() {
        let root = std::env::temp_dir().join(format!("pdf-export-{}", std::process::id()));
        let png = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../slide-typst/tests/fixtures/assets/arch.png"
        ))
        .unwrap();
        std::fs::create_dir_all(root.join("project/figure/sub")).unwrap();
        std::fs::create_dir_all(root.join("media/projects/1/images")).unwrap();
        std::fs::write(root.join("project/figure/sub/a.png"), &png).unwrap();
        std::fs::write(root.join("media/projects/1/images/7.png"), &png).unwrap();

        let media = MediaAsset {
            id: 7,
            project_id: 1,
            slide_index: None,
            filename: "7.png".into(),
            original_name: "架构.png".into(),
            media_type: "image".into(),
            mime_type: Some("image/png".into()),
            storage_path: "projects/1/images/7.png".into(),
            caption: None,
            description: None,
            file_size: None,
            width: None,
            height: None,
            created_at: 0,
        };
        let options = ExportOptions {
            assets: export_assets(&root.join("project"), &root.join("media"), &[media]),
            ..Default::default()
        };
        // Every spelling the generator accepts must resolve; a kind without a PDF
        // layout (here a template a newer version might add) must be reported
        // as a placeholder page.
        let slides = vec![
            serde_json::json!({ "kind": "spotlight", "title": "项目文件", "images": ["figure/sub/a.png", "/figure/sub/a.png", "sub/a.png"] }),
            serde_json::json!({ "kind": "spotlight", "title": "素材库", "image": "media:7" }),
            serde_json::json!({ "kind": "future_kind", "title": "未移植" }),
        ];
        let out = root.join("deck.pdf");
        let report = render_to_file(&slides, &options, &out, None).unwrap();

        assert!(std::fs::read(&out).unwrap().starts_with(b"%PDF"));
        assert_eq!(report.pages, 3);
        assert!(
            report.missing_assets.is_empty(),
            "{:?}",
            report.missing_assets
        );
        assert_eq!(report.placeholder_slides.len(), 1);
        assert_eq!(report.placeholder_slides[0].kind, "future_kind");
        std::fs::remove_dir_all(&root).ok();
    }

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
