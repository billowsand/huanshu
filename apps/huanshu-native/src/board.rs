//! 项目看板：左侧缩略图虚拟列表，右侧当前页大图。
//!
//! 两个 [`PreviewCache`]：缩略图与大图各一个，规则完全相同（见 `huanshu_core::preview`）。
//! 位图宽度 = 显示宽度 × `window.scale_factor()`，按 64px 档位取整；窗口缩放后按新档位重渲。
//!
//! 加载与失败照 `components/SlideFrame.vue`：没有图时显示加载框（等待超过 300ms 才出现
//! 「渲染中…」）；失败时有旧图就继续显示旧图并给出重试，没有图就显示失败与重试按钮。

use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::base::{v_virtual_list, VirtualListScrollHandle};
use gpui_kit::component::button::Button;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::prelude::*;
use gpui_kit::{
    div, img, px, size, Context, Entity, KeystrokeEvent, ObjectFit, Render, RenderImage,
    ScrollStrategy, SharedString, Subscription, Window,
};
use huanshu_core::editor_schema;
use huanshu_core::preview::{PreviewCache, PreviewOptions, PreviewStyle};
use huanshu_core::services::settings;
use huanshu_core::themes;

use crate::app::{self, AppShared};
use crate::images::drop_all;
use crate::shell::Root;

/// 缩略图逻辑宽度（设备像素档位按它 × scale_factor 算）。
const THUMB_W: f32 = 168.0;
/// 大图栏的宽度上限（逻辑像素）。
const STAGE_MAX_W: f32 = 1_600.0;
/// 缩略图可见行之外多预取几行。
const THUMB_OVERSCAN: usize = 4;
/// 加载框要等这么久才出现「渲染中…」，免得翻页就闪。
const PENDING_DELAY: std::time::Duration = std::time::Duration::from_millis(300);

/// 看板的构造参数。
pub struct BoardArgs {
    pub project_id: i64,
    pub start_page: usize,
    pub theme: &'static str,
    pub shared: AppShared,
    pub root: Entity<Root>,
}

pub struct Board {
    args: BoardArgs,
    /// 牌组。空位（没有蓝图）保留为 `None`，位置仍参与页码。
    deck: Vec<Option<serde_json::Value>>,
    /// 版式中文名，来自 core 的编辑器模式表（不另写一份）。
    kind_labels: Vec<(String, String)>,
    project_name: String,
    pub selected: usize,
    theme: &'static str,
    aspect: f32,

    thumbs: PreviewCache<Arc<RenderImage>>,
    stage: PreviewCache<Arc<RenderImage>>,
    thumb_width: u32,
    stage_width: u32,
    visible: (usize, usize),

    pending_since: Option<std::time::Instant>,
    failure: Option<String>,
    exporting: bool,
    export_note: Option<String>,
    /// 上次导出的文件路径，供「在文件夹中显示」使用。
    export_path: Option<String>,
    scroll: VirtualListScrollHandle,
    #[allow(dead_code)]
    subscriptions: Vec<Subscription>,
}

impl Board {
    pub fn new(args: BoardArgs, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let project = huanshu_core::services::projects::get_project(&args.shared, args.project_id)
            .unwrap_or_else(|err| {
                eprintln!("[native] 打开项目失败：{err}");
                huanshu_core::db::Project {
                    id: args.project_id,
                    name: String::new(),
                    md_content: String::new(),
                    blueprints_json: String::new(),
                    media_root: None,
                    aspect_ratio: None,
                    created_at: 0,
                    updated_at: 0,
                }
            });
        let slides = app::load_deck(&args.shared, args.project_id).unwrap_or_default();
        let deck: Vec<Option<serde_json::Value>> = slides.into_iter().map(Some).collect();
        let kind_labels = editor_schema::editor_schema()
            .kinds
            .into_iter()
            .map(|entry| (entry.kind, entry.label))
            .collect();
        let selected = args.start_page.min(deck.len().saturating_sub(1));

        let board = Self {
            aspect: 9.0 / 16.0,
            project_name: project.name,
            deck,
            kind_labels,
            selected,
            theme: args.theme,
            thumbs: PreviewCache::new(PreviewOptions {
                max_cached: Some(64),
                ..PreviewOptions::default()
            }),
            stage: PreviewCache::new(PreviewOptions {
                max_cached: Some(4),
                ..PreviewOptions::default()
            }),
            thumb_width: 0,
            stage_width: 0,
            visible: (0, 0),
            pending_since: None,
            failure: None,
            exporting: false,
            export_note: None,
            export_path: None,
            scroll: VirtualListScrollHandle::new(),
            subscriptions: Vec::new(),
            args,
        };
        // ↑/↓ 换页。
        let subscriptions = vec![cx.observe_keystrokes(
            |board: &mut Board, event: &KeystrokeEvent, _window, _cx| match event
                .keystroke
                .key
                .as_str()
            {
                "down" | "pagedown" => board.step(1),
                "up" | "pageup" => board.step(-1),
                "home" => board.select_index(0),
                "end" => board.select_index(board.deck.len().saturating_sub(1)),
                _ => {}
            },
        )];
        Self {
            subscriptions,
            ..board
        }
    }

