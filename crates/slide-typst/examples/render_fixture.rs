//! Renders the layout fixture to PDF (and optionally one PNG per page).
//!
//!   cargo run -p slide-typst --example render_fixture -- [fixture.json] [out-dir] [--png] [--thumbs 560] [--kinds a,b] [--aspect ratio_32x9] [--assets manifest.json] [--theme light-notes] [--chart-boxes]
//!
//! Defaults: fixtures/all-layout-blueprints.json → target/slide-typst/, only the
//! kinds that have a Typst layout (pass `--kinds all` to include placeholders),
//! theme `light-notes`. `--chart-boxes` also writes `chart-boxes.json`: the
//! plot box of every chart_focus slide, to render chart SVGs at (see
//! scripts/chart-fixtures.ts). `--thumbs <width_px>` writes `thumb-NN-<kind>.png`
//! per page through `Renderer::render_png` (the path the app's thumbnails take)
//! and prints the whole-deck and per-page timings.

use std::path::PathBuf;
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mut positional = Vec::new();
    let mut png = false;
    let mut kinds: Option<String> = None;
    let mut aspect = "ratio_16x9".to_string();
    let mut theme = "light-notes".to_string();
    let mut manifest: Option<PathBuf> = None;
    let mut chart_boxes = false;
    let mut thumbs: Option<u32> = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--png" => png = true,
            "--kinds" => kinds = args.next(),
            "--aspect" => aspect = args.next().unwrap_or(aspect),
            "--theme" => theme = args.next().unwrap_or(theme),
            "--assets" => manifest = args.next().map(PathBuf::from),
            "--chart-boxes" => chart_boxes = true,
            "--thumbs" => {
                thumbs = Some(
                    args.next()
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(560),
                )
            }
            _ => positional.push(arg),
        }
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixture = positional
        .first()
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("fixtures/all-layout-blueprints.json"));
    let out_dir = positional
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target/slide-typst"));
    std::fs::create_dir_all(&out_dir)?;

    let deck: Vec<serde_json::Value> = serde_json::from_str(&std::fs::read_to_string(&fixture)?)?;
    let wanted: Vec<String> = match kinds.as_deref() {
        Some("all") => Vec::new(),
        Some(list) => list.split(',').map(str::to_string).collect(),
        None => slide_typst::SUPPORTED_KINDS
            .iter()
            .map(|k| k.to_string())
            .collect(),
    };
    let slides: Vec<_> = deck
        .into_iter()
        .filter(|s| wanted.is_empty() || wanted.iter().any(|k| s["kind"] == k.as_str()))
        .collect();

    let started = Instant::now();
    let renderer = slide_typst::Renderer::with_system_fonts();
    println!("fonts scanned in {:?}", started.elapsed());

    let options = slide_typst::ExportOptions {
        title: Some("Layout fixture".into()),
        theme,
        aspect_ratio: aspect,
        assets: match &manifest {
            Some(path) => slide_typst::Assets::from_manifest(path)?,
            None => slide_typst::Assets::default(),
        },
        ..Default::default()
    };
    let started = Instant::now();
    let rendered = renderer.render_pdf(&slides, &options)?;
    let pdf_path = out_dir.join("fixture.pdf");
    std::fs::write(&pdf_path, &rendered.pdf)?;
    println!(
        "{} slides → {} pages, {} ({} KB) in {:?}",
        slides.len(),
        rendered.pages,
        pdf_path.display(),
        rendered.pdf.len() / 1024,
        started.elapsed()
    );
    for overflow in &rendered.overflows {
        println!("  overflow: {overflow:?}");
    }
    for missing in &rendered.missing_assets {
        println!("  missing image: {missing:?}");
    }

    if chart_boxes {
        let boxes = renderer.chart_boxes(&slides, &options)?;
        let path = out_dir.join("chart-boxes.json");
        std::fs::write(&path, serde_json::to_string_pretty(&boxes)?)?;
        println!("  {} chart boxes → {}", boxes.len(), path.display());
    }

    if png {
        let document = renderer.compile(&slides, &options)?.document;
        for (i, page) in document.pages().iter().enumerate() {
            // Default 2 px per pt ≈ the 1280×720 canvas at 1.5×.
            let pixmap = typst_render::render(page, &Default::default());
            // Pages beyond the slide count only exist when a layout overflowed.
            let kind = slides
                .get(i)
                .and_then(|s| s["kind"].as_str())
                .unwrap_or("overflow");
            let path = out_dir.join(format!("{:02}-{kind}.png", i + 1));
            pixmap.save_png(&path)?;
            println!("  {}", path.display());
        }
    }

    if let Some(width) = thumbs {
        // The whole deck is compiled and rasterized in one call, exactly as the app's
        // `render_slide_thumbnails` command does; the timings are the reported numbers.
        let started = Instant::now();
        let rendered = renderer.render_png(&slides, &options, width, None)?;
        let elapsed = started.elapsed();
        for page in &rendered.pages {
            let kind = slides
                .get(page.index)
                .and_then(|s| s["kind"].as_str())
                .unwrap_or("overflow");
            let path = out_dir.join(format!("thumb-{:02}-{kind}.png", page.index));
            std::fs::write(&path, &page.png)?;
            println!("  {} ({}×{})", path.display(), page.width, page.height);
        }
        let count = rendered.pages.len().max(1);
        println!(
            "thumbs at {width}px: {} pages in {:?}, {:?} per page",
            rendered.pages.len(),
            elapsed,
            elapsed / count as u32
        );
        for overflow in &rendered.overflows {
            println!("  overflow: {overflow:?}");
        }
    }
    Ok(())
}
