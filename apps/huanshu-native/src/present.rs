//! 演示模式：独立的无边框全屏窗口，一页一屏。
//!
//! 翻页规则与 `src/views/PresentationOverlay.vue` 一致：←/→、PageUp/PageDown、空格、
//! 鼠标左键翻页，Home/End 跳首尾，Esc 退出。位图按屏幕尺寸**非等比**铺满（拉伸模式，
//! 与 Tauri 版相同），32:9 的牌组投到 16:9 屏上就是会被压扁——那正是投到大屏上的真实效果。
//!
//! 预取规则照抄 Tauri 版：**当前页还没有图时只请求当前页**，落图后再预取邻页，
//! `max_cached` = 10。

use std::sync::Arc;

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::prelude::*;
use gpui_kit::{
    div, img, px, rgb, size, App, Bounds, Context, KeystrokeEvent, MouseButton, ObjectFit, Render,
    RenderImage, SharedString, Subscription, Window, WindowBounds, WindowOptions,
};
use huanshu_core::preview::{PreviewCache, PreviewOptions, PreviewStyle};
use huanshu_core::types::SlideBlueprint;

use crate::app::{self, AppShared};
use crate::images::drop_all;

/// 演示窗口里保留几张图。
const MAX_CACHED: usize = 10;
/// 当前页落图后，一次预取到后几页。
const PREFETCH_AHEAD: usize = 3;
/// 当前页之前预取几页。
const PREFETCH_BEHIND: usize = 1;

pub struct Present {
    project_id: i64,
    shared: AppShared,
    deck: Vec<SlideBlueprint>,
    theme: String,
    index: usize,
    cache: PreviewCache<Arc<RenderImage>>,
    width: u32,
    failure: Option<String>,
    #[allow(dead_code)]
    subscriptions: Vec<Subscription>,
}

/// 打开演示窗口：在看板所在的显示器上全屏。
///
/// `deck` 与 `shared` 都由看板传进来——演示不依赖看板还活着，关闭看板也不影响它。
pub fn open(
    project_id: i64,
    deck: Vec<SlideBlueprint>,
    theme: &str,
    shared: AppShared,
    cx: &mut App,
) {
    let options = window_options(cx);
    let opened = cx.open_window(options, move |window, cx| {
        cx.new(|cx| Present::new(project_id, shared, deck, theme.to_string(), window, cx))
    });
    if let Err(error) = opened {
        eprintln!("[native] 无法打开演示窗口：{error}");
    }
}

fn window_options(cx: &mut App) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Fullscreen(Bounds::maximized(None, cx))),
        titlebar: None,
        window_min_size: Some(size(px(640.), px(360.))),
        ..Default::default()
    }
}

impl Present {
    pub fn new(
        project_id: i64,
        shared: AppShared,
        deck: Vec<SlideBlueprint>,
        theme: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let width = bucket(window.bounds().size.width.as_f32() * window.scale_factor());
        let index = std::env::var("HUANSHU_PRESENT_PAGE")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(0)
            .min(deck.len().saturating_sub(1));
        let subscriptions = vec![cx.observe_keystrokes(
            |this: &mut Present, event: &KeystrokeEvent, window, cx| match event
                .keystroke
                .key
                .as_str()
            {
                "right" | "space" | "down" | "pagedown" | "enter" => this.step(1, cx),
                "left" | "up" | "pageup" => this.step(-1, cx),
                "home" => this.jump(0, cx),
                "end" => this.jump(this.deck.len().saturating_sub(1), cx),
                "escape" => this.close(window, cx),
                _ => {}
            },
        )];
        Self {
            project_id,
            shared,
            deck,
            theme,
            index,
            cache: PreviewCache::new(PreviewOptions {
                max_cached: Some(MAX_CACHED),
                ..PreviewOptions::default()
            }),
            width,
            failure: None,
            subscriptions,
        }
    }

    fn jump(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.deck.len() {
            self.index = index;
            cx.notify();
        }
    }

    fn step(&mut self, delta: isize, cx: &mut Context<Self>) {
        let next = self.index as isize + delta;
        if next < 0 || next as usize >= self.deck.len() {
            return;
        }
        self.index = next as usize;
        cx.notify();
    }

    /// 关闭：把剩下的图交还——演示窗口一关，GPUI 就没人替我们释放了。
    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut released = self.cache.reset();
        released.extend(self.cache.take_released());
        drop_all(released, Some(window), cx);
        cx.quit();
    }

    fn deck_values(&self) -> Vec<Option<serde_json::Value>> {
        self.deck
            .iter()
            .map(|slide| serde_json::to_value(slide).ok())
            .collect()
    }

    /// 当前页还没有图时只请求当前页；有了才预取邻页。
    fn pump(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let wanted = self.wanted();
        self.cache.set_wanted(&wanted);
        self.cache.set_deck(
            &self.deck_values(),
            PreviewStyle {
                theme: Some(self.theme.clone()),
                fonts: String::new(),
                width_px: self.width as f32,
            },
        );
        let Some(request) = self.cache.next_request() else {
            return;
        };
        let project_id = self.project_id;
        let shared = self.shared.clone();
        let deck = self.deck.clone();
        let theme = self.theme.clone();
        let width = request.width_px;
        let only = request.only.clone();
        cx.spawn_in(window, async move |this, window| {
            let result = window
                .background_spawn(async move {
                    app::render_pages(shared, Some(project_id), deck, Some(theme), width, only)
                })
                .await;
            let _ = this.update_in(window, |present, window, cx| {
                let mut released = present.cache.complete(&request, result);
                released.extend(present.cache.take_released());
                drop_all(released, Some(window), cx);
                if present.cache.failed() {
                    present.failure = Some("渲染失败".to_string());
                }
                cx.notify();
                present.pump(window, cx);
            });
        })
        .detach();
    }

    fn wanted(&self) -> Vec<usize> {
        if self.cache.image(self.index).is_some() {
            let from = self.index.saturating_sub(PREFETCH_BEHIND);
            let to = (self.index + PREFETCH_AHEAD).min(self.deck.len().saturating_sub(1));
            (from..=to).collect()
        } else {
            vec![self.index]
        }
    }
}

fn bucket(width_px: f32) -> u32 {
    let width = width_px.max(1.0).ceil() as u32;
    64u32.max(width.div_ceil(64) * 64)
}

impl Render for Present {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let wanted = bucket(window.bounds().size.width.as_f32() * window.scale_factor());
        if wanted != self.width {
            self.width = wanted;
            // 换显示器/改窗口尺寸：宽度档位变了，图要重渲。
            let mut released = self.cache.reset();
            released.extend(self.cache.take_released());
            drop_all(released, Some(window), cx);
        }
        self.pump(window, cx);

        let frame = self.cache.image(self.index).cloned();
        let label = format!("{} / {}", self.index + 1, self.deck.len());
        let failure = self.failure.clone();
        div()
            .id("presentation")
            .size_full()
            .bg(rgb(0x000000))
            .when_some(frame, |element, image| {
                // ObjectFit::Fill = 拉伸到整个窗口，与 Tauri 版的拉伸模式一致。
                element.child(img(image).w_full().h_full().object_fit(ObjectFit::Fill))
            })
            .child(
                div()
                    .absolute()
                    .bottom_2()
                    .left_2()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(SharedString::from(match failure {
                        Some(error) => format!("渲染失败：{error}"),
                        None => label,
                    })),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|present, _event, _window, cx| {
                    present.step(1, cx);
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|present, _event, _window, cx| {
                    present.step(-1, cx);
                }),
            )
    }
}
