//! 页面里显示 Typst 位图的唯一方式：[`SlideImages`] 管请求与缓存，[`slide_frame`] 画框。
//!
//! 对应原版的 `useTypstThumbnails`（调度规则在 `huanshu_core::preview`，两边同一套）与
//! `SlideFrame.vue`（加载框、渲染中、失败与重试）。看板、编辑器预览、演示都用它，
//! **不要在页面里自己调 `render_slide_pixels`**。
//!
//! 必须遵守的三条（违反过）：
//! 1. 每张不再显示的图都要 `drop_image`——`SlideImages` 内部处理，页面离开时调 [`SlideImages::release`]。
//! 2. 失败不自动重试；重试按钮调 [`SlideImages::refresh`]。
//! 3. 位图宽度 = 显示宽度 × `window.scale_factor()`，按 64px 档位取整（core 负责取整）。

use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui_kit::prelude::*;
use gpui_kit::{div, img, px, App, Context, ObjectFit, RenderImage, SharedString, Window};
use huanshu_core::preview::{PreviewCache, PreviewOptions, PreviewRequest, PreviewStyle};

use crate::app::{self, AppShared};
use crate::images::drop_all;
use crate::theme::StudioExt as _;
use crate::widgets;

/// 「渲染中…」要等这么久才出现，免得翻页就闪（原版 `pendingDelay`）。
pub const PENDING_DELAY: Duration = Duration::from_millis(300);

pub type Image = Arc<RenderImage>;

/// 一组幻灯片位图：牌组 + 显示宽度 + 想要哪几页 → 请求、落图、释放。
pub struct SlideImages {
    shared: AppShared,
    project_id: Option<i64>,
    cache: PreviewCache<Image>,
    deck: Vec<Option<serde_json::Value>>,
    /// 显示宽度（逻辑像素）与缩放比。
    width: f32,
    scale: f32,
    /// `index` 页从什么时候开始等图（`SlideFrame` 的 300ms 延迟）。
    waiting: Option<(usize, Instant)>,
}

impl SlideImages {
    pub fn new(shared: AppShared, project_id: Option<i64>, options: PreviewOptions) -> Self {
        Self {
            shared,
            project_id,
            cache: PreviewCache::new(options),
            deck: Vec::new(),
            width: 0.0,
            scale: 1.0,
            waiting: None,
        }
    }

    /// 牌组（空位 `None` 不渲染，页码不变）。
    pub fn set_deck(&mut self, deck: Vec<Option<serde_json::Value>>) {
        self.deck = deck;
    }

    /// 显示宽度（逻辑像素）。每帧都可以调：只有换档才会重渲。
    pub fn set_width(&mut self, width: f32, window: &Window) {
        self.width = width;
        self.scale = window.scale_factor();
    }

    pub fn set_wanted(&mut self, indices: &[usize]) {
        self.cache.set_wanted(indices);
    }

    pub fn image(&self, index: usize) -> Option<Image> {
        self.cache.image(index).cloned()
    }

    pub fn failed(&self) -> bool {
        self.cache.failed()
    }

    pub fn overflow(&self, index: usize) -> bool {
        self.cache.overflow(index)
    }

    /// 有请求在途、或刚被标记要重发（原版 `useTypstThumbnails` 的 `pending`：
    /// 编辑器的「渲染中…」标记看它）。
    pub fn pending(&self) -> bool {
        self.cache.is_pending()
    }

    /// 失败后的手动重试（原版「渲染失败 · 重试」）。
    pub fn refresh(&mut self) {
        self.cache.refresh();
    }

    /// `index` 页的显示状态，交给 [`slide_frame`]。
    pub fn frame_state(&self, index: usize) -> FrameState {
        match (self.image(index), self.failed()) {
            (Some(image), failed) => FrameState::Ready { image, failed },
            (None, true) => FrameState::Failed,
            (None, false) => FrameState::Loading {
                show_label: self
                    .waiting
                    .is_some_and(|(page, since)| page == index && since.elapsed() >= PENDING_DELAY),
            },
        }
    }

    /// 页面离开或牌组整体作废时调用：交还全部位图。
    pub fn release(&mut self, cx: &mut App) {
        let mut released = self.cache.reset();
        released.extend(self.cache.take_released());
        self.waiting = None;
        drop_all(released, None, cx);
    }

    /// 换目标（原版 `useTypstThumbnails` 的 `resetKey` 换页清图）：清空全部图并作废在途响应，
    /// 调用方随即显示加载框。牌组与 `wanted` 不动（由调用方随后交新的）。
    pub fn reset(&mut self, cx: &mut App) {
        let mut released = self.cache.reset();
        released.extend(self.cache.take_released());
        self.waiting = None;
        drop_all(released, None, cx);
    }

