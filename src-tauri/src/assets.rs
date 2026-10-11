//! Image assets available to the generator.
//!
//! Two sources feed one catalogue: image files bundled in the project directory
//! (`figure/`, `public/`) and the project's media library, whose files live under the app
//! data directory and are referenced as `media:<id>`. Only media-library assets carry a
//! caption and pixel size, so the prompt can give the model a reason to pick one.
//!
//! [`AssetCatalog::refs`] is the set of references a blueprint may legally contain (the
//! validator checks membership), while `AssetCatalog::entries` is the human-readable
//! catalogue the prompts render — `media:` first, because those are the assets a user
//! deliberately uploaded.

use std::collections::HashSet;
use std::path::Path;

use crate::db::MediaAsset;

/// Per-source caps so a large library cannot flood the prompt.
const MAX_MEDIA_ENTRIES: usize = 24;
const MAX_FILE_ENTRIES: usize = 24;

/// Familiar ratios used to label an asset's shape for the model.
const COMMON_RATIOS: &[(f64, &str)] = &[
    (21.0 / 9.0, "21:9"),
    (16.0 / 9.0, "16:9"),
    (3.0 / 2.0, "3:2"),
    (4.0 / 3.0, "4:3"),
    (1.0, "1:1"),
    (3.0 / 4.0, "3:4"),
    (2.0 / 3.0, "2:3"),
    (9.0 / 16.0, "9:16"),
];

/// One selectable image, described for the model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetEntry {
    /// Value to copy verbatim into `image` / `images` / `preferred_assets`.
    pub reference: String,
    /// Media caption or original file name, when known.
    pub label: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
}

impl AssetEntry {
    fn describe(&self) -> String {
        let mut line = format!("- {}", self.reference);
        if let Some(label) = self.label.as_deref().filter(|label| !label.trim().is_empty()) {
            line.push_str(&format!(" | {label}"));
        }
        if let (Some(width), Some(height)) = (self.width, self.height) {
            line.push_str(&format!(" | {width}x{height}"));
            if let Some(ratio) = aspect_label(width, height) {
                line.push_str(&format!(" ({ratio})"));
            }
        }
        line
    }
}

/// Where a media-library image belongs in the document.
///
/// Produced by the placement pass (`generator::media_match`), which reads the article next
/// to the picture: a caption alone can say what an image shows, never where it belongs.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct MediaPlacement {
    /// Database id of the media-library asset.
    pub media_id: i64,
    /// Reference copied verbatim into a blueprint (`media:<id>`).
    pub reference: String,
    /// Target page, or `None` when the image fits no page in particular.
    pub page_id: Option<String>,
    /// One short sentence: what this image contributes to that page.
    pub purpose: String,
    /// Article-aware caption proposed by the same pass, when it produced one.
    pub caption: Option<String>,
}

/// Everything the pipeline knows about usable images.
#[derive(Debug, Clone, Default)]
pub struct AssetCatalog {
    refs: HashSet<String>,
    entries: Vec<AssetEntry>,
    /// Filled in by the placement pass, before per-page planning starts.
    placements: Vec<MediaPlacement>,
}

impl AssetCatalog {
    /// Project-bundled files plus the project's media-library images.
    ///
    /// Never fails: a missing project directory or an empty library simply yields fewer
    /// entries, which is the normal state of a deck that uses no images.
    pub fn collect(project_dir: &Path, media_assets: &[MediaAsset]) -> Self {
        let mut refs = collect_assets(project_dir);
        let mut entries = Vec::new();

        for asset in media_assets
            .iter()
            .filter(|asset| asset.media_type == "image")
            .take(MAX_MEDIA_ENTRIES)
        {
            let reference = format!("media:{}", asset.id);
            refs.insert(reference.clone());
            let caption = asset
                .caption
                .clone()
                .filter(|caption| !caption.trim().is_empty());
            entries.push(AssetEntry {
                reference,
                label: Some(caption.unwrap_or_else(|| asset.original_name.clone())),
                width: asset.width,
                height: asset.height,
            });
        }

        // Bundled files, canonical `figure/…` / `public/…` form only: the bare and
        // absolute variants stay in `refs` for validation, but must not be listed twice.
        let mut files: Vec<&String> = refs
            .iter()
            .filter(|reference| {
                !reference.starts_with("media:")
                    && (reference.starts_with("figure/") || reference.starts_with("public/"))
            })
            .collect();
        files.sort();
        for reference in files.into_iter().take(MAX_FILE_ENTRIES) {
            entries.push(AssetEntry {
                reference: reference.clone(),
                label: None,
                width: None,
                height: None,
            });
        }

        Self {
            refs,
            entries,
            placements: Vec::new(),
        }
    }

