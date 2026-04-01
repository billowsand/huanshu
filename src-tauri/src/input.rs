use anyhow::{Result, bail};
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
    if h2 > h3 { HeadingLevel::H2 } else { HeadingLevel::H3 }
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subsection {
    pub title: String,
    pub paragraphs: Vec<String>,
}

pub fn parse_markdown(raw: &str) -> Result<ParsedDocument> {
    let mut title = None::<String>;
    let mut intro = Vec::new();
    let mut sections = Vec::new();
    let mut current_section = None::<Section>;
    let mut current_subsection = None::<Subsection>;
    let mut paragraph_buf = Vec::<String>::new();

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
