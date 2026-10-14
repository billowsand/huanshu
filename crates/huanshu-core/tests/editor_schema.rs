//! 编辑器模式表的回归测试。
//!
//! 快照 `fixtures/editor-schema.json` 是从 TypeScript 旧表导出的：字段名、标签、占位符、
//! 下拉选项、每种版式的默认内容与列表项模板都必须与它逐字一致，否则说明搬家时改了值。

use huanshu_core::editor_schema::editor_schema;
use huanshu_core::types::{SlideBlueprint, SlideKind};

const SNAPSHOT: &str = include_str!("fixtures/editor-schema.json");

fn snapshot() -> serde_json::Value {
    serde_json::from_str(SNAPSHOT).expect("the snapshot parses")
}

/// 快照里列表区/object 区是 `{kind: [...]}`，序列化后是 `[[kind, [...]], ...]`。
/// 两边都按 kind 排序后比较，顺序不参与判定。
fn sorted_pairs_from_object(value: &serde_json::Value) -> Vec<serde_json::Value> {
    let mut pairs: Vec<serde_json::Value> = value
        .as_object()
        .expect("an object keyed by kind")
        .iter()
        .map(|(kind, defs)| serde_json::json!([kind, defs]))
        .collect();
    pairs.sort_by_key(|pair| pair[0].as_str().unwrap().to_string());
    pairs
}

fn sorted_pairs_from_array(value: &serde_json::Value) -> Vec<serde_json::Value> {
    let mut pairs = value.as_array().expect("pairs").clone();
    pairs.sort_by_key(|pair| pair[0].as_str().unwrap().to_string());
    pairs
}

#[test]
fn the_schema_matches_the_snapshot_taken_from_typescript() {
    let schema = editor_schema();
    let actual = serde_json::to_value(&schema).unwrap();
    let expected = snapshot();

    assert_eq!(actual["simple_fields"], expected["simple_fields"]);
    assert_eq!(actual["tone_options"], expected["tone_options"]);
    assert_eq!(actual["direction_options"], expected["direction_options"]);
    assert_eq!(
        sorted_pairs_from_array(&actual["list_sections"]),
        sorted_pairs_from_object(&expected["list_sections"]),
    );
    assert_eq!(
        sorted_pairs_from_array(&actual["object_sections"]),
        sorted_pairs_from_object(&expected["object_sections"]),
    );
    assert_eq!(
        serde_json::to_value(&schema.kinds).unwrap(),
        expected["kinds"],
        "kind metadata, field lists and defaults must be unchanged"
    );
}

#[test]
fn every_slide_kind_has_meta_fields_and_defaults() {
    let schema = editor_schema();
    for kind in SlideKind::ALL {
        let name = kind.as_str();
        let entry = schema
            .kind(name)
            .unwrap_or_else(|| panic!("{name} has no entry in the editor schema"));
        assert!(!entry.label.is_empty(), "{name} needs a label");
        assert!(entry.icon.starts_with("i-carbon:"), "{name} icon");
        assert!(!entry.desc.is_empty(), "{name} needs a description");
        assert!(
            entry.fields.contains(&"kind".to_string()),
            "{name} must list the kind field"
        );
        assert!(
            !entry.defaults.is_null(),
            "{name} needs default content for a new slide"
        );
    }
}

#[test]
fn listed_fields_exist_on_a_serialized_blueprint() {
    let schema = editor_schema();
    let sample = serde_json::to_value(
        serde_json::from_value::<SlideBlueprint>(serde_json::json!({
            "kind": "feature_grid",
            "title": "t"
        }))
        .unwrap(),
    )
    .unwrap();
    let known: Vec<String> = sample.as_object().unwrap().keys().cloned().collect();
    for entry in &schema.kinds {
        for field in &entry.fields {
            assert!(
                known.contains(field),
                "{} lists `{field}`, which a serialized SlideBlueprint does not have",
                entry.kind
            );
        }
    }
}

#[test]
fn defaults_plus_kind_deserialize_into_a_blueprint() {
    for entry in editor_schema().kinds {
        let mut value = entry.defaults.clone();
        value
            .as_object_mut()
            .unwrap()
            .insert("kind".to_string(), serde_json::json!(entry.kind));
        let parsed: Result<SlideBlueprint, _> = serde_json::from_value(value);
        assert!(
            parsed.is_ok(),
            "{} defaults do not deserialize into a SlideBlueprint: {:?}",
            entry.kind,
            parsed.err()
        );
    }
}

#[test]
fn list_and_object_sections_are_reachable_by_kind() {
    let schema = editor_schema();
    assert!(schema
        .list_sections("overview")
        .iter()
        .any(|section| section.key == "overview_items"));
    assert!(schema.list_sections("no_such_kind").is_empty());
    assert!(schema
        .object_sections("compare")
        .iter()
        .any(|section| section.key == "compare_data.left"));

    // A nested list carries the template the editor writes when the user adds an item.
    let steps = schema
        .list_sections("process")
        .iter()
        .flat_map(|section| section.nested_lists.iter())
        .find(|nested| nested.key == "steps")
        .expect("process phases have steps");
    assert_eq!(steps.new_item["label"], serde_json::json!(""));
    assert_eq!(steps.new_item["desc"], serde_json::json!(""));
    assert!(
        steps.fields.iter().any(|field| field.key == "label"),
        "the template and the form must describe the same fields"
    );
}
