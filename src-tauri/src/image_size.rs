//! Dependency-free image header parsing.
//!
//! Imported media needs pixel dimensions so the generator can reason about aspect ratio
//! when it picks layouts (and, later, image slots). Only the file header is inspected, so
//! importing a large asset stays cheap and no image decoding crate is required.
//!
//! Supported containers: PNG, JPEG, GIF, BMP, WebP (VP8 / VP8L / VP8X) and SVG
//! (`width`/`height` attributes, falling back to `viewBox`).

use std::sync::OnceLock;

use regex::Regex;

/// Pixel dimensions of `bytes`, or `None` when the format is unsupported, malformed, or
/// truncated before the dimension fields are reached.
pub fn image_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return png_dimensions(bytes);
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return gif_dimensions(bytes);
    }
    if bytes.starts_with(&[0xFF, 0xD8]) {
        return jpeg_dimensions(bytes);
    }
    if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return webp_dimensions(&bytes[12..]);
    }
    if bytes.starts_with(b"BM") {
        return bmp_dimensions(bytes);
    }
    svg_dimensions(bytes)
}

/// Zero-sized images are never useful; treat them as unknown.
fn dims(width: u32, height: u32) -> Option<(u32, u32)> {
    (width > 0 && height > 0).then_some((width, height))
}

fn be_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_be_bytes(bytes.get(offset..offset + 4)?.try_into().ok()?))
}

fn be_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_be_bytes(bytes.get(offset..offset + 2)?.try_into().ok()?))
}

fn le_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.get(offset..offset + 2)?.try_into().ok()?))
}

fn le_u24(bytes: &[u8], offset: usize) -> Option<u32> {
    let raw = bytes.get(offset..offset + 3)?;
    Some(u32::from_le_bytes([raw[0], raw[1], raw[2], 0]))
}

/// PNG: 8-byte signature, then the IHDR chunk whose first two fields are width and height.
fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.get(12..16)? != b"IHDR".as_slice() {
        return None;
    }
    dims(be_u32(bytes, 16)?, be_u32(bytes, 20)?)
}

/// GIF: logical screen descriptor sits at a fixed offset after the version signature.
fn gif_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    dims(le_u16(bytes, 6)? as u32, le_u16(bytes, 8)? as u32)
}

/// JPEG: walk the marker segments up to the first Start-Of-Frame, which carries the size.
fn jpeg_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    let mut cursor = 2;
    while cursor + 3 < bytes.len() {
        if bytes[cursor] != 0xFF {
            cursor += 1;
            continue;
        }
        let marker = bytes[cursor + 1];
        // 0xFF fill bytes, plus markers that carry no length field.
        if marker == 0xFF {
            cursor += 1;
            continue;
        }
        if marker == 0x01 || (0xD0..=0xD8).contains(&marker) {
            cursor += 2;
            continue;
        }
        let length = be_u16(bytes, cursor + 2)? as usize;
        if length < 2 {
            return None;
        }
        let is_start_of_frame =
            (0xC0..=0xCF).contains(&marker) && !matches!(marker, 0xC4 | 0xC8 | 0xCC);
        if is_start_of_frame {
            // FF C0 | length(2) | precision(1) | height(2) | width(2)
            let height = be_u16(bytes, cursor + 5)? as u32;
            let width = be_u16(bytes, cursor + 7)? as u32;
            return dims(width, height);
        }
        cursor += 2 + length;
    }
    None
}

/// WebP: the three chunk flavours store the canvas size differently.
fn webp_dimensions(chunk: &[u8]) -> Option<(u32, u32)> {
    let fourcc = chunk.get(0..4)?;
    if fourcc == b"VP8X".as_slice() {
        // fourcc(4) | size(4) | flags(1) | reserved(3) | width-1(3) | height-1(3)
        return dims(le_u24(chunk, 12)? + 1, le_u24(chunk, 15)? + 1);
    }
    if fourcc == b"VP8 ".as_slice() {
        // fourcc(4) | size(4) | frame tag(3) | start code(3) | width(2) | height(2)
        if chunk.get(11..14)? != b"\x9d\x01\x2a".as_slice() {
            return None;
        }
        let width = le_u16(chunk, 14)? as u32 & 0x3FFF;
        let height = le_u16(chunk, 16)? as u32 & 0x3FFF;
        return dims(width, height);
    }
    if fourcc == b"VP8L".as_slice() {
        // fourcc(4) | size(4) | signature(1) | width-1:14 | height-1:14
        if chunk.get(8..9)? != b"\x2f".as_slice() {
            return None;
        }
        let bits = u32::from_le_bytes(chunk.get(9..13)?.try_into().ok()?);
        return dims((bits & 0x3FFF) + 1, ((bits >> 14) & 0x3FFF) + 1);
    }
    None
}

