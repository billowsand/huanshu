//! Standalone project export: the archived files alone must compile with a
//! filesystem-backed Typst world — what `typst compile main.typ` does from the
//! extracted project root. The one thing this cannot cover is the Typst CLI's
//! system-font scan; the project deliberately carries the full configured font
//! chain so a system that has those fonts renders it exactly like the app.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use slide_typst::{Assets, ExportOptions, FontRoles, ProjectFile, Renderer, TypstProject};
use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};
use typst_kit::fonts::FontStore;
use typst_layout::PagedDocument;

fn renderer() -> &'static Renderer {
    static RENDERER: OnceLock<Renderer> = OnceLock::new();
    RENDERER.get_or_init(Renderer::with_system_fonts)
}

fn file_id(path: &str) -> FileId {
    RootedPath::new(
        VirtualRoot::Project,
        VirtualPath::new(path).expect("valid virtual path"),
    )
    .intern()
}

/// A Typst world rooted at the extracted project directory. `/x` inside the
/// templates resolves to `<root>/x`, like the Typst CLI's project root.
struct DirWorld {
    root: PathBuf,
    main: FileId,
    library: LazyHash<Library>,
    fonts: FontStore,
    sources: Mutex<HashMap<FileId, Source>>,
}

impl DirWorld {
    /// `root` is Typst's project root; `main` is the main file's path inside it
    /// (`/main.typ` when the root is the project directory).
    fn new(root: &Path, main: &str) -> Self {
        let mut fonts = FontStore::new();
        fonts.extend(typst_kit::fonts::system());
        Self {
            root: root.to_path_buf(),
            main: file_id(main),
            library: LazyHash::new(Library::default()),
            fonts,
            sources: Mutex::new(HashMap::new()),
        }
    }

    fn virtual_path(id: FileId) -> &'static str {
        id.get().vpath().get_with_slash()
    }

    fn on_disk(&self, path: &str) -> PathBuf {
        self.root.join(path.trim_start_matches('/'))
    }
}

impl World for DirWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        self.fonts.book()
    }

    fn main(&self) -> FileId {
        self.main
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if let Some(source) = self.sources.lock().unwrap().get(&id) {
            return Ok(source.clone());
        }
        let path = Self::virtual_path(id);
        let text = std::fs::read_to_string(self.on_disk(path))
            .map_err(|error| FileError::Other(Some(format!("read {path}: {error}").into())))?;
        let source = Source::new(id, text);
        self.sources.lock().unwrap().insert(id, source.clone());
        Ok(source)
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        let path = Self::virtual_path(id);
        std::fs::read(self.on_disk(path))
            .map(Bytes::new)
            .map_err(|error| FileError::Other(Some(format!("read {path}: {error}").into())))
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.font(index)
    }

    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        None
    }
}

fn write_project(files: &[ProjectFile], root: &Path) {
    for file in files {
        let target = root.join(&file.path);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).expect("create project dir");
        }
        std::fs::write(&target, &file.bytes)
            .unwrap_or_else(|error| panic!("write {}: {error}", target.display()));
    }
}

