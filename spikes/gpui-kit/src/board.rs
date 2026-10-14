//! B. The slide board: a virtualized thumbnail list on the left, the selected page
//! large on the right, a Typst theme dropdown, and a single-line editor for the
//! selected page's `title` that re-renders the large image live.
//!
//! Every raster runs on GPUI's background executor; the UI thread only turns finished
//! BGRA buffers into `RenderImage`s. The renderer itself is built once per process
//! (`deck::renderer`). One request is in flight at a time and changes that arrive
//! meanwhile are merged into the next round, which is the `serial` rule the Tauri
//! app's `useTypstThumbnails` uses.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui_kit::base::{VirtualListScrollHandle, v_virtual_list};
use gpui_kit::component::button::Button;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::select::{SearchableVec, Select, SelectEvent, SelectState};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{ActiveTheme as _, IndexPath};
use gpui_kit::prelude::*;
use gpui_kit::{
    Context, Entity, ObjectFit, RenderImage, ScrollStrategy, SharedString, Subscription, Window, div,
    img, px, size,
};
use serde_json::Value;
use slide_typst::SUPPORTED_THEMES;

use crate::deck::{self, Transfer};
use crate::image_bridge::to_render_image;
use crate::metrics;

/// Sidebar width and the width a thumbnail is drawn at, in CSS px.
const SIDEBAR_W: f32 = 300.0;
const THUMB_W: f32 = 248.0;
/// Room left for the stage's padding and the toolbar.
const STAGE_PADDING: f32 = 48.0;
/// Everything above and below the stage: title bar, tab strip, toolbar, title box,
/// status line and their paddings. Measured on the running window; the stage is
/// fitted into what is left, so the window does not need to be measured per frame.
const CHROME_H: f32 = 258.0;

/// Key of the frame the stage currently shows. A frame is only installed when its key
/// still matches the state, so a slow render of an older title or page can never
/// overwrite a newer one.
#[derive(Clone, PartialEq, Eq)]
struct StageKey {
    theme: usize,
    width: u32,
    index: usize,
    title: String,
}

/// One rasterized batch: pages plus where its time went.
struct Outcome {
    pages: Vec<(usize, Arc<RenderImage>)>,
    timing: deck::Timing,
    /// Device-pixel width the pages were rasterized at (the 64 px bucket).
    width: u32,
}

/// B2: rounds of both transfer modes, alternating so both see the same compile cache.
struct TransferBench {
    round: usize,
    samples: Vec<(Transfer, f64, f64)>,
}

/// B3: driving the title box the way typing does, one batch of keystrokes per round.
struct EditBench {
    target: String,
    started: Instant,
    samples: Vec<f64>,
    typed: usize,
}

const BENCH_ROUNDS: usize = 10;
const EDIT_BENCH_ROUNDS: usize = 10;
const EDIT_BENCH_KEYSTROKES: usize = 6;
const EDIT_BENCH_GAP_MS: u64 = 30;
/// E: how long the memory soak keeps editing the title, and how often it reports.
/// Overridable from the command line with `--soak-seconds`.
const SOAK_SECONDS: u64 = 120;
const SOAK_TICK_MS: u64 = 50;
const SOAK_REPORT_EVERY_S: u64 = 30;
/// Keep the soaked title short: every this many ticks it is reset to the original.
const SOAK_RESET_TICKS: u64 = 40;

pub struct Board {
    deck: Vec<Value>,
    deck_path: PathBuf,
    epoch: u64,
    theme_index: usize,
    transfer: Transfer,
    selected: usize,
    aspect_ratio: String,

    thumbs: HashMap<usize, Arc<RenderImage>>,
    thumb_key: Option<(usize, u32)>,
    thumb_width: u32,
    stage: Option<Arc<RenderImage>>,
    stage_key: Option<StageKey>,
    stage_width: u32,
    /// Height / width of the stage raster, taken from the frame itself so a 32:9 deck
    /// gets a 32:9 box.
    stage_ratio: f32,

    in_flight: bool,
    dirty: bool,
    /// The "everything is on screen" memory sample is reported once.
    reported_ready: bool,
    status: String,
    failure: Option<String>,
    transfer_bench: Option<TransferBench>,
    edit_bench: Option<EditBench>,