    pub fn page_count(&self) -> usize {
        self.deck.len()
    }

    /// 演示与导出要用的数据。
    pub fn deck_for_present(&self) -> (i64, Vec<serde_json::Value>, &'static str, AppShared) {
        (
            self.args.project_id,
            self.deck.iter().flatten().cloned().collect(),
            self.theme,
            self.args.shared.clone(),
        )
    }

    pub fn set_theme(&mut self, id: &'static str, cx: &mut Context<Self>) {
        if self.theme == id {
            return;
        }
        self.theme = id;
        if let Err(error) = settings::set_app_theme(&self.args.shared, id) {
            eprintln!("[native] 保存主题失败：{error}");
        }
        self.deck_changed(cx);
    }

    fn step(&mut self, delta: isize) {
        let next = self.selected as isize + delta;
        if next < 0 || next as usize >= self.deck.len() {
            return;
        }
        self.select_index(next as usize);
    }

    pub fn select_index(&mut self, index: usize) {
        if index >= self.deck.len() || index == self.selected {
            return;
        }
        self.selected = index;
        self.scroll.scroll_to_item(index, ScrollStrategy::Nearest);
    }

    fn kind_label(&self, index: usize) -> String {
        let Some(kind) = self.deck[index]
            .as_ref()
            .and_then(|slide| slide.get("kind"))
            .and_then(|kind| kind.as_str())
        else {
            return "—".to_string();
        };
        self.kind_labels
            .iter()
            .find(|(name, _)| *name == kind)
            .map(|(_, label)| label.clone())
            .unwrap_or_else(|| kind.to_string())
    }

    /// 牌组、主题或宽度变了：两个缓存都要重新评估。
    fn deck_changed(&mut self, cx: &mut Context<Self>) {
        let theme = self.theme.to_string();
        let fonts = self.fonts_json();
        let style = |width_px: u32| PreviewStyle {
            theme: Some(theme.clone()),
            fonts: fonts.clone(),
            width_px: width_px as f32,
        };
        let deck = self.deck.clone();
        self.thumbs.set_deck(&deck, style(self.thumb_width));
        self.stage.set_deck(&deck, style(self.stage_width));
        cx.notify();
    }

    fn fonts_json(&self) -> String {
        self.args
            .shared
            .lock()
            .ok()
            .and_then(|state| serde_json::to_string(&state.app_settings.fonts).ok())
            .unwrap_or_default()
    }

    /// 设备像素宽度按 64px 档位取整；窗口变了就重算。
    fn recompute_widths(&mut self, window: &mut Window) -> bool {
        let scale = window.scale_factor();
        let (stage_w, _) = stage_box(window, self.aspect);
        let thumb = PreviewCache::<Arc<RenderImage>>::bucket_width(THUMB_W * scale);
        let stage = PreviewCache::<Arc<RenderImage>>::bucket_width(stage_w * scale);
        if thumb != self.thumb_width || stage != self.stage_width {
            self.thumb_width = thumb;
            self.stage_width = stage;
            return true;
        }
        false
    }

    /// 缩略图只渲染可见行 ±4；大图只渲染当前页。
    fn update_wanted(&mut self, window: &mut Window) {
        let (first, last) = self.visible;
        let from = first.saturating_sub(THUMB_OVERSCAN);
        let to = (last + THUMB_OVERSCAN).min(self.deck.len().saturating_sub(1));
        let wanted: Vec<usize> = if from <= to {
            (from..=to).collect()
        } else {
            Vec::new()
        };
        self.thumbs.set_wanted(&wanted);
        self.stage.set_wanted(&[self.selected]);
        let _ = window;
    }

