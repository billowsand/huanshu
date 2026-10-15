//! End-to-end checks of the Typst export. One slide must always be one page:
//! an extra page means a layout overflowed its canvas instead of fitting or
//! clipping, which is the main regression these tests guard.

use std::sync::OnceLock;

use serde_json::{json, Value};
use slide_typst::{ExportOptions, Renderer, SUPPORTED_KINDS, SUPPORTED_THEMES};

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
fn missing_role_fonts_use_custom_fallback_for_the_entire_deck() {
    let slides = supported_fixture_slides();
    let mut fonts = slide_typst::FontRoles::uniform("Huanshu Missing Font 12345");
    let list = slide_typst::FontRoles::with_custom_fallback(
        "Huanshu Missing Font 12345",
        "Microsoft YaHei",
    );
    fonts.title = list.clone();
    fonts.subtitle = list.clone();
    fonts.body = list.clone();
    fonts.caption = list.clone();
    fonts.number = list.clone();
    fonts.code = list;
    fonts.math = vec!["Huanshu Missing Math 12345".into()];
    render(
        &slides,
        &ExportOptions {
            fonts: fonts.clone(),
            ..Default::default()
        },
    );
    renderer()
        .render_formula_with_fonts(r"\frac{1}{2} + \text{中文}", false, [0, 0, 0], 16.0, &fonts)
        .unwrap();
}

/// The layout fixture restricted to the kinds with a Typst layout.
fn supported_fixture_slides() -> Vec<Value> {
    load("../../fixtures/all-layout-blueprints.json")
        .into_iter()
        .filter(|s| SUPPORTED_KINDS.contains(&s["kind"].as_str().unwrap_or("")))
        .collect()
}

#[test]
fn every_theme_renders_the_fixture_deck() {
    let slides = supported_fixture_slides();
    for theme in SUPPORTED_THEMES {
        let options = ExportOptions {
            theme: theme.to_string(),
            ..Default::default()
        };
        let rendered = render(&slides, &options);
        assert!(
            rendered.overflows.is_empty(),
            "{theme} overflowed: {:?}",
            rendered.overflows
        );
    }
}

#[test]
fn stress_deck_overflows_match_notes_on_every_theme() {
    let deck = load("tests/fixtures/stress.json");
    for theme in SUPPORTED_THEMES {
        let options = ExportOptions {
            theme: theme.to_string(),
            assets: fixture_assets(),
            ..Default::default()
        };
        let rendered = render(&deck, &options);
        // Same contract as `stress_deck_fits_or_reports_overflow`: only the
        // 6-card feature grid and the overloaded spotlight may clip. Themes
        // change colors and radii, not the amount of content, so the overflow
        // set must be identical on every theme.
        let overflowing: Vec<_> = rendered.overflows.iter().filter_map(|o| o.slide).collect();
        assert_eq!(
            overflowing,
            vec![1, 6],
            "{theme}: unexpected overflow set: {:?}",
            rendered.overflows
        );
    }
}

#[test]
fn unknown_theme_falls_back_to_light_notes_byte_for_byte() {
    let slides = supported_fixture_slides();
    let fallback = render(
        &slides,
        &ExportOptions {
            theme: "no-such-theme".to_string(),
            ..Default::default()
        },
    );
    let notes = render(&slides, &ExportOptions::default());
    assert_eq!(
        fallback.pdf, notes.pdf,
        "an unknown theme must render exactly the light-notes output"
    );
}

