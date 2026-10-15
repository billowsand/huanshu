//! 第一步 Markdown 预览里的数学公式：行内 `$…$` 与块级 `$$…$$` 渲染成图。
//!
//! 出图走 `slide-typst` 的公式管线（`Renderer::render_formula`：mitex 转 LaTeX → Typst 排版 →
//! 透明底 PNG），**不是 KaTeX**。这是有意的：预览里的公式与幻灯片、PDF 里的公式是同一套排版
//! （`CLAUDE.md`「原生界面」）。
//!
//! 与 `gpui-base` 的插件契约分工（`text/markdown_ext.rs`）：
//! - `parse` 可能在后台线程跑，只判定「这个节点算不算公式」并把 latex 放进 `MarkdownNode` 的
//!   类型化 data 里；不碰窗口、不出图。（`flatten_unclaimed_math` 出于回落判断会再问一次，
//!   所以判定必须是纯函数、可重复。）
//! - `render_inline` / `render` 每帧都会调，**只读缓存**：有图就画图，没图先显示公式原文并
//!   请后台出图，出完 `cx.notify()` 让预览重画一次。
//!
//! 美元符号的定界规则来自 core 的 `huanshu_core::markdown_tools`（与生成侧、PDF 侧同一套）：
//! 「价格 $5 和 $10」会被 markdown-rs 解析成 InlineMath，插件必须拒认，框架随即把那段按普通
//! Markdown 重新解析、美元文本原样保留。

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use gpui_kit::component::text::markdown_ast as mdast;
use gpui_kit::component::text::{
    InlineElement, InlineRenderContext, MarkdownExtensions, MarkdownNode, MarkdownParseContext,
    MarkdownPlugin,
};
use gpui_kit::{
    div, img, px, AnyElement, App, AppContext as _, Context, Entity, Hsla, IntoElement,
    ParentElement as _, RenderImage, Styled as _, WeakEntity, Window,
};
use huanshu_core::markdown_tools::{
    display_math_content_ok, inline_math_close_ok, inline_math_content_ok,
};
use slide_typst::FORMULA_PIXEL_PER_PT;

use crate::images;
use crate::theme::StudioExt as _;

/// 行内插件的名字。**恒定**：框架的 `has_same_parser_configuration` 按插件数量与名字比对
/// parser 形状，形状稳定就不重新解析文档——每帧重建插件实例是允许的，改名不行。
const INLINE_NAME: &str = "formula-inline";

/// 块级插件的名字（`is_block()` 为真）。同上，恒定。
const BLOCK_NAME: &str = "formula-block";

/// 缓存条数上限。
///
/// 一份文稿里的公式是几十条的量级，512 条装得下好几页 × 几个字号档（主题色变了也是新键）；
/// 每条只是一张很小的位图（16pt 公式约 90×70 设备像素）。超上限按 FIFO 淘汰最早的：
/// 公式随时能重新出图（Typst 编译 + 光栅化，几十毫秒），淘汰的代价只是那一屏重画一次。
const MAX_ENTRIES: usize = 512;

/// 出图倍率（设备像素 / 点），来自 `slide-typst`；摆到界面里要除以它换成逻辑像素。
const PIXELS_PER_PT: f64 = FORMULA_PIXEL_PER_PT;

/// 逻辑像素 → 点（96dpi 界面 / 72pt 排版）。
const PT_PER_PX: f64 = 0.75;

/// 行内公式的基线启发：图高 × 0.8 处当作基线（`InlineElement::with_baseline` 从图顶量起）。
///
/// `render_formula` 出的图紧贴墨迹（上下各留 1pt），没有现成的基线信息。0.8 是对 `x^2`、
/// 分式、求和上下限这些常见形状的折中；真机上看行内公式的垂直位置，偏了就调这里。
const BASELINE_RATIO: f32 = 0.8;

/// 块级公式图上下留的空隙（原版 KaTeX 的 display 公式也有自己的上下间距）。
const BLOCK_GAP: f32 = 6.0;

