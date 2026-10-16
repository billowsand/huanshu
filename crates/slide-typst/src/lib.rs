//! Native PDF export of slide blueprints through an embedded Typst compiler.
//!
//! The Vue templates in `src/components/Keynote*.vue` stay the on-screen
//! renderer; this crate re-implements layouts in Typst (`typst/`) and compiles
//! them in-process, so no Typst CLI or browser is needed at export time.
//!
//! ```no_run
//! let renderer = slide_typst::Renderer::with_system_fonts();
//! let slides: Vec<serde_json::Value> = vec![];
//! let rendered = renderer.render_pdf(&slides, &slide_typst::ExportOptions::default()).unwrap();
//! std::fs::write("deck.pdf", &rendered.pdf).unwrap();
//! ```

mod assets;
mod icons;
mod math;

pub use assets::{AssetData, Assets, MissingAsset};
/// The Carbon SVG builder the templates use, for hosts that draw the same icons in their UI
/// (the native shell renders the editor's Carbon icon names with it).
pub use icons::render_svg as render_icon_svg;

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use typst::diag::{FileError, FileResult, Severity, SourceDiagnostic};
use typst::foundations::{Bytes, Datetime, Duration, Label, Selector};
use typst::introspection::Introspector;
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::{LazyHash, PicoStr};
use typst::{Library, LibraryExt, World, WorldExt};
use typst_kit::fonts::FontStore;
pub use typst_layout::PagedDocument;

/// Template sources compiled into the binary, keyed by their virtual path.
const TEMPLATES: &[(&str, &str)] = &[
    ("/main.typ", include_str!("../typst/main.typ")),
    ("/lib/base.typ", include_str!("../typst/lib/base.typ")),
    (
        "/lib/components.typ",
        include_str!("../typst/lib/components.typ"),
    ),
    ("/lib/chart.typ", include_str!("../typst/lib/chart.typ")),
    (
        "/themes/notes.typ",
        include_str!("../typst/themes/notes.typ"),
    ),
    (
        "/themes/light-terminal.typ",
        include_str!("../typst/themes/light-terminal.typ"),
    ),
    (
        "/themes/light-editorial.typ",
        include_str!("../typst/themes/light-editorial.typ"),
    ),
    (
        "/themes/dark-terminal.typ",
        include_str!("../typst/themes/dark-terminal.typ"),
    ),
    (
        "/themes/dark-yellow.typ",
        include_str!("../typst/themes/dark-yellow.typ"),
    ),
    (
        "/themes/dark-blue.typ",
        include_str!("../typst/themes/dark-blue.typ"),
    ),
    (
        "/themes/_check.typ",
        include_str!("../typst/themes/_check.typ"),
    ),
    (
        "/layouts/cover.typ",
        include_str!("../typst/layouts/cover.typ"),
    ),
    (
        "/layouts/feature-grid.typ",
        include_str!("../typst/layouts/feature-grid.typ"),
    ),
    (
        "/layouts/step-flow.typ",
        include_str!("../typst/layouts/step-flow.typ"),
    ),
    (
        "/layouts/compare.typ",
        include_str!("../typst/layouts/compare.typ"),
    ),
    (
        "/layouts/swot.typ",
        include_str!("../typst/layouts/swot.typ"),
    ),
    (
        "/layouts/spotlight.typ",
        include_str!("../typst/layouts/spotlight.typ"),
    ),
    (
        "/layouts/section-intro.typ",
        include_str!("../typst/layouts/section-intro.typ"),
    ),
    (
        "/layouts/issue-stack.typ",
        include_str!("../typst/layouts/issue-stack.typ"),
    ),
    (
        "/layouts/closing.typ",
        include_str!("../typst/layouts/closing.typ"),
    ),
    (
        "/layouts/overview.typ",
        include_str!("../typst/layouts/overview.typ"),
    ),
    (
        "/layouts/timeline.typ",
        include_str!("../typst/layouts/timeline.typ"),
    ),
    (
        "/layouts/process.typ",
        include_str!("../typst/layouts/process.typ"),
    ),
    (
        "/layouts/split-layers.typ",
        include_str!("../typst/layouts/split-layers.typ"),
    ),
    (
        "/layouts/section-list.typ",
        include_str!("../typst/layouts/section-list.typ"),
    ),
    (
        "/layouts/focus-example.typ",
        include_str!("../typst/layouts/focus-example.typ"),
    ),
    (
        "/layouts/outcome-grid.typ",
        include_str!("../typst/layouts/outcome-grid.typ"),
    ),
    (
        "/layouts/center-grid.typ",
        include_str!("../typst/layouts/center-grid.typ"),
    ),
    (
        "/layouts/chart-focus.typ",
        include_str!("../typst/layouts/chart-focus.typ"),
    ),
    // Vendored Typst package (see typst/vendor/mitex/typst.toml for provenance;
    // `arrow-handle` in standard.typ is patched locally). Its spec files build
    // the eval scope for converted LaTeX math spans.
    (
        "/vendor/mitex/specs/mod.typ",
        include_str!("../typst/vendor/mitex/specs/mod.typ"),
    ),
    (
        "/vendor/mitex/specs/prelude.typ",
        include_str!("../typst/vendor/mitex/specs/prelude.typ"),
    ),
    (
        "/vendor/mitex/specs/latex/standard.typ",
        include_str!("../typst/vendor/mitex/specs/latex/standard.typ"),
    ),
    // Compile check for converted math spans (see `Renderer::failing_math`).
    ("/math-check.typ", include_str!("../typst/math-check.typ")),
    // One formula, sized to its content (see `Renderer::render_formula`).
    ("/formula.typ", include_str!("../typst/formula.typ")),
];

