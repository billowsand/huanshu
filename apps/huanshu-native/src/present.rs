//! 演示（原版 `PresentationOverlay.vue`）：盖在当前页上的一层，不是新窗口。
//!
//! 已实现：打开即全屏、拉伸/等比（S）、全览（O）、全屏（F）、←/→/↑/↓/空格/Home/End 翻页、
//! 点舞台下一页、触摸滑动翻页、Esc（先关全览；全屏时只退全屏，再按才退出演示）、
//! 底部胶片条、页码与进度、主题圆点、当前页先出图再预取前 1 后 3 页（`max_cached` 10）。
//!
//! 舞台几何照原版 `stretchedViewport`：非全屏留出 96 / 88 的边距，全屏铺满；
//! 原生覆盖层画在标题栏之下（原版覆盖层铺满窗口、标题栏用更高的 z-index 盖住），所以
//! 「视口」取覆盖层自己的区域，见 [`Presentation::stage_size`]。全屏时底部控制条改为浮在舞台上、
//! 悬停才淡入（原版 `.pres-controls--floating`；GPUI 没有 `backdrop-filter`，模糊省略）。
//!
//! 按键走焦点（`track_focus` + `on_key_down`），只在演示层获得焦点时生效，不会串到别的页面。
//! 待实现见 `docs/native/parity.md`「演示」一节（切页动画、快捷键提示浮层）。

use std::time::{Duration, Instant};

use gpui_kit::prelude::*;
use gpui_kit::{
    div, px, Animation, AnimationExt as _, App, Context, FocusHandle, KeyDownEvent, MouseButton,
    Render, ScrollDelta, ScrollWheelEvent, SharedString, TouchPhase, Window,
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
/// 滑动翻页的最小水平位移（原版 `handleTouchEnd` 的 50）。
const SWIPE_MIN: f32 = 50.0;
/// 浮动控制条淡入淡出的时长（原版 `.pres-controls--floating` 的 `transition: 0.3s`）。
const CONTROLS_FADE: Duration = Duration::from_millis(300);
/// 浮动控制条隐藏时下移的距离（原版 `transform: translateY(8px)`）。
const CONTROLS_LIFT: f32 = 8.0;
/// 浮动控制条底色 / 上边框的不透明度（原版 `bg` 72%、`border` 45%）。
const CONTROLS_BG_ALPHA: f32 = 0.72;
const CONTROLS_BORDER_ALPHA: f32 = 0.45;

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
    /// 鼠标是否停在浮动控制条上（原版 `.pres-controls--floating:hover`）。
    controls_hovered: bool,
    /// 进行中的触摸滑动，见 [`Presentation::on_swipe`]。
    swipe: Swipe,
    /// 刚完成一次滑动翻页：吞掉紧随其后的那一次「点舞台下一页」（原版 `didSwipe`）。
    did_swipe: bool,
}

/// 一次触摸滑动的手势状态（原版 `handleTouchStart` / `handleTouchEnd` 的 `touchStartX/Y`）。
#[derive(Default)]
struct Swipe {
    /// 手势进行中（收到 `TouchPhase::Started` 之后、`Ended`/`Cancelled` 之前）。
    /// 只有它为真时才累计位移：鼠标滚轮从不发 `Started`，因此滚轮不会翻页。
    active: bool,
    dx: f32,
    dy: f32,
}

impl Swipe {
    /// 累计一次触摸平移：`Started` 开一次新手势，手势进行中的 `Moved` 才累计。
    /// 鼠标滚轮恒为 `Moved`、从没有 `Started`，所以它的位移永远进不来。
    fn push(&mut self, phase: TouchPhase, delta: (f32, f32)) {
        match phase {
            TouchPhase::Started => {
                *self = Swipe {
                    active: true,
                    dx: delta.0,
                    dy: delta.1,
                }
            }
            TouchPhase::Moved if self.active => {
                self.dx += delta.0;
                self.dy += delta.1;
            }
            TouchPhase::Moved => {}
            // 结束由 [`Swipe::take`] 处理。
            TouchPhase::Ended | TouchPhase::Cancelled => {}
        }
    }