/// `parse` 交给 `render` 的东西：latex 与行内/块级，都在后台算好、只读。
#[derive(Debug, Clone)]
pub struct FormulaData {
    /// 公式内容，不含定界符（块级已 trim）。
    pub latex: String,
    /// 块级（`$$…$$`）为真：Typst 按 display 排版（分式更大、上下限在算符上下）。
    pub display: bool,
    /// 节点源码（含定界符）：未出图 / 坏公式时原样显示。
    pub source: String,
}

/// 缓存键：公式内容 + 行内/块级 + 正文色 + 字号档。
///
/// 每个分量都必须进键：换主题后正文色变了就重新出图（旧图靠上限淘汰），
/// 界面字号比例变了也就是另一个档。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FormulaKey {
    latex: String,
    display: bool,
    /// 主题正文色的 RGB（`cx.studio().colors.text`）。
    color: [u8; 3],
    /// 字号档位 = 0.5pt 的个数（[`size_half_pt`]）。
    size_half_pt: u32,
}

impl FormulaKey {
    pub fn new(latex: &str, display: bool, color: [u8; 3], size_pt: f64) -> Self {
        Self {
            latex: latex.to_string(),
            display,
            color,
            size_half_pt: size_half_pt(size_pt),
        }
    }

    /// 出图用的点大小（档位中心）。
    fn size_pt(&self) -> f64 {
        f64::from(self.size_half_pt) / 2.0
    }
}

/// 字号取整到 0.5pt 档。
///
/// `InlineRenderContext::font_size()` 是浮点像素，界面缩放 / 字号比例一变就是另一个值；
/// 不取整的话每 0.1px 都算一个新键，缓存会被同一张图的不同尺寸塞满。
fn size_half_pt(size_pt: f64) -> u32 {
    if !size_pt.is_finite() {
        return 0;
    }
    (size_pt.max(0.0) * 2.0).round() as u32
}

/// 一张出好的公式图：位图 + 逻辑尺寸（设备像素 ÷ [`PIXELS_PER_PT`]）。
#[derive(Clone)]
pub struct FormulaImage {
    pub image: Arc<RenderImage>,
    pub width: f32,
    pub height: f32,
}

/// 一条缓存：出好的图，或「这个公式渲染不了」。
#[derive(Clone)]
pub enum FormulaEntry {
    Ready(FormulaImage),
    /// 坏公式（mitex 转不了或 Typst 拒了）。**失败也永久缓存**：每帧重试会把 UI 线程
    /// 和 Typst 都拖住；公式改了就是另一个键，不会挡住修正后的文字。
    Failed,
}

/// 出图失败的原因（只用来打一行日志，界面按「显示原文 + 错误色」处理）。
pub struct FormulaFailure(pub String);

/// 公式图缓存。`Step1` 持有这个实体并 observe 它：出图落地后 `cx.notify()` → 预览重画。
#[derive(Default)]
pub struct FormulaCache {
    entries: HashMap<FormulaKey, FormulaEntry>,
    /// 插入顺序（FIFO）。同一个键重复插入不重复入队。
    order: VecDeque<FormulaKey>,
    /// 在途请求：同一个键出图期间不重复排队。
    in_flight: HashSet<FormulaKey>,
    /// 被淘汰、等着真正释放的图。
    /// **必须由有 window 的调用方走 `images::drop_all`**：GPUI 不会回收图集 tile。
    released: Vec<Arc<RenderImage>>,
}