    /// 发请求、落图、释放。`get` 从视图里取出这个 `SlideImages`（结果回来时要用）。
    /// `focus` 是当前最关心的页：它没图时开始计 300ms，到点重绘一次以显示「渲染中…」。
    ///
    /// 每帧调用都安全：内容没变就不会发请求（core 只在真变了时才重新打开失败闸）。
    pub fn pump<T: 'static>(
        &mut self,
        focus: Option<usize>,
        get: fn(&mut T) -> &mut SlideImages,
        window: &mut Window,
        cx: &mut Context<T>,
    ) {
        let theme = cx.studio().theme.id.to_string();
        let fonts = self
            .shared
            .lock()
            .ok()
            .and_then(|state| serde_json::to_string(&state.app_settings.fonts).ok())
            .unwrap_or_default();
        self.cache.set_deck(
            &self.deck,
            PreviewStyle {
                theme: Some(theme.clone()),
                fonts,
                width_px: self.width * self.scale,
            },
        );
        self.track_waiting(focus, cx);
        let released = self.cache.take_released();
        drop_all(released, Some(window), cx);

        let Some(request) = self.cache.next_request() else {
            return;
        };
        let released = self.cache.take_released();
        drop_all(released, Some(window), cx);
        let shared = self.shared.clone();
        let project_id = self.project_id;
        let PreviewRequest {
            ref slides,
            width_px,
            ref only,
            ..
        } = request;
        let (slides, only) = (slides.clone(), only.clone());
        crate::tasks::run_bg(
            window,
            cx,
            move || app::render_pages(shared, project_id, slides, Some(theme), width_px, only),
            move |view: &mut T, result, window, cx| {
                let images = get(view);
                let mut released = images.cache.complete(&request, result);
                released.extend(images.cache.take_released());
                drop_all(released, Some(window), cx);
                cx.notify();
                // 在途期间的变化合并成这一次尾部请求（serial）。
                get(view).pump(focus, get, window, cx);
            },
        );
    }

    fn track_waiting<T: 'static>(&mut self, focus: Option<usize>, cx: &mut Context<T>) {
        let Some(page) = focus.filter(|page| self.image(*page).is_none() && !self.failed()) else {
            self.waiting = None;
            return;
        };
        if self.waiting.is_some_and(|(waiting, _)| waiting == page) {
            return;
        }
        self.waiting = Some((page, Instant::now()));
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(PENDING_DELAY).await;
            let _ = this.update(cx, |_view, cx| cx.notify());
        })
        .detach();
    }
}

/// 一页的显示状态。
#[derive(Clone)]
pub enum FrameState {
    /// 有图；`failed` 为真时是旧图 + 角上「渲染失败 · 重试」。
    Ready { image: Image, failed: bool },
    /// 还没有图；`show_label` 为真时显示「渲染中…」。
    Loading { show_label: bool },
    /// 没有图且渲染失败。
    Failed,
}

/// 原版 `SlideFrame.vue`：固定尺寸的框，位图铺满（`stretch` 为真时非等比拉伸，演示用）。
/// `on_retry` 为 `None` 时不显示重试按钮。
pub fn slide_frame(
    id: impl Into<gpui_kit::ElementId>,
    state: FrameState,
    width: f32,
    height: f32,
    stretch: bool,
    on_retry: Option<Box<dyn Fn(&mut Window, &mut App) + 'static>>,
    cx: &App,
) -> impl IntoElement {
    let c = cx.studio().colors;
    let frame = div()
        .id(id.into())
        .relative()
        .flex_none()
        .w(px(width))
        .h(px(height))
        .overflow_hidden()
        .when_some(cx.studio().paper, |sheet, p| {
            sheet.shadow(widgets::paper_shadow(&p))
        });
    let retry = on_retry.map(|on_retry| {
        widgets::small(widgets::ghost_button("slide-retry", "重试"))
            .on_click(move |_event, window, cx| on_retry(window, cx))
    });
    match state {
        FrameState::Ready { image, failed } => frame
            .child(img(image).size_full().object_fit(if stretch {
                ObjectFit::Fill
            } else {
                ObjectFit::Contain
            }))
            .when(failed, |frame| {
                frame.child(
                    div()
                        .absolute()
                        .top(px(6.0))
                        .right(px(6.0))
                        .flex()
                        .items_center()
                        .gap(px(4.0))
                        .px(px(6.0))
                        .rounded(px(6.0))
                        .bg(c.error_bg)
                        .border_1()
                        .border_color(c.error_border)
                        .text_size(px(11.0))
                        .text_color(c.error)
                        .child("渲染失败")
                        .children(retry),
                )
            }),
        FrameState::Loading { show_label } => frame
            .flex()
            .items_center()
            .justify_center()
            .bg(c.surface)
            .border_1()
            .border_color(c.border)
            .text_size(px(12.0))
            .text_color(c.muted)
            .when(show_label, |frame| {
                frame.child(SharedString::from("渲染中…"))
            }),
        FrameState::Failed => frame
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(6.0))
            .bg(c.error_bg)
            .border_1()
            .border_color(c.error_border)
            .text_size(px(12.0))
            .text_color(c.error)
            .child("渲染失败")
            .children(retry),
    }
}
