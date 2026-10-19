//! PDF 导出与缩略图光栅化（都走 `slide-typst` 的进程内 Typst 编译器）。
//!
//! 缩略图与导出共用 [`prepare_export`]：主题、比例、字体角色与素材解析只有一处，
//! 因此缩略图不可能与导出页在「这张图片/这个主题指向什么」上不一致。
//! 渲染是同步的重活（编译 + 光栅化），调用方（命令层或将来的 GPUI 线程）负责放到后台线程。

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::Instant;

use base64::Engine;
use serde::Serialize;
use slide_typst::{Assets, ExportOptions, FontRoles, Renderer};
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

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
    /// to [`FALLBACK_THEME`]. `None` when the requested theme rendered as-is.
    pub theme_fallback: Option<String>,
    pub elapsed_ms: u128,
}

/// Report of a complete Typst project export (a zip the user can compile with
/// `typst compile main.typ`).
#[derive(Debug, Serialize)]
pub struct TypstProjectReport {
    pub path: String,
    pub pages: usize,
    /// Entries in the archive (sources, `deck.json`, assets, icons, README).
    pub files: usize,
    /// Font families the project asks Typst for, in role order, unique.
    pub fonts: Vec<String>,
    /// Entries of `fonts` that are not installed on this machine. They are kept
    /// in the project: a system that has them renders it like the app, and a
    /// system without them only makes Typst fall back (with a warning).
    pub missing_fonts: Vec<String>,
    /// Slides whose template has no Typst layout (rendered as placeholder pages).
    pub placeholder_slides: Vec<PlaceholderSlide>,
    /// Slides whose content was clipped because it did not fit even when shrunk.
    pub overflow_slides: Vec<usize>,
    pub missing_assets: Vec<slide_typst::MissingAsset>,
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

/// 没有 PDF 版式的主题回落到它（Typst 侧同样回落到 `themes.light-notes`，见 `typst/main.typ`）。
/// 报告里的 `theme_fallback` 是**请求的** id；这个常量是实际用的那套版式，
/// 界面要写出「已回落到「便签·稿纸」」就得知道它是谁。
pub const FALLBACK_THEME: &str = "light-notes";

/// Resolves the requested theme to one with a Typst layout. Unknown ids fall
/// back to [`FALLBACK_THEME`] (the Typst side would fall back too, silently — see
/// `themes` in `typst/main.typ`); the second return value names the requested
/// id so the report can surface the fallback.
fn resolve_theme(theme: Option<String>) -> (String, Option<String>) {
    let requested = theme.unwrap_or_else(|| FALLBACK_THEME.to_string());
    if slide_typst::SUPPORTED_THEMES.contains(&requested.as_str()) {
        (requested, None)
    } else {
        (FALLBACK_THEME.to_string(), Some(requested))
    }
}

/// Exports `slides` (the editor's current deck) or, when absent, the project's saved
/// blueprints to a PDF at `path`. Charts are drawn by Typst itself (`lib/chart.typ`).
pub fn export_pdf(
    state: &SharedState,
    project_id: Option<i64>,
    slides: Option<Vec<serde_json::Value>>,
    path: String,
    theme: Option<String>,
) -> Result<PdfExportReport> {
    let (slides, options, theme_fallback) =
        prepare_export_values(state, project_id, slides, theme)?;
    render_to_file(&slides, &options, Path::new(&path), theme_fallback)
}

/// Resolves the deck, theme, fonts, aspect ratio and images of an export. Also reused by
/// [`render_thumbnails`], so a thumbnail and the exported page can never resolve a theme,
/// aspect ratio or image reference differently.
pub fn prepare_export_values(
    state: &SharedState,
    project_id: Option<i64>,
    slides: Option<Vec<serde_json::Value>>,
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

    // Known layouts obey the same contract as the form/JSON editor. Unknown layouts
    // still reach the renderer's explicit diagnostic page.
    let schema = crate::editor_schema::editor_schema();
    for (i, slide) in slides.iter().enumerate() {
        if slide
            .get("kind")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|kind| schema.kind(kind).is_some())
        {
            let errors = crate::slide_contract::errors(&schema, slide, false);
            if !errors.is_empty() {
                return Err(format!("第 {} 页：{}", i + 1, errors.join("；")));
            }
        }
    }