/// Slide kinds with a native Typst layout; others render a visible placeholder page.
pub const SUPPORTED_KINDS: &[&str] = &[
    "cover",
    "feature_grid",
    "step_flow",
    "compare",
    "swot",
    "spotlight",
    "section_intro",
    "issue_stack",
    "closing",
    "overview",
    "timeline",
    "process",
    "split_layers",
    "section_list",
    "focus_example",
    "outcome_grid",
    "center_grid",
    "chart_focus",
];

/// 本平台的默认中文字体：设置页「恢复默认」、界面字体与 Typst 兜底链都从这里取。
/// Windows（以及 Linux）保持原值；macOS 没有微软雅黑，用系统自带的苹方。
#[cfg(not(target_os = "macos"))]
pub const DEFAULT_FONT: &str = "Microsoft YaHei";
#[cfg(target_os = "macos")]
pub const DEFAULT_FONT: &str = "PingFang SC";

/// 本平台的默认数学字体（必须在 [`MATH_FONTS`] 里）：Windows 自带 Cambria Math，
/// macOS 自带 STIX Two Math（`/System/Library/Fonts/Supplemental`）。
#[cfg(not(target_os = "macos"))]
pub const DEFAULT_MATH_FONT: &str = "Cambria Math";
#[cfg(target_os = "macos")]
pub const DEFAULT_MATH_FONT: &str = "STIX Two Math";

/// 默认等宽字体（[`FontRoles::uniform`] 的代码角色排在中文字体前面）。
#[cfg(not(target_os = "macos"))]
pub const DEFAULT_CODE_FONTS: &[&str] = &["Cascadia Mono", "Consolas"];
#[cfg(target_os = "macos")]
pub const DEFAULT_CODE_FONTS: &[&str] = &["Menlo", "Monaco"];

/// 追加在通用中文兜底之后、只在本平台才有的中文字体。macOS 的苹方是按需下载的系统资源
/// （`/System/Library/AssetsV2`），万一没下载，这几款随系统常驻在 `/System/Library/Fonts`。
#[cfg(not(target_os = "macos"))]
const PLATFORM_CJK_FALLBACKS: &[&str] = &[];
#[cfg(target_os = "macos")]
const PLATFORM_CJK_FALLBACKS: &[&str] = &["Hiragino Sans GB", "Heiti SC", "Songti SC"];

/// Theme ids with a Typst token file under `typst/themes/`; unknown ids fall
/// back to `light-notes` (see `themes` in `typst/main.typ`).
pub const SUPPORTED_THEMES: &[&str] = &[
    "light-notes",
    "light-terminal",
    "light-editorial",
    "dark-terminal",
    "dark-yellow",
    "dark-blue",
];

/// Font fallback list per slide font role (mirrors `src/typography.ts` `fontRoles`).
#[derive(Debug, Clone, Serialize)]
pub struct FontRoles {
    pub title: Vec<String>,
    pub subtitle: Vec<String>,
    pub body: Vec<String>,
    pub caption: Vec<String>,
    pub number: Vec<String>,
    pub code: Vec<String>,
    pub math: Vec<String>,
}

impl FontRoles {
    /// Every role uses `family`, followed by common CJK fallbacks.
    pub fn uniform(family: &str) -> Self {
        let list = Self::with_fallbacks(family);
        Self {
            math: MATH_FONTS.iter().map(|f| f.to_string()).collect(),
            title: list.clone(),
            subtitle: list.clone(),
            body: list.clone(),
            caption: list.clone(),
            number: list.clone(),
            code: [
                DEFAULT_CODE_FONTS.iter().map(|f| f.to_string()).collect(),
                list,
            ]
            .concat(),
        }
    }

    pub fn with_fallbacks(family: &str) -> Vec<String> {
        Self::with_custom_fallback(family, DEFAULT_FONT)
    }

    pub fn with_custom_fallback(family: &str, fallback: &str) -> Vec<String> {
        let mut list = Vec::new();
        for name in [family.trim(), fallback.trim()] {
            if !name.is_empty() && !list.iter().any(|f: &String| f.eq_ignore_ascii_case(name)) {
                list.push(name.to_string());
            }
        }
        let common = [DEFAULT_FONT, "PingFang SC", "Noto Sans CJK SC", "SimHei"];
        for fallback in common
            .into_iter()
            .chain(PLATFORM_CJK_FALLBACKS.iter().copied())
        {
            if !list.iter().any(|f| f.eq_ignore_ascii_case(fallback)) {
                list.push(fallback.to_string());
            }
        }
        list
    }
}

impl Default for FontRoles {
    fn default() -> Self {
        Self::uniform(DEFAULT_FONT)
    }
}

/// Math fonts tried in order: Windows ships Cambria Math, the others come with
/// TeX distributions.
pub const MATH_FONTS: &[&str] = &[
    "Cambria Math",
    "New Computer Modern Math",
    "STIX Two Math",
    "Libertinus Math",
];

#[derive(Debug, Clone)]
pub struct ExportOptions {
    /// Theme id from `src/themes/catalog.ts`; see [`SUPPORTED_THEMES`].
    /// Unknown ids fall back to `light-notes`.
    pub theme: String,
    /// `ratio_16x9` / `ratio_32x9` / `ratio_48x9`.
    pub aspect_ratio: String,
    pub fonts: FontRoles,
    /// PDF document title metadata.
    pub title: Option<String>,
    /// Image references (`media:<id>`, `figure/x.png`, …) → their bytes.
    pub assets: Assets,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            theme: "light-notes".to_string(),
            aspect_ratio: "ratio_16x9".to_string(),
            fonts: FontRoles::default(),
            title: None,
            assets: Assets::default(),
        }
    }
}