impl FormulaCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn lookup(&self, key: &FormulaKey) -> Option<&FormulaEntry> {
        self.entries.get(key)
    }

    pub fn is_in_flight(&self, key: &FormulaKey) -> bool {
        self.in_flight.contains(key)
    }

    /// 这个键要不要请后台出图：没图、没失败标记、也不在途。
    pub fn needs_request(&self, key: &FormulaKey) -> bool {
        !self.entries.contains_key(key) && !self.is_in_flight(key)
    }

    /// 标在途。出图路径与单测都从这里进（`finish` 是唯一出口）。
    pub fn start(&mut self, key: &FormulaKey) {
        self.in_flight.insert(key.clone());
    }

    /// 出图回来的**唯一**落地点：Ok 存图、Err 记失败标记，两条路都清在途。
    ///
    /// 第三个出口是「实体已经销毁」：那种情况连缓存一起没了，`this.update` 返回 Err，
    /// 由调用方静默忽略（视图生命周期），不需要清理。
    pub fn finish(&mut self, key: FormulaKey, result: Result<FormulaImage, FormulaFailure>) {
        self.in_flight.remove(&key);
        match result {
            Ok(image) => self.insert(key, FormulaEntry::Ready(image)),
            Err(failure) => {
                eprintln!("[native] 公式渲染失败：{} —— {}", key.latex, failure.0);
                self.insert(key, FormulaEntry::Failed);
            }
        }
    }

    /// 请后台出图（公式重活 = Typst 编译 + 光栅化，绝不能在 UI 线程上做）。
    ///
    /// 每帧调都安全：有图 / 在途 / 失败过都直接返回（`needs_request`）。
    pub fn request(&mut self, key: FormulaKey, cx: &mut Context<Self>) {
        if !self.needs_request(&key) {
            return;
        }
        self.start(&key);
        let requested = key.clone();
        cx.spawn(async move |cache: WeakEntity<Self>, cx| {
            let result = cx
                .background_spawn(async move { render_formula_image(&requested) })
                .await;
            // 页面 / 实体已经没了时 `update` 返回 Err，静默忽略（视图生命周期）。
            let _ = cache.update(cx, move |cache, cx| {
                cache.finish(key, result);
                cx.notify();
            });
        })
        .detach();
    }

    fn insert(&mut self, key: FormulaKey, entry: FormulaEntry) {
        if self.entries.insert(key.clone(), entry).is_none() {
            self.order.push_back(key);
        }
        self.evict();
    }

    /// FIFO 淘汰到上限内。淘汰下来的图进 `released`，等 [`Self::take_released`] 交还 GPUI。
    fn evict(&mut self) {
        while self.entries.len() > MAX_ENTRIES {
            let Some(key) = self.order.pop_front() else {
                break;
            };
            let Some(entry) = self.entries.remove(&key) else {
                continue;
            };
            if let FormulaEntry::Ready(image) = entry {
                self.released.push(image.image);
            }
        }
    }

    /// 取出被淘汰的图。调用方（`Step1::render`，手上有 window）必须 `images::drop_all`。
    pub fn take_released(&mut self) -> Vec<Arc<RenderImage>> {
        std::mem::take(&mut self.released)
    }

    /// 页面离开时交还**全部**位图（README 规矩 5，同 `SlideImages::release`）。
    ///
    /// 公式随时能重新出图，清空没有代价；留着却有代价——GPUI 不会回收图集 tile。
    pub fn drain(&mut self) -> Vec<Arc<RenderImage>> {
        let mut images = std::mem::take(&mut self.released);
        images.extend(self.entries.drain().filter_map(|(_, entry)| match entry {
            FormulaEntry::Ready(image) => Some(image.image),
            FormulaEntry::Failed => None,
        }));
        self.order.clear();
        self.in_flight.clear();
        images
    }

    /// 缓存条数（单测用）。
    #[cfg(test)]
    fn len(&self) -> usize {
        self.entries.len()
    }

    /// 淘汰队列长度（单测用：同一个键重复插入不该入队两次）。
    #[cfg(test)]
    fn order_len(&self) -> usize {
        self.order.len()
    }
}

/// 预览卡片用的 Markdown 扩展：一个行内插件 + 一个块级插件。
///
/// 每帧重建没关系（插件实例只是持有缓存实体的句柄，状态都在实体里）：框架用
/// `has_same_parser_configuration` 比对 parser 形状，形状稳定就不会重新解析文档。
pub fn extensions(cache: &Entity<FormulaCache>) -> MarkdownExtensions {
    MarkdownExtensions::default()
        .plugin(InlineFormulaPlugin {
            cache: cache.clone(),
        })
        .plugin(BlockFormulaPlugin {
            cache: cache.clone(),
        })
}

/// 一个公式节点的定界判定结果。
#[derive(Debug, PartialEq, Eq)]
pub enum Claim {
    /// 认领：`latex` 已去掉定界符（块级已 trim）。
    Yes { latex: String, display: bool },
    /// 不算公式：交回框架按普通 Markdown 处理。
    No,
}