    /// 手势结束时的判定（原版 `handleTouchEnd`）：水平位移压过垂直位移且超过 50px 才翻页。
    /// 返回翻页方向（1 下一页 / -1 上一页），**并复位状态**（无论判定结果如何）。
    fn take(&mut self) -> Option<isize> {
        let swipe = std::mem::take(self);
        if !swipe.active || swipe.dx.abs() <= swipe.dy.abs() || swipe.dx.abs() <= SWIPE_MIN {
            return None;
        }
        // 左滑（dx < 0）下一页、右滑上一页。
        Some(if swipe.dx < 0.0 { 1 } else { -1 })
    }
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
            controls_hovered: false,
            swipe: Swipe::default(),
            did_swipe: false,
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
            // 原版 `handlePresKey`：全览开着先关全览；**全屏时 Esc 只退全屏**（`!document.fullscreenElement`
            // 才 `exitPresentation`），非全屏才退出演示。
            "escape" if self.overview => self.overview = false,
            "escape" if window.is_fullscreen() => window.toggle_fullscreen(),
            "escape" => self.exit(window, cx),
            _ => return,
        }
        cx.stop_propagation();
        cx.notify();
    }

    /// 舞台上的触摸平移（原版 `handleTouchStart` / `handleTouchEnd`）。
    ///
    /// GPUI 把触摸平移转成 `ScrollWheelEvent`（Windows 走 Direct Manipulation，见
    /// `gpui_pre_windows::direct_manipulation`）：一次手势的首个事件是
    /// `TouchPhase::Started`，中间是 `Moved`，最后是 `Ended`；而**鼠标滚轮恒为 `Moved`**
    /// （`gpui_pre_windows::events` 里写死）。所以「先见 `Started` 才累计、`Ended` 时判定」
    /// 正好把触摸平移和鼠标滚轮分开：滚轮既不会累计也不会判定（判定规则在 [`Swipe`] 里）。
    fn on_swipe(&mut self, event: &ScrollWheelEvent, cx: &mut Context<Self>) {
        let (dx, dy) = match event.delta {
            ScrollDelta::Pixels(delta) => (delta.x.as_f32(), delta.y.as_f32()),
            // 触摸平移恒为 `Pixels`；行数只出现在鼠标滚轮上，照原样累计即可（滚轮不会判定）。
            ScrollDelta::Lines(delta) => (delta.x, delta.y),
        };
        if event.touch_phase == TouchPhase::Started {
            // 新手势开始：原版 `handleTouchStart` 同时清掉上一次的 `didSwipe`。
            self.did_swipe = false;
        }
        if matches!(event.touch_phase, TouchPhase::Ended | TouchPhase::Cancelled) {
            // 手势结束（或被系统收走）：判定并复位，位移不再累计。
            if let Some(steps) = self.swipe.take() {
                self.did_swipe = true;
                self.step(steps, cx);
            }
            return;
        }
        self.swipe.push(event.touch_phase, (dx, dy));
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

    /// 舞台（幻灯片）的像素尺寸（原版 `stageSize` ← `stretchedViewport`）。
    ///
    /// 原版：非全屏时可用区 = `window.innerWidth - 96` × `window.innerHeight - 88`
    /// （`padH` / `padV`），全屏时都是 0（幻灯片铺满）；两个方向都 `max(…, 1)`。
    ///
    /// 原生差异：原版的覆盖层是 `position: fixed; inset: 0`，铺满整个窗口（连标题栏那条也盖住，
    /// 标题栏再用更高的 z-index 浮在上面），`window.innerHeight` 因此含标题栏；原生覆盖层是外壳里
    /// 标题栏**下面**的一层（`shell.rs`，标题栏仍在），可视区只有 `viewport_size` 减掉标题栏。
    /// 所以这里按覆盖层自己的区域算：全屏时正好铺满，非全屏时留出原版的 96 / 88 边距，
    /// 否则全屏那张图会溢出容器 36px（上下各露 18px 到标题栏和屏幕外）。
    /// 窗口视口用 `window.viewport_size()`（含标题栏，与原版 `window.innerHeight` 同口径）。
    fn stage_size(&self, window: &Window) -> (f32, f32) {
        let size = window.viewport_size();
        let area_w = size.width.as_f32().max(1.0);
        let area_h = (size.height.as_f32() - crate::theme::TITLEBAR_HEIGHT).max(1.0);
        let (avail_w, avail_h) = if window.is_fullscreen() {
            (area_w, area_h)
        } else {
            ((area_w - 96.0).max(1.0), (area_h - 88.0).max(1.0))
        };
        if self.stretch {
            return (avail_w, avail_h);
        }
        let mut width = avail_w;
        let mut height = width * self.aspect;
        if height > avail_h {
            height = avail_h;
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
                    cx.listener(|this, _event, _window, cx| {
                        // 刚滑动过的那一下会跟一次点击，吞掉它（原版 `handleStageClick` 的 `didSwipe`）。
                        if this.did_swipe {
                            this.did_swipe = false;
                            return;
                        }
                        this.step(1, cx);
                    }),
                )
                .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, _window, cx| {
                    this.on_swipe(event, cx)
                }))
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
        // 全屏时控制条浮在舞台底部（原版 `.pres-controls--floating`）：不占位、半透明底、
        // 默认透明且下移 8px，鼠标停在控制条上才淡入上移（0.3s，见下面的 `with_animation`）。
        let floating = window.is_fullscreen();
        let hovered = self.controls_hovered;
        let mut bar = div()
            .id("pres-controls")
            .flex()
            .flex_row()
            .items_center()
            .gap(px(16.0))
            .px(px(16.0));
        bar = if floating {
            bar.h(px(CONTROLS_H))
                .absolute()
                .bottom_0()
                .left_0()
                .right_0()
                // 盖住舞台下沿：点控制条不该翻页（原版 `.pres-controls @click.stop`，
                // 用 `occlude` 把下面舞台的命中也挡住）。
                .occlude()
                .bg(c.bg.opacity(CONTROLS_BG_ALPHA))
                .border_t_1()
                .border_color(c.border.opacity(CONTROLS_BORDER_ALPHA))
                // 悬停判断挂在控制条自己身上（不是整个舞台）；键盘输入（←/→ 翻页）之后
                // 鼠标没动也保持显示，与原版 CSS `:hover` 一致。
                .hover_listener_mode(gpui_kit::HoverListenerMode::InputModalityIndependent)
                .on_hover(cx.listener(|this, hovered: &bool, _window, cx| {
                    if this.controls_hovered != *hovered {
                        this.controls_hovered = *hovered;
                        cx.notify();
                    }
                }))
        } else {
            bar.flex_none()
                .h(px(CONTROLS_H))
                .bg(c.panel)
                .border_t_1()
                .border_color(c.border)
        };
        let controls = bar
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

        // 浮动控制条的淡入淡出：`with_animation` 的动画状态按元素 id 存取、只跑一次，
        // 所以用「悬停与否」拼进 id——状态一变就是新元素，动画从 0 重新开始
        // （进入 0 → 1、退出 1 → 0），`Animation` 本身不需要 hover 相关的钩子。
        let controls: gpui_kit::AnyElement = if floating {
            controls
                .with_animation(
                    ("pres-controls-float", usize::from(hovered)),
                    Animation::new(CONTROLS_FADE),
                    move |bar, delta| {
                        // 原版：默认 `opacity: 0; translateY(8px)`，悬停 `opacity: 1; translateY(0)`。
                        let (opacity, lift) = if hovered {
                            (delta, 1.0 - delta)
                        } else {
                            (1.0 - delta, delta)
                        };
                        // 绝对定位在底边，`bottom: -8px` 即原版的下移 8px。
                        bar.opacity(opacity).bottom(px(-CONTROLS_LIFT * lift))
                    },
                )
                .into_any_element()
        } else {
            controls.into_any_element()
        };

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
            // 全屏时控制条 `absolute` 贴在底边，相对本层定位。
            .relative()
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
///
/// 原版里它们是舞台的**兄弟**节点（`.pres-stage` 与 `.pres-nav` 平级），点箭头不会触发
/// 「点舞台下一页」；原生版把箭头放在舞台区域内（要跟舞台一起居中），所以箭头自己
/// `occlude`：点箭头时舞台的 `on_mouse_down` 收不到（否则左箭头 −1 加 +1 抵掉、右箭头 +2）。
fn nav_arrow(
    id: &'static str,
    icon: &str,
    left: bool,
    cx: &App,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    let c = cx.studio().colors;
    let button = div()
        .id(id)
        .occlude()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn gesture(events: &[(TouchPhase, f32, f32)]) -> Swipe {
        let mut swipe = Swipe::default();
        for (phase, dx, dy) in events {
            swipe.push(*phase, (*dx, *dy));
        }
        swipe
    }

    /// 鼠标滚轮恒为 `Moved`（没有 `Started`）：滚多少都不翻页。
    #[test]
    fn mouse_wheel_never_swipes() {
        let mut swipe = Swipe::default();
        for _ in 0..20 {
            swipe.push(TouchPhase::Moved, (-90.0, 0.0));
        }
        assert_eq!(swipe.take(), None);
        assert!(!swipe.active);
    }

    /// 左滑 65px、纵向只 1px → 下一页。
    #[test]
    fn horizontal_travel_pages_forward() {
        let swipe = gesture(&[
            (TouchPhase::Started, -20.0, 4.0),
            (TouchPhase::Moved, -25.0, -3.0),
            (TouchPhase::Moved, -20.0, 0.0),
        ]);
        assert_eq!(swipe.dx, -65.0);
        assert_eq!(swipe.dy, 1.0);
        assert_eq!(
            gesture(&[(TouchPhase::Started, -60.0, 0.0)]).take(),
            Some(1)
        );
    }

    /// 右滑 → 上一页。
    #[test]
    fn horizontal_travel_pages_backward() {
        let mut swipe = gesture(&[
            (TouchPhase::Started, 30.0, 0.0),
            (TouchPhase::Moved, 30.0, 5.0),
        ]);
        assert_eq!(swipe.take(), Some(-1));
        // `take` 复位：状态清空、位移清零、再判定没有结果。
        assert!(!swipe.active);
        assert_eq!((swipe.dx, swipe.dy), (0.0, 0.0));
        assert_eq!(swipe.take(), None);
    }

    /// 纵向为主、位移不够 50px、以及从没开始过的手势都不翻页。
    #[test]
    fn vertical_and_short_gestures_do_not_page() {
        assert_eq!(
            gesture(&[
                (TouchPhase::Started, 10.0, -80.0),
                (TouchPhase::Moved, 5.0, -60.0)
            ])
            .take(),
            None
        );
        assert_eq!(
            gesture(&[
                (TouchPhase::Started, 30.0, 0.0),
                (TouchPhase::Moved, 15.0, 0.0)
            ])
            .take(),
            None
        );
        // 恰好 50px：原版是「大于 50」。
        assert_eq!(gesture(&[(TouchPhase::Started, -50.0, 0.0)]).take(), None);
        assert_eq!(Swipe::default().take(), None);
    }

    /// 手势结束时（`Ended`，或被系统收走的 `Cancelled`）都要判定并复位：
    /// `push` 不管结束阶段，结束一律由 `take` 处理（`on_swipe` 在两个阶段上都调它）。
    #[test]
    fn gestures_settle_on_end_or_cancel() {
        let mut ended = gesture(&[(TouchPhase::Started, -80.0, 2.0)]);
        assert_eq!(ended.take(), Some(1));
        assert!(!ended.active);

        let mut cancelled = gesture(&[(TouchPhase::Started, -80.0, 2.0)]);
        cancelled.push(TouchPhase::Cancelled, (0.0, 0.0)); // 结束阶段不再累计位移
        assert_eq!(cancelled.dx, -80.0);
        assert_eq!(cancelled.take(), Some(1));
        assert!(!cancelled.active);
        assert_eq!((cancelled.dx, cancelled.dy), (0.0, 0.0));
    }
}