#[derive(Serialize)]
struct DeckInput<'a> {
    theme: &'a str,
    aspect_ratio: &'a str,
    title: Option<&'a str>,
    fonts: &'a FontRoles,
    /// Installed entries of [`MATH_FONTS`], in order; empty if none is.
    math_fonts: Vec<String>,
    /// Image reference → path and size; unusable references are absent.
    assets: &'a HashMap<String, assets::ImageInfo>,
    slides: &'a [serde_json::Value],
}

/// The single formula `/formula.typ` lays out (see [`Renderer::render_formula`]).
#[derive(Serialize)]
struct FormulaInput<'a> {
    /// Typst math source from mitex.
    source: &'a str,
    /// Block-level (centered) instead of inline.
    display: bool,
    /// Text color as `#rrggbb`; `rgb(…)` in Typst parses this string.
    fill: String,
    /// Font size in points.
    size_pt: f64,
    /// Body font fallbacks, for `\text{…}` parts of the formula.
    fonts: Vec<String>,
    /// Installed entries of [`MATH_FONTS`], in order; empty if none is.
    math_fonts: Vec<String>,
}

/// A slide whose content did not fit even at the smallest fit factor; it was
/// clipped. Emitted by `overflow-marker` in `typst/lib/base.typ`.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Overflow {
    /// Index into the `slides` passed to the renderer.
    pub slide: Option<usize>,
    #[serde(rename = "needed-pt")]
    pub needed_pt: f64,
    #[serde(rename = "available-pt")]
    pub available_pt: f64,
}

/// A laid-out deck plus layout diagnostics.
pub struct Compiled {
    pub document: PagedDocument,
    pub overflows: Vec<Overflow>,
    pub missing_assets: Vec<MissingAsset>,
}

/// Exported PDF bytes plus layout diagnostics.
pub struct Rendered {
    pub pdf: Vec<u8>,
    pub pages: usize,
    pub overflows: Vec<Overflow>,
    /// Image references that rendered as placeholders, with the reason.
    pub missing_assets: Vec<MissingAsset>,
}

/// One rasterized deck page (the app shows these as thumbnails).
#[derive(Debug)]
pub struct RenderedPng {
    /// Index into the `slides` passed to the renderer.
    pub index: usize,
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// Rasterized deck pages plus the diagnostics of the single compile behind them.
pub struct RenderedPages {
    pub pages: Vec<RenderedPng>,
    pub overflows: Vec<Overflow>,
    pub missing_assets: Vec<MissingAsset>,
}

/// One rasterized deck page as uncompressed pixels: the same raster as
/// [`RenderedPng`] without the PNG encoding, for hosts that upload the pixels
/// to a GPU themselves (see [`Renderer::render_pixels`]).
#[derive(Debug)]
pub struct RenderedPixels {
    /// Index into the `slides` passed to the renderer.
    pub index: usize,
    pub width: u32,
    pub height: u32,
    /// Top-to-bottom rows of `width * 4` bytes, non-premultiplied RGBA8.
    pub pixels: Vec<u8>,
}

/// Rasterized deck pages as raw pixels, plus the diagnostics of the single
/// compile behind them (the counterpart of [`RenderedPages`]).
pub struct RenderedFrames {
    pub pages: Vec<RenderedPixels>,
    pub overflows: Vec<Overflow>,
    pub missing_assets: Vec<MissingAsset>,
}

/// One formula as a transparent PNG (see [`Renderer::render_formula`]).
#[derive(Debug)]
pub struct RenderedFormula {
    /// PNG bytes with an alpha channel; the formula's surroundings are fully
    /// transparent, so the host can draw it on any background.
    pub png: Vec<u8>,
    /// Frame size in device pixels, at [`FORMULA_PIXEL_PER_PT`] pixels per point.
    pub width: u32,
    pub height: u32,
}

/// Allowed thumbnail widths, in device pixels.
const MIN_PNG_WIDTH: u32 = 64;
const MAX_PNG_WIDTH: u32 = 3840;

/// Rasterization scale of [`Renderer::render_formula`]: 4 device pixels per
/// point, so the image stays crisp on HiDPI displays. Hosts that lay the image
/// out in the surrounding text divide [`RenderedFormula::width`] and
/// `height` by this to get logical pixels.
pub const FORMULA_PIXEL_PER_PT: f64 = 4.0;

/// Formula font sizes outside this range (in points) are clamped to it: an
/// editor only asks for a body-text size, and a nonsensical one must not blow
/// up the page size or the raster.
const MIN_FORMULA_SIZE_PT: f64 = 4.0;
const MAX_FORMULA_SIZE_PT: f64 = 200.0;

/// A compile failure with Typst's diagnostics resolved to `file:line:col`.
/// [`Renderer::render_formula`] reports a formula's problems — LaTeX mitex
/// cannot convert, or Typst rejecting the converted source — the same way.
#[derive(Debug)]
pub struct RenderError {
    pub diagnostics: Vec<String>,
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Typst export failed:\n{}", self.diagnostics.join("\n"))
    }
}

impl std::error::Error for RenderError {}

/// Owns the expensive, reusable parts: the standard library and the font index.
/// Scanning system fonts takes a moment, so keep one renderer per process.
pub struct Renderer {
    library: LazyHash<Library>,
    fonts: FontStore,
}

impl Renderer {
    pub fn with_system_fonts() -> Self {
        let mut fonts = FontStore::new();
        fonts.extend(typst_kit::fonts::system());
        Self {
            library: LazyHash::new(Library::default()),
            fonts,
        }
    }

    /// Compiles `slides` (serialized `SlideBlueprint`s) into PDF bytes.
    pub fn render_pdf(
        &self,
        slides: &[serde_json::Value],
        options: &ExportOptions,
    ) -> Result<Rendered, RenderError> {
        let Compiled {
            document,
            overflows,
            missing_assets,
            ..
        } = self.compile(slides, options)?;
        let pdf =
            typst_pdf::pdf(&document, &typst_pdf::PdfOptions::default()).map_err(|errors| {
                RenderError {
                    diagnostics: errors.iter().map(|d| d.message.to_string()).collect(),
                }
            })?;
        Ok(Rendered {
            pdf,
            pages: document.pages().len(),
            overflows,
            missing_assets,
        })
    }

