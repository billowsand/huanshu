//! Images referenced by blueprints (`image` / `images` fields).
//!
//! The caller maps every reference spelling it knows (`media:<id>`,
//! `figure/x.png`, …) to a file or bytes. Before compiling, each referenced
//! image is loaded, sniffed and fully decoded with the same decoders Typst uses:
//! one broken file would otherwise abort the whole export. Images that cannot
//! be used are reported as [`MissingAsset`] and the template shows a placeholder.
//! Typst only ever sees index-based virtual paths, never the reference strings.

use std::collections::HashMap;
use std::fmt;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use base64::Engine;
use serde::Serialize;

/// Where the bytes of one referenced image come from.
#[derive(Clone)]
pub enum AssetData {
    Path(PathBuf),
    Bytes(Vec<u8>),
}

impl fmt::Debug for AssetData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Path(path) => f.debug_tuple("Path").field(path).finish(),
            Self::Bytes(bytes) => write!(f, "Bytes({} bytes)", bytes.len()),
        }
    }
}

/// Reference → image source, keyed exactly as the reference appears in blueprints.
#[derive(Debug, Clone, Default)]
pub struct Assets {
    entries: HashMap<String, AssetData>,
}

impl Assets {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_path(&mut self, reference: impl Into<String>, path: impl Into<PathBuf>) {
        self.entries
            .insert(reference.into(), AssetData::Path(path.into()));
    }

    pub fn insert_bytes(&mut self, reference: impl Into<String>, bytes: Vec<u8>) {
        self.entries
            .insert(reference.into(), AssetData::Bytes(bytes));
    }

    /// Reads a JSON manifest `{ "<reference>": "<path relative to the manifest>" }`.
    pub fn from_manifest(path: &Path) -> std::io::Result<Self> {
        let raw = std::fs::read_to_string(path)?;
        let map: HashMap<String, String> = serde_json::from_str(&raw)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
        let base = path.parent().unwrap_or(Path::new("."));
        let mut assets = Self::new();
        for (reference, file) in map {
            assets.insert_path(reference, base.join(file));
        }
        Ok(assets)
    }

    fn load(&self, reference: &str) -> Result<Vec<u8>, String> {
        if let Some(data) = reference.strip_prefix("data:") {
            return decode_data_uri(data);
        }
        match self.entries.get(reference) {
            Some(AssetData::Bytes(bytes)) => Ok(bytes.clone()),
            Some(AssetData::Path(path)) => {
                std::fs::read(path).map_err(|err| format!("读取 {} 失败：{err}", path.display()))
            }
            None if reference.starts_with("http://") || reference.starts_with("https://") => {
                Err("导出时不下载远程图片".to_string())
            }
            None => Err("素材目录中没有这个引用".to_string()),
        }
    }
}

/// An image reference that rendered as a placeholder.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MissingAsset {
    /// Index into the `slides` passed to the renderer.
    pub slide: usize,
    pub reference: String,
    pub reason: String,
}

/// What the templates learn about one usable image (sent in `/deck.json`).
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ImageInfo {
    /// Virtual path served by the world.
    pub path: String,
    /// Intrinsic size in pixels. Raster images are not upscaled past it, like
    /// the browser's `object-contain max-w-full max-h-full`; SVGs scale freely.
    pub width: f64,
    pub height: f64,
    pub vector: bool,
}

/// Images ready for the Typst world.
pub(crate) struct PreparedAssets {
    /// Reference → image info, sent to Typst inside `/deck.json`.
    pub paths: HashMap<String, ImageInfo>,
    /// Virtual path → bytes, served by the world.
    pub files: HashMap<String, Vec<u8>>,
    pub missing: Vec<MissingAsset>,
}

/// Templates that render their `image` / `images` fields. Other kinds may still
/// carry an image reference (blueprints keep fields across kind changes); those
/// are neither loaded nor reported, since the page never shows them.
const IMAGE_KINDS: &[&str] = &["spotlight"];

/// Image references a slide actually displays.
fn image_refs(slide: &serde_json::Value) -> Vec<&str> {
    let kind = slide.get("kind").and_then(|v| v.as_str()).unwrap_or("");
    if !IMAGE_KINDS.contains(&kind) {
        return Vec::new();
    }
    let mut refs = Vec::new();
    if let Some(image) = slide.get("image").and_then(|v| v.as_str()) {
        refs.push(image);
    }
    if let Some(images) = slide.get("images").and_then(|v| v.as_array()) {
        refs.extend(images.iter().filter_map(|v| v.as_str()));
    }
    refs.into_iter()
        .map(str::trim)
        .filter(|r| !r.is_empty())
        .collect()
}

/// Synthetic reference of a chart_focus slide's SSR SVG. The app renders the
/// chart in the webview (ECharts SSR) and hands the SVG bytes over `Assets`;
/// the template looks the result up by this exact spelling.
pub fn chart_asset_ref(slide_index: usize) -> String {
    format!("chart:{slide_index}")
}