fn temp_dir(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("slide-typst-project-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// A deck that exercises the layouts, charts, images and tinted icons.
fn deck() -> Vec<serde_json::Value> {
    let charts: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/charts.json")).expect("charts fixture");
    vec![
        serde_json::json!({
            "kind": "cover",
            "title": "工程导出",
            "subtitle": "完整 Typst 项目",
            "footer": "幻述",
        }),
        serde_json::json!({
            "kind": "feature_grid",
            "title": "功能",
            "cards": [
                // Inline math becomes a sentinel span in deck.json and is eval'd by
                // the templates; the project must carry the vendored mitex scope and
                // the MATH font chain for this to compile.
                { "title": "数据", "body": "正文与公式 $E = mc^2$", "icon": "i-carbon:data-base", "tone": "blue" },
                { "title": "增长", "body": "正文", "icon": "i-carbon:chart-line", "tone": "green" },
                { "title": "提醒", "body": "正文", "icon": "i-carbon:warning-alt", "tone": "amber" },
            ],
        }),
        charts[0].clone(),
        charts[1].clone(),
        serde_json::json!({
            "kind": "spotlight",
            "title": "图片",
            "image": "figure/arch.png",
        }),
        serde_json::json!({
            "kind": "closing",
            "title": "结束",
            "subtitle": "谢谢观看",
        }),
    ]
}

fn options() -> ExportOptions {
    let mut assets = Assets::new();
    assets.insert_bytes(
        "figure/arch.png",
        include_bytes!("fixtures/assets/arch.png").to_vec(),
    );
    ExportOptions {
        theme: "light-notes".to_string(),
        aspect_ratio: "ratio_16x9".to_string(),
        // A family no test machine has: it must stay in deck.json (the project may
        // be compiled where it *is* installed) and must not break the export.
        fonts: FontRoles::uniform("No Such Font For Project Test"),
        title: Some("工程导出测试".to_string()),
        assets,
        ..Default::default()
    }
}

fn entry<'a>(project: &'a TypstProject, path: &str) -> &'a ProjectFile {
    project
        .files
        .iter()
        .find(|file| file.path == path)
        .unwrap_or_else(|| panic!("project is missing {path}: {:?}", paths(project)))
}

fn paths(project: &TypstProject) -> Vec<&str> {
    project
        .files
        .iter()
        .map(|file| file.path.as_str())
        .collect()
}

