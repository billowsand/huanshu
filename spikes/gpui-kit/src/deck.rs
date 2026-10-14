//! Deck loading and the Typst render service the pages share.
//!
//! Everything here runs on GPUI's background executor: [`Renderer::with_system_fonts`]
//! scans the system fonts once per process (0.5 s or so) and one compile lays out the
//! whole deck, so neither may happen on the UI thread.

use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use serde_json::Value;
use slide_typst::{ExportOptions, Renderer};

/// Thumbnail widths are rounded up to this many device pixels, so a window drag does
/// not recompile on every pixel (mirrors `WIDTH_BUCKET` in `useTypstThumbnails.ts`).
pub const WIDTH_BUCKET: u32 = 64;

/// How the rasterized page crosses into the UI process.
///
/// `Png` is what the Tauri app does today (`render_png` → `<img src="data:…">`);
/// `Pixels` is `slide_typst::Renderer::render_pixels`, which skips both the PNG
/// encoder here and the PNG decoder on the other side.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Transfer {
    Png,
    Pixels,
}

impl Transfer {
    pub fn label(self) -> &'static str {
        match self {
            Transfer::Png => "render_png + decode",
            Transfer::Pixels => "render_pixels",
        }
    }
}

/// One rasterized page in GPUI's pixel order.
pub struct Frame {
    pub width: u32,
    pub height: u32,
    /// Top-to-bottom rows of BGRA8, straight (non-premultiplied) alpha.
    pub bgra: Vec<u8>,
}

/// One rasterized page plus the slide index it belongs to.
pub struct Raster {
    pub index: usize,
    pub frame: Frame,
}

/// Where the time of one batch went, so the two transfer modes can be compared.
#[derive(Clone, Copy, Default)]
pub struct Timing {
    /// Time inside `render_png` / `render_pixels`.
    pub render: Duration,
    /// Time spent turning the result into BGRA8: PNG decode for `Png`, a channel
    /// swap for `Pixels`.
    pub transfer: Duration,
}

impl Timing {
    pub fn total(&self) -> Duration {
        self.render + self.transfer
    }
}

pub struct Batch {
    pub pages: Vec<Raster>,
    pub timing: Timing,
    /// Width of the produced bitmaps, in device pixels (the bucket, not the request).
    pub width: u32,
}

/// The one renderer of the process; building it scans the system fonts.
pub fn renderer() -> &'static Renderer {
    static RENDERER: OnceLock<Renderer> = OnceLock::new();
    RENDERER.get_or_init(|| {
        let started = Instant::now();
        let renderer = Renderer::with_system_fonts();
        eprintln!(
            "[spike] system fonts scanned in {:.0} ms",
            started.elapsed().as_secs_f64() * 1000.0
        );
        renderer
    })
}

/// Rounds a requested width up to the next bucket, following `useTypstThumbnails`.
pub fn bucket_width(width_px: f32) -> u32 {
    let width = width_px.max(1.0).ceil() as u32;
    WIDTH_BUCKET.max(width.div_ceil(WIDTH_BUCKET) * WIDTH_BUCKET)
}

/// Compiles the deck once and rasterizes `only` (the whole deck when `None`).
pub fn rasterize(
    slides: &[Value],
    theme: &str,
    aspect_ratio: &str,
    width_px: u32,
    only: Option<&[usize]>,
    transfer: Transfer,
) -> Result<Batch, String> {
    let options = ExportOptions {
        theme: theme.to_string(),
        aspect_ratio: aspect_ratio.to_string(),
        ..Default::default()
    };
    let width = bucket_width(width_px as f32);
    let mut timing = Timing::default();

    let started = Instant::now();
    let mut pages = Vec::new();
    match transfer {
        Transfer::Png => {
            let rendered = renderer()
                .render_png(slides, &options, width, only)
                .map_err(|err| err.to_string())?;
            timing.render = started.elapsed();
            let transfer_started = Instant::now();
            for page in rendered.pages {
                // GPUI's own PNG path decodes to RGBA and then swaps to BGRA; the spike
                // does exactly that here so both modes end in the same pixel layout.
                let decoded = image::load_from_memory(&page.png)
                    .map_err(|err| format!("decode slide {}: {err}", page.index))?
                    .to_rgba8();
                let (width, height) = decoded.dimensions();
                let mut bgra = decoded.into_raw();
                for pixel in bgra.chunks_exact_mut(4) {
                    pixel.swap(0, 2);
                }
                pages.push(Raster {
                    index: page.index,
                    frame: Frame {
                        width,
                        height,
                        bgra,
                    },
                });
            }
            timing.transfer = transfer_started.elapsed();
        }
        Transfer::Pixels => {
            let rendered = renderer()
                .render_pixels(slides, &options, width, only)
                .map_err(|err| err.to_string())?;
            timing.render = started.elapsed();
            let transfer_started = Instant::now();
            for page in rendered.pages {
                let mut bgra = page.pixels;
                for pixel in bgra.chunks_exact_mut(4) {
                    pixel.swap(0, 2);
                }
                pages.push(Raster {
                    index: page.index,
                    frame: Frame {
                        width: page.width,
                        height: page.height,
                        bgra,
                    },
                });
            }
            timing.transfer = transfer_started.elapsed();
        }
    }
    pages.sort_by_key(|page| page.index);
    Ok(Batch {
        pages,
        timing,
        width,
    })
}

/// Reads a deck from disk: an array of `SlideBlueprint`s, as `render_fixture` takes.
pub fn load_deck(path: &PathBuf) -> Result<Vec<Value>, String> {
    let text = std::fs::read_to_string(path).map_err(|err| format!("{}: {err}", path.display()))?;
    let deck: Vec<Value> =
        serde_json::from_str(&text).map_err(|err| format!("{}: {err}", path.display()))?;
    if deck.is_empty() {
        return Err(format!("{}: the deck is empty", path.display()));
    }
    Ok(deck)
}

/// The deck the board opens with.
pub fn default_deck_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/all-layout-blueprints.json")
}

/// Median of a sample set, half-up on even counts.
pub fn median(mut values: Vec<f64>) -> f64 {
    if values.is_empty() {
        return f64::NAN;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let middle = values.len() / 2;
    if values.len() % 2 == 0 {
        (values[middle - 1] + values[middle]) / 2.0
    } else {
        values[middle]
    }
}