/// BMP: DIB header starts at offset 14, width and height are signed 32-bit little endian.
fn bmp_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    let width = i32::from_le_bytes(bytes.get(18..22)?.try_into().ok()?);
    let height = i32::from_le_bytes(bytes.get(22..26)?.try_into().ok()?);
    dims(width.unsigned_abs(), height.unsigned_abs())
}

/// SVG is text: only the opening tag matters, so a bounded prefix is enough.
fn svg_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    let head = &bytes[..bytes.len().min(8192)];
    let text = std::str::from_utf8(head).ok()?;
    if !text.to_ascii_lowercase().contains("<svg") {
        return None;
    }
    if let (Some(width), Some(height)) = (
        svg_length_attribute(text, &SVG_WIDTH, "width"),
        svg_length_attribute(text, &SVG_HEIGHT, "height"),
    ) {
        if let Some(size) = dims(width, height) {
            return Some(size);
        }
    }
    // Percentages and unit-less sizing fall back to the viewBox.
    let captured = SVG_VIEWBOX
        .get_or_init(|| Regex::new(r#"(?i)viewBox\s*=\s*["']([^"']+)["']"#).unwrap())
        .captures(text)?;
    let numbers: Vec<f64> = captured[1]
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter_map(|part| part.trim().parse::<f64>().ok())
        .collect();
    if numbers.len() != 4 {
        return None;
    }
    dims(
        numbers[2].round().max(0.0) as u32,
        numbers[3].round().max(0.0) as u32,
    )
}

static SVG_WIDTH: OnceLock<Regex> = OnceLock::new();
static SVG_HEIGHT: OnceLock<Regex> = OnceLock::new();
static SVG_VIEWBOX: OnceLock<Regex> = OnceLock::new();

/// A positive pixel length from a `width`/`height` attribute; `None` for units that
/// cannot be resolved without a layout (percentages, `em`, `pt`).
fn svg_length_attribute(text: &str, cache: &OnceLock<Regex>, name: &str) -> Option<u32> {
    let pattern = cache.get_or_init(|| {
        Regex::new(&format!(r#"(?i)(?:^|[\s;]){name}\s*=\s*["']([^"']*)["']"#)).unwrap()
    });
    let raw = pattern.captures(text)?.get(1)?.as_str().trim().to_ascii_lowercase();
    let numeric = raw.strip_suffix("px").unwrap_or(&raw).trim();
    if numeric.contains('%') {
        return None;
    }
    let value: f64 = numeric.parse().ok()?;
    (value > 0.0).then(|| value.round() as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn webp(fourcc: &[u8], payload: &[u8]) -> Vec<u8> {
        let mut bytes = b"RIFF".to_vec();
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(b"WEBP");
        bytes.extend_from_slice(fourcc);
        bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        bytes.extend_from_slice(payload);
        bytes
    }

    #[test]
    fn reads_png_dimensions() {
        let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
        bytes.extend_from_slice(&13u32.to_be_bytes());
        bytes.extend_from_slice(b"IHDR");
        bytes.extend_from_slice(&1920u32.to_be_bytes());
        bytes.extend_from_slice(&1080u32.to_be_bytes());
        bytes.extend_from_slice(&[8, 2, 0, 0, 0]);
        assert_eq!(image_dimensions(&bytes), Some((1920, 1080)));
    }

    #[test]
    fn reads_gif_dimensions() {
        let mut bytes = b"GIF89a".to_vec();
        bytes.extend_from_slice(&320u16.to_le_bytes());
        bytes.extend_from_slice(&200u16.to_le_bytes());
        bytes.extend_from_slice(&[0, 0, 0]);
        assert_eq!(image_dimensions(&bytes), Some((320, 200)));
    }

    #[test]
    fn reads_jpeg_dimensions_past_metadata_segments() {
        let mut bytes = vec![0xFF, 0xD8];
        // APP0/JFIF segment that must be skipped on the way to the SOF marker.
        bytes.extend_from_slice(&[0xFF, 0xE0, 0x00, 0x10]);
        bytes.extend_from_slice(&[0u8; 14]);
        // SOF0 | length | precision | height | width
        bytes.extend_from_slice(&[0xFF, 0xC0, 0x00, 0x11, 0x08]);
        bytes.extend_from_slice(&720u16.to_be_bytes());
        bytes.extend_from_slice(&1280u16.to_be_bytes());
        bytes.push(0x03);
        assert_eq!(image_dimensions(&bytes), Some((1280, 720)));
    }

    #[test]
    fn reads_bmp_dimensions() {
        let mut bytes = b"BM".to_vec();
        bytes.extend_from_slice(&[0u8; 16]);
        bytes.extend_from_slice(&1024i32.to_le_bytes());
        bytes.extend_from_slice(&512i32.to_le_bytes());
        assert_eq!(image_dimensions(&bytes), Some((1024, 512)));
    }

    #[test]
    fn reads_webp_vp8x_canvas_size() {
        let mut payload = vec![0u8; 4]; // flags + reserved
        payload.extend_from_slice(&[0x7F, 0x02, 0x00]); // 640 - 1
        payload.extend_from_slice(&[0x67, 0x01, 0x00]); // 360 - 1
        assert_eq!(image_dimensions(&webp(b"VP8X", &payload)), Some((640, 360)));
    }

    #[test]
    fn reads_webp_lossy_and_lossless_sizes() {
        let mut lossy = vec![0u8, 0, 0]; // frame tag
        lossy.extend_from_slice(&[0x9D, 0x01, 0x2A]); // start code
        lossy.extend_from_slice(&1024u16.to_le_bytes());
        lossy.extend_from_slice(&768u16.to_le_bytes());
        assert_eq!(image_dimensions(&webp(b"VP8 ", &lossy)), Some((1024, 768)));

        let bits: u32 = 799 | (599 << 14);
        let mut lossless = vec![0x2F];
        lossless.extend_from_slice(&bits.to_le_bytes());
        assert_eq!(
            image_dimensions(&webp(b"VP8L", &lossless)),
            Some((800, 600))
        );
    }

    #[test]
    fn reads_svg_dimensions_from_attributes_or_viewbox() {
        let attributes =
            br#"<svg xmlns="http://www.w3.org/2000/svg" width="640px" height="360"></svg>"#;
        assert_eq!(image_dimensions(attributes), Some((640, 360)));

        let view_box = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 320 240"></svg>"#;
        assert_eq!(image_dimensions(view_box), Some((320, 240)));

        let percent = br#"<svg width="100%" height="100%" viewBox="0 0 200 100"></svg>"#;
        assert_eq!(image_dimensions(percent), Some((200, 100)));
    }

    #[test]
    fn unknown_or_truncated_input_has_no_dimensions() {
        assert_eq!(image_dimensions(b"not an image at all"), None);
        assert_eq!(image_dimensions(b"\x89PNG\r\n\x1a\n"), None);
        assert_eq!(image_dimensions(&[]), None);
        assert_eq!(image_dimensions(b"RIFF\x00\x00\x00\x00WEBP"), None);
        // A zero-width IHDR is not a usable size.
        let mut zero = b"\x89PNG\r\n\x1a\n".to_vec();
        zero.extend_from_slice(&13u32.to_be_bytes());
        zero.extend_from_slice(b"IHDR");
        zero.extend_from_slice(&0u32.to_be_bytes());
        zero.extend_from_slice(&1080u32.to_be_bytes());
        zero.extend_from_slice(&[8, 2, 0, 0, 0]);
        assert_eq!(image_dimensions(&zero), None);
    }
}