    /// Compiles the whole deck once and rasterizes the requested pages (all when `only` is None).
    ///
    /// The whole deck is compiled even when one page is wanted: the slide index (`ctx.index`)
    /// keys the overflow markers (and layouts may depend on it), so pages cannot be rasterized
    /// from a subset compile. `width_px` is the thumbnail width in device pixels; each page is
    /// rasterized at `width_px / page width` pixels per point, so its height follows the page's
    /// own aspect ratio. Pages beyond `slides.len()` (a layout that overflowed its canvas) and
    /// out-of-range entries in `only` are ignored.
    pub fn render_png(
        &self,
        slides: &[serde_json::Value],
        options: &ExportOptions,
        width_px: u32,
        only: Option<&[usize]>,
    ) -> Result<RenderedPages, RenderError> {
        let (rasters, overflows, missing_assets) =
            self.rasterize(slides, options, width_px, only)?;
        let mut pages = Vec::with_capacity(rasters.len());
        for (index, pixmap) in rasters {
            let png = pixmap.encode_png().map_err(|err| RenderError {
                diagnostics: vec![format!("encode slide {index} as PNG: {err}")],
            })?;
            pages.push(RenderedPng {
                index,
                png,
                width: pixmap.width(),
                height: pixmap.height(),
            });
        }
        Ok(RenderedPages {
            pages,
            overflows,
            missing_assets,
        })
    }

    /// Same compile and rasterization as [`Renderer::render_png`], but the pages
    /// come back as raw non-premultiplied RGBA8 instead of PNG bytes.
    ///
    /// A host with its own GPU pipeline (GPUI / wgpu in a native shell) uploads
    /// these directly and skips both the PNG encoding here and the decoding on
    /// the other side. `width_px` and `only` follow `render_png`.
    pub fn render_pixels(
        &self,
        slides: &[serde_json::Value],
        options: &ExportOptions,
        width_px: u32,
        only: Option<&[usize]>,
    ) -> Result<RenderedFrames, RenderError> {
        let (rasters, overflows, missing_assets) =
            self.rasterize(slides, options, width_px, only)?;
        let pages = rasters
            .into_iter()
            .map(|(index, pixmap)| RenderedPixels {
                index,
                width: pixmap.width(),
                height: pixmap.height(),
                pixels: unpremultiply(pixmap.data()),
            })
            .collect();
        Ok(RenderedFrames {
            pages,
            overflows,
            missing_assets,
        })
    }

    /// Typesets one LaTeX formula into a small transparent PNG, for a host that
    /// shows formulas in its own text (the native editor's Markdown preview).
    ///
    /// `latex` is the formula body without `$` delimiters; `display` lays it out
    /// block-level (centered) instead of inline; `color` is the text color, which
    /// is what a formula inherits — pass the surrounding body color so it stays
    /// readable on a dark theme; `size_pt` is the font size in points, clamped to
    /// 4–200pt.
    ///
    /// The conversion is the one slides use (`math::convert_formula`: sentinel
    /// characters stripped, then mitex), and the formula is `eval`ed onto a page
    /// that hugs its ink plus a 1pt margin — so superscripts, fraction
    /// denominators and the limits of sums and integrals are never clipped — and
    /// rasterized at `pixel_per_pt` = [`FORMULA_PIXEL_PER_PT`]. A formula mitex
    /// cannot convert, or one Typst rejects, comes back as `Err` with the reason
    /// — the caller shows the source text instead. Formula problems never touch
    /// other formulas or the deck export.
    pub fn render_formula(
        &self,
        latex: &str,
        display: bool,
        color: [u8; 3],
        size_pt: f64,
    ) -> Result<RenderedFormula, RenderError> {
        self.render_formula_with_fonts(latex, display, color, size_pt, &FontRoles::default())
    }

    pub fn render_formula_with_fonts(
        &self,
        latex: &str,
        display: bool,
        color: [u8; 3],
        size_pt: f64,
        fonts: &FontRoles,
    ) -> Result<RenderedFormula, RenderError> {
        // Why an eval error is fatal here: `formula.typ` evals the converted
        // source, and Typst cannot catch that — a rejected formula is a failed
        // compile, which is exactly the `Err` the caller falls back on.
        let source = math::convert_formula(latex).map_err(|err| RenderError {
            diagnostics: vec![format!("convert LaTeX {:?}: {err}", latex.trim())],
        })?;
        let [r, g, b] = color;
        let deck = serde_json::to_vec(&FormulaInput {
            source: &source,
            display,
            fill: format!("#{r:02x}{g:02x}{b:02x}"),
            size_pt: if size_pt.is_finite() {
                size_pt.clamp(MIN_FORMULA_SIZE_PT, MAX_FORMULA_SIZE_PT)
            } else {
                MIN_FORMULA_SIZE_PT
            },
            fonts: self.installed_fonts(fonts).body,
            math_fonts: self.installed_math_fonts(&fonts.math),
        })
        .map_err(|err| RenderError {
            diagnostics: vec![format!("serialize formula: {err}")],
        })?;
        let world = SlideWorld {
            renderer: self,
            main: file_id("/formula.typ"),
            deck: Bytes::new(deck),
            images: HashMap::new(),
            sources: Mutex::new(HashMap::new()),
            missing_icons: Mutex::new(HashSet::new()),
        };
        let result = typst::compile::<PagedDocument>(&world);
        // Same process-wide layout cache as `compile()`; see the note there.
        typst::comemo::evict(50);
        let document = result.output.map_err(|errors| RenderError {
            diagnostics: errors.iter().map(|d| world.describe(d)).collect(),
        })?;
        let page = document.pages().first().ok_or_else(|| RenderError {
            diagnostics: vec!["formula.typ laid out no page".to_string()],
        })?;
        let pixmap = typst_render::render(
            page,
            &typst_render::RenderOptions {
                pixel_per_pt: typst::utils::Scalar::new(FORMULA_PIXEL_PER_PT),
                render_bleed: false,
            },
        );
        let png = pixmap.encode_png().map_err(|err| RenderError {
            diagnostics: vec![format!("encode formula as PNG: {err}")],
        })?;
        Ok(RenderedFormula {
            png,
            width: pixmap.width(),
            height: pixmap.height(),
        })
    }

