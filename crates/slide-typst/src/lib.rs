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

pub use assets::{chart_asset_ref, AssetData, Assets, MissingAsset};

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
    (
        "/themes/notes.typ",
        include_str!("../typst/themes/notes.typ"),
    ),
    (
        "/themes/light-terminal.typ",
        include_str!("../typst/themes/light-terminal.typ"),
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

const DEFAULT_FONT: &str = "Microsoft YaHei";

/// Theme ids with a Typst token file under `typst/themes/`; unknown ids fall
/// back to `light-notes` (see `themes` in `typst/main.typ`).
pub const SUPPORTED_THEMES: &[&str] = &[
    "light-notes",
    "light-terminal",
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
}

impl FontRoles {
    /// Every role uses `family`, followed by common CJK fallbacks.
    pub fn uniform(family: &str) -> Self {
        let list = Self::with_fallbacks(family);
        Self {
            title: list.clone(),
            subtitle: list.clone(),
            body: list.clone(),
            caption: list.clone(),
            number: list.clone(),
            code: [
                vec!["Cascadia Mono".to_string(), "Consolas".to_string()],
                list,
            ]
            .concat(),
        }
    }

    pub fn with_fallbacks(family: &str) -> Vec<String> {
        let mut list = vec![family.to_string()];
        for fallback in [DEFAULT_FONT, "PingFang SC", "Noto Sans CJK SC", "SimHei"] {
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
    /// Image reference → path and size; unusable references are absent.
    assets: &'a HashMap<String, assets::ImageInfo>,
    slides: &'a [serde_json::Value],
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

/// The plot box of a `chart_focus` slide (`<chart-box>` marker in chart-focus.typ), in CSS px
/// of the slide canvas, plus the slide's `--type-scale`. The app renders each chart's SVG at
/// exactly this size and scale so it embeds 1:1 (see `Renderer::chart_boxes`).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ChartBox {
    /// Index into the `slides` passed to the renderer.
    pub slide: usize,
    pub width: f64,
    pub height: f64,
    pub scale: f64,
}

/// A laid-out deck plus layout diagnostics.
pub struct Compiled {
    pub document: PagedDocument,
    pub overflows: Vec<Overflow>,
    pub missing_assets: Vec<MissingAsset>,
    pub chart_boxes: Vec<ChartBox>,
}

/// Exported PDF bytes plus layout diagnostics.
pub struct Rendered {
    pub pdf: Vec<u8>,
    pub pages: usize,
    pub overflows: Vec<Overflow>,
    /// Image references that rendered as placeholders, with the reason.
    pub missing_assets: Vec<MissingAsset>,
}

/// A compile failure with Typst's diagnostics resolved to `file:line:col`.
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

    /// Plot boxes of every chart_focus slide. The chart SVGs are rendered by the app (ECharts
    /// SSR) and must be sized before the export, so the app lays the deck out once without them
    /// (the layout does not depend on whether the SVG is present), renders each chart at its
    /// box, and then calls `render_pdf` with the SVGs in `options.assets`.
    pub fn chart_boxes(
        &self,
        slides: &[serde_json::Value],
        options: &ExportOptions,
    ) -> Result<Vec<ChartBox>, RenderError> {
        Ok(self.compile(slides, options)?.chart_boxes)
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
        // Typst rejects (`rac{1}`, `ef{x}` …) would fail the whole export:
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
        let chart_boxes = query_markers(&document, "chart-box");
        Ok(Compiled {
            document,
            overflows,
            missing_assets: prepared.missing,
            chart_boxes,
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
        }
    }
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