    title_text: String,
    title: Entity<InputState>,
    theme_select: Entity<SelectState<SearchableVec<&'static str>>>,
    scroll: VirtualListScrollHandle,
    /// Dropping a subscription cancels it, so they are kept for the view's lifetime.
    #[allow(dead_code)]
    subscriptions: Vec<Subscription>,
}

impl Board {
    pub fn new(deck_path: Option<PathBuf>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let deck_path = deck_path.unwrap_or_else(deck::default_deck_path);
        let deck = deck::load_deck(&deck_path).unwrap_or_else(|err| {
            eprintln!("[spike] cannot load {}: {err}", deck_path.display());
            Vec::new()
        });
        let title_text = slide_title(&deck, 0);
        let title = cx.new(|cx| InputState::new(window, cx).default_value(title_text.clone()));
        // `SelectState::new` needs a real window, so both entities are built here.
        let theme_select = cx.new(|cx| {
            SelectState::new(
                SearchableVec::new(SUPPORTED_THEMES.to_vec()),
                Some(IndexPath::new(0)),
                window,
                cx,
            )
        });
        let mut subscriptions = Vec::new();
        // `subscribe_in` rather than `subscribe`: the callbacks need the window to be
        // able to release superseded bitmaps (`App::drop_image`).
        subscriptions.push(cx.subscribe_in(
            &title,
            window,
            |this: &mut Board,
             state: &Entity<InputState>,
             event: &InputEvent,
             window,
             cx| {
                if matches!(event, InputEvent::Change) {
                    let text = state.read(cx).value().to_string();
                    this.set_title(text, window, cx);
                }
            },
        ));
        subscriptions.push(cx.subscribe_in(
            &theme_select,
            window,
            |this: &mut Board,
             _state: &Entity<SelectState<SearchableVec<&'static str>>>,
             event: &SelectEvent<SearchableVec<&'static str>>,
             window,
             cx| {
                if let SelectEvent::Confirm(Some(theme)) = event {
                    if let Some(index) = SUPPORTED_THEMES.iter().position(|it| it == theme) {
                        this.theme_index = index;
                        this.pump(window, cx);
                    }
                }
            },
        ));
        // The stage follows the window, so its raster width does too.
        subscriptions.push(cx.observe_window_bounds(window, |this, window, cx| {
            this.recompute_widths(window);
            this.pump(window, cx);
        }));

        let mut board = Self {
            deck,
            deck_path,
            epoch: 0,
            theme_index: 0,
            transfer: Transfer::Pixels,
            selected: 0,
            aspect_ratio: "ratio_16x9".to_string(),
            thumbs: HashMap::new(),
            thumb_key: None,
            thumb_width: 0,
            stage: None,
            stage_key: None,
            stage_width: 0,
            stage_ratio: 9.0 / 16.0,
            in_flight: false,
            dirty: false,
            reported_ready: false,
            status: String::new(),
            failure: None,
            transfer_bench: None,
            edit_bench: None,
            title_text,
            title,
            theme_select,
            scroll: VirtualListScrollHandle::new(),
            subscriptions,
        };
        board.recompute_widths(window);
        board.pump(window, cx);
        board
    }

    pub fn deck(&self) -> &[Value] {
        &self.deck
    }

