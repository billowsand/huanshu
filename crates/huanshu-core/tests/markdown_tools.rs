//! `markdown_tools` 与原版 TypeScript 函数的快照对照。
//!
//! 期望值由 `target/gen-markdown-tools.ts`（不提交的一次性脚本，函数体逐字拷贝自
//! `src/views/steps/Step1Prepare.vue` / `src/components/MarkdownWorkbench.vue`）生成。
//! 原版改动后重跑该脚本，Rust 实现必须跟着改到快照重新全绿。

use std::path::PathBuf;

use huanshu_core::markdown_tools as mt;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Estimate {
    auto: usize,
    h2: usize,
    h3: usize,
}

#[derive(Debug, Deserialize)]
struct Section {
    paragraphs: Vec<String>,
    subsections: Vec<Subsection>,
}

#[derive(Debug, Deserialize)]
struct Subsection {
    paragraphs: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Case {
    name: String,
    md: String,
    h2_count: usize,
    h3_count: usize,
    detected: String,
    estimate: Estimate,
    sections: Vec<Section>,
    split_h2: Vec<String>,
    split_h3: Vec<String>,
    auto_detect: String,
    clear_bold: String,
    remove_numbers: String,
    add_cn_section: String,
    add_cn_subsection: String,
    add_decimal: String,
    h1: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Snapshot {
    cases: Vec<Case>,
}

fn load() -> Snapshot {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/markdown-tools.json");
    let raw =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_str(&raw).expect("parse markdown-tools.json")
}

#[test]
fn markdown_tools_match_the_original_typescript() {
    let snapshot = load();
    assert!(
        !snapshot.cases.is_empty(),
        "snapshot has no cases — did gen-markdown-tools.ts run?"
    );
    for case in &snapshot.cases {
        let name = &case.name;
        assert_eq!(
            mt::count_exact_headings(&case.md, 2),
            case.h2_count,
            "{name}: h2_count"
        );
        assert_eq!(
            mt::count_exact_headings(&case.md, 3),
            case.h3_count,
            "{name}: h3_count"
        );

        let detected = match mt::detect_granularity(&case.md) {
            mt::Granularity::H2 => "h2",
            _ => "h3",
        };
        assert_eq!(detected, case.detected, "{name}: detected");

        assert_eq!(
            mt::estimate_slides(&case.md, mt::Granularity::Auto),
            case.estimate.auto,
            "{name}: estimate auto"
        );
        assert_eq!(
            mt::estimate_slides(&case.md, mt::Granularity::H2),
            case.estimate.h2,
            "{name}: estimate h2"
        );
        assert_eq!(
            mt::estimate_slides(&case.md, mt::Granularity::H3),
            case.estimate.h3,
            "{name}: estimate h3"
        );

        let sections: Vec<Section> = mt::parse_sections(&case.md)
            .into_iter()
            .map(|sec| Section {
                paragraphs: sec.paragraphs,
                subsections: sec
                    .subsections
                    .into_iter()
                    .map(|sub| Subsection {
                        paragraphs: sub.paragraphs,
                    })
                    .collect(),
            })
            .collect();
        assert_eq!(sections.len(), case.sections.len(), "{name}: section count");
        for (got, want) in sections.iter().zip(&case.sections) {
            assert_eq!(
                got.paragraphs, want.paragraphs,
                "{name}: section paragraphs"
            );
            assert_eq!(
                got.subsections.len(),
                want.subsections.len(),
                "{name}: subsection count"
            );
            for (got_sub, want_sub) in got.subsections.iter().zip(&want.subsections) {
                assert_eq!(
                    got_sub.paragraphs, want_sub.paragraphs,
                    "{name}: subsection paragraphs"
                );
            }
        }

        let trimmed = case.md.trim();
        let preview_input = if trimmed.is_empty() { "" } else { trimmed };
        assert_eq!(
            mt::split_preview_pages(preview_input, mt::Granularity::H2),
            case.split_h2,
            "{name}: split h2"
        );
        assert_eq!(
            mt::split_preview_pages(preview_input, mt::Granularity::H3),
            case.split_h3,
            "{name}: split h3"
        );

        assert_eq!(
            mt::auto_detect_headings(&case.md),
            case.auto_detect,
            "{name}: auto_detect"
        );
        assert_eq!(
            mt::clear_heading_bold(&case.md),
            case.clear_bold,
            "{name}: clear_bold"
        );
        assert_eq!(
            mt::remove_heading_numbers(&case.md),
            case.remove_numbers,
            "{name}: remove_numbers"
        );
        assert_eq!(
            mt::add_heading_numbers(&case.md, 2, mt::HeadingNumberStyle::CnSection),
            case.add_cn_section,
            "{name}: add_cn_section"
        );
        assert_eq!(
            mt::add_heading_numbers(&case.md, 3, mt::HeadingNumberStyle::CnSubsection),
            case.add_cn_subsection,
            "{name}: add_cn_subsection"
        );
        assert_eq!(
            mt::add_heading_numbers(&case.md, 4, mt::HeadingNumberStyle::Decimal),
            case.add_decimal,
            "{name}: add_decimal"
        );

        assert_eq!(
            mt::extract_h1(&case.md).as_deref(),
            case.h1.as_deref(),
            "{name}: h1"
        );
    }
}

#[test]
fn to_chinese_number_matches_the_original() {
    // 原版 toChineseNumber 的边界：≤10、十、11–19、整十、两位、≥100。
    let cases = [
        (1, "一"),
        (9, "九"),
        (10, "十"),
        (11, "十一"),
        (15, "十五"),
        (20, "二十"),
        (25, "二十五"),
        (99, "九十九"),
        (100, "100"),
    ];
    for (value, want) in cases {
        assert_eq!(
            mt::to_chinese_number(value),
            want,
            "to_chinese_number({value})"
        );
    }
}

#[test]
fn granularity_round_trips_through_strings() {
    for text in ["auto", "h2", "h3"] {
        let parsed = mt::Granularity::parse(text).expect(text);
        assert_eq!(parsed.as_str(), text);
    }
    assert!(mt::Granularity::parse("H2").is_none());
    assert!(mt::Granularity::parse("").is_none());
}
