//! 编辑器的编辑规则：路径读写、表单取值、列表增删移、版式蓝图与牌组的插删移。
//!
//! 这些规则原本只存在于 TypeScript（`src/composables/useSlideEditor.ts` 与
//! `src/views/steps/Step3Editor.vue`）。**改这里必须改原版**，反过来也一样——两边由
//! `tests/slide_edit.rs` 对着 `tests/fixtures/slide-edit.json` 快照逐条**按值**比对
//! （快照由 `target/gen-slide-edit.ts` 从原版 TypeScript **逐字拷贝**的函数生成）。
//!
//! 移植注意（与 JS 的语义差，快照已覆盖）：
//!
//! - 牌组与草稿一律是 [`Value`]（原始 JSON），与 `ProjectModel.deck` 一致：强类型往返会挡掉
//!   能渲染的牌组。
//! - 原版用「提前返回、不换掉状态」表达边界（`moveEditorSlide` 越界、`deleteEditorSlide`
//!   只剩一页、`moveItem` 越界、草稿为 `null`……），这里统一用 [`Option::None`]：`None` 就是
//!   「原版没有调用 `replaceEditorDraft` / `setActiveSlide`」。
//! - 原版 `setAtPath` 对数组越界赋值会留空洞，`JSON.stringify` 把空洞写成 `null`；这里按下标
//!   赋值时不足的部分补 [`Value::Null`]，两边序列化后同值。空洞下标有上限
//!   （[`MAX_PADDED_INDEX`]）：JS 的稀疏数组能到 2^32-2，Rust 的 `Vec` 不能，超过上限的下标
//!   按「不动」处理（编辑器路径来自 `editor_schema`，不会产生这种下标）。
//! - 下标超过 `usize` 的数字（正则能匹配任意位数）记作 `usize::MAX`：JS 把这种下标当普通属性，
//!   序列化时同样丢掉。
//! - 原版对非对象游标赋值在严格模式下会抛 `TypeError`（Vue SFC 是模块）；这里当作「不动」。
//!   同理，原版 `getAtPath` 对字符串也能按下标取字符（`'标题'[0]`），这里不支持——编辑器的路径
//!   都来自 `editor_schema`，不会落在字符串上。
//!
//! **有意修改**（两边一起改）：原版 `filterSlideJson` 只保留该 kind 的 `fields`，而没有一个
//! kind 的 `fields` 含 `aspect_ratio`——应用一次更改就把这一页的比例丢了，第 2 步看板与
//! `get_retry_layouts` 会退回 16:9。这里额外保留原本就有的 `aspect_ratio`；TypeScript 侧同步
//! 改了同一处（见 `src/composables/useSlideEditor.ts`），快照里带 `aspect_ratio` 的用例锁住它。

use serde_json::{json, Value};

use crate::editor_schema::EditorSchema;

/// 原版 `SHARED_TEMPLATE_FIELDS`：新建一页时沿用上一页的这些字段（标题、页脚、图片……），
/// 让整副牌组风格一致。
pub const SHARED_TEMPLATE_FIELDS: &[&str] = &[
    "section",
    "title",
    "subtitle",
    "badge",
    "accent",
    "note",
    "label",
    "label_tone",
    "image",
    "images",
    "placeholder",
    "side_width",
    "badges",
    "footer",
    "aspect_ratio",
];

/// 原版 `NON_CHAPTER_KINDS`：这三种页不显示章节号，因此它们不参与「新页继承章节」。
pub const NON_CHAPTER_KINDS: &[&str] = &["cover", "overview", "closing"];

/// 数组空洞的上限（见模块注释）。
pub const MAX_PADDED_INDEX: usize = 10_000;

/// 原版 `parsePath` 解析出的一段路径。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathSegment {
    /// `a.b` 里的 `b`（正则的第一分支 `[^[.\]]+`）。
    Key(String),
    /// `a[2]` 里的 `2`（正则的第二分支 `\[(\d+)\]`）。
    Index(usize),
}