    pub fn theme_name(&self) -> &'static str {
        SUPPORTED_THEMES[self.theme_index]
    }

    pub fn aspect_ratio(&self) -> &str {
        &self.aspect_ratio
    }

    /// Device-pixel widths for both surfaces: display width × scale factor, which is
    /// what a 150 % display needs to stay sharp, rounded up to the 64 px bucket.
    fn recompute_widths(&mut self, window: &mut Window) {
        let scale = window.scale_factor();
        let (stage_box_w, _) = stage_box(window, self.stage_ratio);
        self.thumb_width = deck::bucket_width(THUMB_W * scale);
        self.stage_width = deck::bucket_width(stage_box_w * scale);
    }

    pub fn set_theme(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.theme_index == index {
            return;
        }
        self.theme_index = index;
        // Keep the dropdown in step when the theme came from the command line.
        self.theme_select.update(cx, |state, cx| {
            state.set_selected_index(Some(IndexPath::new(index)), window, cx)
        });
        // Keys change; cached frames stay on screen until the new ones land, which is
        // what the Tauri app does too (nothing is cleared on a theme switch).
        self.pump(window, cx);
    }

    fn set_title(&mut self, text: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.title_text == text {
            return;
        }
        self.title_text = text.clone();
        if let Some(slide) = self.deck.get_mut(self.selected) {
            slide["title"] = Value::String(text);
        }
        // The board shows the same page as the stage, so its thumbnail is stale too.
        // Releasing it here is what keeps the window's sprite atlas from growing with
        // every keystroke (see the report, E).
        if let Some(old) = self.thumbs.remove(&self.selected) {
            cx.drop_image(old, Some(window));
        }
        self.pump(window, cx);
    }

    pub fn select(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.deck.len() || index == self.selected {
            return;
        }
        eprintln!("[spike] select page {index}");
        self.selected = index;
        // Same rule as Step3: the preview and the title box follow the page.
        self.title_text = slide_title(&self.deck, index);
        let text = self.title_text.clone();
        self.title
            .update(cx, |state, cx| state.set_value(text, window, cx));
        self.pump(window, cx);
    }

    /// One request at a time; changes arriving meanwhile are merged into the next round.
    fn pump(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.in_flight {
            self.dirty = true;
            return;
        }
        let thumb_key = (self.theme_index, self.thumb_width);
        if self.thumb_key != Some(thumb_key) {
            for (_, image) in self.thumbs.drain() {
                cx.drop_image(image, Some(window));
            }
            self.thumb_key = Some(thumb_key);
        }
        let missing: Vec<usize> = (0..self.deck.len())
            .filter(|index| !self.thumbs.contains_key(index))
            .collect();
        if self.deck.is_empty() || (missing.is_empty() && self.stage_is_current()) {
            self.dirty = false;
            return;
        }
        let want_stage = !self.stage_is_current();
        let stage_key = self.requested_stage_key();
        let deck = self.deck.clone();
        let theme = self.theme_name();
        let ratio = self.aspect_ratio.clone();
        let transfer = self.transfer;
        let thumb_width = self.thumb_width;
        let stage_width = self.stage_width;
        let selected = self.selected;
        let epoch = self.epoch;
        let started = Instant::now();
        self.in_flight = true;
        self.failure = None;

        cx.spawn_in(window, async move |this, window| {
            let computed = window
                .background_spawn(async move {
                    let thumbs = if missing.is_empty() {
                        None
                    } else {
                        Some(rasterize(
                            &deck,
                            theme,
                            &ratio,
                            thumb_width,
                            Some(&missing),
                            transfer,
                        ))
                    };
                    let stage = if want_stage {
                        Some(rasterize(
                            &deck,
                            theme,
                            &ratio,
                            stage_width,
                            Some(&[selected]),
                            transfer,
                        ))
                    } else {
                        None
                    };
                    (thumbs, stage)
                })
                .await;
            let _ = this.update_in(window, |board, window, cx| {
                board.finish(computed, epoch, started, stage_key, window, cx)
            });
        })
        .detach();
    }

    fn requested_stage_key(&self) -> StageKey {
        StageKey {
            theme: self.theme_index,
            width: self.stage_width,
            index: self.selected,
            title: self.title_text.clone(),
        }
    }

    fn stage_is_current(&self) -> bool {
        self.stage.is_some() && self.stage_key.as_ref() == Some(&self.requested_stage_key())
    }

    fn finish(
        &mut self,
        computed: (Option<Result<Outcome, String>>, Option<Result<Outcome, String>>),
        epoch: u64,
        started: Instant,
        stage_key: StageKey,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.in_flight = false;
        if epoch != self.epoch {
            // The deck was replaced while this batch was in flight; its frames are stale.
            cx.notify();
            self.pump(window, cx);
            return;
        }
        let mut notes = Vec::new();
        match computed.0 {
            Some(Ok(outcome)) => {
                let width = outcome.width;
                for (index, image) in outcome.pages {
                    // Replacements (the page being edited) hand their old bitmap back to
                    // GPUI, which frees the atlas tile with it.
                    if let Some(old) = self.thumbs.insert(index, image) {
                        cx.drop_image(old, Some(window));
                    }
                }
                notes.push(describe(
                    &format!("缩略图 {} 页", self.thumbs.len()),
                    &outcome.timing,
                    width,
                ));
            }
            Some(Err(err)) => self.failure = Some(err),
            None => {}
        }
        match computed.1 {
            Some(Ok(outcome)) => {
                let width = outcome.width;
                if let Some((_, image)) = outcome.pages.into_iter().next() {
                    let size = image.size(0);
                    if size.width.0 > 0 {
                        self.stage_ratio = size.height.0 as f32 / size.width.0 as f32;
                    }
                    if let Some(old) = self.stage.replace(image) {
                        cx.drop_image(old, Some(window));
                    }
                    self.stage_key = Some(stage_key.clone());
                }
                notes.push(describe("大图", &outcome.timing, width));
                if let Some(bench) = self.edit_bench.as_mut() {
                    if bench.target == stage_key.title {
                        let latency = started.elapsed().as_secs_f64() * 1000.0;
                        bench.samples.push(latency);
                        eprintln!("[time] 标题输入 → 画面就绪 {latency:.1} ms");
                    }
                }
            }
            Some(Err(err)) => self.failure = Some(err),
            None => {}
        }
        self.status = if notes.is_empty() {
            "已是最新".to_string()
        } else {
            notes.join(" · ")
        };
        cx.notify();
        if let Some(bench) = self.edit_bench.as_ref() {
            if bench.samples.len() >= EDIT_BENCH_ROUNDS {
                metrics::time_median("B3 标题编辑（末端按键 → 画面就绪）", &bench.samples);
                self.edit_bench = None;
            }
        }
        if !self.reported_ready && !self.deck.is_empty() && self.thumbs.len() == self.deck.len() {
            self.reported_ready = true;
            metrics::report(&format!("{} 页缩略图 + 大图就绪", self.deck.len()));
        }
        if self.dirty {
            self.pump(window, cx);
        }
    }

    /// B2: five rounds per transfer mode, alternating, each measuring request → the
    /// frames being available to draw.
    pub fn start_transfer_bench(&mut self, cx: &mut Context<Self>) {
        self.transfer_bench = Some(TransferBench {
            round: 0,
            samples: Vec::new(),
        });
        self.run_bench_round(cx);
    }

    fn run_bench_round(&mut self, cx: &mut Context<Self>) {
        let Some(bench) = self.transfer_bench.as_ref() else {
            return;
        };
        let mode = if bench.round % 2 == 0 {
            Transfer::Png
        } else {
            Transfer::Pixels
        };
        let round = bench.round;
        let deck = self.deck.clone();
        let theme = self.theme_name();
        let ratio = self.aspect_ratio.clone();
        let thumb_width = self.thumb_width;
        let stage_width = self.stage_width;
        let selected = self.selected;
        self.status = format!("{} 基准测试，第 {} / {BENCH_ROUNDS} 轮…", mode.label(), round + 1);
        cx.notify();
        cx.spawn(async move |this, cx| {
            let measured = cx
                .background_spawn(async move {
                    let started = Instant::now();
                    let thumbs = rasterize(&deck, theme, &ratio, thumb_width, None, mode);
                    let thumb_total = started.elapsed();
                    let started = Instant::now();
                    let stage =
                        rasterize(&deck, theme, &ratio, stage_width, Some(&[selected]), mode);
                    let stage_total = started.elapsed();
                    (thumbs, thumb_total, stage, stage_total, mode)
                })
                .await;
            let _ = this.update(cx, |board, cx| board.finish_bench_round(measured, cx));
        })
        .detach();
    }

    fn finish_bench_round(
        &mut self,
        measured: (
            Result<Outcome, String>,
            Duration,
            Result<Outcome, String>,
            Duration,
            Transfer,
        ),
        cx: &mut Context<Self>,
    ) {
        let (thumbs, thumb_total, stage, stage_total, mode) = measured;
        let mut note = match &thumbs {
            Ok(outcome) => format!(
                "整副 {} 页 {:.0} ms（其中渲染 {:.0} / 传递 {:.0}）",
                outcome.pages.len(),
                thumb_total.as_secs_f64() * 1000.0,
                outcome.timing.render.as_secs_f64() * 1000.0,
                outcome.timing.transfer.as_secs_f64() * 1000.0
            ),
            Err(err) => format!("缩略图失败：{err}"),
        };
        match &stage {
            Ok(outcome) => note.push_str(&format!(
                " · 单页 {:.0} ms（渲染 {:.0} / 传递 {:.0}）",
                stage_total.as_secs_f64() * 1000.0,
                outcome.timing.render.as_secs_f64() * 1000.0,
                outcome.timing.transfer.as_secs_f64() * 1000.0
            )),
            Err(err) => note.push_str(&format!(" · 大图失败：{err}")),
        }
        if let Some(bench) = self.transfer_bench.as_mut() {
            bench.samples.push((
                mode,
                thumb_total.as_secs_f64() * 1000.0,
                stage_total.as_secs_f64() * 1000.0,
            ));
            bench.round += 1;
            if bench.round >= BENCH_ROUNDS {
                let pick = |mode: Transfer, stage: bool| -> Vec<f64> {
                    bench
                        .samples
                        .iter()
                        .filter(|(sample, _, _)| *sample == mode)
                        .map(|(_, thumbs, single)| if stage { *single } else { *thumbs })
                        .collect()
                };
                metrics::time_median("B2 整副 18 页 · render_png + 解码", &pick(Transfer::Png, false));
                metrics::time_median("B2 整副 18 页 · render_pixels", &pick(Transfer::Pixels, false));
                metrics::time_median("B2 单页大图 · render_png + 解码", &pick(Transfer::Png, true));
                metrics::time_median("B2 单页大图 · render_pixels", &pick(Transfer::Pixels, true));
                metrics::report("位图基准之后");
                self.transfer_bench = None;
                note.push_str(" · 基准完成，数字见日志");
            }
        }
        self.status = note;
        cx.notify();
        if self.transfer_bench.is_some() {
            self.run_bench_round(cx);
        }
    }

    /// B3: ten rounds of six keystrokes 30 ms apart; each round measures the last
    /// keystroke → the frame for the text it produced. This goes through the same
    /// change handler the input box drives, so coalescing behaves exactly as when
    /// typing; the IME path itself is on the manual checklist.
    pub fn start_edit_bench(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.edit_bench = Some(EditBench {
            target: self.title_text.clone(),
            started: Instant::now(),
            samples: Vec::new(),
            typed: 0,
        });
        cx.spawn_in(window, async move |this, window| {
            for _ in 0..EDIT_BENCH_ROUNDS {
                for _ in 0..EDIT_BENCH_KEYSTROKES {
                    window
                        .background_executor()
                        .timer(Duration::from_millis(EDIT_BENCH_GAP_MS))
                        .await;
                    let _ = this.update_in(window, |board, window, cx| {
                        board.type_bench_char(window, cx)
                    });
                }
                window
                    .background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
            }
        })
        .detach();
    }

    fn type_bench_char(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.edit_bench.as_mut() else {
            return;
        };
        bench.typed += 1;
        let last = bench.typed % EDIT_BENCH_KEYSTROKES == 0;
        let next = format!("{}·", self.title_text);
        self.title_text = next.clone();
        if let Some(slide) = self.deck.get_mut(self.selected) {
            slide["title"] = Value::String(next.clone());
        }
        self.thumbs.remove(&self.selected);
        if last {
            bench.target = next.clone();
            bench.started = Instant::now();
        }
        self.title
            .update(cx, |state, cx| state.set_value(next, window, cx));
        self.pump(window, cx);
    }

    /// E: 120 seconds of continuous title editing while the working set is reported
    /// every 30 s; this is what the memory numbers in the report come from.
    pub fn start_soak(&mut self, window: &mut Window, cx: &mut Context<Self>, seconds: u64) {
        let base = self.title_text.clone();
        self.status = format!("连续编辑标题 {seconds} 秒，每 {SOAK_REPORT_EVERY_S} 秒报一次内存…");
        cx.notify();
        cx.spawn_in(window, async move |this, window| {
            let started = Instant::now();
            let mut ticks = 0u64;
            let mut next_report = SOAK_REPORT_EVERY_S;
            while started.elapsed() < Duration::from_secs(seconds) {
                window
                    .background_executor()
                    .timer(Duration::from_millis(SOAK_TICK_MS))
                    .await;
                ticks += 1;
                let reset = ticks % SOAK_RESET_TICKS == 0;
                let base = base.clone();
                let _ = this.update_in(window, |board, window, cx| {
                    board.soak_tick(&base, reset, window, cx)
                });
                let elapsed = started.elapsed().as_secs();
                if elapsed >= next_report {
                    metrics::report(&format!("连续编辑标题 {elapsed} 秒"));
                    next_report += SOAK_REPORT_EVERY_S;
                }
            }
            metrics::report(&format!("连续编辑标题 {seconds} 秒后"));
            let _ = this.update_in(window, |board, _window, cx| {
                board.status = format!("内存采样结束（{ticks} 次改动）");
                cx.notify();
            });
        })
        .detach();
    }

    /// E: the same 2-minute loop as [`Board::start_soak`], but without Typst: every tick
    /// installs a fresh `RenderImage` of the stage size and releases the previous one.
    /// This is the smallest reproduction of the growth the soak shows.
    pub fn start_paint_soak(&mut self, window: &mut Window, cx: &mut Context<Self>, seconds: u64) {
        let width = self.stage_width;
        let height = (width as f32 * self.stage_ratio) as u32;
        self.status = format!("只画不发请求：{width}×{height}，每 50 ms 一帧");
        cx.notify();
        cx.spawn_in(window, async move |this, window| {
            let started = Instant::now();
            let mut ticks = 0u64;
            let mut next_report = SOAK_REPORT_EVERY_S;
            while started.elapsed() < Duration::from_secs(seconds) {
                window
                    .background_executor()
                    .timer(Duration::from_millis(SOAK_TICK_MS))
                    .await;
                ticks += 1;
                let _ = this.update_in(window, |board, window, cx| {
                    board.paint_tick(width, height, ticks, window, cx)
                });
                let elapsed = started.elapsed().as_secs();
                if elapsed >= next_report {
                    metrics::report(&format!("只画不发请求 {elapsed} 秒"));
                    next_report += SOAK_REPORT_EVERY_S;
                }
            }
            metrics::report(&format!("只画不发请求 {seconds} 秒后"));
            let _ = this.update_in(window, |board, _window, cx| {
                board.status = format!("只画不发请求结束（{ticks} 帧）");
                cx.notify();
            });
        })
        .detach();
    }

    /// One synthetic frame: stage-sized BGRA with a single changing byte, so nothing
    /// can be deduplicated or cached by content.
    fn paint_tick(&mut self, width: u32, height: u32, tick: u64, window: &mut Window, _cx: &mut Context<Self>) {
        let mut bgra = vec![0u8; (width * height * 4) as usize];
        bgra[0] = (tick % 251) as u8;
        let image = to_render_image(deck::Frame {
            width,
            height,
            bgra,
        });
        if let Some(old) = self.stage.replace(image) {
            // `cx` is the view context here; `drop_image` lives on `App`.
            _cx.drop_image(old, Some(window));
        }
        // `stage_key` is left alone: the key still describes the last real render, so
        // the normal pipeline sees no reason to request anything.
        _cx.notify();
    }

    fn soak_tick(&mut self, base: &str, reset: bool, window: &mut Window, cx: &mut Context<Self>) {        let next = if reset {
            base.to_string()
        } else if self.title_text.ends_with('·') {
            self.title_text.clone()
        } else {
            format!("{}·", self.title_text)
        };
        self.title_text = next.clone();
        if let Some(slide) = self.deck.get_mut(self.selected) {
            slide["title"] = Value::String(next.clone());
        }
        self.thumbs.remove(&self.selected);
        self.title
            .update(cx, |state, cx| state.set_value(next, window, cx));
        self.pump(window, cx);
    }

    fn open_deck(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        cx.spawn_in(window, async move |this, window| {
            let picked = rfd::AsyncFileDialog::new()
                .add_filter("slide deck json", &["json"])
                .pick_file()
                .await;
            let Some(file) = picked else {
                return;
            };
            let path = file.path().to_path_buf();
            let loaded = window
                .background_spawn(async move { deck::load_deck(&path) })
                .await;
            let _ = this.update_in(window, |board, window, cx| match loaded {
                Ok(deck) => board.set_deck(deck, window, cx),
                Err(err) => {
                    board.failure = Some(err);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn set_deck(&mut self, deck: Vec<Value>, window: &mut Window, cx: &mut Context<Self>) {
        self.deck = deck;
        self.selected = 0;
        self.epoch += 1;
        self.reported_ready = false;
        self.thumbs.clear();
        self.thumb_key = None;
        self.stage = None;
        self.stage_key = None;
        self.title_text = slide_title(&self.deck, 0);
        let text = self.title_text.clone();
        self.title
            .update(cx, |state, cx| state.set_value(text, window, cx));
        self.status = format!("已载入 {} 页", self.deck.len());
        self.scroll.scroll_to_item(0, ScrollStrategy::Top);
        self.pump(window, cx);
    }
}

impl Render for Board {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let deck_len = self.deck.len();
        let thumb_h = THUMB_W * 9.0 / 16.0;
        let row_h = thumb_h + 26.0;
        let sizes = std::rc::Rc::new(vec![size(px(THUMB_W), px(row_h)); deck_len]);
        let theme = self.theme_name().to_string();
        let transfer = self.transfer;
        let status = self.status.clone();
        let failure = self.failure.clone();
        let (stage_box_w, stage_box_h) = stage_box(window, self.stage_ratio);
        let stage = self.stage.clone();

        let list = v_virtual_list(
            cx.entity(),
            "thumbnails",
            sizes,
            move |board: &mut Board, range: std::ops::Range<usize>, _window, cx| {
                range
                    .map(|index| {
                        let selected = index == board.selected;
                        let frame = board.thumbs.get(&index).cloned();
                        let label = format!(
                            "{:02} · {}",
                            index + 1,
                            board
                                .deck
                                .get(index)
                                .and_then(|slide| slide["kind"].as_str())
                                .unwrap_or("?")
                        );
                        let row = div()
                            .id(("thumb", index))
                            .flex()
                            .flex_col()
                            .gap_1()
                            .w_full()
                            .py_1()
                            .pl_2()
                            .child(match frame {
                                Some(image) => div()
                                    .w(px(THUMB_W))
                                    .h(px(thumb_h))
                                    .child(img(image).w_full().h_full()),
                                // No frame yet: the same empty box the Tauri app shows
                                // instead of a blank (its `SlideFrame` pending state).
                                None => div()
                                    .w(px(THUMB_W))
                                    .h(px(thumb_h))
                                    .bg(cx.theme().muted),
                            })
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(if selected {
                                        cx.theme().primary
                                    } else {
                                        cx.theme().muted_foreground
                                    })
                                    .child(SharedString::from(label)),
                            )
                            .on_click(cx.listener(move |board, _event, window, cx| {
                                board.select(index, window, cx)
                            }));
                        if selected {
                            row.bg(cx.theme().accent)
                        } else {
                            row
                        }
                    })
                    .collect::<Vec<_>>()
            },
        )
        .track_scroll(&self.scroll);

        let stage_box = div()
            .w(px(stage_box_w))
            .h(px(stage_box_h))
            .when_some(stage, |element, image| {
                element.child(img(image).w_full().h_full().object_fit(ObjectFit::Fill))
            });

        div()
            .flex()
            .flex_row()
            .size_full()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .w(px(SIDEBAR_W))
                    .h_full()
                    .border_r_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .p_2()
                            .child(
                                div()
                                    .flex_1()
                                    .text_sm()
                                    .child(SharedString::from(format!(
                                        "{} 页 · {}",
                                        deck_len,
                                        self.deck_path
                                            .file_name()
                                            .map(|name| name.to_string_lossy().to_string())
                                            .unwrap_or_default()
                                    ))),
                            )
                            .child(
                                Button::new("open-deck")
                                    .label("打开牌组…")
                                    .on_click(cx.listener(|board, _event, window, cx| {
                                        board.open_deck(window, cx)
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            // Without this the virtual list paints the partially scrolled
                            // row above its own bounds, over the tab strip.
                            .overflow_hidden()
                            .child(list),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .h_full()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_3()
                            .p_2()
                            .child(
                                div()
                                    .w(px(230.0))
                                    .child(Select::new(&self.theme_select).id("typst-theme")),
                            )
                            .child(
                                Switch::new("transfer-mode")
                                    .label(if transfer == Transfer::Pixels {
                                        "render_pixels"
                                    } else {
                                        "render_png + 解码"
                                    })
                                    .checked(transfer == Transfer::Pixels)
                                    .on_change(cx.listener(|board, checked: &bool, window, cx| {
                                        let wanted = if *checked {
                                            Transfer::Pixels
                                        } else {
                                            Transfer::Png
                                        };
                                        if board.transfer != wanted {
                                            board.transfer = wanted;
                                            board.pump(window, cx);
                                        }
                                    })),
                            )
                            .child(Button::new("bench-b2").label("位图基准 B2").on_click(
                                cx.listener(|board, _event, _window, cx| {
                                    board.start_transfer_bench(cx)
                                }),
                            ))
                            .child(Button::new("bench-b3").label("输入延迟 B3").on_click(
                                cx.listener(|board, _event, window, cx| {
                                    board.start_edit_bench(window, cx)
                                }),
                            ))
                            .child(Button::new("soak").label("内存 2 分钟").on_click(
                                cx.listener(|board, _event, window, cx| {
                                    board.start_soak(window, cx, SOAK_SECONDS)
                                }),
                            ))
                            .child(div().flex_1())
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(SharedString::from(format!(
                                        "Typst 主题 {theme} · 缩略图 {} px · 大图 {} px",
                                        self.thumb_width, self.stage_width
                                    ))),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(stage_box),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .p_2()
                            .child(
                                div().text_xs().child(
                                    "选中页标题（输入即重渲；同一时刻只有一个请求在途）",
                                ),
                            )
                            .child(Input::new(&self.title).id("slide-title")),
                    )
                    .child(
                        div()
                            .px_2()
                            .pb_2()
                            .text_xs()
                            .text_color(if failure.is_some() {
                                cx.theme().danger
                            } else {
                                cx.theme().muted_foreground
                            })
                            .child(SharedString::from(match failure {
                                Some(err) => format!("渲染失败：{err}"),
                                None => status,
                            })),
                    ),
            )
    }
}

/// The stage box in CSS px: the deck's ratio fitted into what is left of the window.
fn stage_box(window: &Window, ratio: f32) -> (f32, f32) {
    let bounds = window.bounds().size;
    let available_w = (bounds.width.as_f32() - SIDEBAR_W - STAGE_PADDING).max(200.0);
    let available_h = (bounds.height.as_f32() - CHROME_H).max(160.0);
    let width = available_w.min(available_h / ratio.max(0.1));
    (width, width * ratio)
}

/// Rasterizes on the calling (background) thread and converts to GPUI images.
fn rasterize(
    deck: &[Value],
    theme: &str,
    ratio: &str,
    width: u32,
    only: Option<&[usize]>,
    transfer: Transfer,
) -> Result<Outcome, String> {
    let batch = deck::rasterize(deck, theme, ratio, width, only, transfer)?;
    Ok(Outcome {
        pages: batch
            .pages
            .into_iter()
            .map(|page| (page.index, to_render_image(page.frame)))
            .collect(),
        timing: batch.timing,
        width: batch.width,
    })
}

/// `名称 @宽度 · 合计 x ms（渲染 y / 传递 z）`, the shape the report quotes.
fn describe(name: &str, timing: &deck::Timing, width: u32) -> String {
    format!(
        "{name} @{width}px {:.0} ms（渲染 {:.0} / 传递 {:.0}）",
        timing.total().as_secs_f64() * 1000.0,
        timing.render.as_secs_f64() * 1000.0,
        timing.transfer.as_secs_f64() * 1000.0
    )
}

fn slide_title(deck: &[Value], index: usize) -> String {
    deck.get(index)
        .and_then(|slide| slide["title"].as_str())
        .unwrap_or("")
        .to_string()
}
