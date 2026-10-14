//! 图表数据的结构校验：一份规则，两种语言。
//!
//! 规则只在这里写一次。生成流水线解析 Markdown 时要英文（`validate.rs`、给模型的报错都按英文写），
//! Step3 编辑器里手改图表 JSON 时要中文；除此之外两端完全一致。
//!
//! 输入是任意 JSON：编辑器里的 JSON 可能根本不是对象、`type` 不合法、`data` 不是数组。
//! [`ChartSpec::problems`](crate::types::ChartSpec::problems) 反序列化之后复用同一套规则。

use serde_json::Value;

/// 一条图表问题。`message_en()` 与 `message_zh()` 的措辞分别锁死在各自的历史输出上：
/// 英文被 `input.rs` 的解析报错和 LLM 报错依赖，中文是编辑器直接显示给用户的文案。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChartProblem {
    /// 整个 chart 不是对象。
    NotAnObject,
    /// `type` 不在支持列表里；参数是它在 JSON 里的原始文本。
    InvalidType(String),
    EmptyTitle,
    EmptyData,
    /// 折线图/柱状图缺 `encoding.x` 或 `encoding.y`。
    MissingXY,
    /// 环形图缺 `encoding.label` 或 `encoding.value`。
    MissingDonutEncoding,
    /// 分组柱状图/堆叠面积图缺 `encoding.x`。
    MissingGroupedX,
    /// 分组柱状图/堆叠面积图一个 series 都没有。
    NoSeries,
    /// 某个 series 条目的 `key` 为空。
    SeriesWithoutKey,
    /// 某一行不是对象；参数是行号（从 1 开始）。
    RowNotAnObject(usize),
    /// 某一行里某个应当为数值的列不是数字；参数是行号、列名、列的用途描述。
    NonNumericCell {
        row: usize,
        key: String,
        what: &'static str,
    },
}

impl ChartProblem {
    pub fn message_en(&self) -> String {
        match self {
            ChartProblem::NotAnObject => "chart must be an object".to_string(),
            ChartProblem::InvalidType(kind) => format!(
                "chart type '{kind}' is invalid; must be one of {}",
                crate::types::CHART_TYPES.join(", ")
            ),
            ChartProblem::EmptyTitle => "chart title is empty".to_string(),
            ChartProblem::EmptyData => "chart data is empty".to_string(),
            ChartProblem::MissingXY => {
                "line/bar chart requires encoding.x and encoding.y".to_string()
            }
            ChartProblem::MissingDonutEncoding => {
                "donut chart requires encoding.label and encoding.value".to_string()
            }
            ChartProblem::MissingGroupedX => {
                "grouped/stacked chart requires encoding.x".to_string()
            }
            ChartProblem::NoSeries => {
                "grouped/stacked chart requires at least one series entry".to_string()
            }
            ChartProblem::SeriesWithoutKey => "chart series entry has empty key".to_string(),
            ChartProblem::RowNotAnObject(row) => format!("chart data row {row} is not an object"),
            ChartProblem::NonNumericCell { row, key, what } => {
                format!("chart data row {row} is missing a numeric value for {what} '{key}'")
            }
        }
    }

    pub fn message_zh(&self) -> String {
        let types = crate::types::CHART_TYPES.join(" / ");
        match self {
            ChartProblem::NotAnObject => "chart 必须是对象".to_string(),
            ChartProblem::InvalidType(kind) => format!("type \"{kind}\" 无效，应为 {types}"),
            ChartProblem::EmptyTitle => "title 不能为空".to_string(),
            ChartProblem::EmptyData => "data 不能为空".to_string(),
            ChartProblem::MissingXY => "折线图/柱状图需要 encoding.x 与 encoding.y".to_string(),
            ChartProblem::MissingDonutEncoding => {
                "环形图需要 encoding.label 与 encoding.value".to_string()
            }
            ChartProblem::MissingGroupedX => "分组柱状图/堆叠面积图需要 encoding.x".to_string(),
            ChartProblem::NoSeries => "分组柱状图/堆叠面积图至少需要一个 series".to_string(),
            ChartProblem::SeriesWithoutKey => "series 条目的 key 不能为空".to_string(),
            ChartProblem::RowNotAnObject(row) => format!("data 第 {row} 行不是对象"),
            ChartProblem::NonNumericCell { row, key, what } => {
                format!("data 第 {row} 行的 {what}「{key}」不是数字")
            }
        }
    }
}

