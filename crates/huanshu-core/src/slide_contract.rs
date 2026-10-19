//! The executable slide contract. Form definitions supply field names, types and enums;
//! generation and JSON editing consume the same rules instead of maintaining allowlists.
use serde_json::{json, Map, Value};

use crate::editor_schema::{EditorField, EditorSchema, ListSectionDef};

fn object() -> Value {
    json!({"type":"object", "properties":{}, "additionalProperties":false})
}

fn array(items: Value) -> Value {
    json!({"type":"array", "items":items})
}

fn field_schema(field: &EditorField) -> Value {
    let mut rule = if field.field_type == "boolean" {
        json!({"type":"boolean"})
    } else {
        json!({"type":["string", "null"]})
    };
    if let Some(options) = &field.options {
        rule["enum"] = json!(options);
        rule["enum"].as_array_mut().unwrap().push(Value::Null);
    }
    rule
}

fn required(rule: &mut Value, keys: &[&str]) {
    rule["required"] = json!(keys);
    for key in keys {
        if let Some(field) = rule["properties"].get_mut(*key) {
            if let Some(types) = field["type"].as_array() {
                field["type"] = types
                    .iter()
                    .find(|t| **t != "null")
                    .cloned()
                    .unwrap_or(Value::Null);
            }
            if let Some(options) = field.get_mut("enum").and_then(Value::as_array_mut) {
                options.retain(|v| !v.is_null());
            }
        }
    }
}

fn nullable(rule: &mut Value) {
    if let Some(kind) = rule["type"].as_str() {
        rule["type"] = json!([kind, "null"]);
    }
    if let Some(options) = rule.get_mut("enum").and_then(Value::as_array_mut) {
        if !options.contains(&Value::Null) {
            options.push(Value::Null);
        }
    }
}

fn insert(rule: &mut Value, path: &str, value: Value) {
    let (head, tail) = path.split_once('.').unwrap_or((path, ""));
    let props = rule["properties"].as_object_mut().expect("object rule");
    if tail.is_empty() {
        props.insert(head.to_string(), value);
    } else {
        let child = props.entry(head.to_string()).or_insert_with(object);
        if child["type"] != "object" {
            *child = object();
        }
        insert(child, tail, value);
    }
}

fn list_schema(def: &ListSectionDef) -> Value {
    let mut item = object();
    for field in &def.fields {
        insert(&mut item, &field.key, field_schema(field));
    }
    for list in &def.string_lists {
        insert(&mut item, &list.key, array(json!({"type":"string"})));
    }
    for list in &def.nested_lists {
        let mut nested = object();
        for field in &list.fields {
            insert(&mut nested, &field.key, field_schema(field));
        }
        required(&mut nested, &["label"]);
        insert(&mut item, &list.key, array(nested));
    }
    let required_keys: &[&str] = match def.key.as_str() {
        "overview_items" => &["number", "title", "desc"],
        "cards" | "panels" | "steps" => &["title"],
        "left_items" | "list_items" | "points" => &["title", "body"],
        "layers" => &["title", "meta"],
        "ranking" => &["index", "label", "meta"],
        "center_items" => &["title", "desc"],
        "timeline_events" => &["date", "title", "body"],
        "phases" => &["phase", "title", "steps"],
        "items" => &["label"],
        "quadrants" => &["key", "title", "items"],
        _ => &[],
    };
    required(&mut item, required_keys);
    array(item)
}

