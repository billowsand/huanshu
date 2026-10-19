//! Arithmetic evidence for chart interpretation. The language model writes prose;
//! differences, rates, ties and shares are computed from the immutable source data.
use crate::types::ChartSpec;
use serde_json::{json, Value};

fn number(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str()?.parse().ok())
        .filter(|v| v.is_finite())
}
fn trend(values: &[f64]) -> &'static str {
    if values.len() < 2 {
        return "insufficient_intervals";
    }
    let signs: Vec<_> = values
        .windows(2)
        .map(|v| {
            let delta = v[1] - v[0];
            if delta.abs() <= 1e-9 * v[0].abs().max(v[1].abs()).max(1.0) {
                0
            } else if delta > 0.0 {
                1
            } else {
                -1
            }
        })
        .collect();
    if signs.iter().all(|s| *s == 0) {
        "constant"
    } else if signs.iter().all(|s| *s >= 0) {
        "increasing"
    } else if signs.iter().all(|s| *s <= 0) {
        "decreasing"
    } else {
        "mixed"
    }
}
fn rounded(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

pub fn computed_facts(chart: &ChartSpec) -> Value {
    let donut = chart.chart_type == "donut";
    let label_key = if donut {
        &chart.encoding.label
    } else {
        &chart.encoding.x
    };
    let Some(label_key) = label_key else {
        return json!({"status":"missing_label_encoding"});
    };
    let series: Vec<_> = if matches!(chart.chart_type.as_str(), "grouped_bar" | "stacked_area") {
        chart
            .series
            .iter()
            .map(|s| (s.name.as_str(), s.key.as_str()))
            .collect()
    } else {
        let key = if donut {
            chart.encoding.value.as_deref()
        } else {
            chart.encoding.y.as_deref()
        };
        let Some(key) = key else {
            return json!({"status":"missing_value_encoding"});
        };
        vec![(chart.title.as_str(), key)]
    };
    let mut facts = Vec::new();
    for (name, key) in series {
        let values: Option<Vec<_>> = chart
            .data
            .iter()
            .map(|row| {
                let label = row.get(label_key).map(|v| {
                    v.as_str()
                        .map(str::to_string)
                        .unwrap_or_else(|| v.to_string())
                })?;
                Some((label, number(row.get(key)?)?))
            })
            .collect();
        let Some(values) = values.filter(|v| !v.is_empty()) else {
            facts.push(json!({"series":name,"status":"invalid_numeric_data"}));
            continue;
        };
        let max = values
            .iter()
            .map(|(_, v)| *v)
            .fold(f64::NEG_INFINITY, f64::max);
        let min = values.iter().map(|(_, v)| *v).fold(f64::INFINITY, f64::min);
        let mut fact = json!({"series":name,"observations":values.len(),
            "maximum":{"value":max,"labels":values.iter().filter(|(_,v)|*v==max).map(|(l,_)|l).collect::<Vec<_>>()},
            "minimum":{"value":min,"labels":values.iter().filter(|(_,v)|*v==min).map(|(l,_)|l).collect::<Vec<_>>()}});
        if donut {
            let total: f64 = values.iter().map(|(_, v)| v.max(0.0)).sum();
            fact["positive_total"] = json!(total);
            fact["shares_percent"]=json!(values.iter().map(|(label,value)|json!({"label":label,"percent":if total>0.0 {Some(rounded(value.max(0.0)/total*100.0))} else {None}})).collect::<Vec<_>>());
        } else {
            let deltas: Vec<_> = values.windows(2).map(|v| v[1].1 - v[0].1).collect();
            let rates: Vec<_> = values
                .windows(2)
                .map(|v| {
                    if v[0].1 > 0.0 {
                        Some((v[1].1 - v[0].1) / v[0].1 * 100.0)
                    } else {
                        None
                    }
                })
                .collect();
            let absolute_trend = trend(&deltas);
            let relative_trend = if rates.iter().all(Option::is_some) {
                trend(&rates.iter().flatten().copied().collect::<Vec<_>>())
            } else {
                "undefined_base"
            };
            fact["interval_count"] = json!(deltas.len());
            fact["changes"]=json!(values.windows(2).enumerate().map(|(i,v)|json!({"from":v[0].0,"to":v[1].0,"absolute_change":deltas[i],"relative_change_percent":rates[i].map(rounded)})).collect::<Vec<_>>());
            fact["absolute_increment_trend"] = json!(absolute_trend);
            fact["relative_growth_rate_trend"] = json!(relative_trend);
            if absolute_trend == "constant" {
                fact["absolute_trend_explanation"] =
                    json!("相邻绝对增量相同，不支持增量扩大或加速增长的结论。");
            }
            if relative_trend == "decreasing" {
                fact["relative_trend_explanation"] =
                    json!("环比增速下降，不能描述为环比增速加快。");
            }
        }
        facts.push(fact);
    }
    json!({"status":"computed_from_source","series":facts,"rules":"区分数值水平、绝对增量和相对增速。最大值不等于最大增量；并列值不得描述为唯一最大。不要增加不存在的时间区间。"})
}

pub fn prompt(chart: &ChartSpec) -> String {
    let facts = computed_facts(chart);
    let mut explanation = String::new();
    for series in facts["series"].as_array().into_iter().flatten() {
        let name = series["series"].as_str().unwrap_or("");
        for change in series["changes"].as_array().into_iter().flatten() {
            explanation.push_str(&format!(
                "\n【{}→{}】{}：绝对变化{}，相对变化{}%。增速属于终点期间，不属于起点期间。",
                change["from"],
                change["to"],
                name,
                change["absolute_change"],
                change["relative_change_percent"]
            ));
        }
    }
    if chart.chart_type == "donut" {
        explanation.push_str("\n本页是占比图，三条解读仅围绕总量、各项占比和最大/最小份额。禁止讨论环比、加速、增速、增长贡献，禁止自行增加派生计算。");
    }
    format!("\n\nVERIFIED CHART FACTS (computed by code from source data):\n{}\n{}\n每条解读必须直接对应上述已计算的事实，不得引入未列出的计算、单位或因果结论。并列最大必须明确为并列，不能称唯一最大。不要把期末数值称为累计总量。Each interpretation point must explain a supported fact above. Do not contradict these facts, claim acceleration from rising levels alone, or invent extra intervals. Do not repeat arithmetic yourself. Describe a trend only when there are enough intervals. Never emit these metadata keys into the slide JSON.\n",
        serde_json::to_string_pretty(&facts).expect("finite source facts"), explanation)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample(values: &[f64]) -> ChartSpec {
        serde_json::from_value(json!({"type":"line","title":"交付量","data":values.iter().enumerate().map(|(i,v)|json!({"q":format!("Q{}",i+1),"v":v})).collect::<Vec<_>>(),"encoding":{"x":"q","y":"v"}})).unwrap()
    }
    #[test]
    fn equal_increments_are_not_accelerating_and_growth_rates_can_decline() {
        let facts = computed_facts(&sample(&[12.0, 18.0, 24.0]));
        let s = &facts["series"][0];
        assert_eq!(s["interval_count"], 2);
        assert_eq!(s["absolute_increment_trend"], "constant");
        assert_eq!(s["relative_growth_rate_trend"], "decreasing");
        assert_eq!(s["changes"][0]["relative_change_percent"], 50.0);
        assert_eq!(s["changes"][1]["relative_change_percent"], 33.33);
    }
    #[test]
    fn zero_and_negative_bases_do_not_produce_infinite_or_misleading_percentages() {
        let facts = computed_facts(&sample(&[0.0, -2.0, 4.0]));
        let s = &facts["series"][0];
        assert_eq!(s["relative_growth_rate_trend"], "undefined_base");
        assert!(s["changes"][0]["relative_change_percent"].is_null());
        assert!(s["changes"][1]["relative_change_percent"].is_null());
    }
    #[test]
    fn donut_shares_use_the_same_positive_values_as_the_renderer() {
        let mut chart = sample(&[12.0, 18.0, 24.0]);
        chart.chart_type = "donut".into();
        chart.encoding.label = Some("q".into());
        chart.encoding.value = Some("v".into());
        let facts = computed_facts(&chart);
        assert_eq!(facts["series"][0]["positive_total"], 54.0);
        assert_eq!(facts["series"][0]["shares_percent"][2]["percent"], 44.44);
    }
}