    /// The compile and rasterization shared by `render_png` and `render_pixels`: the
    /// requested pages as tiny-skia pixmaps (premultiplied), plus the diagnostics.
    #[allow(clippy::type_complexity)]
    fn rasterize(
        &self,
        slides: &[serde_json::Value],
        options: &ExportOptions,
        width_px: u32,
        only: Option<&[usize]>,
    ) -> Result<
        (
            Vec<(usize, tiny_skia::Pixmap)>,
            Vec<Overflow>,
            Vec<MissingAsset>,
        ),
        RenderError,
    > {
        let Compiled {
            document,
            overflows,
            missing_assets,
            ..
        } = self.compile(slides, options)?;
        let width_px = width_px.clamp(MIN_PNG_WIDTH, MAX_PNG_WIDTH);
        let mut pages = Vec::new();
        for (index, page) in document.pages().iter().enumerate().take(slides.len()) {
            if only.is_some_and(|wanted| !wanted.contains(&index)) {
                continue;
            }
            let page_width_pt = page.frame.size().x.to_pt();
            if page_width_pt <= 0.0 {
                continue;
            }
            let pixmap = typst_render::render(
                page,
                &typst_render::RenderOptions {
                    pixel_per_pt: typst::utils::Scalar::new(width_px as f64 / page_width_pt),
                    render_bleed: false,
                },
            );
            pages.push((index, pixmap));
        }
        // An overflow marker carries the index of the slide that was clipped; markers pointing
        // past the deck are dropped. Markers of slides outside `only` are kept: the compile
        // covered the whole deck either way.
        let overflows = overflows
            .into_iter()
            .filter(|overflow| {
                overflow
                    .slide
                    .is_none_or(|slide| slides.get(slide).is_some())
            })
            .collect();
        Ok((pages, overflows, missing_assets))
    }

    /// Lays out the deck without exporting, e.g. to rasterize pages for visual checks.
    pub fn compile(
        &self,
        slides: &[serde_json::Value],
        options: &ExportOptions,
    ) -> Result<Compiled, RenderError> {
        let prepared = assets::prepare(slides, &options.assets);
        for missing in &prepared.missing {
            eprintln!(
                "[slide-typst] slide {}: image {} unavailable ({})",
                missing.slide, missing.reference, missing.reason
            );
        }
        // Convert LaTeX math spans to sentinel-wrapped Typst math before the
        // deck is serialized; `rich()` in lib/base.typ evals the spans. Typst
        // cannot catch an eval error, so one formula that mitex converts but
        // Typst rejects (`\frac{1}`, `\ref{x}` …) would fail the whole export:
        // such spans are found up front and kept as literal text instead.
        let mut rewritten = slides.to_vec();
        let spans = math::rewrite_deck(&mut rewritten, &HashSet::new());
        let rejected = self.failing_math(&spans);
        if !rejected.is_empty() {
            rewritten = slides.to_vec();
            math::rewrite_deck(&mut rewritten, &rejected);
        }
        let slides = rewritten;
        let deck = serde_json::to_vec(&DeckInput {
            theme: &options.theme,
            aspect_ratio: &options.aspect_ratio,
            title: options.title.as_deref(),
            fonts: &self.installed_fonts(&options.fonts),
            math_fonts: self.installed_math_fonts(&options.fonts.math),
            assets: &prepared.paths,
            slides: &slides,
        })
        .map_err(|err| RenderError {
            diagnostics: vec![format!("serialize deck: {err}")],
        })?;

        let world = SlideWorld {
            renderer: self,
            main: file_id("/main.typ"),
            deck: Bytes::new(deck),
            images: prepared
                .files
                .into_iter()
                .map(|(path, bytes)| (path, Bytes::new(bytes)))
                .collect(),
            sources: Mutex::new(HashMap::new()),
            missing_icons: Mutex::new(HashSet::new()),
        };
        let result = typst::compile::<PagedDocument>(&world);
        // Typst memoizes layout in a process-wide cache that only grows unless it is evicted.
        // The app compiles on every thumbnail/stage refresh, so drop entries unused for the
        // last 50 compiles; without this, memory grows without bound the longer the app runs.
        // The typst CLI's watch mode uses 10, which is too tight here: the thumbnail grid, the
        // presentation stage and the PDF export interleave different decks, and a page's
        // layouts get evicted between two edits of it.
        typst::comemo::evict(50);
        for warning in &result.warnings {
            eprintln!("[slide-typst] warning: {}", world.describe(warning));
        }
        let document = result.output.map_err(|errors| RenderError {
            diagnostics: errors.iter().map(|d| world.describe(d)).collect(),
        })?;
        // Each slide is one page; extra pages mean a layout overflowed its canvas.
        if document.pages().len() != slides.len() {
            eprintln!(
                "[slide-typst] warning: {} slides produced {} pages (a layout overflowed)",
                slides.len(),
                document.pages().len()
            );
        }
        let overflows = query_markers(&document, "slide-overflow");
        Ok(Compiled {
            document,
            overflows,
            missing_assets: prepared.missing,
        })
    }