/// Machine-readable JSON Schema for this layout, including advanced chart content.
pub fn json_schema(schema: &EditorSchema, kind: &str) -> Option<Value> {
    let entry = schema.kind(kind)?;
    let mut root = object();
    for key in &entry.fields {
        let rule = if let Some(field) = schema.simple_fields.iter().find(|f| &f.key == key) {
            field_schema(field)
        } else if matches!(key.as_str(), "badges" | "images") {
            array(json!({"type":"string"}))
        } else {
            json!({"type":"string"})
        };
        insert(&mut root, key, rule);
    }
    insert(&mut root, "kind", json!({"type":"string", "enum":[kind]}));
    insert(
        &mut root,
        "aspect_ratio",
        json!({"type":"string", "enum":["ratio_16x9", "ratio_32x9", "ratio_48x9"]}),
    );
    for def in schema.list_sections(kind) {
        insert(&mut root, &def.key, list_schema(def));
    }
    for def in schema.object_sections(kind) {
        for field in &def.fields {
            insert(
                &mut root,
                &format!("{}.{}", def.key, field.key),
                field_schema(field),
            );
        }
        for list in &def.string_lists {
            insert(
                &mut root,
                &format!("{}.{}", def.key, list.key),
                array(json!({"type":"string"})),
            );
        }
        for list in &def.list_sections {
            insert(
                &mut root,
                &format!("{}.{}", def.key, list.key),
                list_schema(list),
            );
        }
    }
    if kind == "chart_focus" {
        insert(&mut root, "chart", chart_schema());
    }
    for key in ["aspect_ratio", "compare_data", "swot_data", "chart"] {
        if let Some(rule) = root["properties"].get_mut(key) {
            nullable(rule);
        }
    }
    if let Some(compare) = root["properties"].get_mut("compare_data") {
        required(compare, &["left", "right"]);
        for side in ["left", "right"] {
            required(&mut compare["properties"][side], &["title", "items"]);
        }
    }
    if let Some(swot) = root["properties"].get_mut("swot_data") {
        required(swot, &["quadrants"]);
    }
    required(&mut root, &["kind", "title"]);
    Some(root)
}

fn chart_schema() -> Value {
    let mut chart = object();
    for key in ["title", "source_note"] {
        insert(&mut chart, key, json!({"type":"string"}));
    }
    insert(
        &mut chart,
        "type",
        json!({"type":"string", "enum":crate::types::CHART_TYPES}),
    );
    insert(
        &mut chart,
        "source",
        json!({"type":"string", "enum":["user_input", "llm_generated", "tool"]}),
    );
    // Data column names belong to the user's dataset, rather than the slide schema.
    insert(&mut chart, "data", array(json!({"type":"object"})));
    let mut encoding = object();
    for key in ["x", "y", "label", "value"] {
        insert(&mut encoding, key, json!({"type":"string"}));
    }
    insert(&mut chart, "encoding", encoding);
    let mut series = object();
    for key in ["name", "key"] {
        insert(&mut series, key, json!({"type":"string"}));
    }
    required(&mut series, &["name", "key"]);
    insert(&mut chart, "series", array(series));
    let mut emphasis = object();
    insert(&mut emphasis, "highlight", array(json!({"type":"string"})));
    insert(&mut chart, "emphasis", emphasis);
    for key in ["source", "source_note", "emphasis"] {
        nullable(&mut chart["properties"][key]);
    }
    for key in ["x", "y", "label", "value"] {
        nullable(&mut chart["properties"]["encoding"]["properties"][key]);
    }
    required(&mut chart, &["type", "title", "data", "encoding"]);
    chart
}

pub fn chart_errors(value: &Value) -> Vec<String> {
    let mut errors = Vec::new();
    check(value, &chart_schema(), "chart", false, &mut errors);
    errors
}

fn inert(value: &Value) -> bool {
    value.is_null()
        || value.as_array().is_some_and(Vec::is_empty)
        || value.as_str().is_some_and(str::is_empty)
}

/// Typed blueprints serialize unused shared fields as null/empty. Only this internal
/// representation permits those empty fields; user-authored JSON is checked strictly.
pub fn errors(schema: &EditorSchema, slide: &Value, typed: bool) -> Vec<String> {
    let Some(kind) = slide.get("kind").and_then(Value::as_str) else {
        return vec!["kind 必须是有效版式名称".into()];
    };
    let Some(rule) = json_schema(schema, kind) else {
        return vec![format!("kind \"{kind}\" 不受支持")];
    };
    let mut errors = Vec::new();
    check(slide, &rule, "", typed, &mut errors);
    if let Some(quadrants) = slide
        .pointer("/swot_data/quadrants")
        .and_then(Value::as_array)
    {
        for key in crate::editor_schema::SWOT_KEYS {
            if quadrants
                .iter()
                .filter(|q| q["key"].as_str() == Some(*key))
                .count()
                != 1
            {
                errors.push(format!("swot_data.quadrants 必须恰好包含一个 {key}"));
            }
        }
    }
    if let Some(panels) = slide.get("panels").and_then(Value::as_array) {
        for (i, panel) in panels.iter().enumerate() {
            if panel.get("kind").and_then(Value::as_str).unwrap_or("text") == "text"
                && panel.get("items").is_some_and(|v| !inert(v))
            {
                errors.push(format!(
                    "panels[{i}].items 不适用于 text 面板，请选择 flow、matrix 或 steps"
                ));
            }
        }
    }
    errors
}

