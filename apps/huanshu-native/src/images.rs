//! 位图的交接：Typst 给的像素 → GPUI 能画的 `RenderImage`，以及释放。
//!
//! GPUI 以 BGRA8 上传图片（直通道 alpha）：Windows 图集是 `DXGI_FORMAT_B8G8R8A8_UNORM`，
//! macOS 的 Metal 图集是 `BGRA8Unorm`，内置解码器最后一步都是 `pixel.swap(0, 2)`。core 的 `render_pixels` 给的是 RGBA8，
//! 所以这里换一下通道——这也是唯一一处需要知道通道顺序的地方。

use std::sync::Arc;

use gpui_kit::{App, RenderImage, Window};
use huanshu_core::services::export::PixelPage;
use image::{Frame as ImageFrame, ImageBuffer, Rgba};

/// 把一页像素包成 GPUI 图片。
pub fn to_render_image(page: PixelPage) -> Arc<RenderImage> {
    from_rgba(page.width, page.height, page.pixels)
}

/// RGBA8 原始像素（宽 × 高 × 4 字节）→ GPUI 图片。
///
/// 公式预览图（PNG 解码后也是 RGBA8）走这里：通道顺序的秘密只留在本模块。
pub fn from_rgba(width: u32, height: u32, pixels: Vec<u8>) -> Arc<RenderImage> {
    let bgra = to_bgra(&pixels);
    let buffer = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(width, height, bgra)
        .expect("a raster page always holds width × height × 4 bytes");
    Arc::new(RenderImage::new([ImageFrame::new(buffer)]))
}

/// RGBA8 → BGRA8，只换红蓝两个通道。
fn to_bgra(rgba: &[u8]) -> Vec<u8> {
    let mut out = rgba.to_vec();
    out.chunks_exact_mut(4).for_each(|pixel| {
        pixel.swap(0, 2);
    });
    out
}

/// **每张不再显示的图都要走这里。** GPUI 不会自动回收图集 tile，宿主必须自己释放，
/// 否则每次换图都会在图集里留下一个 tile（spike 报告第 4 节第 2 条：约 6 MiB/s）。
pub fn drop_all(images: Vec<Arc<RenderImage>>, window: Option<&mut Window>, cx: &mut App) {
    match window {
        Some(window) => {
            for image in images {
                cx.drop_image(image, Some(window));
            }
        }
        None => {
            for image in images {
                cx.drop_image(image, None);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgba_becomes_bgra() {
        let out = to_bgra(&[1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(out, vec![3, 2, 1, 4, 7, 6, 5, 8]);
        assert_eq!(to_bgra(&[]), Vec::<u8>::new());
    }

    /// 长度不是 4 的倍数时不 panic：尾部被整块忽略。
    #[test]
    fn a_trailing_partial_pixel_is_ignored() {
        let out = to_bgra(&[1, 2, 3, 4, 9]);
        assert_eq!(out, vec![3, 2, 1, 4, 9]);
    }
}
