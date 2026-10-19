//! 编辑器模式表的回归测试。
//!
//! 快照 `fixtures/editor-schema.json` 是从 TypeScript 旧表导出的：字段名、标签、占位符、
//! 下拉选项、每种版式的默认内容与列表项模板都必须与它逐字一致，否则说明搬家时改了值。

use huanshu_core::editor_schema::{editor_schema, EditorField};
use huanshu_core::slide_edit as edit;
use huanshu_core::types::{SlideBlueprint, SlideKind};
use serde_json::{json, Value};

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
fn the_schema_matches_the_contract_snapshot() {
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
    for entry in &schema.kinds {
        let mut value = entry.defaults.clone();
        value["kind"] = json!(entry.kind);
        let typed: SlideBlueprint = serde_json::from_value(value).unwrap();
        let sample = serde_json::to_value(typed).unwrap();
        for field in &entry.fields {
            assert!(
                sample.get(field).is_some(),
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

// ── 表单 ↔ JSON 一致性（第三步审计的不变量） ─────────────────────────────────
//
// step3.rs 的表单、`json_text`（`filter_slide_json` 之后）与 `create_kind_blueprint`
// （新页蓝图）三方的键集合都必须从 `editor_schema` 的同一批定义推导。下面这组测试把
// 逐字段核对得到的结论钉死：将来给任何 kind 加字段 / 加区块时，漏登记进 `fields`
// （表单改了被过滤掉）、漏写进 `defaults`（新页一编辑就丢）、或者类型混用（布尔写成
// 字符串），都会在这里红。

/// 核对 (b)：每种 kind 的列表区 / 对象区 / 默认内容的键都登记在 `fields` 里——
/// `filter_slide_json` 只保留 `fields`，漏登记的键会在表单第一次改动后从
/// `json_text` 里消失（应用更改时写回的就是这份过滤结果）。
#[test]
fn every_section_and_default_key_is_listed_in_kind_fields() {
    let schema = editor_schema();
    for entry in &schema.kinds {
        let fields: Vec<&str> = entry.fields.iter().map(String::as_str).collect();
        for section in schema.list_sections(&entry.kind) {
            assert!(
                fields.contains(&section.key.as_str()),
                "{}: list section `{}` is not in fields; edits there would be dropped by filter_slide_json",
                entry.kind,
                section.key
            );
        }
        for section in schema.object_sections(&entry.kind) {
            let top = section.key.split('.').next().unwrap();
            assert!(
                fields.contains(&top),
                "{}: object section `{}` (top key `{top}`) is not in fields; edits there would be dropped by filter_slide_json",
                entry.kind,
                section.key
            );
        }
        for key in entry
            .defaults
            .as_object()
            .expect("defaults is an object")
            .keys()
        {
            assert!(
                fields.contains(&key.as_str()),
                "{}: default key `{key}` is not in fields; a fresh slide would lose it on the first form edit",
                entry.kind
            );
        }
    }
}

/// 核对 (a) 覆盖：每个 kind 登记在 `fields` 里的键都有表单控件——简单字段（含
/// `image` / `placeholder` / `side_width` 等）、`badges` / `images` 字符串数组，
/// 或某个列表区 / 对象区。只有 `kind` 本身例外（它由 KindPicker 更换）。
#[test]
fn every_listed_field_has_a_form_control() {
    let schema = editor_schema();
    let simple: Vec<&str> = schema
        .simple_fields
        .iter()
        .map(|field| field.key.as_str())
        .collect();
    for entry in &schema.kinds {
        let sections: Vec<&str> = schema
            .list_sections(&entry.kind)
            .iter()
            .map(|section| section.key.as_str())
            .chain(
                schema
                    .object_sections(&entry.kind)
                    .iter()
                    .map(|section| section.key.split('.').next().unwrap()),
            )
            .collect();
        for field in &entry.fields {
            if field == "kind" {
                continue;
            }
            assert!(
                simple.contains(&field.as_str())
                    || matches!(field.as_str(), "badges" | "images")
                    || sections.contains(&field.as_str()),
                "{} lists `{field}`, but no form control renders it",
                entry.kind
            );
        }
    }
}

/// 核对任务 3：新建一页（`create_kind_blueprint`，KindPicker「新建」走的路径）后，
/// 蓝图原样穿过 `filter_slide_json`——表单显示的区块、JSON 文本里的键、新页内容
/// 三方完全一致（`aspect_ratio` 那类有意保留另算，蓝图里本来就没有它）。
#[test]
fn a_fresh_blueprint_survives_its_own_filter() {
    let schema = editor_schema();
    for entry in &schema.kinds {
        let slide = edit::create_kind_blueprint(&schema, &entry.kind, None);
        let filtered = edit::filter_slide_json(&schema, &slide);
        assert_eq!(
            filtered, slide,
            "{}: a fresh slide must round-trip filter_slide_json unchanged",
            entry.kind
        );
    }
}

/// 路径顶层键：`cards[0].title` → `cards`、`compare_data.left.items[1].label`
/// → `compare_data`。
fn top_key(path: &str) -> &str {
    path.split(|c: char| c == '.' || c == '[').next().unwrap()
}

/// 按表单控件的类型往 `prefix.key` 写一个类型正确的值，并立即按表单读取的方式读回
/// （`string_value` / `bool_value`）。`touched` 记下写过的顶层键。
fn write_field(slide: &mut Value, prefix: &str, field: &EditorField, touched: &mut Vec<String>) {
    let path = if prefix.is_empty() {
        field.key.clone()
    } else {
        format!("{prefix}.{}", field.key)
    };
    match field.field_type.as_str() {
        "boolean" => {
            edit::set_at_path(slide, &path, json!(true));
            assert!(
                edit::bool_value(slide, &path),
                "{path}: the checkbox must read back true"
            );
        }
        "select" => {
            let value = field
                .options
                .clone()
                .unwrap_or_default()
                .first()
                .cloned()
                .expect("a select field offers options");
            edit::set_at_path(slide, &path, json!(value));
            assert_eq!(
                edit::string_value(slide, &path),
                value,
                "{path}: the select must read back its option"
            );
        }
        _ => {
            edit::set_at_path(slide, &path, json!("改"));
            assert_eq!(
                edit::string_value(slide, &path),
                "改",
                "{path}: the input must read back its text"
            );
        }
    }
    let top = top_key(&path).to_string();
    if !touched.contains(&top) {
        touched.push(top);
    }
}

/// 对「数组路径 + 区块定义」做一遍列表区级别的表单演练：第 0 项的每个字段、每个
/// 字符串列表、每个嵌套列表（递归走对象区的子列表区）。
fn exercise_list_section(
    slide: &mut Value,
    array_path: &str,
    fields: &[EditorField],
    string_lists: &[huanshu_core::editor_schema::StringListDef],
    nested_lists: &[huanshu_core::editor_schema::NestedListDef],
    touched: &mut Vec<String>,
) {
    let item_path = format!("{array_path}[0]");
    for field in fields {
        write_field(slide, &item_path, field, touched);
    }
    for list in string_lists {
        let path = format!("{item_path}.{}[0]", list.key);
        edit::set_at_path(slide, &path, json!("项"));
        assert_eq!(
            edit::string_value(slide, &path),
            "项",
            "{path}: a string-list item must read back"
        );
        touched.push(top_key(&path).to_string());
    }
    for nested in nested_lists {
        exercise_list_section(
            slide,
            &format!("{item_path}.{}", nested.key),
            &nested.fields,
            &[],
            &[],
            touched,
        );
    }
}

/// 核对 (a)+(c)+(d) 的端到端走查：对每种 kind，从新建蓝图出发，把表单上每个控件
/// 都按它的类型写一遍（文本/下拉写字符串、勾选框写布尔、字符串列表写数组项），
/// 然后要求：
/// - 写进去的值能按表单的读取方式原样读回（JSON 里手改的值表单也能显示）；
/// - 写过的每个顶层键都仍然出现在 `filter_slide_json` 的结果里（表单改动会进
///   `json_text`，应用更改时不会被悄悄丢掉）。
#[test]
fn form_edits_round_trip_and_survive_the_filter() {
    let schema = editor_schema();
    for entry in &schema.kinds {
        let mut slide = edit::create_kind_blueprint(&schema, &entry.kind, None);
        let mut touched: Vec<String> = Vec::new();

        // ③ 基础信息（按该 kind 的 fields 过滤，与 render_form 同一判据）。
        for field in schema
            .simple_fields
            .iter()
            .filter(|field| entry.fields.contains(&field.key))
        {
            write_field(&mut slide, "", field, &mut touched);
        }
        // ④ 标签组 / ⑤ 图集：字符串数组（标签行 / 素材条的写入路径等价）。
        for key in ["badges", "images"] {
            if entry.fields.iter().any(|field| field == key) {
                let path = format!("{key}[0]");
                edit::set_at_path(&mut slide, &path, json!("项"));
                assert_eq!(edit::string_value(&slide, &path), "项");
                touched.push(key.to_string());
            }
        }
        // ⑥ 列表区（含字符串列表、嵌套列表）。
        for section in schema.list_sections(&entry.kind) {
            exercise_list_section(
                &mut slide,
                &section.key,
                &section.fields,
                &section.string_lists,
                &section.nested_lists,
                &mut touched,
            );
        }
        // ⑦ 对象区：区字段 + 子列表区（compare 的左右两侧、swot 的四象限、
        // chart_focus 的图表标题）。
        for section in schema.object_sections(&entry.kind) {
            for field in &section.fields {
                write_field(&mut slide, &section.key, field, &mut touched);
            }
            for sub in &section.list_sections {
                exercise_list_section(
                    &mut slide,
                    &format!("{}.{}", section.key, sub.key),
                    &sub.fields,
                    &sub.string_lists,
                    &sub.nested_lists,
                    &mut touched,
                );
            }
        }

        let filtered = edit::filter_slide_json(&schema, &slide);
        for key in touched {
            assert!(
                filtered.get(&key).is_some(),
                "{}: `{key}` was edited from the form but is missing after filter_slide_json",
                entry.kind
            );
        }
    }
}

/// 核对 (c)：高级 JSON 框的写回路径（能解析就成为草稿、**不过滤**），表单必须能
/// 显示新值——键名就是 schema 里的键名，`string_value` / `bool_value` 原样读回。
#[test]
fn values_edited_in_the_json_editor_reach_the_form() {
    let schema = editor_schema();
    // 用户在高级 Blueprint JSON 里整页替换（`on_json_text_changed` 的解析成功分支）。
    let text = r#"{
      "kind": "focus_example",
      "section": "06",
      "title": "JSON 改的标题",
      "subtitle": "s",
      "points": [{ "step": "01", "title": "要点", "body": "b", "icon": "" }],
      "example_title": "et",
      "example_body": "eb",
      "ranking": [{ "index": "A", "label": "l", "meta": "m", "muted": true }]
    }"#;
    let draft: Value = serde_json::from_str(text).expect("the page parses");
    assert_eq!(edit::string_value(&draft, "title"), "JSON 改的标题");
    assert_eq!(edit::string_value(&draft, "points[0].title"), "要点");
    assert!(edit::bool_value(&draft, "ranking[0].muted"));
    // 表单每改一次都会重算 `json_text`（过滤后）；表单字段在过滤后必须仍在。
    let filtered = edit::filter_slide_json(&schema, &draft);
    assert_eq!(edit::string_value(&filtered, "title"), "JSON 改的标题");
    assert!(edit::bool_value(&filtered, "ranking[0].muted"));
}

/// 核对 (d)：图表框的写回路径（校验通过后 `set_at_path("chart", 解析结果)`）不允许
/// 数字 / 字符串混型——数据列保持 JSON 数字，encoding 保持字符串，对象区的
/// `chart.title` 与图表框共用同一份 `chart`。
#[test]
fn chart_json_keeps_its_types_through_the_editor() {
    let schema = editor_schema();
    let chart: Value = serde_json::from_str(
        r#"{ "type": "bar", "title": "季度交付量", "data": [{ "quarter": "Q1", "count": 12 }],
             "encoding": { "x": "quarter", "y": "count" } }"#,
    )
    .expect("the chart parses");
    let mut slide = edit::create_kind_blueprint(&schema, "chart_focus", None);
    edit::set_at_path(&mut slide, "chart", chart.clone());
    assert_eq!(
        edit::get_at_path(&slide, "chart.data[0].count"),
        Some(&json!(12))
    );
    assert_eq!(
        edit::get_at_path(&slide, "chart.encoding.x"),
        Some(&json!("quarter"))
    );
    assert_eq!(edit::string_value(&slide, "chart.title"), "季度交付量");
    // chart_focus 的 fields 含 `chart`：过滤后整份图表（含数字）原样保留。
    let filtered = edit::filter_slide_json(&schema, &slide);
    assert_eq!(filtered["chart"], chart);
}

/// 核对 (d)：勾选框字段（`muted`、`highlight`）的模板初值必须是 JSON 布尔——
/// 表单写布尔、表单读布尔，JSON 里出现字符串 / 数字会让回显与写回对不上。
#[test]
fn boolean_fields_keep_boolean_templates() {
    let schema = editor_schema();
    let mut checked = 0usize;
    let mut check = |new_item: &Value, fields: &[EditorField], where_: &str| {
        for field in fields.iter().filter(|field| field.field_type == "boolean") {
            if let Some(value) = new_item.get(&field.key) {
                assert!(
                    value.is_boolean(),
                    "{where_}: boolean field `{}` has a non-boolean new_item value {value}",
                    field.key
                );
                checked += 1;
            }
        }
    };
    for kind in schema.kinds.iter().map(|entry| entry.kind.as_str()) {
        for section in schema.list_sections(&kind) {
            check(
                &section.new_item,
                &section.fields,
                &format!("{kind}.{}", section.key),
            );
            for nested in &section.nested_lists {
                for item in section.new_item[&nested.key]
                    .as_array()
                    .into_iter()
                    .flatten()
                {
                    check(
                        item,
                        &nested.fields,
                        &format!("{kind}.{}.{}", section.key, nested.key),
                    );
                }
                check(
                    &nested.new_item,
                    &nested.fields,
                    &format!("{kind}.{}.{} (template)", section.key, nested.key),
                );
            }
        }
        for section in schema.object_sections(&kind) {
            for sub in &section.list_sections {
                check(
                    &sub.new_item,
                    &sub.fields,
                    &format!("{kind}.{}.{}", section.key, sub.key),
                );
            }
        }
    }
    assert!(
        checked > 0,
        "the audit must actually cover the boolean templates"
    );
}