/// Numbers, or strings that parse as one (the renderer applies `Number()` to every value).
fn is_numeric(value: &Value) -> bool {
    value.is_number()
        || value
            .as_str()
            .is_some_and(|s| s.trim().parse::<f64>().is_ok_and(f64::is_finite))
}

fn present_str(value: Option<&Value>) -> bool {
    value
        .and_then(Value::as_str)
        .is_some_and(|s| !s.trim().is_empty())
}

/// `String(spec.type)` in the old TypeScript: `undefined` and `null` both read as "undefined".
fn type_text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Null) | None => "undefined".to_string(),
        Some(other) => other.to_string(),
    }
}

/// Validates an arbitrary JSON value as a chart. An empty result means it can be rendered.
///
/// The editor hands over whatever the user typed, so this accepts non-objects, a missing or
/// invalid `type`, and a `data` that is not an array of objects.
pub fn chart_problems_json(value: &Value) -> Vec<ChartProblem> {
    if !value.is_object() {
        return vec![ChartProblem::NotAnObject];
    }
    let spec = value.as_object().expect("checked above");
    let chart_type = spec.get("type");
    let chart_type = match chart_type.and_then(Value::as_str) {
        Some(kind) if crate::types::CHART_TYPES.contains(&kind) => kind,
        _ => return vec![ChartProblem::InvalidType(type_text(chart_type))],
    };

    let mut out = Vec::new();
    if !present_str(spec.get("title")) {
        out.push(ChartProblem::EmptyTitle);
    }
    let data = spec.get("data").and_then(Value::as_array);
    if data.map_or(true, |rows| rows.is_empty()) {
        out.push(ChartProblem::EmptyData);
    }

    let encoding = spec.get("encoding").and_then(Value::as_object);
    // (column, what) pairs whose values must be numeric in every row.
    let mut numeric: Vec<(String, &'static str)> = Vec::new();
    match chart_type {
        "line" | "bar" => {
            let x = encoding.and_then(|e| e.get("x"));
            let y = encoding.and_then(|e| e.get("y"));
            if !present_str(x) || !present_str(y) {
                out.push(ChartProblem::MissingXY);
            } else if let Some(y) = y.and_then(Value::as_str) {
                numeric.push((y.to_string(), "encoding.y"));
            }
        }
        "donut" => {
            let label = encoding.and_then(|e| e.get("label"));
            let value = encoding.and_then(|e| e.get("value"));
            if !present_str(label) || !present_str(value) {
                out.push(ChartProblem::MissingDonutEncoding);
            } else if let Some(value) = value.and_then(Value::as_str) {
                numeric.push((value.to_string(), "encoding.value"));
            }
        }
        _ => {
            if !present_str(encoding.and_then(|e| e.get("x"))) {
                out.push(ChartProblem::MissingGroupedX);
            }
            let series = spec.get("series").and_then(Value::as_array);
            let series = series.filter(|entries| !entries.is_empty());
            if series.is_none() {
                out.push(ChartProblem::NoSeries);
            }
            for entry in series.into_iter().flatten() {
                let key = entry.as_object().and_then(|s| s.get("key"));
                match present_str(key) {
                    true => {
                        if let Some(name) = key.and_then(Value::as_str) {
                            numeric.push((name.to_string(), "series"));
                        }
                    }
                    // The old TypeScript read `s?.key` on a possibly non-object entry.
                    false => out.push(ChartProblem::SeriesWithoutKey),
                }
            }
        }
    }

    for (index, row) in data.into_iter().flatten().enumerate() {
        let Some(row) = row.as_object() else {
            out.push(ChartProblem::RowNotAnObject(index + 1));
            continue;
        };
        for (key, what) in &numeric {
            if !row.get(key).is_some_and(is_numeric) {
                out.push(ChartProblem::NonNumericCell {
                    row: index + 1,
                    key: key.clone(),
                    what,
                });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Every branch of the old TypeScript `chartProblems()`, with the output it produced.
    #[test]
    fn the_chinese_branch_table_matches_the_old_typescript() {
        let cases: Vec<(Value, Vec<&str>)> = vec![
            (json!(null), vec!["chart 必须是对象"]),
            (json!([1, 2]), vec!["chart 必须是对象"]),
            (
                json!({ "type": "pie", "title": "t", "data": [{}], "encoding": {} }),
                vec!["type \"pie\" 无效，应为 line / bar / donut / grouped_bar / stacked_area"],
            ),
            (
                json!({ "title": "t", "data": [{}] }),
                vec![
                    "type \"undefined\" 无效，应为 line / bar / donut / grouped_bar / stacked_area",
                ],
            ),
            (
                json!({ "type": null, "title": "t" }),
                vec![
                    "type \"undefined\" 无效，应为 line / bar / donut / grouped_bar / stacked_area",
                ],
            ),
            (
                json!({ "type": "bar", "title": "  ", "data": [], "encoding": { "x": "a", "y": "b" } }),
                vec!["title 不能为空", "data 不能为空"],
            ),
            (
                json!({ "type": "bar", "title": "t", "data": "nope", "encoding": { "x": "a", "y": "b" } }),
                vec!["data 不能为空"],
            ),
            (
                json!({ "type": "bar", "title": "t", "data": [{ "a": 1, "b": 2 }] }),
                vec!["折线图/柱状图需要 encoding.x 与 encoding.y"],
            ),
            (
                json!({ "type": "donut", "title": "t", "data": [{ "a": 1 }], "encoding": { "x": "a" } }),
                vec!["环形图需要 encoding.label 与 encoding.value"],
            ),
            (
                json!({ "type": "grouped_bar", "title": "t", "data": [{ "a": 1 }], "encoding": {} }),
                vec![
                    "分组柱状图/堆叠面积图需要 encoding.x",
                    "分组柱状图/堆叠面积图至少需要一个 series",
                ],
            ),
            (
                json!({ "type": "stacked_area", "title": "t", "data": [{ "a": 1 }], "encoding": { "x": "a" }, "series": [{ "name": "s", "key": "" }] }),
                vec!["series 条目的 key 不能为空"],
            ),
            (
                json!({ "type": "bar", "title": "t", "data": [1, { "a": 1, "b": "x" }], "encoding": { "x": "a", "y": "b" } }),
                vec![
                    "data 第 1 行不是对象",
                    "data 第 2 行的 encoding.y「b」不是数字",
                ],
            ),
        ];
        for (value, expected) in cases {
            let actual = chart_problems_json(&value);
            assert_eq!(
                actual
                    .iter()
                    .map(ChartProblem::message_zh)
                    .collect::<Vec<_>>(),
                expected,
                "input: {value}"
            );
        }
    }

    #[test]
    fn numeric_strings_and_floats_count_as_numbers() {
        let value = json!({
            "type": "line",
            "title": "t",
            "data": [{ "x": "a", "y": "1.5" }, { "x": "b", "y": 2.0 }],
            "encoding": { "x": "x", "y": "y" },
        });
        assert!(chart_problems_json(&value).is_empty());
    }

    #[test]
    fn a_deserialized_spec_reports_the_same_problems_as_the_english_wording() {
        let broken = json!({
            "type": "bar",
            "title": "",
            "data": [{ "a": "x" }],
            "encoding": { "x": "a", "y": "b" },
        });
        let parsed: crate::types::ChartSpec = serde_json::from_value(broken.clone()).unwrap();
        assert_eq!(
            parsed.problems(),
            chart_problems_json(&broken)
                .iter()
                .map(ChartProblem::message_en)
                .collect::<Vec<_>>()
        );
    }
}