fn is_chart_slide(slide: &serde_json::Value) -> bool {
    slide.get("kind").and_then(|v| v.as_str()) == Some("chart_focus")
}

/// ECharts SSR SVG → 合法 XML。SSR 把字体列表原样写进 style 属性：
/// `style="font-family:"Microsoft YaHei", "PingFang SC", sans-serif;"`，内层裸双引号会提前
/// 结束属性值，XML 解析直接失败。把 `font-family:` 值里的双引号换成单引号即可（保留整条回退列表）。
/// 紧凑 path 语法（`M48 400.5L880`）是合法 SVG，usvg 能直接解析，不要去改写 path：
/// 按 `d="` 查找会同时命中 `id="…"`，曾把 `id="zr0-c0"` 改成 `id="zr0 -c0"`，
/// clip-path / 渐变引用随之失效。
fn sanitize_svg_for_typst(svg: &str) -> String {
    const KEY: &str = "font-family:";
    let mut out = String::with_capacity(svg.len() + 40);
    let mut rest = svg;
    while let Some(pos) = rest.find(KEY) {
        let value_start = pos + KEY.len();
        out.push_str(&rest[..value_start]);
        let after = &rest[value_start..];
        // The value ends at the declaration's `;` (font names never contain one); `>` guards
        // against a malformed tail so the scan never runs past the element.
        let end = after.find([';', '>']).unwrap_or(after.len());
        out.push_str(&after[..end].replace('"', "'"));
        rest = &after[end..];
    }
    out.push_str(rest);
    // usvg only accepts elements in the SVG namespace.
    if !out.contains("xmlns=") {
        out = out.replacen("<svg", "<svg xmlns=\"http://www.w3.org/2000/svg\"", 1);
    }
    out
}

pub(crate) fn prepare(slides: &[serde_json::Value], assets: &Assets) -> PreparedAssets {
    let mut prepared = PreparedAssets {
        paths: HashMap::new(),
        files: HashMap::new(),
        missing: Vec::new(),
    };
    // Failures are cached per reference but reported for every slide using it.
    let mut failures: HashMap<String, String> = HashMap::new();
    for (index, slide) in slides.iter().enumerate() {
        let mut refs: Vec<String> = image_refs(slide).iter().map(|r| r.to_string()).collect();
        if is_chart_slide(slide) {
            refs.push(chart_asset_ref(index));
        }
        for reference in refs {
            if prepared.paths.contains_key(&reference) {
                continue;
            }
            let reason = match failures.get(&reference) {
                Some(reason) => reason.clone(),
                None => match assets.load(&reference).and_then(normalize_image) {
                    Ok(image) => {
                        let path = format!("/assets/{}.{}", prepared.files.len(), image.ext);
                        prepared.files.insert(path.clone(), image.bytes);
                        let info = ImageInfo {
                            path,
                            width: image.width,
                            height: image.height,
                            vector: image.ext == "svg",
                        };
                        prepared.paths.insert(reference.clone(), info);
                        continue;
                    }
                    Err(reason) => {
                        failures.insert(reference.clone(), reason.clone());
                        reason
                    }
                },
            };
            prepared.missing.push(MissingAsset {
                slide: index,
                reference,
                reason,
            });
        }
    }
    prepared
}

fn decode_data_uri(data: &str) -> Result<Vec<u8>, String> {
    let (meta, payload) = data.split_once(',').ok_or("data URI 缺少逗号")?;
    if meta.ends_with(";base64") {
        base64::engine::general_purpose::STANDARD
            .decode(payload.trim())
            .map_err(|err| format!("data URI base64 无效：{err}"))
    } else {
        Ok(payload.as_bytes().to_vec())
    }
}

#[derive(Debug, PartialEq)]
enum Format {
    Png,
    Jpg,
    Gif,
    Webp,
    Bmp,
    Svg,
}

fn sniff(bytes: &[u8]) -> Option<Format> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(Format::Png)
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(Format::Jpg)
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some(Format::Gif)
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some(Format::Webp)
    } else if bytes.starts_with(b"BM") {
        Some(Format::Bmp)
    } else {
        let head = String::from_utf8_lossy(&bytes[..bytes.len().min(1024)]).to_ascii_lowercase();
        head.contains("<svg").then_some(Format::Svg)
    }
}

struct NormalizedImage {
    bytes: Vec<u8>,
    ext: &'static str,
    width: f64,
    height: f64,
}

