use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeadingLevel {
    H2,
    H3,
}

/// Count heading occurrences and recommend the level with the most headings.
/// Falls back to H3 when tied or when neither level has headings.
pub fn detect_granularity(raw: &str) -> HeadingLevel {
    let h2 = raw.lines().filter(|l| l.starts_with("## ")).count();
    let h3 = raw.lines().filter(|l| l.starts_with("### ")).count();
    if h2 > h3 {
        HeadingLevel::H2
    } else {
        HeadingLevel::H3
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedDocument {
    pub title: String,
    pub intro: Vec<String>,
    pub sections: Vec<Section>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Section {
    pub title: String,
    pub paragraphs: Vec<String>,
    pub subsections: Vec<Subsection>,
    /// chart 围栏块（H2 直属、未进入任何 H3 之前出现的）
    #[serde(default)]
    pub charts: Vec<crate::types::ChartSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subsection {
    pub title: String,
    pub paragraphs: Vec<String>,
    #[serde(default)]
    pub charts: Vec<crate::types::ChartSpec>,
}

pub fn parse_markdown(raw: &str) -> Result<ParsedDocument> {
    let mut title = None::<String>;
    let mut intro = Vec::new();
    let mut sections = Vec::new();
    let mut current_section = None::<Section>;
    let mut current_subsection = None::<Subsection>;
    let mut paragraph_buf = Vec::<String>::new();
    // ```chart 围栏块捕获缓冲（None = 不在块内）
    let mut chart_fence = None::<Vec<String>>;
    // $$ 块级公式状态：公式块可能跨空行，期间空行不 flush、标题 / --- 不生效
    let mut in_display_math = false;

    let flush_paragraphs = |buf: &mut Vec<String>,
                            section: &mut Option<Section>,
                            subsection: &mut Option<Subsection>,
                            intro: &mut Vec<String>| {
        if buf.is_empty() {
            return;
        }
        let chunk = buf.join("\n").trim().to_string();
        buf.clear();
        if chunk.is_empty() {
            return;
        }
        if let Some(sub) = subsection.as_mut() {
            sub.paragraphs.push(chunk);
        } else if let Some(sec) = section.as_mut() {
            sec.paragraphs.push(chunk);
        } else {
            intro.push(chunk);
        }
    };

    let flush_subsection = |section: &mut Option<Section>, subsection: &mut Option<Subsection>| {
        if let Some(sub) = subsection.take() {
            if let Some(sec) = section.as_mut() {
                sec.subsections.push(sub);
            }
        }
    };

    let flush_section = |sections: &mut Vec<Section>, section: &mut Option<Section>| {
        if let Some(sec) = section.take() {
            sections.push(sec);
        }
    };

    for raw_line in raw.lines() {
        let line = raw_line.trim();

        // ```chart 围栏块：收集 JSON 行，闭合时解析为 ChartSpec 挂到当前小节
        if let Some(buf) = chart_fence.as_mut() {
            if line == "```" {
                let json = buf.join("\n");
                chart_fence = None;
                let chart: crate::types::ChartSpec = serde_json::from_str(&json)
                    .map_err(|e| anyhow::anyhow!("```chart 块 JSON 解析失败: {e}\n{json}"))?;
                // 数据错误只有用户能修；放进生成流程只会被 repair 反复打回，最后换模板把图丢掉。
                let problems = chart.problems();
                if !problems.is_empty() {
                    bail!(
                        "```chart 块「{}」数据不合法：{}\n{json}",
                        chart.title,
                        problems.join("；")
                    );
                }
                if let Some(sub) = current_subsection.as_mut() {
                    sub.charts.push(chart);
                } else if let Some(sec) = current_section.as_mut() {
                    sec.charts.push(chart);
                } else {
                    bail!("```chart 块必须位于某个 H2/H3 小节内（文档标题之后）");
                }
            } else {
                buf.push(raw_line.to_string());
            }
            continue;
        }
        if line == "```chart" {
            flush_paragraphs(
                &mut paragraph_buf,
                &mut current_section,
                &mut current_subsection,
                &mut intro,
            );
            chart_fence = Some(Vec::new());
            continue;
        }

        // $$ 块级公式：某行 trim 后以 `$$` 开头且该行 `$$` 出现奇数次时切换状态
        //（`$$x$$` 单行出现偶数次，不切换）。在公式块期间所有行原样进段落缓冲，
        // 空行不 flush、标题 / --- 不生效，保证跨空行的公式完整留在同一段落。
        // ```chart 围栏在上面先匹配，优先级更高。
        if in_display_math {
            paragraph_buf.push(raw_line.to_string());
            if line.starts_with("$$") && line.matches("$$").count() % 2 == 1 {
                in_display_math = false;
            }
            continue;
        }
        if line.starts_with("$$") && line.matches("$$").count() % 2 == 1 {
            in_display_math = true;
            paragraph_buf.push(raw_line.to_string());
            continue;
        }

        if line == "---" {
            flush_paragraphs(
                &mut paragraph_buf,
                &mut current_section,
                &mut current_subsection,
                &mut intro,
            );
            continue;
        }
        if let Some(rest) = line.strip_prefix("# ") {
            flush_paragraphs(
                &mut paragraph_buf,
                &mut current_section,
                &mut current_subsection,
                &mut intro,
            );
            flush_subsection(&mut current_section, &mut current_subsection);
            flush_section(&mut sections, &mut current_section);
            title = Some(clean_heading(rest));
            continue;
        }
        if let Some(rest) = line.strip_prefix("## ") {
            flush_paragraphs(
                &mut paragraph_buf,
                &mut current_section,
                &mut current_subsection,
                &mut intro,
            );
            flush_subsection(&mut current_section, &mut current_subsection);
            flush_section(&mut sections, &mut current_section);
            current_section = Some(Section {
                title: clean_heading(rest),
                paragraphs: Vec::new(),
                subsections: Vec::new(),
                charts: Vec::new(),
            });
            continue;
        }
        if let Some(rest) = line.strip_prefix("### ") {
            flush_paragraphs(
                &mut paragraph_buf,
                &mut current_section,
                &mut current_subsection,
                &mut intro,
            );
            flush_subsection(&mut current_section, &mut current_subsection);
            current_subsection = Some(Subsection {
                title: clean_heading(rest),
                paragraphs: Vec::new(),
                charts: Vec::new(),
            });
            continue;
        }

        if line.is_empty() {
            flush_paragraphs(
                &mut paragraph_buf,
                &mut current_section,
                &mut current_subsection,
                &mut intro,
            );
        } else {
            paragraph_buf.push(line.to_string());
        }
    }

    if chart_fence.is_some() {
        bail!("```chart 块未闭合（缺少结尾 ```）");
    }

    flush_paragraphs(
        &mut paragraph_buf,
        &mut current_section,
        &mut current_subsection,
        &mut intro,
    );
    flush_subsection(&mut current_section, &mut current_subsection);
    flush_section(&mut sections, &mut current_section);

    let title = title
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .ok_or_else(|| anyhow::anyhow!("missing document H1 title"))?;

    if sections.is_empty() {
        bail!("input markdown has no H2 sections");
    }

    Ok(ParsedDocument {
        title,
        intro,
        sections,
    })
}

fn clean_heading(raw: &str) -> String {
    raw.replace('*', "").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHART_JSON: &str = r#"{
      "type": "bar",
      "title": "季度交付量",
      "data": [{"quarter": "Q1", "count": 12}],
      "encoding": {"x": "quarter", "y": "count"},
      "source": "user_input"
    }"#;

    #[test]
    fn chart_fence_attaches_to_subsection() {
        let raw = format!(
            "# 标题\n\n## 章节\n\n### 小节\n\n段落。\n\n```chart\n{CHART_JSON}\n```\n\n后续段落。\n"
        );
        let doc = parse_markdown(&raw).unwrap();
        assert_eq!(doc.sections.len(), 1);
        assert_eq!(doc.sections[0].charts.len(), 0);
        assert_eq!(doc.sections[0].subsections.len(), 1);
        let charts = &doc.sections[0].subsections[0].charts;
        assert_eq!(charts.len(), 1);
        assert_eq!(charts[0].chart_type, "bar");
        assert_eq!(charts[0].title, "季度交付量");
        assert_eq!(charts[0].data.len(), 1);
    }

    #[test]
    fn chart_fence_before_any_h3_attaches_to_section() {
        let raw = format!("# 标题\n\n## 章节\n\n```chart\n{CHART_JSON}\n```\n");
        let doc = parse_markdown(&raw).unwrap();
        assert_eq!(doc.sections[0].charts.len(), 1);
        assert!(doc.sections[0].subsections.is_empty());
    }

    #[test]
    fn chart_fence_flushes_pending_paragraph() {
        let raw = format!("# 标题\n\n## 章节\n\n同段落第一行\n```chart\n{CHART_JSON}\n```\n");
        let doc = parse_markdown(&raw).unwrap();
        assert_eq!(doc.sections[0].paragraphs, vec!["同段落第一行"]);
        assert_eq!(doc.sections[0].charts.len(), 1);
    }

    #[test]
    fn chart_fence_in_intro_is_rejected() {
        let raw = format!("# 标题\n\n```chart\n{CHART_JSON}\n```\n\n## 章节\n\n");
        assert!(parse_markdown(&raw).is_err());
    }

    #[test]
    fn unclosed_chart_fence_is_rejected() {
        let raw = format!("# 标题\n\n## 章节\n\n```chart\n{CHART_JSON}\n");
        assert!(parse_markdown(&raw).is_err());
    }

    #[test]
    fn chart_with_non_numeric_values_is_rejected_at_parse_time() {
        let raw = "# 标题\n\n## 章节\n\n```chart\n{\"type\": \"bar\", \"title\": \"T\", \"data\": [{\"q\": \"Q1\", \"n\": \"很多\"}], \"encoding\": {\"x\": \"q\", \"y\": \"n\"}}\n```\n";
        let err = parse_markdown(raw).unwrap_err().to_string();
        assert!(err.contains("数据不合法"), "{err}");
    }

    #[test]
    fn invalid_chart_json_reports_source() {
        let raw = "# 标题\n\n## 章节\n\n```chart\n{\"type\": 123}\n```\n";
        let err = parse_markdown(raw).unwrap_err().to_string();
        assert!(err.contains("```chart 块 JSON 解析失败"), "{err}");
    }

    #[test]
    fn display_math_spanning_blank_lines_stays_in_one_paragraph() {
        let raw =
            "# 标题\n\n## 章节\n\n公式如下\n$$\n\\frac{a}{b}\n\n= c\n$$\n\n后续段落。\n";
        let doc = parse_markdown(raw).unwrap();
        assert_eq!(
            doc.sections[0].paragraphs,
            vec![
                "公式如下\n$$\n\\frac{a}{b}\n\n= c\n$$".to_string(),
                "后续段落。".to_string()
            ]
        );
    }

    #[test]
    fn headings_and_hrules_inside_display_math_are_kept_as_text() {
        let raw = "# 标题\n\n## 章节\n\n$$\n# 不是标题\n---\n$$\n\n后续段落。\n";
        let doc = parse_markdown(raw).unwrap();
        assert_eq!(doc.sections.len(), 1);
        assert_eq!(doc.sections[0].title, "章节");
        assert_eq!(
            doc.sections[0].paragraphs,
            vec![
                "$$\n# 不是标题\n---\n$$".to_string(),
                "后续段落。".to_string()
            ]
        );
    }

    #[test]
    fn single_line_display_math_does_not_swallow_following_lines() {
        // `$$...$$` 单行出现两次 $$（偶数），不进入块级公式状态
        let raw = "# 标题\n\n## 章节\n\n$$\\frac{a}{b}$$\n\n后续段落。\n";
        let doc = parse_markdown(raw).unwrap();
        assert_eq!(
            doc.sections[0].paragraphs,
            vec!["$$\\frac{a}{b}$$".to_string(), "后续段落。".to_string()]
        );
    }
}