    // 页面比例从 JSON 的同一个键读（强类型那版读 `SlideBlueprint.aspect_ratio`）。
    let slide_ratio = slides.first().and_then(|slide| {
        slide
            .get("aspect_ratio")
            .and_then(serde_json::Value::as_str)
            .and_then(|value| serde_json::from_value::<AspectRatio>(value.into()).ok())
    });
    let aspect = project
        .as_ref()
        .and_then(|p| p.aspect_ratio)
        .or(slide_ratio)
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
    Ok((slides, options, theme_fallback))
}

/// Typed entry point for the Tauri command layer; the deck itself is handled as JSON.
pub fn prepare_export(
    state: &SharedState,
    project_id: Option<i64>,
    slides: Option<Vec<SlideBlueprint>>,
    theme: Option<String>,
) -> Result<(Vec<serde_json::Value>, ExportOptions, Option<String>)> {
    let slides = match slides {
        Some(slides) => Some(
            slides
                .iter()
                .map(serde_json::to_value)
                .collect::<Result<Vec<serde_json::Value>, _>>()
                .map_err(|e| e.to_string())?,
        ),
        None => None,
    };
    prepare_export_values(state, project_id, slides, theme)
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
    let placeholder_slides = placeholder_slides(slides);
    let overflow_slides = overflow_slides(&rendered.overflows);
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

/// Slides whose template has no Typst layout; they render as visible placeholder
/// pages in both the PDF and the exported project.
fn placeholder_slides(slides: &[serde_json::Value]) -> Vec<PlaceholderSlide> {
    slides
        .iter()
        .enumerate()
        .filter_map(|(index, slide)| {
            let kind = slide.get("kind")?.as_str()?;
            (!slide_typst::SUPPORTED_KINDS.contains(&kind)).then(|| PlaceholderSlide {
                index,
                kind: kind.to_string(),
            })
        })
        .collect()
}

/// One-based page indices of the slides clipped by the fit search, deduplicated.
fn overflow_slides(overflows: &[slide_typst::Overflow]) -> Vec<usize> {
    let mut slides: Vec<usize> = overflows
        .iter()
        .filter_map(|overflow| overflow.slide)
        .collect();
    slides.dedup();
    slides
}

/// Exports the deck as a complete, standalone Typst project in a zip: the same
/// theme, aspect ratio, fonts and images as [`export_pdf`], but as editable
/// sources instead of a rendered PDF.
///
/// Slides (the editor's current deck) are used when present; otherwise the
/// project's saved blueprints are read. The archive unpacks to one directory and
/// is compiled with "typst compile main.typ deck.pdf" (see its README.md).
pub fn export_typst_project(
    state: &SharedState,
    project_id: Option<i64>,
    slides: Option<Vec<serde_json::Value>>,
    path: String,
    theme: Option<String>,
) -> Result<TypstProjectReport> {
    let (slides, options, theme_fallback) =
        prepare_export_values(state, project_id, slides, theme)?;
    write_project_archive(&slides, &options, Path::new(&path), theme_fallback)
}

/// Writes the project archive. Split from [`export_typst_project`] so tests can
/// give it a deck and options without a database.
pub(crate) fn write_project_archive(
    slides: &[serde_json::Value],
    options: &ExportOptions,
    path: &Path,
    theme_fallback: Option<String>,
) -> Result<TypstProjectReport> {
    let started = Instant::now();
    let project = renderer()
        .export_project(slides, options)
        .map_err(|error| error.to_string())?;
    let readme = project_readme(options, &project, theme_fallback.as_deref());
    let root = archive_root(path);

    let file = std::fs::File::create(path)
        .map_err(|error| format!("创建 {} 失败：{error}", path.display()))?;
    let mut zip = ZipWriter::new(file);
    let entry = SimpleFileOptions::default().unix_permissions(0o644);
    // A visible directory for optional font files (see README).
    zip.add_directory(format!("{root}/fonts/"), entry)
        .map_err(|error| format!("写入 {} 失败：{error}", path.display()))?;
    let mut files = 0usize;
    for project_file in &project.files {
        let name = format!("{root}/{}", project_file.path);
        zip.start_file(&name, entry)
            .map_err(|error| format!("写入 {name} 失败：{error}"))?;
        zip.write_all(&project_file.bytes)
            .map_err(|error| format!("写入 {name} 失败：{error}"))?;
        files += 1;
    }
    let readme_name = format!("{root}/README.md");
    zip.start_file(&readme_name, entry)
        .map_err(|error| format!("写入 {readme_name} 失败：{error}"))?;
    zip.write_all(readme.as_bytes())
        .map_err(|error| format!("写入 {readme_name} 失败：{error}"))?;
    files += 1;
    zip.finish()
        .map_err(|error| format!("完成 {} 失败：{error}", path.display()))?;

    Ok(TypstProjectReport {
        path: path.display().to_string(),
        pages: project.pages,
        files,
        fonts: project.fonts,
        missing_fonts: project.unavailable_fonts,
        placeholder_slides: placeholder_slides(slides),
        overflow_slides: overflow_slides(&project.overflows),
        missing_assets: project.missing_assets,
        theme_fallback,
        elapsed_ms: started.elapsed().as_millis(),
    })
}

/// The zip's single top-level directory, named after the chosen file
/// (deck.zip becomes deck/). Extraction then never spills files into the folder.
fn archive_root(path: &Path) -> String {
    let stem = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().trim().to_string())
        .unwrap_or_default();
    let clean: String = stem
        .chars()
        .filter(|ch| !matches!(ch, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        .collect();
    if clean.is_empty() {
        "typst-project".to_string()
    } else {
        clean
    }
}

/// ratio_16x9 becomes 16:9, for the README.
fn aspect_label(value: &str) -> &'static str {
    match value {
        "ratio_32x9" => "32:9",
        "ratio_48x9" => "48:9",
        _ => "16:9",
    }
}

/// The README shipped inside the archive: how to compile it, which fonts it
/// expects, and what each directory holds. Fonts are the one thing the archive
/// cannot carry (licensing and size), so it names the whole fallback chain and
/// the optional fonts/ directory.
fn project_readme(
    options: &ExportOptions,
    project: &slide_typst::TypstProject,
    theme_fallback: Option<&str>,
) -> String {
    let title = options.title.as_deref().unwrap_or("演示文稿");
    let theme = crate::themes::theme_name(&options.theme);
    let mut out = String::new();
    out.push_str(&format!("# {title} · Typst 工程\n\n"));
    out.push_str(
        r#"由幻述（huanshu）导出的完整 Typst 工程：`main.typ`、全部版式与主题、幻灯片数据、图片素材，以及按当前牌组用到的图标与颜色预先着色的 SVG。排版结果与应用内的 PDF 导出一致。

## 编译

需要 Typst **0.15.x**（本工程按 0.15.1 的模板语法编写）。在本目录下执行：

```bash
typst compile main.typ deck.pdf
```

模板内部全部使用相对路径（`deck.json`、`../vendor/…`、`../icons/…`、`../assets/…`），因此在任何目录执行 `typst compile <路径>/main.typ` 都可以，不需要 `--root`。

"#,
    );
    out.push_str(&format!(
        "画布比例：{}；主题：{}。\n\n## 字体\n\n",
        aspect_label(&options.aspect_ratio),
        theme
    ));
    out.push_str(
        r#"工程按「设置 → 外观与字体」的字体设置排版。`deck.json` 的 `fonts` 保存每个角色的完整回退链（同一角色的字族依次尝试），数学公式的候选字族在 `math_fonts` 里：

| 用途 | 字体（按回退顺序） |
| --- | --- |
"#,
    );
    // The math chain is deck.json's `math_fonts`: the configured family plus every
    // known OpenType MATH family, so the README says what Typst actually tries.
    let mut math_chain: Vec<String> = Vec::new();
    for family in options
        .fonts
        .math
        .iter()
        .map(String::as_str)
        .chain(slide_typst::MATH_FONTS.iter().copied())
    {
        let family = family.trim();
        if !family.is_empty()
            && !math_chain
                .iter()
                .any(|known| known.eq_ignore_ascii_case(family))
        {
            math_chain.push(family.to_string());
        }
    }
    for (label, list) in [
        ("标题", &options.fonts.title),
        ("副标题", &options.fonts.subtitle),
        ("正文", &options.fonts.body),
        ("注释与标签", &options.fonts.caption),
        ("数字与序号", &options.fonts.number),
        ("代码", &options.fonts.code),
        ("数学公式", &math_chain),
    ] {
        let chain = list
            .iter()
            .map(|family| format!("`{family}`"))
            .collect::<Vec<_>>()
            .join(" → ");
        out.push_str(&format!("| {label} | {chain} |\n"));
    }
    out.push_str("\n");
    if project.unavailable_fonts.is_empty() {
        out.push_str(
            "这些字体在导出所用的这台机器上都能找到。编译本工程的目标系统也需要安装同样的字体（Typst 会自动扫描系统字体）。\n\n",
        );
    } else {
        let missing = project
            .unavailable_fonts
            .iter()
            .map(|family| format!("`{family}`"))
            .collect::<Vec<_>>()
            .join("、");
        out.push_str(&format!(
            "导出所用的这台机器上未安装：{missing}。目标系统如果安装了这些字体，就会按上面的设置排版；缺失的字体不会导致编译失败（Typst 会回落到默认字体），但字形与换行可能与应用内预览不同。\n\n",
        ));
    }
    out.push_str(
        "（编译时若出现 unknown font family 警告，那是回退链里本机没装的备选字体；Typst 会继续尝试后面的字体，不影响编译。）\n\n",
    );
    out.push_str(
        r#"字体文件不在系统字体目录时，也可以把它们放进本目录的 `fonts/` 再指定路径：

```bash
typst compile --font-path fonts main.typ deck.pdf
```

## 目录结构

- `main.typ`：入口（分发版式、设置页面尺寸与字体）
- `deck.json`：幻灯片数据（主题、比例、字体、素材映射与每页内容）
- `lib/`：公共组件、图表与自适应缩放
- `themes/`：主题 token 与页面背景
- `layouts/`：每种版式一个文件
- `vendor/mitex/`：数学公式（LaTeX → Typst）规则
- `assets/`：图片素材（spotlight 版式按相对 `layouts/` 的 `../assets/<n>.<ext>` 引用）
- `icons/`：按当前牌组用到的图标与颜色生成的 SVG
- `fonts/`：可选的字体目录（见上）

## 说明

- 图标按导出时的牌组与主题预先着色；改了 `deck.json` 里的图标名、颜色或主题后，重新导出一次工程即可。
- 修改 `deck.json` 的 `aspect_ratio`（`ratio_16x9` / `ratio_32x9` / `ratio_48x9`）即可换画布比例。
- 未知图标回落到 `circle-dash`；缺失的图片显示占位框，都不会让编译失败。
"#,
    );
    if let Some(requested) = theme_fallback {
        out.push_str(&format!(
            "\n> 注意：请求的主题 `{requested}` 还没有 Typst 版式，本工程已回落到「{theme}」。\n"
        ));
    }
    out
}

/// Rasterizes [`slides`] (the deck currently on screen) into PNG thumbnails.
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
pub fn font_roles(fonts: &FontSettings) -> FontRoles {
    FontRoles {
        math: vec![fonts.math.clone()],
        title: FontRoles::with_custom_fallback(&fonts.title, &fonts.fallback),
        subtitle: FontRoles::with_custom_fallback(&fonts.subtitle, &fonts.fallback),
        body: FontRoles::with_custom_fallback(&fonts.body, &fonts.fallback),
        caption: FontRoles::with_custom_fallback(&fonts.caption, &fonts.fallback),
        number: FontRoles::with_custom_fallback(&fonts.number, &fonts.fallback),
        code: FontRoles::with_custom_fallback(&fonts.code, &fonts.fallback),
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
    fn font_settings_keep_new_roles_and_fallback_across_save() {
        let mut fonts: FontSettings = serde_json::from_value(serde_json::json!({
            "code": "Consolas", "body": "SimSun"
        }))
        .unwrap();
        assert_eq!(fonts.editor, slide_typst::DEFAULT_FONT);
        assert_eq!(fonts.math, slide_typst::DEFAULT_MATH_FONT);
        fonts.editor = "KaiTi".into();
        fonts.fallback = "DengXian".into();
        fonts.math = "STIX Two Math".into();
        let restored: FontSettings =
            serde_json::from_str(&serde_json::to_string(&fonts).unwrap()).unwrap();
        assert_eq!(restored, fonts);
        let roles = font_roles(&restored);
        for list in [
            &roles.title,
            &roles.subtitle,
            &roles.body,
            &roles.caption,
            &roles.number,
            &roles.code,
        ] {
            assert_eq!(list[1], "DengXian");
        }
        assert_eq!(roles.math, ["STIX Two Math"]);
        assert_eq!(roles.body[0], "SimSun");
    }

    #[test]
    fn unknown_theme_falls_back_to_notes_and_is_reported() {
        assert_eq!(resolve_theme(None), (FALLBACK_THEME.to_string(), None));
        assert_eq!(
            resolve_theme(Some("dark-blue".to_string())),
            ("dark-blue".to_string(), None)
        );
        assert_eq!(
            resolve_theme(Some("no-such-theme".to_string())),
            (
                FALLBACK_THEME.to_string(),
                Some("no-such-theme".to_string())
            )
        );
    }

    /// 回落目标必须是真主题：界面要写「已回落到「便签·稿纸」」，靠的是它的中文名；
    /// 同时它得有 Typst 版式，否则回落之后还是占位页。
    #[test]
    fn the_fallback_theme_has_a_name_and_a_typst_layout() {
        assert!(
            slide_typst::SUPPORTED_THEMES.contains(&FALLBACK_THEME),
            "{FALLBACK_THEME} 没有 Typst 版式"
        );
        let name = crate::themes::theme_name(FALLBACK_THEME);
        assert_ne!(name, FALLBACK_THEME, "{FALLBACK_THEME} 没有中文名");
        assert!(crate::themes::is_known_theme(FALLBACK_THEME));
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

    /// 同一页、同一宽度，像素与 PNG 解码后必须逐点相同：两条路只差一个编码器，
    /// 界面上看到的两者不能有任何差别。
    #[test]
    fn pixels_match_the_png_decoded_byte_for_byte() {
        let slide = serde_json::json!({
            "kind": "feature_grid",
            "title": "像素一致性",
            "cards": [{ "title": "卡片", "body": "正文", "icon": "i-carbon:square", "tone": "blue" }],
        });
        let options = ExportOptions::default();
        let width = 320;

        let png = renderer()
            .render_png(std::slice::from_ref(&slide), &options, width, Some(&[0]))
            .unwrap();
        let pixels = renderer()
            .render_pixels(std::slice::from_ref(&slide), &options, width, Some(&[0]))
            .unwrap();
        let page = pixels.pages.first().expect("one page");

        let decoded =
            image::load_from_memory_with_format(&png.pages[0].png, image::ImageFormat::Png)
                .expect("the PNG decodes")
                .to_rgba8();
        assert_eq!(
            (page.width, page.height),
            (decoded.width(), decoded.height()),
            "the two paths must agree on the page size"
        );
        assert_eq!(decoded.as_raw().len(), page.pixels.len());
        let mismatches = decoded
            .as_raw()
            .iter()
            .zip(&page.pixels)
            .filter(|(left, right)| **left != **right)
            .count();
        assert_eq!(
            mismatches, 0,
            "{mismatches} bytes differ between pixels and PNG"
        );
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

    /// The project archive is a self-contained Typst project: sources,
    /// deck.json, the fonts it expects (unfiltered), a README and the optional
    /// fonts/ directory, all under one top-level folder.
    #[test]
    fn project_archive_contains_sources_readme_and_fonts() {
        use std::io::Read;

        let root = std::env::temp_dir().join(format!("typst-project-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("deck.zip");
        let slides = vec![
            serde_json::json!({ "kind": "cover", "title": "封面" }),
            serde_json::json!({
                "kind": "feature_grid",
                "title": "功能",
                "cards": [{ "title": "一", "body": "正文", "icon": "i-carbon:data-base", "tone": "blue" }],
            }),
            serde_json::json!({ "kind": "closing", "title": "结束" }),
        ];
        let options = ExportOptions {
            title: Some("归档测试".to_string()),
            // A family no machine has: it must be kept in deck.json and reported,
            // not dropped like the in-process PDF compile drops it.
            fonts: FontRoles::uniform("No Such Font For Project Test"),
            ..Default::default()
        };
        let report = write_project_archive(&slides, &options, &path, None).unwrap();
        assert_eq!(report.pages, 3);
        assert!(report.files > 20, "only {} files", report.files);
        assert!(report
            .fonts
            .contains(&"No Such Font For Project Test".to_string()));
        assert!(report
            .missing_fonts
            .contains(&"No Such Font For Project Test".to_string()));
        assert!(report.missing_assets.is_empty());
        assert!(report.placeholder_slides.is_empty());

        let mut archive = zip::ZipArchive::new(std::fs::File::open(&path).unwrap()).unwrap();
        let names: Vec<String> = (0..archive.len())
            .map(|index| archive.by_index(index).unwrap().name().to_string())
            .collect();
        for expected in ["main.typ", "deck.json", "README.md", "fonts/"] {
            let name = format!("deck/{expected}");
            assert!(
                names.iter().any(|entry| entry == &name),
                "missing {name}: {names:?}"
            );
        }
        assert!(
            names
                .iter()
                .any(|name| name.starts_with("deck/icons/carbon/")),
            "no tinted icons: {names:?}"
        );
        assert!(
            names.iter().all(|name| name.starts_with("deck/")),
            "archive must keep one top-level folder: {names:?}"
        );

        let mut readme = String::new();
        archive
            .by_name("deck/README.md")
            .unwrap()
            .read_to_string(&mut readme)
            .unwrap();
        assert!(
            readme.contains("typst compile main.typ deck.pdf"),
            "{readme}"
        );
        assert!(readme.contains("No Such Font For Project Test"), "{readme}");
        assert!(readme.contains("未安装"), "{readme}");
        assert!(readme.contains("归档测试"), "{readme}");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn archive_root_is_the_file_stem_and_never_escapes_the_folder() {
        assert_eq!(archive_root(Path::new("C:/tmp/演示.zip")), "演示");
        assert_eq!(archive_root(Path::new("deck.zip")), "deck");
        // A stem made only of characters a zip entry cannot hold falls back.
        assert_eq!(archive_root(Path::new("C:/tmp/???.zip")), "typst-project");
    }
}

/// 一页光栅化后的像素。`pixels` 是自上而下、每行 `width * 4` 字节的 **RGBA8**（非预乘）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PixelPage {
    /// 请求里 `slides` 的下标（子集位置）。
    pub index: usize,
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

/// 一批像素，附带这一次编译的诊断。
#[derive(Debug, Clone, Default)]
pub struct PixelBatch {
    pub pages: Vec<PixelPage>,
    /// 内容被裁切的页（子集位置）。
    pub overflow: Vec<usize>,
    /// 显示成占位图的图片引用。
    pub missing_assets: Vec<slide_typst::MissingAsset>,
    pub theme_fallback: Option<String>,
    /// 这一批的实际光栅化宽度（档位，不是请求值）。
    pub width: u32,
}

/// 与 [`render_slide_thumbnails`] 走同一个 [`prepare_export_values`]，因此主题、比例、
/// 字体角色与素材解析完全一致——只是把 PNG 编码换成直接交像素（GPUI 要的就是这个，
/// 少一次编码、少一次解码）。
///
/// 牌组按**原始 JSON** 传入而不是 `SlideBlueprint`：数据库里存的就是 JSON，而 Typst 版式
/// 对字段比 Rust 结构体宽松（例如 `cards[].items` 可以混进对象）。强类型往返会把这样的牌组
/// 挡在门外，只读界面没有理由替用户「修正」它。
///
/// `index` 同样是**子集位置**：空位不会发出去。
pub fn render_slide_pixels(
    state: &SharedState,
    project_id: Option<i64>,
    slides: Vec<serde_json::Value>,
    theme: Option<String>,
    width_px: u32,
    only: Option<Vec<usize>>,
) -> Result<PixelBatch> {
    let (slides, options, theme_fallback) =
        prepare_export_values(state, project_id, Some(slides), theme)?;
    let width = width_px.max(1);
    let rendered = renderer()
        .render_pixels(&slides, &options, width, only.as_deref())
        .map_err(|e| e.to_string())?;
    Ok(PixelBatch {
        pages: rendered
            .pages
            .into_iter()
            .map(|page| PixelPage {
                index: page.index,
                width: page.width,
                height: page.height,
                pixels: page.pixels,
            })
            .collect(),
        overflow: rendered
            .overflows
            .iter()
            .filter_map(|overflow| overflow.slide)
            .collect(),
        missing_assets: rendered.missing_assets,
        theme_fallback,
        width,
    })
}

/// 在系统文件管理器里选中导出的文件。Tauri 的 `reveal_exported_file` 命令转调这里。
pub fn reveal_in_file_manager(path: &Path) -> Result<()> {
    if !path.is_file() {
        return Err("文件不存在".to_string());
    }
    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("explorer")
        .arg("/select,")
        .arg(path)
        .spawn();
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open")
        .arg("-R")
        .arg(path)
        .spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let result = std::process::Command::new("xdg-open")
        .arg(path.parent().unwrap_or(Path::new(".")))
        .spawn();
    result.map(|_| ()).map_err(|e| e.to_string())
}
