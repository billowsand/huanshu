//! `slide_edit` 与原版 TypeScript 函数的快照对照。
//!
//! 期望值由 `target/gen-slide-edit.ts`（不提交的一次性脚本，函数体逐字拷贝自
//! `src/composables/useSlideEditor.ts` 与 `src/views/steps/Step3Editor.vue`）生成。原版改动后
//! 重跑该脚本，Rust 实现必须跟着改到快照重新全绿。
//!
//! 比对一律**按值**（`serde_json::Value` 相等），不关心键的顺序。

use std::path::PathBuf;

use huanshu_core::editor_schema::editor_schema;
use huanshu_core::slide_edit::{self as edit, PathSegment};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Debug, Deserialize)]
struct Snapshot {
    version: u32,
    kinds: Vec<String>,
    kind_switches: Vec<KindSwitch>,
    filters: Vec<FilterCase>,
    parse_paths: Vec<PathProbe>,
    get_at_path: Vec<GetProbe>,
    set_at_path: Vec<SetCase>,
    values: Vec<ValueProbe>,
    list_ops: Vec<ListOp>,
    deck_ops: Vec<DeckOp>,
}

#[derive(Debug, Deserialize)]
struct KindSwitch {
    name: String,
    kind: String,
    seed: Option<Value>,
    result: Value,
}

#[derive(Debug, Deserialize)]
struct FilterCase {
    name: String,
    slide: Value,
    result: Value,
}

#[derive(Debug, Deserialize)]
struct PathProbe {
    path: String,
    segments: Value,
}

#[derive(Debug, Deserialize)]
struct GetProbe {
    name: String,
    source: Value,
    path: String,
    found: bool,
    result: Value,
}

#[derive(Debug, Deserialize)]
struct SetCase {
    name: String,
    target: Value,
    path: String,
    value: Value,
    result: Value,
}

#[derive(Debug, Deserialize)]
struct ValueProbe {
    name: String,
    draft: Value,
    path: String,
    string: String,
    #[serde(rename = "bool")]
    boolean: bool,
    array: Value,
}