/// 原版 `parsePath`：`cards[2].title` → `["cards", 2, "title"]`。
///
/// 逐字对齐原正则 `/([^[.\]]+)|\[(\d+)\]/g` 的匹配顺序：先试「一段非 `[`、`.`、`]` 的文本」，
/// 失败再看是不是 `[数字]`，都不匹配就前进一格（所以孤立的 `[` 或 `]` 会被跳过）。
pub fn parse_path(path: &str) -> Vec<PathSegment> {
    let mut out = Vec::new();
    let bytes = path.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let start = i;
        // 第一分支：连续的、不含 `[`、`.`、`]` 的字符（按字符边界切，多字节字符整段取走）。
        if !matches!(bytes[i], b'[' | b'.' | b']') {
            while i < bytes.len() && !matches!(bytes[i], b'[' | b'.' | b']') {
                i += 1;
            }
            out.push(PathSegment::Key(path[start..i].to_string()));
            continue;
        }
        // 第二分支：`[` + 至少一位数字 + `]`。
        if bytes[i] == b'[' {
            let mut j = i + 1;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
            }
            if j > i + 1 && j < bytes.len() && bytes[j] == b']' {
                let digits = &path[i + 1..j];
                // 位数超过 `usize` 时记作 `usize::MAX`（见模块注释）。
                out.push(PathSegment::Index(digits.parse().unwrap_or(usize::MAX)));
                i = j + 1;
                continue;
            }
        }
        // 都不匹配：正则引擎前进一格。
        i += 1;
    }
    out
}

/// JS 的数组下标规则：只有规范的数字串（`"0"`、`"12"`，不能有前导零或 `+`）才是下标，
/// 其余是普通属性名（序列化时会被丢掉）。
fn canonical_index(key: &str) -> Option<usize> {
    if key.is_empty() || !key.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    if key.len() > 1 && key.starts_with('0') {
        return None;
    }
    key.parse::<usize>().ok()
}

/// 把一段路径按 JS 的规则落到对象键 / 数组下标上。
fn object_key(segment: &PathSegment) -> String {
    match segment {
        PathSegment::Key(key) => key.clone(),
        // JS 里 `obj[0]` 就是 `obj["0"]`。
        PathSegment::Index(index) => index.to_string(),
    }
}

fn array_index(segment: &PathSegment) -> Option<usize> {
    match segment {
        PathSegment::Index(index) => Some(*index),
        PathSegment::Key(key) => canonical_index(key),
    }
}

/// 读一层：`cursor[segment]`，越界 / 类型不符 / 走到 `null` 都给 `None`（即 JS 的 `undefined`）。
fn child<'a>(cursor: &'a Value, segment: &PathSegment) -> Option<&'a Value> {
    match cursor {
        Value::Object(map) => map.get(&object_key(segment)),
        Value::Array(items) => items.get(array_index(segment)?),
        _ => None,
    }
}

/// 原版 `getAtPath`：沿路径取值，中途遇到 `null` / 缺层一律 `undefined`（这里 `None`）。
pub fn get_at_path<'a>(source: &'a Value, path: &str) -> Option<&'a Value> {
    let mut cursor = source;
    for segment in parse_path(path) {
        cursor = child(cursor, &segment)?;
    }
    Some(cursor)
}

/// 中间缺层时按下一段的类型建容器（原版 `typeof next === 'number' ? [] : {}`）。
fn fresh_container(next_is_index: bool) -> Value {
    if next_is_index {
        Value::Array(Vec::new())
    } else {
        json!({})
    }
}

