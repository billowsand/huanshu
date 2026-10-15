//! 演示（原版 `PresentationOverlay.vue`）：盖在当前页上的一层，不是新窗口。
//!
//! 已实现：打开即全屏、拉伸/等比（S）、全览（O）、全屏（F）、←/→/↑/↓/空格/Home/End 翻页、
//! 点舞台下一页、Esc（先关全览，再退出演示）、底部胶片条、页码与进度、主题圆点、
//! 当前页先出图再预取前 1 后 3 页（`max_cached` 10）。
//!
//! 按键走焦点（`track_focus` + `on_key_down`），只在演示层获得焦点时生效，不会串到别的页面。
//! 待实现见 `docs/native/parity.md`「演示」一节（切页动画、快捷键提示浮层、触摸滑动）。

use std::time::{Duration, Instant};

use gpui_kit::prelude::*;
use gpui_kit::{
    div, px, App, Context, FocusHandle, KeyDownEvent, MouseButton, Render, SharedString, Window,
};
use huanshu_core::preview::PreviewOptions;

use crate::icons::carbon;
use crate::shell::ShellHandle;
use crate::slides::{slide_frame, SlideImages};
use crate::theme::StudioExt as _;
use crate::widgets;

/// 一页最多预取到后面几页 / 前面几页（原版 `wanted` = 当前 -1 … +3）。
const PREFETCH_AHEAD: usize = 3;
const PREFETCH_BEHIND: usize = 1;
/// 底部控制条高度（原版 `.pres-controls`）。
const CONTROLS_H: f32 = 60.0;
/// 快捷键提示显示多久（原版 3 秒）。
const SHORTCUT_HINT: Duration = Duration::from_secs(3);

pub struct PresentArgs {
    pub project_id: Option<i64>,
    pub deck: Vec<serde_json::Value>,
    pub start: usize,
    /// 页面高宽比（16:9 → 0.5625）。
    pub aspect: f32,
}

pub struct Presentation {
    handle: ShellHandle,
    deck: Vec<serde_json::Value>,
    titles: Vec<String>,
    index: usize,
    aspect: f32,
    stretch: bool,
    overview: bool,
    stage: SlideImages,
    thumbs: SlideImages,
    focus: FocusHandle,
    opened_at: Instant,
}

impl Presentation {
    pub fn new(
        args: PresentArgs,
        handle: ShellHandle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let shared = handle.shared().clone();
        let mut stage = SlideImages::new(
            shared.clone(),
            args.project_id,
            PreviewOptions {
                max_cached: Some(10),
                ..PreviewOptions::default()
            },
        );
        let mut thumbs = SlideImages::new(shared, args.project_id, PreviewOptions::default());
        let deck_values: Vec<Option<serde_json::Value>> =
            args.deck.iter().cloned().map(Some).collect();
        stage.set_deck(deck_values.clone());
        thumbs.set_deck(deck_values);
        let titles = args
            .deck
            .iter()
            .map(|slide| {
                slide
                    .get("title")
                    .and_then(|title| title.as_str())
                    .unwrap_or("")
                    .to_string()
            })
            .collect();
        // 原版打开演示即进入全屏（被拒绝时退回窗口内）。
        if !window.is_fullscreen() {
            window.toggle_fullscreen();
        }
        Self {
            handle,
            index: args.start.min(args.deck.len().saturating_sub(1)),
            deck: args.deck,
            titles,
            aspect: args.aspect,
            stretch: true,
            overview: false,
            stage,
            thumbs,
            focus: cx.focus_handle(),
            opened_at: Instant::now(),
        }
    }

    pub fn focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
    }

    /// 关闭前交还全部位图。
    pub fn release(&mut self, cx: &mut Context<Self>) {
        self.stage.release(cx);
        self.thumbs.release(cx);
    }

    fn go(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.deck.len() && index != self.index {
            self.index = index;
            cx.notify();
        }
    }

    fn step(&mut self, delta: isize, cx: &mut Context<Self>) {
        let next = self.index as isize + delta;
        if next >= 0 {
            self.go(next as usize, cx);
        }
    }

    fn exit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let handle = self.handle.clone();
        // 退出要在外壳里做（它持有本实体），推迟到这一轮事件之后。
        window.defer(cx, move |window, cx| handle.close_presentation(window, cx));
    }

    fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "o" => self.overview = !self.overview,
            "right" | "down" | "space" if !self.overview => self.step(1, cx),
            "left" | "up" if !self.overview => self.step(-1, cx),
            "home" => self.go(0, cx),
            "end" => self.go(self.deck.len().saturating_sub(1), cx),
            "f" => window.toggle_fullscreen(),
            "s" if !self.overview => self.stretch = !self.stretch,
            "escape" if self.overview => self.overview = false,
            "escape" => self.exit(window, cx),
            _ => return,
        }
        cx.stop_propagation();
        cx.notify();
    }

    fn wanted(&self) -> Vec<usize> {
        // 当前页还没有图时只要当前页：一次请求要等 `only` 里每一页都出图才返回。
        if self.stage.image(self.index).is_none() {
            return vec![self.index];
        }
        let from = self.index.saturating_sub(PREFETCH_BEHIND);
        let to = (self.index + PREFETCH_AHEAD).min(self.deck.len().saturating_sub(1));
        (from..=to).collect()
    }

    fn stage_size(&self, window: &Window) -> (f32, f32) {
        let size = window.viewport_size();
        let (area_w, area_h) = (
            size.width.as_f32(),
            (size.height.as_f32() - CONTROLS_H - crate::theme::TITLEBAR_HEIGHT).max(90.0),
        );
        if self.stretch {
            return (area_w, area_h);
        }
        let mut width = area_w;
        let mut height = width * self.aspect;
        if height > area_h {
            height = area_h;
            width = height / self.aspect;
        }
        (width, height)
    }
}