#[derive(Debug, Deserialize)]
struct ListOp {
    name: String,
    op: String,
    path: String,
    index: Option<usize>,
    dir: Option<i64>,
    item: Option<Value>,
    value: Option<String>,
    draft: Value,
    /// `null` = 原版提前返回，没有换掉草稿。
    result: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct DeckOp {
    name: String,
    op: String,
    deck: Vec<Value>,
    active: Option<usize>,
    kind: Option<String>,
    index: Option<usize>,
    dir: Option<i64>,
    result: Option<DeckResult>,
}

#[derive(Debug, Deserialize)]
struct DeckResult {
    deck: Vec<Value>,
    active: usize,
}

fn load() -> Snapshot {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/slide-edit.json");
    let raw =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let snapshot: Snapshot = serde_json::from_str(&raw).expect("parse slide-edit.json");
    assert_eq!(snapshot.version, 1, "snapshot version");
    snapshot
}

fn segments_of(path: &str) -> Value {
    Value::Array(
        edit::parse_path(path)
            .into_iter()
            .map(|segment| match segment {
                PathSegment::Key(key) => Value::String(key),
                PathSegment::Index(index) => json!(index),
            })
            .collect(),
    )
}

#[test]
fn the_snapshot_covers_every_kind() {
    let snapshot = load();
    let schema = editor_schema();
    let names: Vec<String> = schema
        .kinds
        .iter()
        .map(|entry| entry.kind.clone())
        .collect();
    assert_eq!(
        names, snapshot.kinds,
        "the snapshot was taken from another schema"
    );
    // 每种版式都要当过目标（不带种子换过去），也要拿自己的共享字段当过种子。
    for kind in &snapshot.kinds {
        assert!(
            snapshot
                .kind_switches
                .iter()
                .any(|case| case.name == format!("no seed → {kind}")),
            "{kind} is never a blueprint target"
        );
        assert!(
            snapshot
                .kind_switches
                .iter()
                .any(|case| case.name == format!("{kind} (shared fields) → cover")),
            "{kind} is never a blueprint seed"
        );
        assert!(
            snapshot
                .filters
                .iter()
                .any(|case| case.name == format!("{kind}: fields + aspect_ratio")),
            "{kind} has no filter case"
        );
    }
}

#[test]
fn kind_blueprints_match_the_original() {
    let snapshot = load();
    let schema = editor_schema();
    assert!(!snapshot.kind_switches.is_empty(), "no kind switches");
    for case in &snapshot.kind_switches {
        let got = edit::create_kind_blueprint(&schema, &case.kind, case.seed.as_ref());
        assert_eq!(got, case.result, "{}", case.name);
    }
}

#[test]
fn filtered_slides_match_the_original() {
    let snapshot = load();
    let schema = editor_schema();
    assert!(!snapshot.filters.is_empty(), "no filter cases");
    for case in &snapshot.filters {
        let got = edit::filter_slide_json(&schema, &case.slide);
        assert_eq!(got, case.result, "{}", case.name);
    }
}

#[test]
fn paths_match_the_original() {
    let snapshot = load();
    assert!(!snapshot.parse_paths.is_empty(), "no path probes");
    for probe in &snapshot.parse_paths {
        assert_eq!(segments_of(&probe.path), probe.segments, "{:?}", probe.path);
    }

    for case in &snapshot.get_at_path {
        let got = edit::get_at_path(&case.source, &case.path);
        assert_eq!(
            got.is_some(),
            case.found,
            "{}: the original returned {}",
            case.name,
            if case.found { "a value" } else { "undefined" }
        );
        assert_eq!(
            got.cloned().unwrap_or(Value::Null),
            case.result,
            "{}",
            case.name
        );
    }

    for case in &snapshot.set_at_path {
        let mut target = case.target.clone();
        edit::set_at_path(&mut target, &case.path, case.value.clone());
        assert_eq!(target, case.result, "{}", case.name);
    }
}

#[test]
fn form_values_match_the_original() {
    let snapshot = load();
    assert!(!snapshot.values.is_empty(), "no value probes");
    for probe in &snapshot.values {
        assert_eq!(
            edit::string_value(&probe.draft, &probe.path),
            probe.string,
            "{}: string_value",
            probe.name
        );
        assert_eq!(
            edit::bool_value(&probe.draft, &probe.path),
            probe.boolean,
            "{}: bool_value",
            probe.name
        );
        assert_eq!(
            Value::Array(edit::array_value(&probe.draft, &probe.path).to_vec()),
            probe.array,
            "{}: array_value",
            probe.name
        );
    }
}

#[test]
fn list_operations_match_the_original() {
    let snapshot = load();
    assert!(!snapshot.list_ops.is_empty(), "no list operations");
    for case in &snapshot.list_ops {
        let draft = &case.draft;
        let path = case.path.as_str();
        let got = match case.op.as_str() {
            "add_item" => edit::add_item(draft, path, case.item.as_ref().expect("item")),
            "remove_item" => edit::remove_item(draft, path, case.index.expect("index")),
            "move_item" => edit::move_item(
                draft,
                path,
                case.index.expect("index"),
                case.dir.expect("dir"),
            ),
            "add_string_item" => edit::add_string_item(draft, path),
            "update_string_item" => edit::update_string_item(
                draft,
                path,
                case.index.expect("index"),
                case.value.as_deref().expect("value"),
            ),
            "remove_string_item" => {
                edit::remove_string_item(draft, path, case.index.expect("index"))
            }
            other => panic!("{}: unknown list operation {other}", case.name),
        };
        assert_eq!(got, case.result, "{}", case.name);
    }
}

#[test]
fn deck_operations_match_the_original() {
    let snapshot = load();
    let schema = editor_schema();
    assert!(!snapshot.deck_ops.is_empty(), "no deck operations");
    for case in &snapshot.deck_ops {
        let got: Option<edit::DeckEdit> = match case.op.as_str() {
            "insert" => Some(edit::insert_new_slide(
                &schema,
                &case.deck,
                case.active.expect("active"),
                case.kind.as_deref().expect("kind"),
            )),
            "delete" => edit::delete_slide(&case.deck, case.index.expect("index")),
            "move" => edit::move_slide(
                &case.deck,
                case.index.expect("index"),
                case.dir.expect("dir"),
            ),
            other => panic!("{}: unknown deck operation {other}", case.name),
        };
        match (&got, &case.result) {
            (None, None) => {}
            (Some(edited), Some(expected)) => {
                assert_eq!(edited.deck, expected.deck, "{}: deck", case.name);
                assert_eq!(edited.active, expected.active, "{}: active", case.name);
            }
            (None, Some(_)) => panic!("{}: the original changed the deck, we did not", case.name),
            (Some(_), None) => panic!("{}: we changed the deck, the original did not", case.name),
        }
    }
}

/// 有意修改：`fields` 之外原本就有的 `aspect_ratio` 要留着（原版会把它丢掉）。
///
/// 这一条**不**依赖快照：就算有人把 TypeScript 改回去、重跑脚本生成新快照，这里也会红。
#[test]
fn aspect_ratio_survives_a_slide_filter() {
    let schema = editor_schema();
    let slide = json!({
        "kind": "cover",
        "title": "标题",
        "aspect_ratio": "ratio_32x9",
        "cards": ["不该留下的字段"],
    });
    let filtered = edit::filter_slide_json(&schema, &slide);
    assert_eq!(
        filtered,
        json!({"kind": "cover", "title": "标题", "aspect_ratio": "ratio_32x9"})
    );
    // 没有 aspect_ratio 的页不会被凭空加上它。
    let filtered = edit::filter_slide_json(&schema, &json!({"kind": "cover", "title": "标题"}));
    assert_eq!(filtered, json!({"kind": "cover", "title": "标题"}));
}

/// 数组越界赋值留的空洞：JS 序列化成 `null`，这里按下标补 `Null`。
#[test]
fn array_holes_become_nulls() {
    let mut target = json!({});
    edit::set_at_path(&mut target, "cards[2].title", json!("x"));
    assert_eq!(target, json!({"cards": [null, null, {"title": "x"}]}));

    // 中间层是数组、下一段是字符串时建 `{}`；是数字时建 `[]`。
    let mut target = json!({});
    edit::set_at_path(&mut target, "rows[1][0]", json!("x"));
    assert_eq!(target, json!({"rows": [null, ["x"]]}));

    let updated = edit::update_string_item(&json!({"items": ["a"]}), "items", 3, "d");
    assert_eq!(updated, Some(json!({"items": ["a", null, null, "d"]})));
}