    /// A catalogue with only the project's bundled files (no media library).
    #[cfg(test)]
    pub fn from_project_files(project_dir: &Path) -> Self {
        Self::collect(project_dir, &[])
    }

    /// References a blueprint may legally contain.
    pub fn refs(&self) -> &HashSet<String> {
        &self.refs
    }

    pub fn contains(&self, reference: &str) -> bool {
        self.refs.contains(reference)
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.refs.is_empty()
    }

    #[cfg(test)]
    pub fn entries(&self) -> &[AssetEntry] {
        &self.entries
    }

    /// Record where the placement pass decided each image belongs.
    pub fn set_placements(&mut self, placements: Vec<MediaPlacement>) {
        self.placements = placements;
    }

    /// Images the placement pass matched to this page, in deck order.
    pub fn placements_for(&self, page_id: &str) -> Vec<&MediaPlacement> {
        self.placements
            .iter()
            .filter(|placement| placement.page_id.as_deref() == Some(page_id))
            .collect()
    }

    /// Prompt section listing every selectable reference, or `None` when there is none.
    pub fn prompt_section(&self) -> Option<String> {
        self.catalogue_section(None)
    }

    /// Prompt section for one page: the same catalogue, preceded by the images the
    /// placement pass matched to this page together with the reason it gave.
    pub fn prompt_section_for(&self, page_id: &str) -> Option<String> {
        self.catalogue_section(Some(page_id))
    }

    fn catalogue_section(&self, page_id: Option<&str>) -> Option<String> {
        if self.entries.is_empty() {
            return None;
        }
        let mut section =
            String::from("Available image assets (copy a reference verbatim; never invent one):");
        if let Some(page_id) = page_id {
            let matched = self.placements_for(page_id);
            if !matched.is_empty() {
                section.push_str("\n\nMatched to this page by reading the article together with the image");
                section.push_str(" (prefer these when they fit the layout):");
                for placement in matched {
                    section.push_str(&format!(
                        "\n- {} — {}",
                        placement.reference, placement.purpose
                    ));
                }
            }
        }
        for entry in &self.entries {
            section.push_str(&format!("\n{}", entry.describe()));
        }
        Some(section)
    }
}

/// A familiar ratio label ("16:9") when the asset is close to one, else `None`.
fn aspect_label(width: i64, height: i64) -> Option<&'static str> {
    if width <= 0 || height <= 0 {
        return None;
    }
    let ratio = width as f64 / height as f64;
    COMMON_RATIOS
        .iter()
        .map(|(value, label)| (((ratio - value) / value).abs(), *label))
        .filter(|(error, _)| *error < 0.05)
        .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(_, label)| label)
}

/// References for every image file bundled in the project directory: `figure/…`,
/// `/figure/…`, and a bare relative path for files under `figure/`.
pub fn collect_assets(project_root: &Path) -> HashSet<String> {
    let mut set = HashSet::new();
    for base in ["figure", "public"] {
        let root = project_root.join(base);
        collect_dir(&root, &root, &mut set);
    }
    set
}