/// 行内 `$…$` 的判定。
///
/// `source` 是节点源码（含定界符），`value` 是 mdast 给的公式内容，`next` 是闭 `$` 后的
/// 下一个字符（`None` = 文档尾）。规则在 core 的 `markdown_tools`：
/// 内容为空 / 首尾有空白不算，闭 `$` 后紧跟 ASCII 数字不算（「价格 $5 和 $10」不被吞）。
///
/// 单行的 `$$E = mc^2$$` 会被 markdown-rs 解析成 **InlineMath**，但节点源码以 `$$` 开头，
/// 按块级排版（与生成侧 `split_preview_pages`、PDF 侧 `math.rs` 对 `$$` 的处理一致）。
pub fn claim_inline(source: &str, value: &str, next: Option<char>) -> Claim {
    if !inline_math_content_ok(value) || !inline_math_close_ok(next) {
        return Claim::No;
    }
    Claim::Yes {
        latex: value.to_string(),
        display: source.starts_with("$$"),
    }
}

/// 块级 `$$…$$` 的判定（跨行时 markdown-rs 给的是 `Node::Math`，value 不含定界符）。
pub fn claim_block(value: &str) -> Claim {
    if !display_math_content_ok(value) {
        return Claim::No;
    }
    Claim::Yes {
        latex: value.trim().to_string(),
        display: true,
    }
}

/// 造一个插件节点。`text` 设成公式原文：没出图时显示的就是它，辅助功能读的也是它；
/// 长度与源码一致，行内选择映射（框架按 `text` 的字节数推进）才对得上。
fn formula_node(name: &str, latex: String, display: bool, source: &str) -> MarkdownNode {
    MarkdownNode::new(
        name,
        FormulaData {
            latex,
            display,
            source: source.to_string(),
        },
    )
    .text(source.to_string())
}

fn parse_inline_formula(node: &mdast::Node, cx: &MarkdownParseContext<'_>) -> Option<MarkdownNode> {
    let mdast::Node::InlineMath(math) = node else {
        return None;
    };
    let source = cx.node_source(node).unwrap_or_default();
    let next = node
        .position()
        .and_then(|position| cx.source().get(position.end.offset..))
        .and_then(|rest| rest.chars().next());
    match claim_inline(source, &math.value, next) {
        Claim::Yes { latex, display } => Some(formula_node(INLINE_NAME, latex, display, source)),
        Claim::No => None,
    }
}

/// 行内插件：`$…$`，以及被 markdown-rs 当成行内节点的单行 `$$…$$`。
struct InlineFormulaPlugin {
    cache: Entity<FormulaCache>,
}

impl MarkdownPlugin for InlineFormulaPlugin {
    fn name(&self) -> &str {
        INLINE_NAME
    }

    fn parse(&self, node: &mdast::Node, cx: &MarkdownParseContext<'_>) -> Option<MarkdownNode> {
        parse_inline_formula(node, cx)
    }

    fn render_inline(
        &self,
        node: &MarkdownNode,
        context: &InlineRenderContext,
        _window: &mut Window,
        cx: &mut App,
    ) -> Option<InlineElement> {
        let data = node.data::<FormulaData>()?;
        let size_pt = context.font_size().as_f32() as f64 * PT_PER_PX;
        let key = FormulaKey::new(&data.latex, data.display, text_rgb(cx), size_pt);
        // 先读出来（克隆一步，`Ready` 只是多一个 `Arc`），再决定要不要请后台出图：
        // 借用与 `update` 不能同时拿着。
        let entry = self.cache.read(cx).lookup(&key).cloned();
        match entry {
            Some(FormulaEntry::Ready(image)) => Some(inline_image(&image)),
            Some(FormulaEntry::Failed) => {
                Some(inline_text(&data.source, cx.studio().colors.error, context))
            }
            None => {
                self.cache.update(cx, |cache, cx| cache.request(key, cx));
                Some(inline_text(&data.source, cx.studio().colors.text, context))
            }
        }
    }
}

/// 块级插件：跨行的 `$$…$$`。
struct BlockFormulaPlugin {
    cache: Entity<FormulaCache>,
}

