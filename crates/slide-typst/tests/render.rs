//! End-to-end checks of the Typst export. One slide must always be one page:
//! an extra page means a layout overflowed its canvas instead of fitting or
//! clipping, which is the main regression these tests guard.

use std::sync::OnceLock;

use serde_json::{json, Value};
use slide_typst::{ExportOptions, Renderer, SUPPORTED_KINDS};

fn renderer() -> &'static Renderer {
    static RENDERER: OnceLock<Renderer> = OnceLock::new();
    RENDERER.get_or_init(Renderer::with_system_fonts)
}

fn load(path: &str) -> Vec<Value> {
    let full = format!("{}/{path}", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(full).unwrap()).unwrap()
}

fn render(slides: &[Value], options: &ExportOptions) -> slide_typst::Rendered {
    let rendered = renderer()
        .render_pdf(slides, options)
        .unwrap_or_else(|err| panic!("{err}"));
    assert!(rendered.pdf.starts_with(b"%PDF"), "output is not a PDF");
    assert_eq!(
        rendered.pages,
        slides.len(),
        "every slide must be exactly one page"
    );
    rendered
}

#[test]
fn fixture_deck_renders_one_page_per_slide_without_overflow() {
    let deck = load("../../fixtures/all-layout-blueprints.json");
    let rendered = render(&deck, &ExportOptions::default());
    assert!(
        rendered.overflows.is_empty(),
        "fixture overflowed: {:?}",
        rendered.overflows
    );
}

#[test]
fn fixture_covers_every_supported_kind() {
    let deck = load("../../fixtures/all-layout-blueprints.json");
    for kind in SUPPORTED_KINDS {
        assert!(
            deck.iter().any(|s| s["kind"] == *kind),
            "fixture lacks a {kind} slide"
        );
    }
}

#[test]
fn stress_deck_fits_or_reports_overflow() {
    let deck = load("tests/fixtures/stress.json");
    let options = ExportOptions {
        assets: fixture_assets(),
        ..Default::default()
    };
    let rendered = render(&deck, &options);
    // The 6-card feature grid and the spotlight with three overloaded panels
    // cannot fit even at the minimum factor: they must be clipped and reported,
    // never spill onto another page. Everything else fits.
    let overflowing: Vec<_> = rendered.overflows.iter().filter_map(|o| o.slide).collect();
    assert_eq!(
        overflowing,
        vec![1, 6],
        "unexpected overflow set: {:?}",
        rendered.overflows
    );
    for overflow in &rendered.overflows {
        assert!(overflow.needed_pt > overflow.available_pt);
    }
    assert!(
        rendered.missing_assets.is_empty(),
        "{:?}",
        rendered.missing_assets
    );
}

#[test]
fn model_text_is_never_evaluated_as_typst_markup() {
    // Would fail to compile (panic / unknown variable / unclosed math) if any of
    // these reached Typst as markup instead of text.
    let hostile = r#"#panic("x") #let a = 1 $x^2 *b* _i_ @missing <lbl> \ ] [ }"#;
    let slide = json!({
        "kind": "feature_grid",
        "title": hostile,
        "subtitle": hostile,
        "note": format!("<strong>{hostile}</strong><br/>&lt;b&gt;"),
        "cards": [{ "title": hostile, "tone": hostile, "icon": "i-carbon:../../etc", "body": hostile, "items": [hostile] }]
    });
    render(&[slide], &ExportOptions::default());
}

#[test]
fn wide_aspect_ratios_render() {
    let deck = load("../../fixtures/all-layout-blueprints.json");
    let slides: Vec<Value> = deck
        .into_iter()
        .filter(|s| SUPPORTED_KINDS.contains(&s["kind"].as_str().unwrap_or("")))
        .collect();
    for aspect in ["ratio_32x9", "ratio_48x9"] {
        let options = ExportOptions {
            aspect_ratio: aspect.to_string(),
            ..Default::default()
        };
        let document = renderer().compile(&slides, &options).unwrap().document;
        assert_eq!(document.pages().len(), slides.len());
        let size = document.pages()[0].frame.size();
        let ratio = size.x.to_pt() / size.y.to_pt();
        let expected = if aspect == "ratio_32x9" {
            32.0 / 9.0
        } else {
            48.0 / 9.0
        };
        assert!(
            (ratio - expected).abs() < 0.01,
            "{aspect}: page ratio {ratio}"
        );
    }
}