/// 往下走一层，缺层 / 空位（`null`）时补上容器再返回（原版 `if (current == null)`）。
///
/// 游标是标量时返回 `None`（原版严格模式下会抛）。
fn descend<'a>(
    cursor: &'a mut Value,
    segment: &PathSegment,
    next_is_index: bool,
) -> Option<&'a mut Value> {
    match cursor {
        Value::Object(map) => {
            let key = object_key(segment);
            if map.get(&key).map_or(true, Value::is_null) {
                map.insert(key.clone(), fresh_container(next_is_index));
            }
            map.get_mut(&key)
        }
        Value::Array(items) => {
            let index = array_index(segment)?;
            if index > MAX_PADDED_INDEX {
                return None;
            }
            let replace = items.get(index).map_or(true, Value::is_null);
            if index >= items.len() {
                items.resize(index + 1, Value::Null);
            }
            if replace {
                items[index] = fresh_container(next_is_index);
            }
            items.get_mut(index)
        }
        _ => None,
    }
}

/// 写最后一层：`cursor[last] = value`（数组不足补 `null`，非规范键在数组上按普通属性丢掉）。
fn assign(cursor: &mut Value, segment: &PathSegment, value: Value) {
    match cursor {
        Value::Object(map) => {
            map.insert(object_key(segment), value);
        }
        Value::Array(items) => {
            if let Some(index) = array_index(segment) {
                if index > MAX_PADDED_INDEX {
                    return;
                }
                if index >= items.len() {
                    items.resize(index + 1, Value::Null);
                }
                items[index] = value;
            }
        }
        _ => {}
    }
}

/// 原版 `setAtPath`：中间缺的层按下一段是不是数字建 `[]` / `{}`，最后一段赋值。
pub fn set_at_path(target: &mut Value, path: &str, value: Value) {
    let segments = parse_path(path);
    if segments.is_empty() {
        // 原版 `parts[parts.length - 1]` 在空路径上是 `undefined`，于是写的是目标上的
        // `"undefined"` 键（目标若是数组，就是个被序列化丢掉的普通属性）。
        assign(target, &PathSegment::Key("undefined".to_string()), value);
        return;
    }
    let mut cursor = target;
    for index in 0..segments.len() - 1 {
        let next_is_index = matches!(segments[index + 1], PathSegment::Index(_));
        match descend(cursor, &segments[index], next_is_index) {
            Some(next) => cursor = next,
            // 原版在这个位置会抛（严格模式赋在标量上）；这里当作不动。
            None => return,
        }
    }
    assign(cursor, &segments[segments.len() - 1], value);
}

/// 原版 `getStringValue`：非字符串一律空串。
pub fn string_value(draft: &Value, path: &str) -> String {
    get_at_path(draft, path)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

/// 原版 `Boolean(value)`：`false` / `null` / `undefined` / `0` / `NaN` / `""` 为假，其余为真
/// （`"0"`、`[]`、`{}` 都是真值）。JSON 里没有 `undefined` 与 `NaN`。
fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(number) => number.as_f64().map_or(true, |value| value != 0.0),
        Value::String(text) => !text.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

/// 原版 `getBooleanValue`。
pub fn bool_value(draft: &Value, path: &str) -> bool {
    get_at_path(draft, path).is_some_and(is_truthy)
}

/// 原版 `getArrayValue`：非数组一律空数组。
pub fn array_value<'a>(draft: &'a Value, path: &str) -> &'a [Value] {
    get_at_path(draft, path)
        .and_then(Value::as_array)
        .map_or(&[], |items| items.as_slice())
}

/// 列表操作的前置：原版 `cloneDraft()` 之后 `if (!draft) return`。
///
/// 草稿不是对象时也按「不动」处理：JSON 里给数组设字符串键会被 `JSON.stringify` 丢掉。
fn editable_draft(draft: &Value) -> Option<Value> {
    draft.is_object().then(|| draft.clone())
}

/// 原版 `addItem(path, template)`：在 `path` 的数组末尾追加 `new_item` 的深拷贝。
pub fn add_item(draft: &Value, path: &str, new_item: &Value) -> Option<Value> {
    let mut draft = editable_draft(draft)?;
    let mut items = array_value(&draft, path).to_vec();
    items.push(new_item.clone());
    set_at_path(&mut draft, path, Value::Array(items));
    Some(draft)
}