impl Render for Presentation {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.studio().colors;
        let theme_id = cx.studio().theme.id;
        let (stage_w, stage_h) = self.stage_size(window);
        // 拉伸模式按铺满所需的宽度光栅化：max(舞台宽, 舞台高 / 高宽比)。
        let raster_w = if self.stretch {
            stage_w.max(stage_h / self.aspect)
        } else {
            stage_w
        };
        self.stage.set_width(raster_w, window);
        let wanted = self.wanted();
        self.stage.set_wanted(&wanted);
        self.stage.pump(
            Some(self.index),
            |this: &mut Self| &mut this.stage,
            window,
            cx,
        );

        let content = if self.overview {
            self.overview_grid(window, cx).into_any_element()
        } else {
            let frame = slide_frame(
                "pres-stage",
                self.stage.frame_state(self.index),
                stage_w,
                stage_h,
                self.stretch,
                None,
                cx,
            );
            div()
                .id("pres-stage-area")
                .flex_1()
                .min_h_0()
                .relative()
                .flex()
                .items_center()
                .justify_center()
                .bg(gpui_kit::rgb(0x000000))
                .child(frame)
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _event, _window, cx| this.step(1, cx)),
                )
                .child(
                    nav_arrow("pres-prev", "chevron-left", true, cx).on_click(cx.listener(
                        |this, _event, _window, cx| {
                            cx.stop_propagation();
                            this.step(-1, cx);
                        },
                    )),
                )
                .child(
                    nav_arrow("pres-next", "chevron-right", false, cx).on_click(cx.listener(
                        |this, _event, _window, cx| {
                            cx.stop_propagation();
                            this.step(1, cx);
                        },
                    )),
                )
                .when(self.opened_at.elapsed() < SHORTCUT_HINT, |area| {
                    area.child(
                        div()
                            .absolute()
                            .top(px(16.0))
                            .left_0()
                            .right_0()
                            .flex()
                            .justify_center()
                            .child(
                                div()
                                    .px(px(12.0))
                                    .py(px(6.0))
                                    .rounded(px(8.0))
                                    .bg(c.panel)
                                    .border_1()
                                    .border_color(c.border)
                                    .text_size(px(12.0))
                                    .text_color(c.muted)
                                    .child("← → 翻页 · O 全览 · S 拉伸/等比 · F 全屏 · Esc 退出"),
                            ),
                    )
                })
                .into_any_element()
        };

        let total = self.deck.len().max(1);
        let progress = (self.index + 1) as f32 / total as f32;
        let handle = self.handle.clone();
        let controls = div()
            .flex_none()
            .h(px(CONTROLS_H))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(16.0))
            .px(px(16.0))
            .bg(c.panel)
            .border_t_1()
            .border_color(c.border)
            // 胶片条：页码方块，点哪页去哪页。
            .child(
                div()
                    .id("pres-filmstrip")
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_row()
                    .gap(px(6.0))
                    .overflow_x_scroll()
                    .children((0..self.deck.len()).map(|index| {
                        let active = index == self.index;
                        div()
                            .id(("film", index))
                            .flex_none()
                            .min_w(px(36.0))
                            .h(px(26.0))
                            .px(px(6.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(5.0))
                            .border_1()
                            .border_color(if active { c.primary } else { c.border })
                            .bg(if active { c.primary_bg } else { c.surface })
                            .text_size(px(11.0))
                            .text_color(if active { c.primary } else { c.muted })
                            .cursor_pointer()
                            .child(SharedString::from((index + 1).to_string()))
                            .on_click(
                                cx.listener(move |this, _event, _window, cx| this.go(index, cx)),
                            )
                    })),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(4.0))
                    .w(px(110.0))
                    .child(div().text_size(px(13.0)).child(format!(
                        "{} / {}",
                        self.index + 1,
                        self.deck.len()
                    )))
                    .child(
                        div()
                            .w_full()
                            .h(px(3.0))
                            .rounded_full()
                            .bg(c.surface)
                            .child(
                                div()
                                    .h_full()
                                    .w(gpui_kit::relative(progress))
                                    .rounded_full()
                                    .bg(c.primary),
                            ),
                    ),
            )
            .child(widgets::section_label("主题", cx))
            .child(widgets::theme_dots(
                "pres-theme",
                theme_id,
                move |id, window, cx| handle.set_theme(id, true, window, cx),
                cx,
            ))
            .child(
                widgets::icon_button(
                    "pres-overview",
                    "grid",
                    if self.overview {
                        "关闭全览 (O)"
                    } else {
                        "打开全览 (O)"
                    },
                )
                .on_click(cx.listener(|this, _event, _window, cx| {
                    this.overview = !this.overview;
                    cx.notify();
                })),
            )
            .child(
                widgets::icon_button(
                    "pres-stretch",
                    if self.stretch {
                        "fit-to-screen"
                    } else {
                        "fit-to-width"
                    },
                    if self.stretch {
                        "切换为等比显示 (S)"
                    } else {
                        "切换为拉伸铺满 (S)"
                    },
                )
                .on_click(cx.listener(|this, _event, _window, cx| {
                    this.stretch = !this.stretch;
                    cx.notify();
                })),
            )
            .child(
                widgets::icon_button(
                    "pres-fullscreen",
                    if window.is_fullscreen() {
                        "minimize"
                    } else {
                        "maximize"
                    },
                    if window.is_fullscreen() {
                        "退出全屏"
                    } else {
                        "全屏"
                    },
                )
                .on_click(|_event, window, _cx| window.toggle_fullscreen()),
            )
            .child(
                widgets::icon_button("pres-exit", "close", "退出演示 (Esc)")
                    .on_click(cx.listener(|this, _event, window, cx| this.exit(window, cx))),
            );

        div()
            .id("presentation")
            .track_focus(&self.focus)
            .on_key_down(
                cx.listener(|this, event: &KeyDownEvent, window, cx| {
                    this.on_key(event, window, cx)
                }),
            )
            .size_full()
            .flex()
            .flex_col()
            .bg(c.bg)
            .child(content)
            .child(controls)
    }
}