/// Validates an image and returns bytes Typst can load, with extension and size.
fn normalize_image(bytes: Vec<u8>) -> Result<NormalizedImage, String> {
    let decode =
        |bytes: &[u8]| image::load_from_memory(bytes).map_err(|err| format!("图片无法解码：{err}"));
    let raster = |bytes: Vec<u8>, ext| {
        let decoded = decode(&bytes)?;
        let (width, height) = (decoded.width() as f64, decoded.height() as f64);
        Ok(NormalizedImage {
            bytes,
            ext,
            width,
            height,
        })
    };
    match sniff(&bytes).ok_or("不支持的图片格式")? {
        Format::Png => raster(bytes, "png"),
        Format::Jpg => raster(bytes, "jpg"),
        Format::Gif => raster(bytes, "gif"),
        Format::Webp => raster(bytes, "webp"),
        Format::Bmp => {
            let decoded = decode(&bytes)?;
            let mut png = Vec::new();
            decoded
                .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
                .map_err(|err| format!("BMP 转 PNG 失败：{err}"))?;
            Ok(NormalizedImage {
                bytes: png,
                ext: "png",
                width: decoded.width() as f64,
                height: decoded.height() as f64,
            })
        }
        Format::Svg => {
            // ECharts SSR 输出的 style 属性里有裸双引号（非法 XML），先修正再交给 usvg 校验。
            let normalized = sanitize_svg_for_typst(&String::from_utf8_lossy(&bytes));
            let bytes = normalized.into_bytes();
            let tree = usvg::Tree::from_data(&bytes, &usvg::Options::default())
                .map_err(|err| format!("SVG 无法解析：{err}"))?;
            let size = tree.size();
            Ok(NormalizedImage {
                bytes,
                ext: "svg",
                width: size.width() as f64,
                height: size.height() as f64,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn png_bytes(w: u32, h: u32) -> Vec<u8> {
        let mut out = Vec::new();
        image::RgbImage::new(w, h)
            .write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)
            .unwrap();
        out
    }

    #[test]
    fn sniffs_supported_formats() {
        assert_eq!(sniff(&png_bytes(2, 2)), Some(Format::Png));
        assert_eq!(
            sniff(b"<?xml version=\"1.0\"?><svg xmlns=\"\"/>"),
            Some(Format::Svg)
        );
        assert_eq!(sniff(b"plain text"), None);
    }

    #[test]
    fn converts_bmp_to_png() {
        let mut bmp = Vec::new();
        image::RgbImage::new(3, 2)
            .write_to(&mut Cursor::new(&mut bmp), image::ImageFormat::Bmp)
            .unwrap();
        let image = normalize_image(bmp).unwrap();
        assert_eq!(image.ext, "png");
        assert_eq!((image.width, image.height), (3.0, 2.0));
        assert_eq!(sniff(&image.bytes), Some(Format::Png));
    }

    #[test]
    fn rejects_truncated_png() {
        let mut png = png_bytes(16, 16);
        png.truncate(40);
        assert!(normalize_image(png).is_err());
    }

    #[test]
    fn reports_every_slide_using_a_missing_reference_once() {
        let mut assets = Assets::new();
        assets.insert_bytes("media:1", png_bytes(4, 4));
        let slides = vec![
            json!({ "kind": "spotlight", "image": "media:1", "images": ["media:1", "media:9"] }),
            json!({ "kind": "spotlight", "images": ["media:9", "https://example.com/a.png"] }),
        ];
        let prepared = prepare(&slides, &assets);
        assert_eq!(prepared.paths.len(), 1, "media:1 is loaded once");
        let missing: Vec<_> = prepared
            .missing
            .iter()
            .map(|m| (m.slide, m.reference.as_str()))
            .collect();
        assert_eq!(
            missing,
            vec![
                (0, "media:9"),
                (1, "media:9"),
                (1, "https://example.com/a.png")
            ]
        );
    }

    #[test]
    fn images_on_kinds_that_do_not_show_them_are_ignored() {
        let slides = vec![json!({ "kind": "compare", "image": "media:5", "images": ["media:6"] })];
        let prepared = prepare(&slides, &Assets::new());
        assert!(prepared.paths.is_empty());
        assert!(prepared.missing.is_empty(), "{:?}", prepared.missing);
    }

    #[test]
    fn decodes_base64_data_uri() {
        let png = png_bytes(1, 1);
        let uri = format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(&png)
        );
        assert_eq!(Assets::new().load(&uri).unwrap(), png);
    }

    #[test]
    fn raw_echarts_ssr_svg_survives_sanitize_and_usvg() {
        // Captured verbatim from ECharts SSR output (compact path data, bare
        // quotes in font-family style) — the sanitizer must make it parse.
        let raw = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/assets/echarts-ssr-raw.svg"
        ))
        .unwrap();
        let image = normalize_image(raw).expect("raw ECharts SSR SVG must normalize");
        assert_eq!(image.ext, "svg");
        let text = String::from_utf8_lossy(&image.bytes);
        assert!(!text.contains("font-family:\""), "bare quotes must be gone");
        assert!(
            text.contains("font-family:'Microsoft YaHei', 'PingFang SC', sans-serif;"),
            "the fallback list must survive"
        );
        // `id="…"` also ends in `d="`: ids (clip paths, gradients) must stay untouched.
        assert!(
            text.contains(r#"id="zr0-c0""#),
            "clip-path id was rewritten"
        );
        assert!(text.contains(r#"id="zr0-g0""#), "gradient id was rewritten");
    }
}