    /// Converted math spans that fail to compile on their own. The common case
    /// (every span fine) costs one small compile; only when that fails is each
    /// span checked separately.
    fn failing_math(&self, spans: &[math::Span]) -> HashSet<String> {
        let mut unique: Vec<&math::Span> = Vec::new();
        for span in spans {
            if !unique.contains(&span) {
                unique.push(span);
            }
        }
        let compiles = |batch: &[&math::Span]| -> Result<(), String> {
            let world = SlideWorld {
                renderer: self,
                main: file_id("/math-check.typ"),
                deck: Bytes::new(serde_json::to_vec(batch).expect("spans serialize")),
                images: HashMap::new(),
                sources: Mutex::new(HashMap::new()),
                missing_icons: Mutex::new(HashSet::new()),
            };
            typst::compile::<PagedDocument>(&world)
                .output
                .map(|_| ())
                .map_err(|errors| {
                    errors
                        .iter()
                        .map(|d| d.message.to_string())
                        .collect::<Vec<_>>()
                        .join("; ")
                })
        };
        if unique.is_empty() || compiles(&unique).is_ok() {
            return HashSet::new();
        }
        let mut rejected = HashSet::new();
        for span in unique {
            if let Err(reason) = compiles(&[span]) {
                eprintln!(
                    "[slide-typst] math kept as text, Typst rejected {:?}: {reason}",
                    span.source
                );
                rejected.insert(span.source.clone());
            }
        }
        rejected
    }

    /// Compiles `typst/themes/_check.typ`, which asserts that every theme
    /// module exposes the same dictionary keys plus a `background` painter.
    /// Guards against theme drift; used by the test suite.
    #[doc(hidden)]
    pub fn check_theme_interface(&self) -> Result<(), RenderError> {
        let world = SlideWorld {
            renderer: self,
            main: file_id("/themes/_check.typ"),
            deck: Bytes::new(&b"{}"[..]),
            images: HashMap::new(),
            sources: Mutex::new(HashMap::new()),
            missing_icons: Mutex::new(HashSet::new()),
        };
        typst::compile::<PagedDocument>(&world)
            .output
            .map_err(|errors| RenderError {
                diagnostics: errors.iter().map(|d| world.describe(d)).collect(),
            })?;
        Ok(())
    }

    /// Equations need an OpenType MATH font. Only installed ones are passed to
    /// `main.typ`: Typst warns about every missing family in a fallback list.
    fn installed_math_fonts(&self, preferred: &[String]) -> Vec<String> {
        let book = self.fonts.book();
        let mut names = Vec::new();
        // Only known OpenType MATH families can render equations reliably.
        for name in preferred
            .iter()
            .map(String::as_str)
            .chain(MATH_FONTS.iter().copied())
        {
            if MATH_FONTS.iter().any(|f| f.eq_ignore_ascii_case(name))
                && book.contains_family(&name.to_lowercase())
                && !names.iter().any(|f: &String| f.eq_ignore_ascii_case(name))
            {
                names.push(name.to_string());
            }
        }
        names
    }

    /// Drops families that are not installed so Typst does not warn about every
    /// fallback entry; keeps the list as-is if none of them is available.
    fn installed_fonts(&self, roles: &FontRoles) -> FontRoles {
        let book = self.fonts.book();
        let filter = |list: &Vec<String>| {
            let present: Vec<String> = list
                .iter()
                .filter(|f| book.contains_family(&f.to_lowercase()))
                .cloned()
                .collect();
            if present.is_empty() {
                list.clone()
            } else {
                present
            }
        };
        FontRoles {
            title: filter(&roles.title),
            subtitle: filter(&roles.subtitle),
            body: filter(&roles.body),
            caption: filter(&roles.caption),
            number: filter(&roles.number),
            code: filter(&roles.code),
            math: self.installed_math_fonts(&roles.math),
        }
    }
}

/// Undoes the premultiplied alpha of a `tiny_skia::Pixmap`, which is what the
/// rasterizer produces but neither PNG nor a GPU upload expects. Every slide
/// page paints an opaque background, so in practice the alpha is 255 everywhere
/// and this is a copy; the general case is handled anyway, `a == 0` carrying no
/// color at all.
fn unpremultiply(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    for pixel in data.chunks_exact(4) {
        let alpha = u32::from(pixel[3]);
        let straight = |channel: u8| -> u8 {
            if alpha == 0 {
                0
            } else {
                ((u32::from(channel) * 255 + alpha / 2) / alpha).min(255) as u8
            }
        };
        out.extend_from_slice(&[
            straight(pixel[0]),
            straight(pixel[1]),
            straight(pixel[2]),
            pixel[3],
        ]);
    }
    out
}

/// Values of every `metadata(..) <label>` marker in the laid-out document.
fn query_markers<T: serde::de::DeserializeOwned>(document: &PagedDocument, label: &str) -> Vec<T> {
    let label = Label::new(PicoStr::intern(label)).expect("valid label");
    document
        .introspector()
        .query(&Selector::Label(label))
        .iter()
        .filter_map(|content| content.get_by_name("value").ok())
        .filter_map(|value| serde_json::to_value(&value).ok())
        .filter_map(|json| serde_json::from_value(json).ok())
        .collect()
}

fn file_id(path: &str) -> FileId {
    RootedPath::new(
        VirtualRoot::Project,
        VirtualPath::new(path).expect("valid virtual path"),
    )
    .intern()
}

struct SlideWorld<'a> {
    renderer: &'a Renderer,
    main: FileId,
    deck: Bytes,
    /// `/assets/<n>.<ext>` → validated image bytes.
    images: HashMap<String, Bytes>,
    sources: Mutex<HashMap<FileId, Source>>,
    missing_icons: Mutex<HashSet<String>>,
}