fn collect_dir(root: &Path, current: &Path, set: &mut HashSet<String>) {
    let Ok(entries) = std::fs::read_dir(current) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_dir(root, &path, set);
            continue;
        }
        if let Ok(rel) = path.strip_prefix(root) {
            let rel = rel.to_string_lossy().replace('\\', "/");
            let prefix = root.file_name().unwrap_or_default().to_string_lossy();
            set.insert(format!("{prefix}/{rel}"));
            set.insert(format!("/{prefix}/{rel}"));
            if prefix == "figure" {
                set.insert(rel.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn media_image(id: i64, caption: Option<&str>, size: Option<(i64, i64)>) -> MediaAsset {
        MediaAsset {
            id,
            project_id: 1,
            slide_index: None,
            filename: format!("{id}.png"),
            original_name: format!("架构图{id}.png"),
            media_type: "image".to_string(),
            mime_type: Some("image/png".to_string()),
            storage_path: format!("projects/1/images/{id}.png"),
            caption: caption.map(str::to_string),
            description: None,
            file_size: None,
            width: size.map(|(width, _)| width),
            height: size.map(|(_, height)| height),
            created_at: 0,
        }
    }

    #[test]
    fn media_library_images_are_selectable_with_caption_and_ratio() {
        let catalog = AssetCatalog::collect(
            Path::new("no-such-project-dir"),
            &[media_image(7, Some("数据平台分层架构"), Some((1920, 1080)))],
        );
        assert!(catalog.contains("media:7"));
        assert_eq!(catalog.refs().len(), 1);
        let section = catalog.prompt_section().expect("prompt section");
        assert!(
            section.contains("- media:7 | 数据平台分层架构 | 1920x1080 (16:9)"),
            "{section}"
        );
    }

    #[test]
    fn caption_falls_back_to_the_original_file_name() {
        let catalog = AssetCatalog::collect(
            Path::new("no-such-project-dir"),
            &[media_image(3, Some("   "), None)],
        );
        let section = catalog.prompt_section().expect("prompt section");
        assert!(section.contains("- media:3 | 架构图3.png"), "{section}");
    }

    #[test]
    fn videos_are_not_offered() {
        let mut video = media_image(9, None, None);
        video.media_type = "video".to_string();
        let catalog = AssetCatalog::collect(Path::new("no-such-project-dir"), &[video]);
        assert!(catalog.is_empty());
        assert!(catalog.prompt_section().is_none());
    }

    #[test]
    fn bundled_files_are_listed_once_in_canonical_form() {
        let root = std::env::temp_dir().join(format!("huanshu-assets-{}", std::process::id()));
        let figure = root.join("figure");
        std::fs::create_dir_all(&figure).expect("create temp figure dir");
        std::fs::write(figure.join("flow.png"), b"stub").expect("write stub asset");

        let catalog = AssetCatalog::collect(&root, &[]);
        // All three spellings stay valid references…
        assert!(catalog.contains("figure/flow.png"));
        assert!(catalog.contains("/figure/flow.png"));
        assert!(catalog.contains("flow.png"));
        // …but the prompt lists the canonical one exactly once.
        let listed: Vec<&str> = catalog
            .entries()
            .iter()
            .map(|entry| entry.reference.as_str())
            .collect();
        assert_eq!(listed, vec!["figure/flow.png"]);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn aspect_labels_ignore_unusual_shapes_and_unknown_sizes() {
        assert_eq!(aspect_label(1920, 1080), Some("16:9"));
        assert_eq!(aspect_label(800, 800), Some("1:1"));
        assert_eq!(aspect_label(1080, 1920), Some("9:16"));
        assert_eq!(aspect_label(1000, 397), None);
        assert_eq!(aspect_label(0, 100), None);
    }

    #[test]
    fn placements_mark_the_images_that_fit_a_page() {
        let mut catalog = AssetCatalog::collect(
            Path::new("no-such-project-dir"),
            &[media_image(7, Some("架构图"), Some((1920, 1080)))],
        );
        catalog.set_placements(vec![MediaPlacement {
            media_id: 7,
            reference: "media:7".to_string(),
            page_id: Some("3".to_string()),
            purpose: "佐证三层架构".to_string(),
            caption: Some("数据平台三层架构".to_string()),
        }]);

        assert_eq!(catalog.placements_for("3").len(), 1);
        assert!(catalog.placements_for("4").is_empty());

        let section = catalog.prompt_section_for("3").expect("page section");
        assert!(section.contains("Matched to this page"), "{section}");
        assert!(section.contains("- media:7 — 佐证三层架构"), "{section}");
        // The page-agnostic catalogue carries no recommendation.
        assert!(!catalog
            .prompt_section()
            .expect("plain section")
            .contains("Matched to this page"));
    }
}