fn check(value: &Value, rule: &Value, path: &str, typed: bool, errors: &mut Vec<String>) {
    // Nullable optional fields have the same meaning as omitted fields.
    if value.is_null() {
        if !rule["type"]
            .as_array()
            .is_some_and(|types| types.contains(&json!("null")))
        {
            errors.push(format!("{path} 不能为空"));
        }
        return;
    }
    let type_name = rule["type"].as_str().or_else(|| {
        rule["type"].as_array().and_then(|types| {
            types
                .iter()
                .find_map(|t| t.as_str().filter(|t| *t != "null"))
        })
    });
    let valid = match type_name {
        Some("object") => value.is_object(),
        Some("array") => value.is_array(),
        Some("string") => value.is_string(),
        Some("boolean") => value.is_boolean(),
        _ => true,
    };
    if !valid {
        errors.push(format!("{path} 类型错误，应为 {}", rule["type"]));
        return;
    }
    if let Some(options) = rule.get("enum").and_then(Value::as_array) {
        if !options.contains(value) && !(typed && inert(value)) {
            errors.push(format!(
                "{path} 的值 {value} 无效，应为 {}",
                options
                    .iter()
                    .map(Value::to_string)
                    .collect::<Vec<_>>()
                    .join(" / ")
            ));
        }
    }
    if let Some(map) = value.as_object() {
        if let Some(required) = rule.get("required").and_then(Value::as_array) {
            for key in required.iter().filter_map(Value::as_str) {
                if map.get(key).map_or(true, |v| v.is_null()) {
                    let field = if path.is_empty() {
                        key.to_string()
                    } else {
                        format!("{path}.{key}")
                    };
                    errors.push(format!("{field} 不能为空"));
                }
            }
        }
        for (key, child) in map {
            let next = if path.is_empty() {
                key.clone()
            } else {
                format!("{path}.{key}")
            };
            if let Some(child_rule) = rule.get("properties").and_then(|p| p.get(key)) {
                check(child, child_rule, &next, typed, errors);
            } else if rule["additionalProperties"] == false && !(typed && inert(child)) {
                errors.push(format!("{next} 不受当前版式支持"));
            }
        }
    }
    if let (Some(items), Some(item_rule)) = (value.as_array(), rule.get("items")) {
        for (i, item) in items.iter().enumerate() {
            if item.is_null() {
                errors.push(format!("{path}[{i}] 不能为空"));
            } else {
                check(item, item_rule, &format!("{path}[{i}]"), typed, errors);
            }
        }
    }
}

/// Canonical view for generated/typed blueprints. Do not use this to accept user JSON:
/// the editor validates unknown fields before applying, rather than deleting them.
pub fn project(schema: &EditorSchema, slide: &Value) -> Value {
    let Some(rule) = slide
        .get("kind")
        .and_then(Value::as_str)
        .and_then(|k| json_schema(schema, k))
    else {
        return slide.clone();
    };
    project_rule(slide, &rule)
}

fn project_rule(value: &Value, rule: &Value) -> Value {
    if let (Some(map), Some(props)) = (value.as_object(), rule.get("properties")) {
        return Value::Object(
            map.iter()
                .filter_map(|(key, value)| {
                    props
                        .get(key)
                        .map(|r| (key.clone(), project_rule(value, r)))
                })
                .collect::<Map<_, _>>(),
        );
    }
    if let (Some(items), Some(item_rule)) = (value.as_array(), rule.get("items")) {
        return Value::Array(items.iter().map(|v| project_rule(v, item_rule)).collect());
    }
    value.clone()
}
