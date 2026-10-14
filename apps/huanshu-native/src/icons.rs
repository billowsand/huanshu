//! 图标：原版用的是 Carbon（`i-carbon:*`，UnoCSS），这里沿用**同一套 Carbon 图标**。
//!
//! Carbon 的 SVG 由 `slide_typst::render_icon_svg` 现生成（幻灯片里的图标走的也是它），
//! 经 [`NativeAssets`] 以 `carbon/<name>.svg` 的路径交给 GPUI；其余路径（GPUI Kit 组件
//! 自带的 Lucide 图标）转给 `AllAssets`。页面里写 [`carbon`]`("i-carbon:home")` 或
//! `carbon("home")`，与 Vue 模板里的类名一一对应，方便对照搬运。

use std::borrow::Cow;

use gpui_kit::assets::{AllAssets, IconNamed};
use gpui_kit::component::Icon;
use gpui_kit::{AssetSource, SharedString};

/// 原版 `BrandMark.vue` 的 SVG（同一份路径与渐变）。`widgets::brand_mark` 用 `img("brand/mark.svg")` 显示。
static BRAND_MARK: &[u8] = include_bytes!("../assets/brand-mark.svg");

/// 应用的资源源：`carbon/<name>.svg` 现生成，其余交给 GPUI Kit 的完整图标包。
pub struct NativeAssets;

impl AssetSource for NativeAssets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        if let Some(name) = path
            .strip_prefix("carbon/")
            .and_then(|rest| rest.strip_suffix(".svg"))
        {
            // GPUI 把单色 SVG 当遮罩、按文字颜色着色，所以填什么颜色都一样。
            let (svg, _fallback) = slide_typst::render_icon_svg("carbon", name, "000000");
            return Ok(Some(Cow::Owned(svg.into_bytes())));
        }
        if path == "brand/mark.svg" {
            return Ok(Some(Cow::Borrowed(BRAND_MARK)));
        }
        AllAssets.load(path)
    }

    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        AllAssets.list(path)
    }
}

/// 一个 Carbon 图标名（不含 `i-carbon:` 前缀）。
#[derive(Debug, Clone)]
pub struct Carbon(pub SharedString);

impl IconNamed for Carbon {
    fn path(self) -> SharedString {
        format!("carbon/{}.svg", self.0).into()
    }
}

/// `carbon("i-carbon:home")` 或 `carbon("home")` → 可以直接 `.child(…)` 的图标。
/// 未知名字显示 Carbon 的兜底图标（与幻灯片一致），不会报错。
pub fn carbon(name: &str) -> Icon {
    let name = name
        .strip_prefix("i-carbon:")
        .or_else(|| name.strip_prefix("carbon:"))
        .unwrap_or(name);
    Icon::new(Carbon(name.to_string().into()))
}