#[test]
fn the_archive_holds_every_source_and_compiles_outside_the_app() {
    let slides = deck();
    let project = renderer()
        .export_project(&slides, &options())
        .expect("project export");

    // The sources are the real templates, with root-relative paths.
    for path in [
        "main.typ",
        "deck.json",
        "lib/base.typ",
        "lib/components.typ",
        "lib/chart.typ",
        "themes/notes.typ",
        "themes/light-terminal.typ",
        "themes/light-editorial.typ",
        "themes/dark-terminal.typ",
        "themes/dark-yellow.typ",
        "themes/dark-blue.typ",
        "layouts/cover.typ",
        "layouts/chart-focus.typ",
        "vendor/mitex/specs/mod.typ",
        "vendor/mitex/specs/prelude.typ",
        "vendor/mitex/specs/latex/standard.typ",
        "math-check.typ",
        "formula.typ",
    ] {
        assert!(!entry(&project, path).bytes.is_empty(), "{path} is empty");
    }
    assert!(
        project.files.iter().all(|file| !file.path.starts_with('/')),
        "archive paths must be relative: {:?}",
        paths(&project)
    );
    let mut unique = project
        .files
        .iter()
        .map(|file| &file.path)
        .collect::<Vec<_>>();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), project.files.len(), "duplicate archive paths");

    // Images and icons the layouts actually load are part of the project.
    let assets: Vec<&str> = project
        .files
        .iter()
        .filter(|file| file.path.starts_with("assets/"))
        .map(|file| file.path.as_str())
        .collect();
    assert_eq!(assets.len(), 1, "expected the spotlight image: {assets:?}");
    assert!(assets[0].ends_with(".png"), "{assets:?}");
    let icons: Vec<&ProjectFile> = project
        .files
        .iter()
        .filter(|file| file.path.starts_with("icons/carbon/"))
        .collect();
    assert!(
        !icons.is_empty(),
        "no tinted icons collected: {:?}",
        paths(&project)
    );
    for icon in icons {
        assert!(
            icon.path.ends_with(".svg"),
            "icon is not an SVG: {}",
            icon.path
        );
        assert!(
            icon.bytes.starts_with(b"<svg"),
            "icon does not parse as SVG: {}",
            icon.path
        );
    }

    // The deck data: slides were rewritten for math but are otherwise intact,
    // and the configured font chain is kept verbatim (not filtered to installed
    // families) so a machine that has the fonts renders it as the app does.
    let deck: serde_json::Value =
        serde_json::from_slice(&entry(&project, "deck.json").bytes).expect("deck.json parses");
    assert_eq!(deck["slides"].as_array().unwrap().len(), slides.len());
    assert_eq!(deck["theme"], "light-notes");
    assert_eq!(deck["aspect_ratio"], "ratio_16x9");
    // The asset path is relative to `layouts/` (where spotlight.typ calls
    // `image()`), not `/assets/...`: absolute paths require Typst's project root
    // to be the project directory, which the CLI does not guarantee on Windows.
    assert_eq!(deck["assets"]["figure/arch.png"]["path"], "../assets/0.png");
    let body = deck["fonts"]["body"].as_array().unwrap();
    assert_eq!(body[0], "No Such Font For Project Test");
    assert!(
        body.iter().any(|font| font == "Microsoft YaHei")
            || body.iter().any(|font| font == "PingFang SC"),
        "the CJK fallback chain is missing: {body:?}"
    );
    let math = deck["math_fonts"].as_array().unwrap();
    assert!(
        math.iter().any(|font| font == "Cambria Math"),
        "the MATH font chain is missing: {math:?}"
    );
    assert!(
        project
            .unavailable_fonts
            .iter()
            .any(|font| font == "No Such Font For Project Test"),
        "an uninstalled configured font must be reported: {:?}",
        project.unavailable_fonts
    );
    assert_eq!(project.pages, slides.len());
    assert!(
        project.missing_assets.is_empty(),
        "{:?}",
        project.missing_assets
    );

    // The real check: extract the archive and compile it from disk, with no help
    // from this crate's virtual world.
    let root = temp_dir("self-contained");
    write_project(&project.files, &root);
    let world = DirWorld::new(&root, "/main.typ");
    let result = typst::compile::<PagedDocument>(&world);
    let document = result.output.unwrap_or_else(|errors| {
        panic!(
            "exported project does not compile: {}",
            errors
                .iter()
                .map(|error| error.message.to_string())
                .collect::<Vec<_>>()
                .join("; ")
        )
    });
    assert_eq!(document.pages().len(), slides.len());
    let pdf = typst_pdf::pdf(&document, &typst_pdf::PdfOptions::default()).expect("PDF");
    assert!(pdf.starts_with(b"%PDF"));
    let _ = std::fs::remove_dir_all(&root);
}

/// Typst's project root is not always the project directory: the CLI derives it
/// from the main file, and a bare `main.typ` on Windows has made absolute `/...`
/// paths resolve to `C:\...` (users hit "file not found at
/// C:\vendor\mitex\specs\mod.typ"). The archive therefore only uses paths
/// relative to the files that contain them; this compiles it with the root one
/// directory above the project, as the CLI does when its root is not the project.
#[test]
fn the_project_compiles_when_the_typst_root_is_not_the_project_directory() {
    let slides = deck();
    let project = renderer()
        .export_project(&slides, &options())
        .expect("project export");

    // No template may rely on the project root for `/deck.json`, `/vendor/...`,
    // `/icons/...` or `/assets/...`.
    for file in &project.files {
        if !file.path.ends_with(".typ") {
            continue;
        }
        let text = String::from_utf8_lossy(&file.bytes);
        for absolute in ["import \"/", "json(\"/", "image(\"/"] {
            assert!(
                !text.contains(absolute),
                "{} still contains an absolute path ({absolute})",
                file.path
            );
        }
    }

    let outer = temp_dir("root-above-project");
    let project_dir = outer.join("deck");
    write_project(&project.files, &project_dir);
    let world = DirWorld::new(&outer, "/deck/main.typ");
    let document = typst::compile::<PagedDocument>(&world)
        .output
        .unwrap_or_else(|errors| {
            panic!(
                "the project does not compile when its root is the parent directory: {}",
                errors
                    .iter()
                    .map(|error| error.message.to_string())
                    .collect::<Vec<_>>()
                    .join("; ")
            )
        });
    assert_eq!(document.pages().len(), slides.len());
    let _ = std::fs::remove_dir_all(&outer);
}