    /// 发一次请求，在后台线程上渲染，回来后落图并释放被替换的图。
    fn pump(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.recompute_widths(window) {
            self.deck_changed(cx);
        }
        self.update_wanted(window);

        let stage_request = self.stage.next_request();
        let thumb_request = self.thumbs.next_request();
        if stage_request.is_none() && thumb_request.is_none() {
            if self.pending_since.is_none() && (self.stage.is_pending() || self.thumbs.is_pending())
            {
                self.pending_since = Some(std::time::Instant::now());
            }
            return;
        }
        self.pending_since = None;

        let project_id = self.args.project_id;
        let shared = self.args.shared.clone();
        let theme = self.theme.to_string();

        // The stage first: it is the page in front of the user.
        if let Some(request) = stage_request {
            let slides: Vec<serde_json::Value> = self.deck.iter().flatten().cloned().collect();
            let width = request.width_px;
            let only = request.only.clone();
            let shared = shared.clone();
            let theme = theme.clone();
            cx.spawn_in(window, async move |this, window| {
                let result = window
                    .background_spawn(async move {
                        app::render_pages(
                            shared,
                            Some(project_id),
                            slides,
                            Some(theme),
                            width,
                            only,
                        )
                    })
                    .await;
                let _ = this.update_in(window, move |board, window, cx| {
                    board.finish_stage(&request, result, window, cx);
                });
            })
            .detach();
        }

        if let Some(request) = thumb_request {
            let slides: Vec<serde_json::Value> = self.deck.iter().flatten().cloned().collect();
            let width = request.width_px;
            let only = request.only.clone();
            let shared = shared.clone();
            let theme = theme.clone();
            cx.spawn_in(window, async move |this, window| {
                let result = window
                    .background_spawn(async move {
                        app::render_pages(
                            shared,
                            Some(project_id),
                            slides,
                            Some(theme),
                            width,
                            only,
                        )
                    })
                    .await;
                let _ = this.update_in(window, move |board, window, cx| {
                    board.finish_thumbs(&request, result, window, cx);
                });
            })
            .detach();
        }
    }

    fn finish_stage(
        &mut self,
        request: &huanshu_core::preview::PreviewRequest,
        result: Result<app::Pages, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let released = self.stage.complete(request, result);
        drop_all(released, Some(window), cx);
        self.failure = if self.stage.failed() {
            Some("渲染失败".to_string())
        } else {
            None
        };
        // 32:9 / 48:9 的牌组按真实像素比摆舞台，否则大图会被压成 16:9。
        if let Some(image) = self.stage.image(self.selected) {
            let size = image.size(0);
            if size.width.0 > 0 && size.height.0 > 0 {
                let ratio = size.height.0 as f32 / size.width.0 as f32;
                if (ratio - self.aspect).abs() > f32::EPSILON {
                    self.aspect = ratio;
                    let mut released = self.stage.reset();
                    released.extend(self.stage.take_released());
                    drop_all(released, Some(window), cx);
                }
            }
        }
        if self.stage.image(self.selected).is_some() {
            self.pending_since = None;
        }
        cx.notify();
        // Only after the current page has landed do we prefetch its neighbours.
        self.update_wanted(window);
        self.pump(window, cx);
    }

    fn finish_thumbs(
        &mut self,
        request: &huanshu_core::preview::PreviewRequest,
        result: Result<app::Pages, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut released = self.thumbs.complete(request, result);
        released.extend(self.thumbs.take_released());
        drop_all(released, Some(window), cx);
        cx.notify();
        self.pump(window, cx);
    }

    /// 离开看板：把两个缓存里剩下的图全部交还。
    ///
    /// 看板实体被丢弃时 GPUI 不会替我们回收图集 tile，所以回到首页必须走这里。
    /// 传 `None` 窗口：`App::drop_image` 会从**所有**窗口移除这块纹理，正是我们要的
    /// （演示窗口可能还开着同一批图）。
    pub fn release_all(&mut self, cx: &mut Context<Self>) {
        let mut released = self.thumbs.reset();
        released.extend(self.stage.reset());
        released.extend(self.thumbs.take_released());
        released.extend(self.stage.take_released());
        drop_all(released, None, cx);
    }

