//! Turns a rasterized page into something GPUI can draw.
//!
//! GPUI uploads images as BGRA8 with straight alpha (`RenderImage` is documented as
//! "in BGRA format", every built-in decoder ends in `pixel.swap(0, 2)`, and the
//! Windows atlas is `DXGI_FORMAT_B8G8R8A8_UNORM` with `SRC_ALPHA`/`INV_SRC_ALPHA`
//! blending). `deck::Frame` already carries BGRA8, so this is just the buffer hand-off:
//! raw pixels reach the GPU without a PNG encoder or decoder in between.

use std::sync::Arc;

use gpui_kit::RenderImage;
use image::{Frame as ImageFrame, ImageBuffer, Rgba};

use crate::deck::Frame;

/// Wraps a rasterized page as a GPUI image.
pub fn to_render_image(frame: Frame) -> Arc<RenderImage> {
    let buffer = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(frame.width, frame.height, frame.bgra)
        .expect("a raster frame always holds width × height × 4 bytes");
    Arc::new(RenderImage::new([ImageFrame::new(buffer)]))
}
