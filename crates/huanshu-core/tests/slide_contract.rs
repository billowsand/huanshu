use huanshu_core::{
    editor_schema::editor_schema,
    generator::utils::blueprint_schema_hint,
    slide_contract, slide_edit,
    types::{SlideBlueprint, SlideKind},
};
use serde_json::{json, Value};

#[test]
fn all_default_pages_and_their_typed_round_trips_obey_the_same_contract() {
    let schema = editor_schema();
    for entry in &schema.kinds {
        let mut slide = entry.defaults.clone();
        slide["kind"] = json!(entry.kind);
        let errors = slide_contract::errors(&schema, &slide, false);
        assert!(errors.is_empty(), "{} defaults: {errors:?}", entry.kind);
        let typed: SlideBlueprint = serde_json::from_value(slide.clone()).unwrap();
        let serialized = serde_json::to_value(typed).unwrap();
        let errors = slide_contract::errors(&schema, &serialized, false);
        assert!(errors.is_empty(), "{} typed JSON: {errors:?}", entry.kind);
        // No content authored by defaults can be lost in typed generation/storage.
        for (key, value) in slide.as_object().unwrap() {
            fn contains(expected: &Value, actual: &Value) -> bool {
                match expected {
                    Value::Object(map) => map.iter().all(|(k, v)| contains(v, &actual[k])),
                    Value::Array(items) => actual.as_array().is_some_and(|a| {
                        a.len() == items.len() && items.iter().zip(a).all(|(e, a)| contains(e, a))
                    }),
                    _ => expected == actual,
                }
            }
            assert!(
                contains(value, &serialized[key]),
                "{} lost {key}",
                entry.kind
            );
        }
    }
}

#[test]
fn unsupported_fields_report_paths_and_survive_form_edits_and_formatting() {
    let schema = editor_schema();
    for (kind, field) in [
        ("section_intro", "tag"),
        ("feature_grid", "tag"),
        ("feature_grid", "titleTone"),
        ("outcome_grid", "conclusion"),
        ("outcome_grid", "footer_tag"),
        ("outcome_grid", "footer_tone"),
        ("outcome_grid", "top_bar_class"),
    ] {
        let mut slide =
            json!({"kind":kind,"title":"标题","cards":[{"title":"卡片","body":"正文"}]});
        slide["cards"][0][field] = json!("输入内容");
        let errors = slide_edit::semantic_errors(&schema, &slide);
        assert!(
            errors
                .iter()
                .any(|e| e.contains(&format!("cards[0].{field}"))),
            "{kind}: {errors:?}"
        );
        assert_eq!(slide_edit::filter_slide_json(&schema, &slide), slide);
    }
    for kind in ["focus_example", "chart_focus"] {
        let slide = json!({"kind":kind,"title":"标题","points":[{"title":"要点","body":"说明","step":"01"}]});
        assert!(slide_edit::semantic_errors(&schema, &slide)
            .iter()
            .any(|e| e.contains("points[0].step")));
    }
}

#[test]
fn image_capability_and_json_types_are_enforced_before_saving() {
    let schema = editor_schema();
    for slide in [
        json!({"kind":"feature_grid","title":"标题","image":"media:12"}),
        json!({"kind":"outcome_grid","title":"标题","cards":[{"title":"卡片","body":12}]}),
        json!({"kind":"feature_grid","title":"标题","cards":[null]}),
        json!(false),
        json!([]),
    ] {
        assert!(
            !slide_edit::semantic_errors(&schema, &slide).is_empty(),
            "{slide}"
        );
    }
    let slide = json!({"kind":"spotlight","title":"图片","image":"media:12"});
    assert!(slide_edit::semantic_errors(&schema, &slide).is_empty());
}

#[test]
fn swot_keys_are_canonical_and_unique() {
    let schema = editor_schema();
    let mut slide = schema.kind("swot").unwrap().defaults.clone();
    slide["kind"] = json!("swot");
    slide["swot_data"]["quadrants"]
        .as_array_mut()
        .unwrap()
        .reverse();
    assert!(slide_edit::semantic_errors(&schema, &slide).is_empty());
    slide["swot_data"]["quadrants"][0]["key"] = json!("S");
    assert!(slide_edit::semantic_errors(&schema, &slide)
        .iter()
        .any(|e| e.contains("quadrants[0].key")));
    slide["swot_data"]["quadrants"][0]["key"] = slide["swot_data"]["quadrants"][1]["key"].clone();
    assert!(!slide_edit::semantic_errors(&schema, &slide).is_empty());
}

#[test]
fn spotlight_has_one_list_and_explicit_modes() {
    let schema = editor_schema();
    for mode in ["flow", "matrix", "steps"] {
        let slide = json!({"kind":"spotlight","title":"标题","panels":[{"kind":mode,"title":"动作","items":["甲","乙"]}]});
        assert!(slide_edit::semantic_errors(&schema, &slide).is_empty());
        let typed: SlideBlueprint = serde_json::from_value(slide).unwrap();
        assert_eq!(
            serde_json::to_value(typed).unwrap()["panels"][0]["items"],
            json!(["甲", "乙"])
        );
    }
    for panel in [
        json!({"kind":"text","title":"标题","items":["不会显示"]}),
        json!({"kind":"flow","title":"标题","steps":["旧入口"]}),
        json!({"kind":"unknown","title":"标题"}),
    ] {
        let slide = json!({"kind":"spotlight","title":"标题","panels":[panel]});
        assert!(!slide_edit::semantic_errors(&schema, &slide).is_empty());
    }
}

#[test]
fn generation_uses_form_contracts_including_enums() {
    let schema = editor_schema();
    for kind in SlideKind::ALL {
        let actual: Value = serde_json::from_str(blueprint_schema_hint(&kind)).unwrap();
        let mut expected = slide_contract::json_schema(&schema, kind.as_str()).unwrap();
        for field in ["section", "aspect_ratio"] {
            expected["properties"]
                .as_object_mut()
                .unwrap()
                .remove(field);
        }
        if *kind == SlideKind::ChartFocus {
            expected["properties"]
                .as_object_mut()
                .unwrap()
                .remove("chart");
        }
        assert_eq!(actual, expected);
    }
}

#[test]
fn unsupported_chart_trend_is_rejected_by_editor_and_markdown_import() {
    let chart = json!({"type":"bar","title":"图","data":[{"x":"甲","y":1}],"encoding":{"x":"x","y":"y"},"emphasis":{"trend":"up"}});
    let errors = huanshu_core::chart_check::chart_problems_json(&chart);
    assert!(errors
        .iter()
        .any(|e| e.message_zh().contains("emphasis.trend")));
    let markdown = format!("# 文稿\n\n## 章节\n\n```chart\n{chart}\n```\n");
    assert!(huanshu_core::input::parse_markdown(&markdown)
        .unwrap_err()
        .to_string()
        .contains("emphasis.trend"));
}