#[test]
fn missing_images_are_reported_and_still_compile() {
    let slides = vec![
        serde_json::json!({
            "kind": "spotlight",
            "title": "缺图",
            "image": "media:404",
        }),
        serde_json::json!({ "kind": "closing", "title": "结束" }),
    ];
    let project = renderer()
        .export_project(&slides, &options())
        .expect("a missing image must not fail the project export");

    assert_eq!(project.missing_assets.len(), 1);
    assert_eq!(project.missing_assets[0].slide, 0);
    assert_eq!(project.missing_assets[0].reference, "media:404");
    assert!(
        !project
            .files
            .iter()
            .any(|file| file.path.starts_with("assets/")),
        "a missing reference must not leave an asset file behind"
    );

    let root = temp_dir("missing-image");
    write_project(&project.files, &root);
    let world = DirWorld::new(&root, "/main.typ");
    let document = typst::compile::<PagedDocument>(&world)
        .output
        .expect("the placeholder page must compile");
    assert_eq!(document.pages().len(), 2);
    let _ = std::fs::remove_dir_all(&root);
}

/// Manual end-to-end check against a **real** export: unpack the zip produced by
/// the app and point `TYPST_PROJECT_DIR` at the project directory. Ignored in
/// normal runs because it depends on a file outside the repository.
///
/// Set `TYPST_PROJECT_ROOT` to a parent directory to compile with the project
/// root *above* the project, like a Typst CLI invocation whose root is not the
/// project directory.
///
/// ```text
/// TYPST_PROJECT_DIR=C:\tmp\out\deck-typst cargo test -p slide-typst --test project -- --ignored
/// TYPST_PROJECT_DIR=C:\tmp\out\deck-typst TYPST_PROJECT_ROOT=C:\tmp\out cargo test -p slide-typst --test project -- --ignored
/// ```
#[test]
#[ignore = "manual: set TYPST_PROJECT_DIR to an unpacked app export"]
fn an_unpacked_app_export_compiles() {
    let root = match std::env::var("TYPST_PROJECT_DIR") {
        Ok(root) => PathBuf::from(root),
        Err(_) => return,
    };
    assert!(
        root.join("main.typ").is_file(),
        "{} has no main.typ",
        root.display()
    );
    // With TYPST_PROJECT_ROOT set, address the project through its parent: this is
    // the CLI case where `/...` paths would otherwise resolve outside the project.
    let (root_dir, main) = match std::env::var("TYPST_PROJECT_ROOT") {
        Ok(parent) => {
            let parent = PathBuf::from(parent);
            let project = root
                .strip_prefix(&parent)
                .expect("TYPST_PROJECT_DIR must be inside TYPST_PROJECT_ROOT")
                .to_string_lossy()
                .replace('\\', "/");
            (parent, format!("/{project}/main.typ"))
        }
        Err(_) => (root.clone(), "/main.typ".to_string()),
    };
    let world = DirWorld::new(&root_dir, &main);
    let document = typst::compile::<PagedDocument>(&world)
        .output
        .unwrap_or_else(|errors| {
            panic!(
                "the unpacked export does not compile: {}",
                errors
                    .iter()
                    .map(|error| error.message.to_string())
                    .collect::<Vec<_>>()
                    .join("; ")
            )
        });
    assert!(!document.pages().is_empty(), "no pages");
    let pdf = typst_pdf::pdf(&document, &typst_pdf::PdfOptions::default()).expect("PDF");
    assert!(pdf.starts_with(b"%PDF"));
    println!(
        "compiled {} pages from {}",
        document.pages().len(),
        root.display()
    );
}