/// 原版 `removeItem(path, index)`：按下标删（越界等于没删，但草稿仍然换新）。
pub fn remove_item(draft: &Value, path: &str, index: usize) -> Option<Value> {
    let mut draft = editable_draft(draft)?;
    let items: Vec<Value> = array_value(&draft, path)
        .iter()
        .enumerate()
        .filter(|(current, _)| *current != index)
        .map(|(_, item)| item.clone())
        .collect();
    set_at_path(&mut draft, path, Value::Array(items));
    Some(draft)
}

/// 原版 `moveItem(path, index, dir)`：与相邻一项交换，越界不动。
pub fn move_item(draft: &Value, path: &str, index: usize, dir: i64) -> Option<Value> {
    let mut draft = editable_draft(draft)?;
    let mut items = array_value(&draft, path).to_vec();
    let next = index as i64 + dir;
    if next < 0 || next as usize >= items.len() {
        return None;
    }
    items.swap(index, next as usize);
    set_at_path(&mut draft, path, Value::Array(items));
    Some(draft)
}

/// 原版 `addStringItem(path)`：在字符串数组末尾追加空串。
pub fn add_string_item(draft: &Value, path: &str) -> Option<Value> {
    let mut draft = editable_draft(draft)?;
    let mut items = array_value(&draft, path).to_vec();
    items.push(Value::String(String::new()));
    set_at_path(&mut draft, path, Value::Array(items));
    Some(draft)
}

/// 原版 `updateStringItem(path, index, value)`：按下标写值（越界时不足的部分补 `null`）。
pub fn update_string_item(draft: &Value, path: &str, index: usize, value: &str) -> Option<Value> {
    let mut draft = editable_draft(draft)?;
    let mut items = array_value(&draft, path).to_vec();
    if index > MAX_PADDED_INDEX {
        return None;
    }
    if index >= items.len() {
        items.resize(index + 1, Value::Null);
    }
    items[index] = Value::String(value.to_string());
    set_at_path(&mut draft, path, Value::Array(items));
    Some(draft)
}

/// 原版 `removeStringItem(path, index)`：按下标删（越界等于没删，但草稿仍然换新）。
pub fn remove_string_item(draft: &Value, path: &str, index: usize) -> Option<Value> {
    remove_item(draft, path, index)
}

/// 原版 `hasMeaningfulValue`：字符串要去空白后非空，数组 / 对象要非空，其余非空值都算有值。
fn has_meaningful_value(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::String(text) => !text.trim().is_empty(),
        Value::Array(items) => !items.is_empty(),
        Value::Object(map) => !map.is_empty(),
        Value::Bool(_) | Value::Number(_) => true,
    }
}

/// 原版 `createKindBlueprint(kind, seed)`：该 kind 的 `defaults`，再用 `seed` 里
/// [`SHARED_TEMPLATE_FIELDS`] 中有值的字段覆盖（深拷贝），最后写死 `kind`。
pub fn create_kind_blueprint(schema: &EditorSchema, kind: &str, seed: Option<&Value>) -> Value {
    // JS `{...defaults, kind}`：defaults 缺失或不是对象时结果为空对象。
    let mut blueprint = match schema.kind(kind) {
        Some(entry) if entry.defaults.is_object() => entry.defaults.clone(),
        _ => json!({}),
    };
    if let Some(seed) = seed {
        for field in SHARED_TEMPLATE_FIELDS {
            let Some(value) = seed.get(*field) else {
                continue;
            };
            if has_meaningful_value(value) {
                blueprint[*field] = value.clone();
            }
        }
    }
    blueprint["kind"] = Value::String(kind.to_string());
    blueprint
}

