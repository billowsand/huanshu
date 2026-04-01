#[allow(dead_code)]
use crate::types::DeckSpec;

#[allow(dead_code)]
pub fn render_deck(deck: &DeckSpec) -> String {
    let mut out = String::new();
    out.push_str("---\n");
    out.push_str("layout: center\n");
    out.push_str("highlighter: shiki\n");
    out.push_str("css: unocss\n");
    out.push_str("colorSchema: dark\n");
    out.push_str("transition: none\n");
    out.push_str(&format!("title: {}\n", yaml_str(&deck.title)));
    out.push_str(&format!(
        "exportFilename: {}\n",
        yaml_str(&deck.export_filename)
    ));
    out.push_str("lineNumbers: false\n");
    out.push_str("drawings:\n  persist: false\n");
    out.push_str("mdc: true\n");
    out.push_str("clicks: 0\n");
    out.push_str("preload: false\n");
    out.push_str("routerMode: hash\n");
    out.push_str("fonts:\n");
    out.push_str("  local: MiSans\n");
    out.push_str("  sans: 'MiSans'\n");
    out.push_str("  serif: 'MiSans'\n");
    out.push_str("  mono: 'MiSans'\n");
    out.push_str("---\n\n");

    for (idx, slide) in deck.slides.iter().enumerate() {
        if idx > 0 {
            if let Some(layout) = &slide.layout {
                out.push_str(&format!("\n---\nlayout: {layout}\n---\n\n"));
            } else {
                out.push_str("\n---\n\n");
            }
        }
        out.push_str(&slide.component);
        out.push('\n');
    }

    out
}

#[allow(dead_code)]
fn yaml_str(value: &str) -> String {
    if value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "-_ ".contains(c))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "''"))
    }
}

#[allow(dead_code)]
pub fn render_component(tag: &str, lines: &[String]) -> String {
    let mut out = String::new();
    out.push('<');
    out.push_str(tag);
    out.push('\n');
    for line in lines {
        out.push_str("  ");
        out.push_str(line);
        out.push('\n');
    }
    out.push_str("/>");
    out
}

#[allow(dead_code)]
pub fn render_prop(name: &str, value: &str) -> String {
    format!("{name}={}", quoted_js_expr(value))
}

#[allow(dead_code)]
pub fn render_plain_prop(name: &str, value: &str) -> String {
    format!("{name}={}", quoted_plain(value))
}

#[allow(dead_code)]
fn quoted_plain(value: &str) -> String {
    let escaped = value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\n', " ");
    format!("\"{escaped}\"")
}

#[allow(dead_code)]
fn quoted_js_expr(value: &str) -> String {
    let escaped = value
        .replace('&', "&amp;")
        .replace('\'', "&#39;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\n', " ");
    format!("'{escaped}'")
}