#[test]
fn unsupported_and_sparse_slides_still_render() {
    let slides = vec![
        json!({ "kind": "split_layers", "title": "无条目无层级" }),
        json!({ "kind": "section_list", "title": "无条目" }),
        json!({ "kind": "focus_example", "title": "无论点无排序" }),
        json!({ "kind": "outcome_grid", "title": "无卡片" }),
        json!({ "kind": "center_grid", "title": "无条目" }),
        json!({ "kind": "timeline", "title": "无事件" }),
        json!({ "kind": "process", "title": "无阶段" }),
        json!({ "kind": "overview", "title": "无条目" }),
        json!({ "kind": "closing", "title": "只有标题" }),
        json!({ "kind": "section_intro", "title": "无卡片" }),
        json!({ "kind": "issue_stack", "title": "无卡片" }),
        json!({ "kind": "spotlight", "title": "无图无面板" }),
        json!({ "kind": "made_up_kind", "title": "未知模板" }),
        json!({ "kind": "feature_grid", "title": "空卡片", "cards": [] }),
        json!({ "kind": "step_flow", "title": "无步骤" }),
        json!({ "kind": "compare", "title": "无对比数据" }),
        json!({ "kind": "swot", "title": "无象限" }),
        json!({ "kind": "cover", "title": "只有标题" }),
    ];
    render(&slides, &ExportOptions::default());
}

fn fixture_assets() -> slide_typst::Assets {
    let path = format!("{}/tests/fixtures/assets.json", env!("CARGO_MANIFEST_DIR"));
    slide_typst::Assets::from_manifest(std::path::Path::new(&path)).unwrap()
}

#[test]
fn spotlight_images_render_and_unusable_ones_are_reported() {
    let deck = load("tests/fixtures/spotlight.json");
    let options = ExportOptions {
        assets: fixture_assets(),
        ..Default::default()
    };
    let rendered = render(&deck, &options);
    assert!(rendered.overflows.is_empty(), "{:?}", rendered.overflows);
    // PNG, JPG, WebP, BMP (converted) and SVG load; a dangling reference and a
    // corrupt PNG become placeholders without failing the export.
    let missing: Vec<_> = rendered
        .missing_assets
        .iter()
        .map(|m| (m.slide, m.reference.as_str()))
        .collect();
    assert_eq!(missing, vec![(3, "media:99"), (3, "media:5")]);
}

#[test]
fn spotlight_without_asset_source_shows_placeholders() {
    let deck = load("tests/fixtures/spotlight.json");
    let rendered = render(&deck, &ExportOptions::default());
    let referenced: usize = deck
        .iter()
        .map(|s| {
            s["images"]
                .as_array()
                .map_or(s["image"].is_string() as usize, Vec::len)
        })
        .sum();
    assert_eq!(rendered.missing_assets.len(), referenced);
}

#[test]
fn spotlight_accepts_data_uris_and_dense_panels() {
    use base64::Engine;
    let png = std::fs::read(format!(
        "{}/tests/fixtures/assets/arch.png",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let uri = format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(png)
    );
    let long = "侧栏内容非常多时应该整体缩小字号，而不是溢出到画布外面。".repeat(2);
    let panel = |kind: &str| {
        json!({ "title": "面板", "kind": kind, "tone": "green", "body": long,
        "items": [long.clone(), long.clone(), long.clone(), long.clone()], "steps": [long.clone(), long.clone()],
        "highlight": long })
    };
    let slide = json!({ "kind": "spotlight", "title": "密集侧栏", "image": uri, "side_width": "w-[360px]",
        "panels": [panel("default"), panel("flow"), panel("matrix")] });
    let rendered = render(&[slide], &ExportOptions::default());
    assert!(
        rendered.missing_assets.is_empty(),
        "{:?}",
        rendered.missing_assets
    );
}