/// 原版 `filterSlideJson(slide)`：只保留该 kind 的 `fields`（缺的字段不写）。
///
/// **有意修改**：`fields` 之外，原本就有 `aspect_ratio` 的照样保留（原版会在这里丢掉比例）。
pub fn filter_slide_json(schema: &EditorSchema, slide: &Value) -> Value {
    let kind = slide.get("kind").and_then(Value::as_str);
    // 原版 `kindFields[kind] ?? Object.keys(slide)`。
    let fields: Vec<String> = match kind.and_then(|kind| schema.kind(kind)) {
        Some(entry) => entry.fields.clone(),
        None => slide
            .as_object()
            .map(|map| map.keys().cloned().collect())
            .unwrap_or_default(),
    };
    let mut out = serde_json::Map::new();
    let Some(source) = slide.as_object() else {
        return Value::Object(out);
    };
    for field in &fields {
        if let Some(value) = source.get(field) {
            out.insert(field.clone(), value.clone());
        }
    }
    // 有意修改：没有任何 kind 的 fields 里含 aspect_ratio，原版会顺手把它丢掉。
    if !fields.iter().any(|field| field == "aspect_ratio") {
        if let Some(value) = source.get("aspect_ratio") {
            out.insert("aspect_ratio".to_string(), value.clone());
        }
    }
    Value::Object(out)
}

/// 牌组的一次操作结果：新的牌组 + 操作后的当前页。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeckEdit {
    pub deck: Vec<Value>,
    pub active: usize,
}

/// 上一页是不是「不参与章节号」的三种页（缺 `kind` 时按普通页算，与原版一致）。
fn non_chapter(value: &Value) -> bool {
    value
        .get("kind")
        .and_then(Value::as_str)
        .is_some_and(|kind| NON_CHAPTER_KINDS.contains(&kind))
}

/// 原版 `onKindPicked` 的「新建」分支：在当前页之后插一页新牌，并让当前页落在新页上。
///
/// 新页的章节号继承上一页；上一页是 cover / overview / closing 时**不写** `section`
/// （`defaults` 里带的 `section` 也要去掉，原版是赋值 `undefined`）。
/// `active + 1` 越过牌组末尾时插到末尾（JS `splice` 如此），返回的 `active` 仍是 `active + 1`。
pub fn insert_new_slide(
    schema: &EditorSchema,
    deck: &[Value],
    active: usize,
    kind: &str,
) -> DeckEdit {
    let mut new_slide = create_kind_blueprint(schema, kind, None);
    let section = match deck.get(active) {
        Some(previous) if !non_chapter(previous) => previous.get("section").cloned(),
        _ => None,
    };
    if let Some(object) = new_slide.as_object_mut() {
        object.remove("section");
    }
    if let Some(section) = section {
        new_slide["section"] = section;
    }
    let at = active + 1;
    let mut deck = deck.to_vec();
    deck.insert(at.min(deck.len()), new_slide);
    DeckEdit { deck, active: at }
}

/// 原版 `deleteEditorSlide(idx)`：删掉第 `idx` 页，当前页变成 `min(idx, len - 1)`。
///
/// 只剩一页时不动：返回 `None`（原版直接 `return`，牌组与当前页都不变）。
pub fn delete_slide(deck: &[Value], index: usize) -> Option<DeckEdit> {
    if deck.len() <= 1 {
        return None;
    }
    let mut deck = deck.to_vec();
    // JS `splice(idx, 1)` 越界时什么也不删，但当前页仍会被改成 `min(idx, len - 1)`。
    if index < deck.len() {
        deck.remove(index);
    }
    let active = index.min(deck.len() - 1);
    Some(DeckEdit { deck, active })
}

/// 原版 `moveEditorSlide(dir)`：当前页与相邻一页交换，当前页跟到新位置；越界不动。
pub fn move_slide(deck: &[Value], index: usize, dir: i64) -> Option<DeckEdit> {
    let target = index as i64 + dir;
    if target < 0 || target as usize >= deck.len() {
        return None;
    }
    let target = target as usize;
    let mut deck = deck.to_vec();
    deck.swap(index, target);
    Some(DeckEdit {
        deck,
        active: target,
    })
}
