//! D. Presentation mode: a borderless, fullscreen window that shows one page at a
//! time, stretched to the screen like the Studio's `PresentationOverlay` (non-uniform
//! fill, so a 32:9 deck really looks squashed on a 16:9 monitor), prefetching the next
//! page. Arrow keys / space / page keys / mouse clicks advance, Escape closes.

use std::collections::HashMap;
use std::sync::Arc;

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::prelude::*;
use gpui_kit::{
    Bounds, Context, KeystrokeEvent, MouseButton, ObjectFit, RenderImage, SharedString,
    Subscription, Window, WindowBounds, WindowOptions, div, img, px, size,
};
use serde_json::Value;

use crate::deck::{self, Transfer};
use crate::image_bridge::to_render_image;

pub struct Present {
    deck: Arc<Vec<Value>>,
    theme: String,
    ratio: String,
    index: usize,
    width: u32,
    frames: HashMap<usize, Arc<RenderImage>>,
    in_flight: bool,
    dirty: bool,
    failure: Option<String>,
    #[allow(dead_code)]
    subscriptions: Vec<Subscription>,
}

impl Present {
    pub fn new(
        deck: Arc<Vec<Value>>,
        theme: String,
        ratio: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let width = deck::bucket_width(
            window.bounds().size.width.as_f32().max(640.0) * window.scale_factor(),
        );
        let subscriptions = vec![cx.observe_keystrokes(
            |this: &mut Present, event: &KeystrokeEvent, window: &mut Window, cx| {
                match event.keystroke.key.as_str() {
                    "right" | "space" | "down" | "pagedown" | "enter" => this.step(1, cx),
                    "left" | "up" | "pageup" => this.step(-1, cx),
                    "escape" => window.remove_window(),
                    _ => {}
                }
            },
        )];
        let mut present = Self {
            deck,
            theme,
            ratio,
            index: 0,
            width,
            frames: HashMap::new(),
            in_flight: false,
            dirty: false,
            failure: None,
            subscriptions,
        };
        present.pump(cx);
        present
    }

    /// Bounds of the presentation window: fullscreen on the display GPUI picks.
    pub fn window_options(cx: &mut gpui_kit::App) -> WindowOptions {
        WindowOptions {
            window_bounds: Some(WindowBounds::Fullscreen(Bounds::maximized(None, cx))),
            titlebar: None,
            window_min_size: Some(size(px(640.), px(360.))),
            ..Default::default()
        }
    }

    fn step(&mut self, delta: isize, cx: &mut Context<Self>) {
        let next = self.index as isize + delta;
        if next < 0 || next as usize >= self.deck.len() {
            return;
        }
        self.index = next as usize;
        self.pump(cx);
    }

    fn pump(&mut self, cx: &mut Context<Self>) {
        if self.in_flight {
            self.dirty = true;
            return;
        }
        // Current page first, then the next one: one request that includes four
        // full-screen neighbours delays the page in front of the audience.
        let mut wanted = Vec::new();
        if !self.frames.contains_key(&self.index) {
            wanted.push(self.index);
        } else if self.index + 1 < self.deck.len() && !self.frames.contains_key(&(self.index + 1)) {
            wanted.push(self.index + 1);
        }
        if wanted.is_empty() {
            self.dirty = false;
            return;
        }
        let deck = self.deck.clone();
        let theme = self.theme.clone();
        let ratio = self.ratio.clone();
        let width = self.width;
        self.in_flight = true;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    deck::rasterize(
                        &deck,
                        &theme,
                        &ratio,
                        width,
                        Some(&wanted),
                        Transfer::Pixels,
                    )
                })
                .await;
            let _ = this.update(cx, |present, cx| {
                present.in_flight = false;
                match result {
                    Ok(batch) => {
                        for page in batch.pages {
                            present
                                .frames
                                .insert(page.index, to_render_image(page.frame));
                        }
                        // Only the current page and its neighbour are worth keeping.
                        let keep = [present.index, present.index + 1];
                        present.frames.retain(|index, _| keep.contains(index));
                    }
                    Err(err) => present.failure = Some(err),
                }
                cx.notify();
                if present.dirty {
                    present.pump(cx);
                }
            });
        })
        .detach();
    }
}

impl Render for Present {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // The window can be resized (a 32:9 deck on another monitor); the raster width
        // follows it in 64 px steps.
        let wanted = deck::bucket_width(
            window.bounds().size.width.as_f32().max(640.0) * window.scale_factor(),
        );
        if wanted != self.width {
            self.width = wanted;
            self.frames.clear();
            self.pump(cx);
        }
        let frame = self.frames.get(&self.index).cloned();
        let label = format!("{} / {}", self.index + 1, self.deck.len());
        let failure = self.failure.clone();
        div()
            .id("presentation")
            .size_full()
            .bg(gpui_kit::rgb(0x000000))
            .when_some(frame, |element, image| {
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
                        Some(err) => format!("渲染失败：{err}"),
                        None => label,
                    })),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|present, _event, _window, cx| present.step(1, cx)),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|present, _event, _window, cx| present.step(-1, cx)),
            )
    }
}