    fn export_pdf(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.exporting {
            return;
        }
        let shared = self.args.shared.clone();
        let project_id = self.args.project_id;
        let theme = self.theme.to_string();
        let default_name = format!("{}.pdf", self.project_name);
        let Some(path) = crate::export::ask_save_path(&default_name) else {
            return;
        };
        self.exporting = true;
        self.export_note = None;
        self.export_path = None;
        cx.notify();
        cx.spawn_in(window, async move |this, window| {
            let result = window
                .background_spawn(async move { app::export_pdf(&shared, project_id, path, theme) })
                .await;
            let _ = this.update_in(window, |board, window, cx| {
                board.exporting = false;
                board.export_note = Some(match &result {
                    Ok(report) => crate::export::summarize(report),
                    Err(error) => format!("导出失败：{error}"),
                });
                board.export_path = result.as_ref().ok().map(|report| report.path.clone());
                cx.notify();
                let _ = window;
            });
        })
        .detach();
    }
}

/// 大图舞台的尺寸：按页面比例在可用空间里取最大矩形。
fn stage_box(window: &Window, aspect: f32) -> (f32, f32) {
    let size = window.bounds().size;
    let avail_w = (size.width.as_f32() - THUMB_W - 96.0)
        .max(160.0)
        .min(STAGE_MAX_W);
    let avail_h = (size.height.as_f32() - 96.0).max(90.0);
    let mut width = avail_w;
    let mut height = width * aspect;
    if height > avail_h {
        height = avail_h;
        width = height / aspect;
    }
    (width, height)
}

impl Board {
    /// 左侧缩略图虚拟列表。
    fn sidebar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let (thumb_w, row_h) = (THUMB_W, THUMB_W * 9.0 / 16.0 + 24.0);
        let sizes = Rc::new(vec![size(px(thumb_w), px(row_h)); self.deck.len()]);
        let list = v_virtual_list(
            cx.entity(),
            "thumbs",
            sizes,
            move |board: &mut Board, range, _window, cx| {
                // The visible range arrives here; `wanted` follows it (visible ± 4 rows).
                if board.visible != (range.start, range.end) {
                    board.visible = (range.start, range.end);
                    board.update_wanted(_window);
                }
                range
                    .map(|index| board.thumb_row(index, thumb_w, cx))
                    .collect::<Vec<_>>()
            },
        )
        .track_scroll(&self.scroll);
        div()
            .flex()
            .flex_col()
            // 侧栏不参与收缩，否则大图会把缩略图挤成一条缝。
            .flex_none()
            .w(px(thumb_w + 16.0))
            .h_full()
            .border_r_1()
            .border_color(cx.theme().border)
            .child(list)
    }

    /// 虚拟列表的一行：缩略图（或占位框）+ 序号与版式名。
    fn thumb_row(
        &mut self,
        index: usize,
        thumb_w: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_selected = index == self.selected;
        let frame = self.thumbs.image(index).cloned();
        let pending = self.thumbs.is_page_pending(index);
        let failed = self.thumbs.failed();
        let kind = self.kind_label(index);
        let text = if failed {
            format!("{:02} · {kind}（渲染失败）", index + 1)
        } else if frame.is_none() && pending {
            format!("{:02} · {kind}（渲染中…）", index + 1)
        } else {
            format!("{:02} · {kind}", index + 1)
        };
        let picture = match frame {
            Some(image) => div()
                .w(px(thumb_w))
                .h(px(thumb_w * 9.0 / 16.0))
                .child(img(image).w_full().h_full()),
            // 没有图时给一个占位框，不留白屏（与 SlideFrame.vue 的 pending 一致）。
            None => div()
                .w(px(thumb_w))
                .h(px(thumb_w * 9.0 / 16.0))
                .bg(cx.theme().muted),
        };
        let row = div()
            .id(("thumb", index))
            .flex()
            .flex_col()
            .gap_1()
            .w_full()
            .py_1()
            .pl_2()
            .child(picture)
            .child(
                div()
                    .text_xs()
                    .text_color(if is_selected {
                        cx.theme().primary
                    } else {
                        cx.theme().muted_foreground
                    })
                    .child(SharedString::from(text)),
            );
        let row = if is_selected {
            row.bg(cx.theme().accent)
        } else {
            row
        };
        row.on_click(cx.listener(move |board, _event, window, cx| {
            board.select_index(index);
            board.pump(window, cx);
        }))
    }
}