impl Presentation {
    /// 全览（原版「Keynote 全览」）：所有页的缩略图，点哪页进哪页。
    fn overview_grid(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.studio().colors;
        const CARD_W: f32 = 240.0;
        let card_h = CARD_W * self.aspect;
        self.thumbs.set_width(CARD_W, window);
        let all: Vec<usize> = (0..self.deck.len()).collect();
        self.thumbs.set_wanted(&all);
        self.thumbs
            .pump(None, |this: &mut Self| &mut this.thumbs, window, cx);
        div()
            .id("pres-overview")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .p(px(24.0))
            .child(div().text_size(px(18.0)).child("Keynote 全览"))
            .child(widgets::section_label("点击任意缩略图进入对应页", cx))
            .child(
                div()
                    .mt(px(16.0))
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .gap(px(16.0))
                    .children((0..self.deck.len()).map(|index| {
                        let active = index == self.index;
                        let state = self.thumbs.frame_state(index);
                        div()
                            .id(("ov", index))
                            .flex()
                            .flex_col()
                            .gap(px(6.0))
                            .p(px(6.0))
                            .rounded(px(8.0))
                            .border_2()
                            .border_color(if active { c.primary } else { c.border })
                            .cursor_pointer()
                            .child(slide_frame(
                                ("ov-frame", index),
                                state,
                                CARD_W,
                                card_h,
                                false,
                                None,
                                cx,
                            ))
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap(px(6.0))
                                    .w(px(CARD_W))
                                    .text_size(px(12.0))
                                    .child(
                                        div().text_color(c.primary).child((index + 1).to_string()),
                                    )
                                    .child(
                                        div()
                                            .truncate()
                                            .text_color(c.text)
                                            .child(self.titles[index].clone()),
                                    ),
                            )
                            .on_click(cx.listener(move |this, _event, _window, cx| {
                                this.index = index;
                                this.overview = false;
                                cx.notify();
                            }))
                    })),
            )
    }
}

/// 舞台两侧的圆形翻页按钮（原版 `.pres-nav`）。
fn nav_arrow(
    id: &'static str,
    icon: &str,
    left: bool,
    cx: &App,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    let c = cx.studio().colors;
    let button = div()
        .id(id)
        .absolute()
        .top(gpui_kit::relative(0.5))
        .mt(px(-24.0))
        .size(px(48.0))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(c.panel)
        .border_1()
        .border_color(c.border)
        .text_color(c.muted)
        .cursor_pointer()
        .child(carbon(icon));
    if left {
        button.left(px(16.0))
    } else {
        button.right(px(16.0))
    }
}
