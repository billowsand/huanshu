//! Native PDF export through `crates/slide-typst` (embedded Typst compiler).
//!
//! The renderer re-implements slide layouts in Typst; kinds it does not cover yet
//! render as a visible placeholder page, and the report tells the UI which ones.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use serde::Serialize;
use slide_typst::{Assets, ExportOptions, FontRoles, Renderer};
use tauri::State;

use crate::db::{self, MediaAsset};
use crate::types::{AspectRatio, FontSettings, SlideBlueprint};
use crate::AppState;

/// Scanning system fonts takes ~0.5 s; do it once per process, on first export.
fn renderer() -> &'static Renderer {
    static RENDERER: OnceLock<Renderer> = OnceLock::new();
    RENDERER.get_or_init(Renderer::with_system_fonts)
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

/// A chart_focus slide's SSR SVG, rendered in the app webview with the same
/// ECharts option builder as the on-screen preview (static mode).
#[derive(Debug, serde::Deserialize)]
pub struct ChartSvgInput {
    /// Index into the exported `slides` array; the asset pipe keys it `chart:<index>`.
    pub index: usize,
    pub svg: String,
}

/// Exports `slides` (the editor's current deck) or, when absent, the project's saved
/// blueprints to a PDF at `path`.
///
/// chart_focus slides need `chart_svgs`: the frontend first asks `pdf_chart_boxes` for each
/// chart's plot box, renders the SVGs at exactly that size, then calls this.
#[tauri::command]
pub async fn export_pdf(
    project_id: Option<i64>,
    slides: Option<Vec<SlideBlueprint>>,
    path: String,
    theme: Option<String>,
    chart_svgs: Option<Vec<ChartSvgInput>>,
    state: State<'_, Mutex<AppState>>,
) -> Result<PdfExportReport, String> {
    let (slides, options, theme_fallback) =
        prepare_export(project_id, slides, theme, chart_svgs.as_deref(), true, &state)?;
    tauri::async_runtime::spawn_blocking(move || {
        render_to_file(&slides, &options, Path::new(&path), theme_fallback)
    })
    .await
    .map_err(|e| format!("导出任务异常结束：{e}"))?
}

/// Plot box (CSS px) and type scale of every chart_focus slide, as `export_pdf` would lay
/// them out with the same arguments. Indices refer to `slides` (or the saved deck).
#[tauri::command]
pub async fn pdf_chart_boxes(
    project_id: Option<i64>,
    slides: Option<Vec<SlideBlueprint>>,
    theme: Option<String>,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<slide_typst::ChartBox>, String> {
    // Images are not needed: only chart_focus pages are laid out, and they show none.
    let (slides, options, _) = prepare_export(project_id, slides, theme, None, false, &state)?;
    tauri::async_runtime::spawn_blocking(move || measure_chart_boxes(&slides, &options))
        .await
        .map_err(|e| format!("图表测量任务异常结束：{e}"))?
}

/// Each page lays out independently, so only the chart pages are compiled (fast), with their
/// boxes mapped back to deck indices.
fn measure_chart_boxes(
    slides: &[serde_json::Value],
    options: &ExportOptions,
) -> Result<Vec<slide_typst::ChartBox>, String> {
    let indices: Vec<usize> = slides
        .iter()
        .enumerate()
        .filter(|(_, s)| s.get("kind").and_then(|k| k.as_str()) == Some("chart_focus"))
        .map(|(i, _)| i)
        .collect();
    if indices.is_empty() {
        return Ok(Vec::new());
    }
    let subset: Vec<serde_json::Value> = indices.iter().map(|&i| slides[i].clone()).collect();
    let boxes = renderer()
        .chart_boxes(&subset, options)
        .map_err(|e| e.to_string())?;
    Ok(boxes
        .into_iter()
        .filter_map(|mut b| {
            b.slide = *indices.get(b.slide)?;
            Some(b)
        })
        .collect())
}

/// Resolves the deck, theme, fonts and aspect ratio shared by the measuring pass and the
/// export, so both lay the charts out identically.
fn prepare_export(
    project_id: Option<i64>,
    slides: Option<Vec<SlideBlueprint>>,
    theme: Option<String>,
    chart_svgs: Option<&[ChartSvgInput]>,
    with_images: bool,
    state: &State<'_, Mutex<AppState>>,
) -> Result<(Vec<serde_json::Value>, ExportOptions, Option<String>), String> {
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
        assets: if with_images {
            export_assets(&project_dir, &media_dir, &media_assets, chart_svgs)
        } else {
            Assets::new()
        },
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
) -> Result<PdfExportReport, String> {
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
/// generation also resolves here. Chart SVGs rendered by the app webview are keyed
/// `chart:<slide index>` and win over any file-based spelling of the same name.
fn export_assets(
    project_dir: &Path,
    media_dir: &Path,
    media_assets: &[MediaAsset],
    chart_svgs: Option<&[ChartSvgInput]>,
) -> Assets {
    let mut assets = Assets::new();
    for (reference, path) in crate::assets::project_file_refs(project_dir) {
        assets.insert_path(reference, path);
    }
    for asset in media_assets.iter().filter(|a| a.media_type == "image") {
        assets.insert_path(
            format!("media:{}", asset.id),
            media_dir.join(&asset.storage_path),
        );
    }
    for chart in chart_svgs.unwrap_or(&[]) {
        assets.insert_bytes(
            slide_typst::chart_asset_ref(chart.index),
            chart.svg.clone().into_bytes(),
        );
    }
    assets
}

/// Selects the exported file in the system file manager.
#[tauri::command]
pub fn reveal_exported_file(path: String) -> Result<(), String> {
    let path = PathBuf::from(path);
    if !path.is_file() {
        return Err("文件不存在".to_string());
    }
    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("explorer")
        .arg("/select,")
        .arg(&path)
        .spawn();
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open")
        .arg("-R")
        .arg(&path)
        .spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let result = std::process::Command::new("xdg-open")
        .arg(path.parent().unwrap_or(Path::new(".")))
        .spawn();
    result.map(|_| ()).map_err(|e| e.to_string())
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
            "/../crates/slide-typst/tests/fixtures/assets/arch.png"
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
            assets: export_assets(&root.join("project"), &root.join("media"), &[media], None),
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
    fn chart_boxes_map_back_to_deck_indices() {
        let chart = serde_json::json!({
            "kind": "chart_focus",
            "title": "交付量",
            "points": [{ "step": "01", "title": "趋势", "body": "逐季上升" }],
            "chart": {
                "type": "bar", "title": "季度交付量",
                "data": [{ "q": "Q1", "n": 12 }],
                "encoding": { "x": "q", "y": "n" }
            }
        });
        let slides = vec![
            serde_json::json!({ "kind": "cover", "title": "封面" }),
            chart.clone(),
            serde_json::json!({ "kind": "feature_grid", "title": "能力" }),
            chart,
        ];
        let boxes = measure_chart_boxes(&slides, &ExportOptions::default()).unwrap();
        assert_eq!(boxes.iter().map(|b| b.slide).collect::<Vec<_>>(), vec![1, 3]);
        assert_eq!((boxes[0].width, boxes[0].height), (boxes[1].width, boxes[1].height));
        assert!(measure_chart_boxes(&slides[..1], &ExportOptions::default())
            .unwrap()
            .is_empty());
    }
}