/// 「渲染中…」要等一会儿才出现。
fn show_pending(since: Option<std::time::Instant>) -> bool {
    since.is_some_and(|since| since.elapsed() >= PENDING_DELAY)
}

impl Render for Board {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.pump(window, cx);

        let deck_len = self.deck.len();
        let selected = self.selected;
        let project_name = self.project_name.clone();
        let theme_name = themes::theme_or_default(Some(self.theme))
            .0
            .name
            .to_string();
        let failure = self.failure.clone();
        let exporting = self.exporting;
        let export_note = self.export_note.clone();
        let export_path = self.export_path.clone();
        let stage_image = self.stage.image(selected).cloned();
        let stage_overflow = self.stage.overflow(selected);
        let stage_showing_pending = show_pending(self.pending_since);
        let root = self.args.root.clone();
        let theme_ids: Vec<&'static str> = themes::APP_THEMES.iter().map(|t| t.id).collect();
        let page_label = format!("{} / {} · {}", selected + 1, deck_len, theme_name);

        let sidebar = self.sidebar(cx);
        let toolbar = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .child(Button::new("back").label("返回").on_click(cx.listener(
                move |_board, _event, _window, cx| {
                    root.update(cx, |root, cx| root.goto_home(cx));
                },
            )))
            .child(
                div()
                    .flex_1()
                    .text_lg()
                    .child(SharedString::from(project_name)),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(SharedString::from(page_label)),
            )
            .child(
                Button::new("next-theme")
                    .label("换主题")
                    .on_click(cx.listener(move |board, _event, _window, cx| {
                        let index = theme_ids
                            .iter()
                            .position(|id| *id == board.theme)
                            .unwrap_or(0);
                        board.set_theme(theme_ids[(index + 1) % theme_ids.len()], cx);
                        cx.notify();
                    })),
            )
            .child(Button::new("present").label("演示").on_click(cx.listener(
                |board, _event, _window, cx| {
                    let (project_id, slides, theme, shared) = board.deck_for_present();
                    crate::present::open(project_id, slides, theme, shared, cx);
                },
            )))
            .child(
                Button::new("export")
                    .label(if exporting {
                        "导出中…"
                    } else {
                        "导出 PDF"
                    })
                    .on_click(
                        cx.listener(|board, _event, window, cx| board.export_pdf(window, cx)),
                    ),
            );

        // 舞台按页面比例定尺寸：显示框与光栅宽度同源，位图正好铺满、不会被裁。
        let (stage_w, stage_h) = stage_box(window, self.aspect);
        let stage = match stage_image {
            Some(image) => div()
                .w(px(stage_w))
                .h(px(stage_h))
                .child(img(image).w_full().h_full().object_fit(ObjectFit::Contain)),
            None => div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(SharedString::from(
                    match (failure.is_some(), stage_showing_pending) {
                        (true, _) => "渲染失败",
                        (_, true) => "渲染中…",
                        _ => "没有可显示的幻灯片",
                    },
                )),
        };

        let notices =
            div()
                .flex()
                .flex_row()
                .gap_4()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .when(stage_showing_pending, |element| {
                    element.child(SharedString::from("渲染中…"))
                })
                .when(stage_overflow, |element| {
                    element.child(SharedString::from("超出部分被裁切"))
                })
                .when_some(export_note.clone(), |element, note| {
                    element.child(div().flex_1().child(SharedString::from(note)))
                })
                .when_some(export_path, |element, path| {
                    element.child(Button::new("reveal").label("在文件夹中显示").on_click(
                        cx.listener(move |_board, _event, _window, _cx| {
                            if let Err(error) = crate::export::reveal(&path) {
                                eprintln!("[native] 打开文件夹失败：{error}");
                            }
                        }),
                    ))
                });

        div().flex().flex_row().size_full().child(sidebar).child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .h_full()
                .gap_2()
                .p_3()
                .child(toolbar)
                .child(div().flex_1().min_h_0().flex().child(stage))
                .child(notices),
        )
    }
}
