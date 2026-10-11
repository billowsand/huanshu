//! Carbon icons for the Typst templates.
//!
//! Templates request `/icons/<set>/<name>/<rrggbb[aa]>.svg`; this module turns the
//! iconify body into a standalone SVG with `currentColor` replaced by that color.
//! Unknown names fall back to `circle-dash` so a model-invented icon never
//! breaks the whole export (the generator's icon repair should prevent it).

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::Deserialize;

static CARBON_JSON: &str = include_str!(concat!(env!("OUT_DIR"), "/carbon-icons.json"));

pub const FALLBACK_ICON: &str = "circle-dash";

#[derive(Deserialize)]
struct IconSet {
    #[serde(default)]
    icons: HashMap<String, IconData>,
    #[serde(default)]
    aliases: HashMap<String, Alias>,
    #[serde(default = "default_size")]
    width: f64,
    #[serde(default = "default_size")]
    height: f64,
}

#[derive(Deserialize)]
struct IconData {
    body: String,
    width: Option<f64>,
    height: Option<f64>,
}

#[derive(Deserialize)]
struct Alias {
    parent: String,
}

fn default_size() -> f64 {
    32.0
}

fn carbon() -> &'static IconSet {
    static SET: OnceLock<IconSet> = OnceLock::new();
    SET.get_or_init(|| {
        serde_json::from_str(CARBON_JSON).unwrap_or_else(|_| IconSet {
            icons: HashMap::new(),
            aliases: HashMap::new(),
            width: 32.0,
            height: 32.0,
        })
    })
}

fn lookup<'a>(set: &'a IconSet, name: &str) -> Option<&'a IconData> {
    let mut name = name;
    // Aliases may chain; a small bound guards against cycles in bad data.
    for _ in 0..4 {
        if let Some(icon) = set.icons.get(name) {
            return Some(icon);
        }
        name = &set.aliases.get(name)?.parent;
    }
    None
}

/// Returns `(svg, used_fallback)` for an icon tinted with `hex` (`rrggbb` or `rrggbbaa`).
pub fn render_svg(set_name: &str, name: &str, hex: &str) -> (String, bool) {
    let set = carbon();
    let (icon, fallback) = match (set_name, lookup(set, name)) {
        ("carbon", Some(icon)) => (Some(icon), false),
        _ => (lookup(set, FALLBACK_ICON), true),
    };
    let is_hex = hex.chars().all(|c| c.is_ascii_hexdigit());
    let (color, opacity) = match hex.len() {
        6 if is_hex => (format!("#{hex}"), 1.0),
        8 if is_hex => (
            format!("#{}", &hex[..6]),
            u8::from_str_radix(&hex[6..], 16).map_or(1.0, |a| f64::from(a) / 255.0),
        ),
        _ => ("#000000".to_string(), 1.0),
    };
    let Some(icon) = icon else {
        return (
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32"/>"#.to_string(),
            true,
        );
    };
    let w = icon.width.unwrap_or(set.width);
    let h = icon.height.unwrap_or(set.height);
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" fill="{color}"><g opacity="{opacity:.3}">{}</g></svg>"#,
        icon.body.replace("currentColor", &color)
    );
    (svg, fallback)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tints_known_icon() {
        let (svg, fallback) = render_svg("carbon", "data-base", "4e6570");
        assert!(!fallback);
        assert!(svg.contains("#4e6570"));
        assert!(!svg.contains("currentColor"));
    }

    #[test]
    fn alpha_becomes_opacity() {
        let (svg, _) = render_svg("carbon", "image", "78605166");
        assert!(svg.contains("#786051"));
        assert!(svg.contains(r#"opacity="0.400""#), "{svg}");
    }

    #[test]
    fn unknown_icon_falls_back() {
        let (svg, fallback) = render_svg("carbon", "no-such-icon-xyz", "000000");
        assert!(fallback);
        assert!(svg.starts_with("<svg"));
    }
}