#[test]
fn theme_dictionaries_share_one_interface() {
    // Compiles typst/themes/_check.typ: every theme module must expose the same
    // theme keys (plus lead/radius/tracking/deco/tones sub-keys) and a
    // background(w, h) painter.
    renderer()
        .check_theme_interface()
        .unwrap_or_else(|err| panic!("{err}"));
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
    // Would fail to compile (panic / unknown variable) if any of these reached
    // Typst as markup instead of text. `$x^2` has no closing `$`, so it stays
    // literal text; matched `$...$` spans are math by design (see next test).
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
fn latex_math_spans_render_as_equations() {
    // Inline and display math are converted via mitex and rendered; LaTeX that
    // fails conversion keeps its literal `$` text instead of failing the export.
    let slide = json!({
        "kind": "feature_grid",
        "title": "勾股定理 $a^2 + b^2 = c^2$ 与货币 $5 和 $10",
        "subtitle": "$$x_{1,2} = \\frac{-b \\pm \\sqrt{b^2-4ac}}{2a}$$",
        "cards": [
            { "title": "求和", "body": "$\\sum_{i=1}^n i = \\frac{n(n+1)}{2}$ 恒成立" },
            { "title": "坏公式回退", "body": "公式 $\\badcmd{$ 原样保留" },
        ]
    });
    let rendered = render(&[slide], &ExportOptions::default());
    assert_eq!(rendered.pages, 1);
}

#[test]
fn math_that_typst_rejects_falls_back_to_text_instead_of_failing_export() {
    // mitex converts all of these, but the converted source fails in Typst
    // (missing argument, unclosed delimiter, dangling label). Each must degrade
    // to literal text on its own while the valid formula still renders.
    let slide = json!({
        "kind": "feature_grid",
        "title": "坏公式 $\\frac{1}$ 与 $\\left($ 与 $\\ref{eq}$",
        "cards": [
            { "title": "箭头", "body": "$A \\xrightarrow{f} B$ 与 $\\xleftarrow[g]{h}$" },
            { "title": "正常", "body": "$x^2$" },
        ]
    });
    render(&[slide], &ExportOptions::default());
}

#[test]
fn sentinel_chars_in_model_text_are_never_evaluated() {
    // `\u{1}I…\u{2}` is the internal math-span marker; text that already
    // contains it must not reach `eval`.
    let slide = json!({
        "kind": "feature_grid",
        "title": "\u{1}I#panic(\"injected\")\u{2}",
        "cards": [{ "title": "a", "body": "\u{1}D#read(\"/deck.json\")\u{2}" }]
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

/// Every text run on a page, in drawing order.
fn page_texts(document: &typst_layout::PagedDocument, page: usize) -> Vec<String> {
    let mut runs = Vec::new();
    text_fills(&document.pages()[page].frame, &mut runs);
    runs.into_iter().map(|(text, _)| text).collect()
}

/// The five chart types are drawn natively (lib/chart.typ) in every theme and
/// aspect: one page each, nothing clipped, no assets involved.
#[test]
fn every_chart_type_renders_in_every_theme_and_aspect() {
    let deck = load("tests/fixtures/charts.json");
    let types: Vec<&str> = deck
        .iter()
        .map(|s| s["chart"]["type"].as_str().unwrap())
        .collect();
    assert_eq!(
        types,
        ["line", "bar", "donut", "grouped_bar", "stacked_area"]
    );
    for theme in SUPPORTED_THEMES {
        for aspect in ["ratio_16x9", "ratio_32x9", "ratio_48x9"] {
            let options = ExportOptions {
                theme: theme.to_string(),
                aspect_ratio: aspect.to_string(),
                ..Default::default()
            };
            let rendered = render(&deck, &options);
            assert!(
                rendered.overflows.is_empty(),
                "{theme} {aspect}: {:?}",
                rendered.overflows
            );
            assert!(
                rendered.missing_assets.is_empty(),
                "{theme} {aspect}: {:?}",
                rendered.missing_assets
            );
        }
    }
}

/// Axis ticks follow ECharts' nice intervals, values print with thousands
/// separators and without float tails, and donut percentages add up.
#[test]
fn chart_axes_and_labels_print_like_echarts() {
    let deck = load("tests/fixtures/charts.json");
    let document = renderer()
        .compile(&deck, &ExportOptions::default())
        .unwrap()
        .document;
    let line = page_texts(&document, 0);
    for tick in ["0", "10", "20", "30", "40", "50", "1月", "6月", "36", "41"] {
        assert!(
            line.iter().any(|t| t == tick),
            "line chart lacks {tick}: {line:?}"
        );
    }
    let bar = page_texts(&document, 1);
    assert!(
        bar.iter().any(|t| t == "600"),
        "530 must round up to 600: {bar:?}"
    );
    assert!(!bar.iter().any(|t| t == "540"), "{bar:?}");
    let grouped = page_texts(&document, 3);
    assert!(
        grouped.iter().any(|t| t == "180"),
        "170 → 0..180 by 30: {grouped:?}"
    );
    let stacked = page_texts(&document, 4);
    assert!(
        stacked.iter().any(|t| t == "120"),
        "stack total 119 → 120: {stacked:?}"
    );
    let donut = page_texts(&document, 2);
    for label in ["46%", "18%", "13%", "9%", "14%", "100", "合计"] {
        assert!(
            donut.iter().any(|t| t == label),
            "donut lacks {label}: {donut:?}"
        );
    }

    let edge = load("tests/fixtures/charts-edge.json");
    let document = renderer()
        .compile(&edge, &ExportOptions::default())
        .unwrap()
        .document;
    let big = page_texts(&document, 2);
    for label in ["1,234,567", "2,345,678.5", "3,456,789", "3,500,000"] {
        assert!(big.iter().any(|t| t == label), "lacks {label}: {big:?}");
    }
    let decimals = page_texts(&document, 3);
    assert!(decimals.iter().any(|t| t == "0.3"), "{decimals:?}");
    assert!(
        !decimals.iter().any(|t| t.contains("0000")),
        "float tail printed: {decimals:?}"
    );
    let negative = page_texts(&document, 0);
    for label in ["-45", "-80.5", "-100", "150"] {
        assert!(
            negative.iter().any(|t| t == label),
            "lacks {label}: {negative:?}"
        );
    }
    // Zero and negative sectors get no label; the rest still add up to 100%.
    let donut = page_texts(&document, 7);
    assert!(
        !donut.iter().any(|t| t == "零值" || t == "负值"),
        "{donut:?}"
    );
    assert!(donut.iter().any(|t| t == "50%"), "{donut:?}");
}

/// Malformed charts from old or hand-edited decks degrade to the placeholder
/// with the reason; they never fail the compile (one bad chart would fail the
/// whole export and every thumbnail of the deck).
#[test]
fn undrawable_charts_render_a_placeholder_instead_of_failing() {
    let edge = load("tests/fixtures/charts-edge.json");
    let rendered = render(&edge, &ExportOptions::default());
    assert!(rendered.overflows.is_empty(), "{:?}", rendered.overflows);
    let document = renderer()
        .compile(&edge, &ExportOptions::default())
        .unwrap()
        .document;
    let placeholder = |page: usize| {
        page_texts(&document, page)
            .iter()
            .any(|t| t == "图表待补充")
    };
    for (page, slide) in edge.iter().enumerate() {
        let expected = slide["title"].as_str().unwrap().starts_with("占位");
        assert_eq!(placeholder(page), expected, "page {page}");
    }
    let broken = vec![
        json!({ "kind": "chart_focus", "title": "缺 chart" }),
        json!({ "kind": "chart_focus", "title": "chart 不是对象", "chart": "bar" }),
        json!({ "kind": "chart_focus", "title": "data 不是数组", "chart": { "type": "bar", "data": 3 } }),
        json!({ "kind": "chart_focus", "title": "行不是对象", "chart": { "type": "bar", "data": [1, 2], "encoding": { "x": "a", "y": "b" } } }),
        json!({ "kind": "chart_focus", "title": "series 坏", "chart": { "type": "grouped_bar", "data": [{ "x": "a" }], "encoding": { "x": "x" }, "series": [3] } }),
        json!({ "kind": "chart_focus", "title": "布尔与 null", "chart": {
            "type": "bar", "title": "t", "data": [{ "x": true, "y": "1e3" }, { "x": null, "y": 2 }],
            "encoding": { "x": "x", "y": "y" }, "emphasis": { "highlight": "x" }
        } }),
    ];
    render(&broken, &ExportOptions::default());
}

/// Chart strings are data: markup in categories, series names or highlights
/// renders as text and never reaches Typst as code.
#[test]
fn chart_strings_are_never_evaluated_as_typst_markup() {
    let hostile = r#"#panic("x") ] [ $x$ <lbl> @ref *b*"#;
    let slides = vec![
        json!({ "kind": "chart_focus", "title": "t", "chart": {
            "type": "line", "title": hostile,
            "data": [{ "k": hostile, "v": 1 }, { "k": "b", "v": 2 }],
            "encoding": { "x": "k", "y": "v" }, "emphasis": { "highlight": [hostile] }
        } }),
        json!({ "kind": "chart_focus", "title": "t", "chart": {
            "type": "grouped_bar", "title": "t",
            "data": [{ "k": "a", "a": 1 }],
            "encoding": { "x": "k" }, "series": [{ "key": "a", "name": hostile }]
        } }),
        json!({ "kind": "chart_focus", "title": "t", "chart": {
            "type": "donut", "title": "t",
            "data": [{ "k": hostile, "v": 1 }, { "k": "b", "v": 3 }],
            "encoding": { "label": "k", "value": "v" }
        } }),
    ];
    let document = renderer()
        .compile(&slides, &ExportOptions::default())
        .unwrap()
        .document;
    for page in 0..slides.len() {
        let text = page_texts(&document, page).concat();
        assert!(text.contains(r#"#panic("x")"#), "page {page}: {text}");
    }
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

/// Text runs of a page with their fill, for checks on how text is colored.
fn text_fills(frame: &typst::layout::Frame, out: &mut Vec<(String, typst::visualize::Paint)>) {
    for (_, item) in frame.items() {
        match item {
            typst::layout::FrameItem::Group(group) => text_fills(&group.frame, out),
            typst::layout::FrameItem::Text(text) => {
                out.push((text.text.to_string(), text.fill.clone()))
            }
            _ => {}
        }
    }
}

#[test]
fn png_rendering_returns_only_the_requested_pages_at_the_requested_width() {
    let slides = supported_fixture_slides();
    let options = ExportOptions::default();
    let pages = renderer()
        .render_png(&slides, &options, 560, Some(&[0, 3]))
        .unwrap();
    assert_eq!(
        pages.pages.iter().map(|p| p.index).collect::<Vec<_>>(),
        vec![0, 3],
        "exactly the requested pages, in slide order"
    );
    for page in &pages.pages {
        assert!(
            page.png.starts_with(b"\x89PNG"),
            "slide {} is not a PNG",
            page.index
        );
        assert_eq!(page.width, 560);
        // 16:9 slides are 1280×720 CSS px; rounding may be off by a pixel.
        let expected = (560.0_f64 * 9.0 / 16.0).round() as i32;
        assert!(
            (page.height as i32 - expected).abs() <= 1,
            "slide {} height {} != {expected}",
            page.index,
            page.height
        );
    }

    // Wide slides keep their own ratio, and out-of-range indices are ignored.
    let wide = ExportOptions {
        aspect_ratio: "ratio_32x9".to_string(),
        ..Default::default()
    };
    let pages = renderer()
        .render_png(&slides, &wide, 560, Some(&[0, 9999]))
        .unwrap();
    assert_eq!(pages.pages.len(), 1);
    let expected = (560.0_f64 * 9.0 / 32.0).round() as i32;
    assert!(
        (pages.pages[0].height as i32 - expected).abs() <= 1,
        "32:9 height {} != {expected}",
        pages.pages[0].height
    );
}

#[test]
fn png_rendering_clamps_the_width_and_covers_the_whole_deck_by_default() {
    let slides = supported_fixture_slides();
    let options = ExportOptions::default();
    let all = renderer().render_png(&slides, &options, 560, None).unwrap();
    assert_eq!(all.pages.len(), slides.len());
    assert_eq!(
        all.pages.iter().map(|p| p.index).collect::<Vec<_>>(),
        (0..slides.len()).collect::<Vec<_>>()
    );

    // Narrower than the floor and wider than the ceiling are both clamped.
    for (requested, expected) in [(1u32, 64u32), (99_999, 3840)] {
        let pages = renderer()
            .render_png(&slides, &options, requested, Some(&[0]))
            .unwrap();
        assert_eq!(pages.pages[0].width, expected);
    }
}

#[test]
fn render_pixels_matches_the_decoded_png_of_the_same_compile() {
    let slides = supported_fixture_slides();
    let options = ExportOptions::default();
    let requested = [0usize, 3];
    let pngs = renderer()
        .render_png(&slides, &options, 560, Some(&requested))
        .unwrap();
    let frames = renderer()
        .render_pixels(&slides, &options, 560, Some(&requested))
        .unwrap();
    assert_eq!(
        frames.pages.iter().map(|p| p.index).collect::<Vec<_>>(),
        vec![0, 3],
        "exactly the requested pages, in slide order"
    );
    assert_eq!(pngs.pages.len(), frames.pages.len());
    for (png, frame) in pngs.pages.iter().zip(&frames.pages) {
        assert_eq!(frame.index, png.index);
        assert_eq!((frame.width, frame.height), (png.width, png.height));
        let decoded = image::load_from_memory(&png.png)
            .expect("rendered PNG decodes")
            .to_rgba8();
        assert_eq!(decoded.dimensions(), (frame.width, frame.height));
        assert_eq!(
            frame.pixels.as_slice(),
            decoded.as_raw().as_slice(),
            "slide {}: raw pixels differ from the decoded PNG",
            frame.index
        );
    }
}

#[test]
fn render_pixels_follows_the_png_width_rules() {
    let slides = supported_fixture_slides();
    let options = ExportOptions::default();
    // Narrower than the floor and wider than the ceiling are both clamped, and the buffer
    // holds exactly width × height RGBA8 pixels.
    for (requested, expected) in [(1u32, 64u32), (99_999, 3840)] {
        let frames = renderer()
            .render_pixels(&slides, &options, requested, Some(&[0]))
            .unwrap();
        assert_eq!(frames.pages.len(), 1);
        let page = &frames.pages[0];
        assert_eq!(page.width, expected);
        assert_eq!(
            page.pixels.len(),
            (page.width * page.height * 4) as usize,
            "pixel buffer does not match the reported size"
        );
    }

    // Wide slides keep their own ratio, and out-of-range indices are ignored.
    let wide = ExportOptions {
        aspect_ratio: "ratio_32x9".to_string(),
        ..Default::default()
    };
    let frames = renderer()
        .render_pixels(&slides, &wide, 560, Some(&[0, 9999]))
        .unwrap();
    assert_eq!(frames.pages.len(), 1);
    let expected = (560.0_f64 * 9.0 / 32.0).round() as i32;
    assert!(
        (frames.pages[0].height as i32 - expected).abs() <= 1,
        "32:9 height {} != {expected}",
        frames.pages[0].height
    );
}

#[test]
fn editorial_headline_sets_the_tail_in_the_accent() {
    // light-editorial splits cover / closing titles after the first full-width
    // comma or colon (src/utils/titleTail.ts) and colors the tail; other themes
    // keep one color.
    let slides = [
        json!({ "kind": "cover", "title": "拿件真事试，再判断行不行" }),
        json!({ "kind": "closing", "title": "谢谢阅读：少争论强不强" }),
    ];
    let fill_of = |theme: &str, page: usize, needle: &str| {
        let options = ExportOptions {
            theme: theme.to_string(),
            ..Default::default()
        };
        let document = renderer().compile(&slides, &options).unwrap().document;
        let mut runs = Vec::new();
        text_fills(&document.pages()[page].frame, &mut runs);
        runs.into_iter()
            .find(|(text, _)| text.contains(needle))
            .unwrap_or_else(|| panic!("{theme}: no text run containing {needle}"))
            .1
    };
    for (page, head, tail) in [(0, "拿件", "再判断"), (1, "谢谢", "少争论")] {
        assert_ne!(
            fill_of("light-editorial", page, head),
            fill_of("light-editorial", page, tail),
            "editorial page {page}: the tail must be set in the accent"
        );
        assert_eq!(
            fill_of("light-terminal", page, head),
            fill_of("light-terminal", page, tail),
            "terminal page {page}: the title stays one color"
        );
    }
}