impl MarkdownPlugin for BlockFormulaPlugin {
    fn is_block(&self) -> bool {
        true
    }

    fn name(&self) -> &str {
        BLOCK_NAME
    }

    fn parse(&self, node: &mdast::Node, cx: &MarkdownParseContext<'_>) -> Option<MarkdownNode> {
        let mdast::Node::Math(math) = node else {
            return None;
        };
        let source = cx.node_source(node).unwrap_or(&math.value).to_string();
        match claim_block(&math.value) {
            Claim::Yes { latex, display } => {
                Some(formula_node(BLOCK_NAME, latex, display, &source))
            }
            Claim::No => None,
        }
    }

    fn render(&self, node: &MarkdownNode, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let Some(data) = node.data::<FormulaData>() else {
            return div().into_any_element();
        };
        let key = FormulaKey::new(
            &data.latex,
            data.display,
            text_rgb(cx),
            body_size_pt(window),
        );
        let entry = self.cache.read(cx).lookup(&key).cloned();
        match entry {
            Some(FormulaEntry::Ready(image)) => {
                // 块级插件拿不到容器宽度，用 `w_full` + 居中；公式是有界宽度的单行对象，
                // 不会溢出预览卡片（超出时由 TextView 自己裁）。
                div()
                    .w_full()
                    .flex()
                    .justify_center()
                    .py(px(BLOCK_GAP))
                    .child(
                        img(image.image.clone())
                            .w(px(image.width))
                            .h(px(image.height)),
                    )
                    .into_any_element()
            }
            Some(FormulaEntry::Failed) => {
                block_text(&data.source, cx.studio().colors.error, window)
            }
            None => {
                self.cache.update(cx, |cache, cx| cache.request(key, cx));
                block_text(&data.source, cx.studio().colors.text, window)
            }
        }
    }
}

/// 公式图入行：按逻辑像素摆，基线抬到图高 × [`BASELINE_RATIO`]（默认是整块底面贴文字下降线）。
fn inline_image(image: &FormulaImage) -> InlineElement {
    let (width, height) = (image.width, image.height);
    InlineElement::new(img(image.image.clone()).w(px(width)).h(px(height)))
        .with_baseline(px(height * BASELINE_RATIO))
}

/// 未出图 / 坏公式时显示的公式原文。
///
/// 行内对象不与周围的文字一起排版，字号、字族都要自己带上（`InlineRenderContext` 给的就是
/// 那一刻生效的样式）。
fn inline_text(text: &str, color: Hsla, context: &InlineRenderContext) -> InlineElement {
    let style = context.text_style();
    InlineElement::new(
        div()
            .font_family(style.font_family.clone())
            .text_size(px(context.font_size().as_f32()))
            .text_color(color)
            .child(text.to_string()),
    )
}

/// 块级公式没出图 / 坏公式时的那段原文（居中）。
fn block_text(text: &str, color: Hsla, window: &Window) -> AnyElement {
    let style = window.text_style();
    div()
        .w_full()
        .flex()
        .justify_center()
        .py(px(BLOCK_GAP))
        .child(
            div()
                .font_family(style.font_family.clone())
                .text_size(style.font_size.to_pixels(window.rem_size()))
                .text_color(color)
                .child(text.to_string()),
        )
        .into_any_element()
}

/// 块级公式的字号：与正文同号。
///
/// 块级渲染只拿得到 window，这跟 `TextViewState::render` 算正文字号（`body_font_size`）用的是
/// 同一个来源：窗口当前文本样式的字号按 rem 换算。
fn body_size_pt(window: &Window) -> f64 {
    window
        .text_style()
        .font_size
        .to_pixels(window.rem_size())
        .as_f32() as f64
        * PT_PER_PX
}

/// 主题的正文色，取 RGB 三个字节进缓存键（`Hsla` → `Rgba` 是 0–1 的浮点）。
fn text_rgb(cx: &App) -> [u8; 3] {
    let rgba: gpui_kit::Rgba = cx.studio().colors.text.into();
    [channel(rgba.r), channel(rgba.g), channel(rgba.b)]
}