impl SlideWorld<'_> {
    fn path_of(id: FileId) -> FileResult<&'static str> {
        let rooted = id.get();
        if *rooted.root() != VirtualRoot::Project {
            // Packages would need network access; templates must stay self-contained.
            return Err(FileError::Other(Some(
                "Typst packages are not available in slide export".into(),
            )));
        }
        Ok(rooted.vpath().get_with_slash())
    }

    fn describe(&self, diag: &SourceDiagnostic) -> String {
        let level = match diag.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        let location = diag
            .span
            .id()
            .and_then(|id| {
                let source = self.source(id).ok()?;
                let range = self.range(diag.span)?;
                let (line, col) = source.lines().byte_to_line_column(range.start)?;
                Some(format!(
                    "{}:{}:{}",
                    id.get().vpath().get_with_slash(),
                    line + 1,
                    col + 1
                ))
            })
            .unwrap_or_else(|| "<unknown>".to_string());
        let hints: String = diag
            .hints
            .iter()
            .map(|h| format!("\n  hint: {}", h.v))
            .collect();
        format!("{level} at {location}: {}{hints}", diag.message)
    }
}

impl World for SlideWorld<'_> {
    fn library(&self) -> &LazyHash<Library> {
        &self.renderer.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        self.renderer.fonts.book()
    }

    fn main(&self) -> FileId {
        self.main
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if let Some(source) = self.sources.lock().unwrap().get(&id) {
            return Ok(source.clone());
        }
        let path = Self::path_of(id)?;
        let text = TEMPLATES
            .iter()
            .find(|(p, _)| *p == path)
            .map(|(_, text)| *text)
            .ok_or_else(|| FileError::NotFound(path.into()))?;
        let source = Source::new(id, text.to_string());
        self.sources.lock().unwrap().insert(id, source.clone());
        Ok(source)
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        let path = Self::path_of(id)?;
        if path == "/deck.json" {
            return Ok(self.deck.clone());
        }
        if let Some(bytes) = self.images.get(path) {
            return Ok(bytes.clone());
        }
        if let Some(rest) = path.strip_prefix("/icons/") {
            // `/icons/<set>/<name>/<rrggbb[aa]>.svg`
            let parts: Vec<&str> = rest.split('/').collect();
            if let [set, name, file] = parts.as_slice() {
                let hex = file.trim_end_matches(".svg");
                let (svg, fallback) = icons::render_svg(set, name, hex);
                if fallback
                    && self
                        .missing_icons
                        .lock()
                        .unwrap()
                        .insert(format!("{set}:{name}"))
                {
                    eprintln!(
                        "[slide-typst] unknown icon {set}:{name}, using {}",
                        icons::FALLBACK_ICON
                    );
                }
                return Ok(Bytes::from_string(svg));
            }
        }
        self.source(id)
            .map(|source| Bytes::from_string(source.text().to_string()))
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.renderer.fonts.font(index)
    }

    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::OnceLock;

    /// Scanning system fonts is expensive, so the tests share one renderer, as
    /// the app does.
    fn renderer() -> &'static Renderer {
        static RENDERER: OnceLock<Renderer> = OnceLock::new();
        RENDERER.get_or_init(Renderer::with_system_fonts)
    }

    /// A plausible light-theme body color (preview formulas follow the theme).
    const INK: [u8; 3] = [0x1e, 0x1e, 0x1e];

    fn formula(latex: &str, display: bool) -> RenderedFormula {
        renderer()
            .render_formula(latex, display, INK, 16.0)
            .unwrap_or_else(|err| panic!("{latex:?} failed: {err}"))
    }

    fn assert_png(image: &RenderedFormula, what: &str) {
        assert!(image.png.starts_with(b"\x89PNG"), "{what}: not a PNG");
        assert!(image.width > 0 && image.height > 0, "{what}: empty image");
    }

    /// The decoded RGBA of a formula image, for pixel-level assertions.
    fn pixels(image: &RenderedFormula) -> image::RgbaImage {
        image::load_from_memory_with_format(&image.png, image::ImageFormat::Png)
            .expect("formula PNG decodes")
            .to_rgba8()
    }

    #[test]
    fn inline_formula_renders_on_a_transparent_page() {
        let image = formula("x^2 + y^2 = z^2", false);
        assert_png(&image, "inline");
        // `fill: none` on the page: the margin around the formula is see-through,
        // so the host can draw it on the surrounding background.
        assert_eq!(pixels(&image).get_pixel(0, 0)[3], 0, "background is opaque");
        // 4 device pixels per point, so the caller divides by that for logical ones.
        assert_eq!(image.width, pixels(&image).width());
    }

    #[test]
    fn display_formula_renders_its_limits_above_and_below() {
        // Block-level: `\sum`'s limits move above and below the sign, which makes
        // the image taller than the same formula laid out inline.
        let inline = formula("\\sum_{i=1}^{n} i", false);
        let display = formula("\\sum_{i=1}^{n} i", true);
        assert_png(&display, "display");
        assert!(
            display.height > inline.height,
            "display {} is not taller than inline {}",
            display.height,
            inline.height
        );
    }

    #[test]
    fn fraction_renders() {
        // Fractions, roots and accents are the cases where a clipped image shows;
        // the ~1pt page margin must keep the full formula in frame.
        let image = formula("x_{1,2} = \\frac{-b \\pm \\sqrt{b^2-4ac}}{2a}", true);
        assert_png(&image, "fraction");
        assert_eq!(pixels(&image).get_pixel(0, 0)[3], 0);
    }

    #[test]
    fn formula_ink_keeps_clear_of_the_page_border() {
        // The page hugs the formula, so an auto-sized page that is shorter than
        // the formula cuts it off: with Typst's default cap-height/baseline text
        // edges, inline fractions, sums, integrals and superscripts lost their
        // tops and bottoms. Every formula must keep the 1pt margin empty, or it
        // is being clipped.
        let cases: &[(&str, bool)] = &[
            ("x^2", false),
            ("\\frac{1}{2}", false),
            ("\\frac{\\frac{a}{b}}{\\frac{c}{d}}", false),
            ("\\int_{0}^{1} e^{-x^2} dx", false),
            ("\\sqrt[3]{\\frac{a}{b}}", false),
            ("\\hat{\\hat{x}} \\vec{v}", false),
            ("\\sum_{i=1}^{n} \\frac{1}{i^2}", true),
            ("\\begin{pmatrix} a & b \\\\ c & d \\end{pmatrix}", true),
        ];
        for (latex, display) in cases {
            let image = formula(latex, *display);
            let rgba = pixels(&image);
            let (width, height) = rgba.dimensions();
            let border: Vec<(u32, u32)> = (0..width)
                .flat_map(|x| [(x, 0), (x, height - 1)])
                .chain((0..height).flat_map(|y| [(0, y), (width - 1, y)]))
                .collect();
            for (x, y) in border {
                assert_eq!(
                    rgba.get_pixel(x, y)[3],
                    0,
                    "{latex:?} display={display}: ink at the border ({x}, {y}) is clipped",
                );
            }
        }
    }

    #[test]
    fn font_size_scales_the_formula() {
        // Typst math follows the surrounding text size, which is how `size_pt`
        // reaches the equation (see formula.typ).
        let small = renderer().render_formula("x^2", false, INK, 10.0).unwrap();
        let large = renderer().render_formula("x^2", false, INK, 20.0).unwrap();
        assert!(
            large.width > small.width && large.height > small.height,
            "20pt {}x{} is not bigger than 10pt {}x{}",
            large.width,
            large.height,
            small.width,
            small.height
        );
    }

    #[test]
    fn fill_reaches_the_raster() {
        let ink = renderer()
            .render_formula("x^2", false, [0x1e, 0x1e, 0x1e], 16.0)
            .unwrap();
        let paper = renderer()
            .render_formula("x^2", false, [0xff, 0xff, 0xff], 16.0)
            .unwrap();
        assert_ne!(
            ink.png, paper.png,
            "the fill color did not reach the formula"
        );
        assert_png(&paper, "white formula");
    }

    #[test]
    fn latex_mitex_or_typst_rejects_is_an_error() {
        // `\badcmd{` is LaTeX mitex does not know; the rest convert, but Typst
        // rejects the result (missing argument, unclosed delimiter, dangling
        // label). All of them must come back as an error for the caller to show
        // as plain text — never as a panic or a broken image.
        for latex in ["\\badcmd{", "\\frac{1}", "\\left(", "\\ref{eq}"] {
            let err = renderer()
                .render_formula(latex, false, INK, 16.0)
                .expect_err(&format!("{latex:?} should not render"));
            assert!(!err.diagnostics.is_empty(), "{latex:?}: no diagnostics");
        }
    }

    #[test]
    fn sentinel_chars_in_the_formula_are_stripped() {
        // The sentinel characters wrap converted spans inside decks; text that
        // already carries them must be stripped rather than reach `eval` as
        // markup. Stripping is observable: the wrapped formula renders exactly
        // like the bare one.
        let bare = formula("x^2", false);
        let wrapped = formula("\u{1}\u{2}x^2", false);
        assert_eq!(bare.png, wrapped.png);
    }

    #[test]
    fn typst_markup_in_the_formula_is_never_executed() {
        // These would panic or read the deck if they ever reached Typst as
        // markup instead of as LaTeX: `#` is literal in math mode, and the
        // sentinel-wrapped text is stripped first. Either an image or an error
        // is fine; a panic is not.
        for latex in [
            "#panic(\"x\")",
            "\u{1}I#panic(\"injected\")\u{2}",
            "#read(\"/deck.json\")",
        ] {
            match renderer().render_formula(latex, false, INK, 16.0) {
                Ok(image) => assert_png(&image, latex),
                Err(err) => assert!(!err.diagnostics.is_empty(), "{latex:?}: no diagnostics"),
            }
        }
    }

    #[test]
    fn one_renderer_serves_many_formulas() {
        // The preview reuses the process-wide renderer (fonts are scanned once)
        // and asks for the same formula more than once as it re-renders.
        let first = formula("a^2 + b^2 = c^2", false);
        let again = formula("a^2 + b^2 = c^2", false);
        assert_eq!(
            first.png, again.png,
            "the same formula rendered differently"
        );
        assert_png(&formula("\\frac{1}{2}", true), "second formula");
    }

    #[test]
    fn nonsensical_sizes_do_not_break_the_render() {
        // Sizes are clamped instead of reaching Typst as-is: a preview that asks
        // for 0pt or NaN must get an image, not a panic.
        for size in [0.0, -5.0, f64::NAN, 10_000.0] {
            let image = renderer()
                .render_formula("x^2", false, INK, size)
                .unwrap_or_else(|err| panic!("{size}pt: {err}"));
            assert_png(&image, &format!("{size}pt"));
        }
    }

    #[test]
    fn unpremultiply_undoes_the_rasterizer_alpha() {
        // Opaque pixels are copied bit-for-bit, which is every pixel of a rendered slide.
        assert_eq!(
            unpremultiply(&[10, 20, 30, 255, 0, 0, 0, 255]),
            vec![10, 20, 30, 255, 0, 0, 0, 255]
        );
        // Half-transparent white is stored premultiplied as (128, 128, 128, 128).
        assert_eq!(
            unpremultiply(&[128, 128, 128, 128]),
            vec![255, 255, 255, 128]
        );
        // Fully transparent pixels hold no color, whatever the rasterizer left there.
        assert_eq!(unpremultiply(&[7, 8, 9, 0]), vec![0, 0, 0, 0]);
    }
}