fn channel(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// 后台出图：mitex → Typst → PNG → GPUI 位图。`request` 与单测走的是同一个函数。
fn render_formula_image(key: &FormulaKey) -> Result<FormulaImage, FormulaFailure> {
    huanshu_core::services::export::renderer()
        .render_formula(&key.latex, key.display, key.color, key.size_pt())
        .map_err(|error| FormulaFailure(error.to_string()))
        .and_then(|formula| decode(formula).map_err(FormulaFailure))
}

/// PNG（透明底）→ GPUI 位图，并按出图倍率折出逻辑尺寸。
///
/// 解码在后台线程上做（跟出图同一个闭包），UI 线程只拿到包好的位图。
fn decode(formula: slide_typst::RenderedFormula) -> Result<FormulaImage, String> {
    let decoded = image::load_from_memory(&formula.png).map_err(|error| error.to_string())?;
    let rgba = decoded.to_rgba8();
    let (width, height) = (rgba.width(), rgba.height());
    Ok(FormulaImage {
        image: images::from_rgba(width, height, rgba.into_raw()),
        width: width as f32 / PIXELS_PER_PT as f32,
        height: height as f32 / PIXELS_PER_PT as f32,
    })
}

#[cfg(test)]
mod tests {
    //! 定界判定、缓存键、缓存上限与在途复位。
    //!
    //! UI 集成（TextView 里真的画出来、基线位置、主题切换后重出图）没测：无头环境没有
    //! window，也没有图集。那部分由真机验证覆盖。

    use super::*;

    const INK: [u8; 3] = [0x1e, 0x1e, 0x1e];
    const FILL: [u8; 3] = [0xe0, 0xe2, 0xec];

    fn image() -> FormulaImage {
        FormulaImage {
            image: images::from_rgba(1, 1, vec![0, 0, 0, 0]),
            width: 1.0,
            height: 1.0,
        }
    }

    fn key(latex: &str, display: bool, color: [u8; 3], size_pt: f64) -> FormulaKey {
        FormulaKey::new(latex, display, color, size_pt)
    }

    /// 一条完整的出图路径：标在途 → 落地（`Ok` 或 `Err`），在途都要清掉。
    fn round_trip(cache: &mut FormulaCache, key: FormulaKey, result: Result<FormulaImage, String>) {
        cache.start(&key);
        assert!(cache.is_in_flight(&key));
        cache.finish(key.clone(), result.map_err(FormulaFailure));
        assert!(!cache.is_in_flight(&key));
    }

    // ── 定界判定 ──

    #[test]
    fn inline_formula_is_claimed() {
        assert_eq!(
            claim_inline("$v = \\frac{s}{t}$", "v = \\frac{s}{t}", Some(' ')),
            Claim::Yes {
                latex: "v = \\frac{s}{t}".to_string(),
                display: false,
            }
        );
    }

    #[test]
    fn a_formula_at_the_document_end_is_claimed() {
        // 闭 `$` 后面什么都没有（`inline_math_close_ok(None)`）也一样算公式。
        assert_eq!(
            claim_inline("$x^2$", "x^2", None),
            Claim::Yes {
                latex: "x^2".to_string(),
                display: false,
            }
        );
    }

    #[test]
    fn prices_are_declined() {
        // 「价格 $5 和 $10 的对比」：mdast 给 value="5 和 "，闭 `$` 后紧跟 '1'。
        assert_eq!(claim_inline("$5 和 $", "5 和 ", Some('1')), Claim::No);
    }

    #[test]
    fn a_closing_dollar_before_a_digit_is_declined() {
        // 「成本 $x$5 元」：与 PDF / 生成侧一致，有意不认。
        assert_eq!(claim_inline("$x$", "x", Some('5')), Claim::No);
    }

    #[test]
    fn whitespace_padded_content_is_declined() {
        assert_eq!(claim_inline("$ x $", " x ", Some(' ')), Claim::No);
        assert_eq!(claim_inline("$x $", "x ", Some(' ')), Claim::No);
        assert_eq!(claim_inline("$$", "", None), Claim::No);
    }

    #[test]
    fn single_line_double_dollar_is_display() {
        // 单行 `$$E = mc^2$$` 是 InlineMath，但节点源码以 `$$` 开头 → 按块级排版。
        assert_eq!(
            claim_inline("$$E = mc^2$$", "E = mc^2", None),
            Claim::Yes {
                latex: "E = mc^2".to_string(),
                display: true,
            }
        );
    }

    #[test]
    fn block_formula_is_claimed_and_trimmed() {
        assert_eq!(
            claim_block("\n\\sum_{i=1}^{n} i\n"),
            Claim::Yes {
                latex: "\\sum_{i=1}^{n} i".to_string(),
                display: true,
            }
        );
        assert_eq!(
            claim_block("E = mc^2"),
            Claim::Yes {
                latex: "E = mc^2".to_string(),
                display: true,
            }
        );
    }

    #[test]
    fn a_blank_block_is_declined() {
        assert_eq!(claim_block(""), Claim::No);
        assert_eq!(claim_block("  \n "), Claim::No);
    }

    // ── 缓存键 ──

    #[test]
    fn the_same_formula_makes_the_same_key() {
        assert_eq!(key("x^2", false, INK, 12.0), key("x^2", false, INK, 12.0));
    }

    #[test]
    fn color_and_display_change_the_key() {
        assert_ne!(key("x^2", false, INK, 12.0), key("x^2", false, FILL, 12.0));
        assert_ne!(key("x^2", false, INK, 12.0), key("x^2", true, INK, 12.0));
        assert_ne!(key("x^2", false, INK, 12.0), key("x^3", false, INK, 12.0));
    }

    #[test]
    fn size_is_bucketed_to_half_a_point() {
        assert_eq!(key("x^2", false, INK, 12.0), key("x^2", false, INK, 12.2));
        assert_ne!(key("x^2", false, INK, 12.0), key("x^2", false, INK, 12.6));
        // 界面像素 → 点的换算：14px = 10.5pt，15px = 11.25pt（同一档 11.5pt 才对得上）。
        assert_eq!(size_half_pt(14.0 * PT_PER_PX), 21);
        assert_eq!(size_half_pt(14.4 * PT_PER_PX), 22);
    }

    // ── 上限与淘汰 ──

    #[test]
    fn the_cache_stops_at_the_cap_and_drops_the_oldest() {
        let mut cache = FormulaCache::new();
        for i in 0..MAX_ENTRIES + 88 {
            let key = key(&format!("x^{i}"), false, INK, 12.0);
            round_trip(&mut cache, key.clone(), Ok(image()));
            assert!(cache.lookup(&key).is_some());
        }
        assert_eq!(cache.len(), MAX_ENTRIES);
        assert_eq!(cache.order_len(), MAX_ENTRIES);
        // 最早插入的已经淘汰，最后插入的还在。
        assert!(cache.lookup(&key("x^0", false, INK, 12.0)).is_none());
        assert!(cache
            .lookup(&key(&format!("x^{}", MAX_ENTRIES + 87), false, INK, 12.0))
            .is_some());
        // 淘汰的图要交给调用方 drop（图集不会自己回收）。
        assert_eq!(cache.take_released().len(), 88);
        assert!(cache.take_released().is_empty());
    }

    #[test]
    fn a_failed_formula_holds_a_slot_but_no_image() {
        let mut cache = FormulaCache::new();
        let bad = key("\\frac{", false, INK, 12.0);
        round_trip(&mut cache, bad.clone(), Err("mitex".to_string()));
        assert!(matches!(cache.lookup(&bad), Some(FormulaEntry::Failed)));
        assert!(cache.take_released().is_empty());
    }

    #[test]
    fn reinserting_a_key_keeps_one_place_in_the_queue() {
        let mut cache = FormulaCache::new();
        let first = key("x^2", false, INK, 12.0);
        round_trip(&mut cache, first.clone(), Ok(image()));
        for i in 0..MAX_ENTRIES {
            round_trip(
                &mut cache,
                key(&format!("y^{i}"), false, INK, 12.0),
                Ok(image()),
            );
        }
        // 刷新一个已有的键：不新增队列位置，也不该淘汰任何活着的条目。
        let released_before = cache.take_released().len();
        round_trip(&mut cache, first.clone(), Ok(image()));
        assert_eq!(cache.len(), MAX_ENTRIES);
        assert_eq!(cache.order_len(), MAX_ENTRIES);
        assert_eq!(cache.take_released().len(), released_before);
        assert!(cache.lookup(&first).is_some());
    }

    // ── 在途复位 ──

    #[test]
    fn success_clears_the_in_flight_flag() {
        let mut cache = FormulaCache::new();
        let key = key("x^2", false, INK, 12.0);
        assert!(cache.needs_request(&key));
        round_trip(&mut cache, key.clone(), Ok(image()));
        assert!(matches!(cache.lookup(&key), Some(FormulaEntry::Ready(_))));
        // 有图就不再请后台出图。
        assert!(!cache.needs_request(&key));
    }

    #[test]
    fn failure_clears_the_in_flight_flag_and_is_not_retried() {
        let mut cache = FormulaCache::new();
        let key = key("\\frac{", true, INK, 16.0);
        round_trip(&mut cache, key.clone(), Err("Typst".to_string()));
        assert!(!cache.is_in_flight(&key));
        // 失败也留在缓存里：每帧重试会把 UI 线程拖住。
        assert!(!cache.needs_request(&key));
    }

    #[test]
    fn a_request_already_in_flight_is_not_queued_twice() {
        let mut cache = FormulaCache::new();
        let key = key("x^2", false, INK, 12.0);
        cache.start(&key);
        assert!(!cache.needs_request(&key));
        assert!(cache.is_in_flight(&key));
    }

    #[test]
    fn distinct_size_buckets_are_distinct_entries() {
        let mut cache = FormulaCache::new();
        round_trip(&mut cache, key("x^2", false, INK, 12.0), Ok(image()));
        round_trip(&mut cache, key("x^2", false, INK, 16.0), Ok(image()));
        assert_eq!(cache.len(), 2);
    }

    // ── 页面离开 ──

    #[test]
    fn drain_returns_every_bitmap_and_empties_the_cache() {
        let mut cache = FormulaCache::new();
        for i in 0..3 {
            round_trip(
                &mut cache,
                key(&format!("x^{i}"), false, INK, 12.0),
                Ok(image()),
            );
        }
        round_trip(
            &mut cache,
            key("\\frac{", false, INK, 12.0),
            Err("bad".to_string()),
        );
        // 坏公式没有位图，交还的是三张图。
        assert_eq!(cache.drain().len(), 3);
        assert_eq!(cache.len(), 0);
        assert_eq!(cache.order_len(), 0);
        assert!(!cache.is_in_flight(&key("x^0", false, INK, 12.0)));
        assert!(cache.drain().is_empty());
    }

    // ── 出图链路（不碰窗口：字体扫描是进程一次的） ──

    /// mitex → Typst → PNG → `image` 解码 → 逻辑尺寸，整条链路走一遍。
    #[test]
    fn a_real_formula_decodes_into_a_bitmap() {
        let formula = huanshu_core::services::export::renderer()
            .render_formula("x^2 + y^2", false, INK, 16.0)
            .expect("a valid formula renders");
        let (pixel_width, pixel_height) = (formula.width, formula.height);
        assert!(pixel_width > 0 && pixel_height > 0);
        let image = decode(formula).expect("the formula png decodes");
        assert_eq!(image.width, pixel_width as f32 / PIXELS_PER_PT as f32);
        assert_eq!(image.height, pixel_height as f32 / PIXELS_PER_PT as f32);
        assert!(image.width > 4.0, "16pt 的公式不该只有几个像素宽");
        assert!(image.height > 4.0);
    }

    /// 坏公式：`render_formula` 给出 `Err`（不 panic），缓存把它记成 `Failed`、清在途。
    #[test]
    fn a_broken_formula_does_not_render() {
        let key = key("\\frac{", true, INK, 16.0);
        let result = render_formula_image(&key);
        assert!(result.is_err());
        let mut cache = FormulaCache::new();
        cache.start(&key);
        cache.finish(key.clone(), result);
        assert!(matches!(cache.lookup(&key), Some(FormulaEntry::Failed)));
        assert!(!cache.is_in_flight(&key));
    }
}
