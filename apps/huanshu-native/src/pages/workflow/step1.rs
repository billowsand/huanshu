//! 第一步「准备素材」（原版 `steps/Step1Prepare.vue` + `MarkdownWorkbench.vue`）。
//!
//! 顶栏（名称、文件名标签、字数、草稿状态、AI 优化、上传文件）、格式工具行、
//! Markdown 编辑器 + 按页预览（P1/P2… 卡片，滚动同步）、分页粒度、幻灯片比例、
//! 素材库、底栏「开始 AI 生成」。文稿规则全部用 `huanshu_core::markdown_tools`
//! （与原版 TS 逐字对照、快照测试守住）。

use std::path::PathBuf;

use gpui_kit::component::input::{Editor, EditorState, Input, InputEvent, InputState};
use gpui_kit::component::text::TextView;
use gpui_kit::component::Disableable as _;
use gpui_kit::prelude::*;
use gpui_kit::{
    div, img, point, px, Context, Entity, ExternalPaths, Render, ScrollHandle, SharedString,
    Subscription, WeakEntity, Window,
};
use huanshu_core::markdown_tools::{self, Granularity, HeadingNumberStyle};
use huanshu_core::services::projects::Step1ProjectMeta;
use huanshu_core::services::{generation, media, projects};
use huanshu_core::types::AspectRatio;

use super::{ProjectModel, WorkflowPage};
use crate::icons::carbon;
use crate::router::Route;
use crate::shell::ShellHandle;
use crate::tasks::{run_async, run_bg};
use crate::theme::StudioExt as _;
use crate::widgets::{self, Tone2, TooltipText as _};

/// 编辑器占位符（原版 `MarkdownWorkbench` 的 `placeholder`，逐字）。
const EDITOR_PLACEHOLDER: &str = "粘贴 Markdown 内容，或拖拽 .md 文件到此处...\n\n# 演示标题\n\n## 章节一\n内容描述...\n\n## 章节二\n内容描述...";

/// 素材扩展名（原版 `MEDIA_EXTS_IMG` / `MEDIA_EXTS_VID`）。
const MEDIA_EXTS_IMG: [&str; 6] = ["png", "jpg", "jpeg", "gif", "webp", "svg"];
const MEDIA_EXTS_VID: [&str; 3] = ["mp4", "webm", "mov"];

/// 原版 `useMediaLibrary.ts` 的 `getMimeType`。
fn media_mime_type(ext: &str) -> &'static str {
    match ext {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        _ => "application/octet-stream",
    }
}

fn file_ext(name: &str) -> String {
    name.rsplit('.').next().unwrap_or("").to_lowercase()
}

fn is_media_path(path: &std::path::Path) -> bool {
    let ext = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .map(|n| file_ext(&n))
        .unwrap_or_default();
    MEDIA_EXTS_IMG.contains(&ext.as_str()) || MEDIA_EXTS_VID.contains(&ext.as_str())
}

/// 文稿路径（原版拖入 / 上传只认 `.md` / `.txt`）。
fn is_markdown_path(path: &std::path::Path) -> bool {
    matches!(file_ext(&path.to_string_lossy()).as_str(), "md" | "txt")
}

/// 去掉扩展名的文件名（原版 `filename.replace(/\.[^.]+$/, '')`）。
fn strip_extension(name: &str) -> String {
    match name.rfind('.') {
        Some(i) if i > 0 => name[..i].to_string(),
        _ => name.to_string(),
    }
}

/// 六个格式工具（原版 `Step1Prepare.vue` 工具行，`md -> md`）。
fn apply_format_tool(md: &str, index: usize) -> String {
    match index {
        0 => markdown_tools::auto_detect_headings(md),
        1 => markdown_tools::clear_heading_bold(md),
        2 => markdown_tools::remove_heading_numbers(md),
        3 => markdown_tools::add_heading_numbers(md, 2, HeadingNumberStyle::CnSection),
        4 => markdown_tools::add_heading_numbers(md, 3, HeadingNumberStyle::CnSubsection),
        _ => markdown_tools::add_heading_numbers(md, 4, HeadingNumberStyle::Decimal),
    }
}

pub struct Step1 {
    model: Entity<ProjectModel>,
    handle: ShellHandle,
    page: WeakEntity<WorkflowPage>,
    name: Entity<InputState>,
    editor: Entity<EditorState>,
    /// AI 优化进行中（原版 `optimizing`）。
    optimizing: bool,
    /// AI 优化失败信息（原版 `optimizeError`，格式工具行内错误）。
    optimize_error: String,
    /// 文稿拖放悬停（原版 `isDragOver`）。
    ///
    /// 已知偏差：GPUI Kit 只有 `on_file_drop_exit` / `on_drop`，没有文件拖入悬停钩子，
    /// 覆盖层代码保留但 `drag_over` 恒为 false（见 parity.md）。
    drag_over: bool,
    /// 素材拖放悬停（原版 `mediaDropOver`）。同上，框架拿不到拖入事件。
    media_drop_over: bool,
    /// 预览滚动容器（滚动同步的预览侧句柄）。
    preview_scroll: ScrollHandle,
    /// 滚动同步的回声守卫：我们设编辑器滚动时期望的比例，编辑器观察器回来比对，
    /// 匹配则说明是回声，不再反向同步（原版 `syncingFromEditor` / `syncingFromPreview`）。
    sync_echo: Option<f32>,
    /// 模型里的名称被外部改写（自动保存兜底名、侧栏改名、上传/AI 优化取 H1）后，
    /// 观察器里发现与输入框不同步就置位，推迟到 render 里 `set_value` 回填
    /// （观察回调拿不到 window；`set_value` 不发 Change 事件，无回声）。
    name_sync_pending: bool,
    #[allow(dead_code)]
    subscriptions: Vec<Subscription>,
}

impl Step1 {
    pub fn new(
        model: Entity<ProjectModel>,
        handle: ShellHandle,
        page: WeakEntity<WorkflowPage>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let (name_text, md) = {
            let model = model.read(cx);
            (model.name.clone(), model.md_content.clone())
        };
        let name = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("演示名称（从 H1 自动提取或手动输入）...")
                .default_value(name_text)
        });
        let editor = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("markdown")
                .placeholder(EDITOR_PLACEHOLDER)
                .default_value(md)
        });
        let subscriptions = vec![
            cx.subscribe(&name, |this: &mut Self, state, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    let value = state.read(cx).value().to_string();
                    this.model.update(cx, |model, cx| {
                        model.name = value;
                        cx.notify();
                    });
                    // 自动保存（防抖 800ms 落库）在 WorkflowPage 层观察这个模型统一处理。
                }
            }),
            cx.observe(&editor, |this: &mut Self, editor, cx| {
                let value = editor.read(cx).value().to_string();
                this.model.update(cx, |model, cx| {
                    if model.md_content != value {
                        model.md_content = value;
                        cx.notify();
                    }
                });
                this.sync_editor_to_preview(cx);
                cx.notify();
            }),
            // 外部改名（自动保存兜底、侧栏 rename、上传/优化取 H1）回写输入框。
            cx.observe(&model, |this: &mut Self, model, cx| {
                let name = model.read(cx).name.clone();
                if name != this.name.read(cx).value().as_ref() {
                    this.name_sync_pending = true;
                    cx.notify();
                }
            }),
        ];
        Self {
            model,
            handle,
            page,
            name,
            editor,
            optimizing: false,
            optimize_error: String::new(),
            drag_over: false,
            media_drop_over: false,
            preview_scroll: ScrollHandle::new(),
            sync_echo: None,
            name_sync_pending: false,
            subscriptions,
        }
    }

    fn set_aspect(&mut self, aspect: AspectRatio, cx: &mut Context<Self>) {
        let id = self.model.read(cx).id;
        self.model.update(cx, |model, cx| {
            model.aspect = aspect;
            cx.notify();
        });
        if let Some(id) = id {
            if let Err(error) =
                projects::update_project_aspect_ratio(self.handle.shared(), id, aspect)
            {
                eprintln!("[native] 保存比例失败：{error}");
            }
        }
        cx.notify();
    }

    fn set_granularity(&mut self, granularity: Granularity, cx: &mut Context<Self>) {
        self.model.update(cx, |model, cx| {
            model.granularity = granularity;
            cx.notify();
        });
        // 持久化随 WorkflowPage 的自动保存一起写（`Step1ProjectMeta`）。
    }

    /// 把一份文稿载入编辑器（原版 `loadFilePath` / `onDrop`）：
    /// 整体替换并进入撤销历史，`mdFilename` 记进模型，名称取 H1（没有则取去扩展名的文件名）。
    fn load_markdown(
        &mut self,
        text: String,
        filename: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let h1 = markdown_tools::extract_h1(&text).map(|h| h.trim().to_string());
        self.editor.update(cx, |state, cx| {
            state.replace_all(text.clone(), window, cx);
        });
        self.model.update(cx, |model, cx| {
            model.md_content = text;
            model.md_filename = filename.clone();
            match h1 {
                Some(h1) => model.name = h1,
                None if model.name.trim().is_empty() => {
                    model.name = strip_extension(&filename);
                }
                _ => {}
            }
            cx.notify();
        });
        let name = self.model.read(cx).name.clone();
        self.name.update(cx, |state, cx| {
            state.set_value(name, window, cx);
        });
        cx.notify();
    }

    /// 上传文件（原版 `openFile`：`rfd` 选 `.md` / `.txt`）。
    fn open_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Markdown", &["md", "txt"])
            .pick_file()
        else {
            return;
        };
        self.load_path(path, window, cx);
    }

    fn load_path(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) => {
                eprintln!("[native] 读取文稿失败：{error}");
                return;
            }
        };
        let filename = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string_lossy().to_string());
        self.load_markdown(text, filename, window, cx);
    }

    /// AI 优化（原版 `optimizeMarkdown`）：禁用按钮 → 异步调用 core →
    /// 成功整体替换文稿（保留撤销历史）并从 H1 更新名称；失败显示行内错误。
    fn optimize(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (md, title_hint, project_id) = {
            let model = self.model.read(cx);
            let hint = model.name.trim().to_string();
            (
                model.md_content.clone(),
                if hint.is_empty() { None } else { Some(hint) },
                model.id,
            )
        };
        if md.trim().is_empty() || self.optimizing {
            return;
        }
        self.optimizing = true;
        self.optimize_error.clear();
        cx.notify();

        let shared = self.handle.shared().clone();
        // 身份比对：发起时记下模型实体与项目 id，回来时若模型已对准别的项目则作废
        // （原版换项目会重建整个页面；这里兜底防异步结果落错地方）。
        let model_id = self.model.entity_id();
        run_async(
            window,
            cx,
            async move { generation::optimize_markdown_headings(&shared, md, title_hint).await },
            move |this, result, window, cx| {
                if this.model.entity_id() != model_id || this.model.read(cx).id != project_id {
                    return;
                }
                this.optimizing = false;
                match result {
                    Ok(optimized) => {
                        let next = optimized.trim().to_string();
                        let h1 = markdown_tools::extract_h1(&next).map(|h| h.trim().to_string());
                        let has_h1 = h1.is_some();
                        this.editor.update(cx, |state, cx| {
                            state.replace_all(next.clone(), window, cx);
                        });
                        this.model.update(cx, |model, cx| {
                            model.md_content = next;
                            if let Some(h1) = h1 {
                                model.name = h1;
                            }
                            cx.notify();
                        });
                        if has_h1 {
                            let name = this.model.read(cx).name.clone();
                            this.name.update(cx, |state, cx| {
                                state.set_value(name, window, cx);
                            });
                        }
                    }
                    Err(error) => {
                        this.optimize_error = error;
                    }
                }
                cx.notify();
            },
        );
    }

    /// 格式工具（原版六个按钮）：改写编辑器内容并进入撤销历史。
    fn run_format_tool(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let md = self.model.read(cx).md_content.clone();
        if md.trim().is_empty() || self.optimizing {
            return;
        }
        let next = apply_format_tool(&md, index);
        self.editor.update(cx, |state, cx| {
            state.replace_all(next.clone(), window, cx);
        });
        self.model.update(cx, |model, cx| {
            model.md_content = next;
            cx.notify();
        });
        cx.notify();
    }

    // ── 素材库（原版 `useMediaLibrary.ts` + `ensureProjectForMedia`） ──

    /// 添加素材入口：`.rfd` 多选 或 拖入的文件路径统一走这里。
    fn add_media_paths(
        &mut self,
        paths: Vec<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let paths: Vec<PathBuf> = paths.iter().filter(|p| is_media_path(p)).cloned().collect();
        if paths.is_empty() {
            return;
        }
        if self.model.read(cx).id.is_none() {
            self.ensure_project_for_media(paths, window, cx);
        } else {
            self.import_media(paths, window, cx);
        }
    }

    /// 原版 `ensureProjectForMedia`：新建项目还没有 id 时先落库再导入。
    fn ensure_project_for_media(
        &mut self,
        paths: Vec<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (name, md, aspect) = {
            let model = self.model.read(cx);
            (
                if model.name.trim().is_empty() {
                    "未命名演示".to_string()
                } else {
                    model.name.trim().to_string()
                },
                model.md_content.clone(),
                model.aspect,
            )
        };
        let shared = self.handle.shared().clone();
        run_bg(
            window,
            cx,
            move || {
                let id = projects::create_project(&shared, name, md)?;
                projects::open_project(&shared, id)?;
                // 原版 `persistStep1Meta`：新建即写下附加信息。
                let meta = Step1ProjectMeta {
                    md_filename: String::new(),
                    granularity: Granularity::Auto.as_str().to_string(),
                    aspect_ratio: match aspect {
                        AspectRatio::Ratio16x9 => "ratio_16x9",
                        AspectRatio::Ratio32x9 => "ratio_32x9",
                        AspectRatio::Ratio48x9 => "ratio_48x9",
                    }
                    .to_string(),
                };
                projects::set_step1_project_meta(&shared, id, &meta)?;
                Ok::<i64, String>(id)
            },
            move |this: &mut Self, result, window, cx| {
                match result {
                    Ok(id) => {
                        this.model.update(cx, |model, cx| {
                            model.id = Some(id);
                            cx.notify();
                        });
                        // 原版 `router.replace(/project/<id>)`：只改路由、不重建页面
                        // （`navigate` 会丢掉本页的编辑器与撤销历史）。
                        this.handle.retarget_route(
                            Route::Project {
                                id: Some(id),
                                step: None,
                            },
                            cx,
                        );
                        this.import_media(paths, window, cx);
                    }
                    Err(error) => eprintln!("[native] 创建项目失败：{error}"),
                }
            },
        );
    }

    /// 逐个导入（原版 `addMediaFromPath` 的 for 循环，串行）。
    fn import_media(
        &mut self,
        mut paths: Vec<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(path) = paths.first().cloned() else {
            return;
        };
        paths.remove(0);
        let Some(project_id) = self.model.read(cx).id else {
            return;
        };
        let shared = self.handle.shared().clone();
        let model_id = self.model.entity_id();
        run_async(
            window,
            cx,
            async move {
                let filename = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.to_string_lossy().to_string());
                let ext = file_ext(&filename);
                let data = std::fs::read(&path).map_err(|e| e.to_string())?;
                let media_type = if MEDIA_EXTS_VID.contains(&ext.as_str()) {
                    "video"
                } else {
                    "image"
                };
                media::import_media_asset(
                    &shared,
                    media::ImportMediaRequest {
                        project_id,
                        slide_index: None,
                        filename,
                        mime_type: Some(media_mime_type(&ext).to_string()),
                        data,
                        media_type: media_type.to_string(),
                    },
                )
                .await
            },
            move |this, result, window, cx| {
                // 迟到的结果作废：模型已对准别的项目。
                if this.model.entity_id() != model_id || this.model.read(cx).id != Some(project_id)
                {
                    return;
                }
                match result {
                    Ok(asset) => {
                        this.model.update(cx, |model, cx| {
                            model.media.push(asset);
                            cx.notify();
                        });
                    }
                    Err(error) => eprintln!("[native] 导入素材失败：{error}"),
                }
                this.import_media(paths, window, cx);
            },
        );
    }

    /// 删除素材（原版 `removeMedia`：调 core 删除后从列表移除）。
    fn remove_media(&mut self, id: i64, window: &mut Window, cx: &mut Context<Self>) {
        let shared = self.handle.shared().clone();
        run_bg(
            window,
            cx,
            move || media::delete_media_asset(&shared, id).map_err(|e| e.to_string()),
            move |this: &mut Self, result, _window, cx| {
                if let Err(error) = result {
                    // 原版即使删除失败也从列表移除。
                    eprintln!("[native] 删除素材失败：{error}");
                }
                this.model.update(cx, |model, cx| {
                    model.media.retain(|a| a.id != id);
                    cx.notify();
                });
            },
        );
    }

    // ── 滚动同步（原版 `syncEditorToPreview` / `syncPreviewToEditor`，按比例） ──

    /// 编辑器按像素的「最大滚动量」估计：内容行数 × 行高 − 视口高。
    ///
    /// 已知偏差：编辑器默认 soft-wrap，折行会让这里的行数估计偏小、比例偏大，
    /// 长文稿折行多时会比原版更「激进」地把预览滚到底（编辑器侧的真实最大滚动量
    /// 框架没有公开 API）。底部被钳住，两端一致，中间可能有偏差。见 parity.md。
    fn editor_est_max_scroll(&self, cx: &gpui_kit::App) -> gpui_kit::Pixels {
        let state = self.editor.read(cx);
        let Some(line_height) = state.line_height() else {
            return px(0.);
        };
        let viewport = state.input_bounds().size.height;
        if viewport <= px(0.) {
            return px(0.);
        }
        let lines = state.value().to_string().lines().count().max(1) as f32;
        (lines * line_height - viewport).max(px(0.))
    }

    /// 编辑器当前滚动比例（0 = 顶，1 = 底；原版 `getScrollRatio(view.scrollDOM)`）。
    fn editor_scroll_ratio(&self, cx: &gpui_kit::App) -> f32 {
        let max = self.editor_est_max_scroll(cx);
        if max <= px(0.) {
            return 0.;
        }
        let offset = self.editor.read(cx).scroll_offset().y;
        (-offset / max).clamp(0., 1.)
    }

    /// 编辑区滚动 → 预览区滚到相同比例（原版 `syncEditorToPreview`）。
    fn sync_editor_to_preview(&mut self, cx: &mut Context<Self>) {
        if let Some(target) = self.sync_echo {
            // 上一次是我们设的编辑器滚动：比例对得上就是回声，不再反向同步。
            if (self.editor_scroll_ratio(cx) - target).abs() < 0.02 {
                self.sync_echo = None;
                return;
            }
            self.sync_echo = None;
        }
        let ratio = self.editor_scroll_ratio(cx);
        let max = self.preview_scroll.max_offset().y;
        self.preview_scroll
            .set_offset(point(px(0.), -(ratio * max)));
    }

    /// 预览区滚轮 → 编辑器滚到相同比例（原版 `syncPreviewToEditor`）。
    ///
    /// 本回调注册在容器内置滚动处理**之前**，此刻句柄里的偏移还是旧值，因此用
    /// `旧偏移 + delta` 先算出滚动后的比例；内置处理随后会把同样的 delta 应用进句柄。
    fn sync_preview_wheel(
        &mut self,
        event: &gpui_kit::ScrollWheelEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let delta = event.delta.pixel_delta(window.line_height()).y;
        let max = self.preview_scroll.max_offset().y;
        if max <= px(0.) {
            // 原版 `applyScrollRatio`：maxScroll <= 0 时滚回顶部。
            self.editor.update(cx, |state, cx| {
                state.set_scroll_offset(point(px(0.), px(0.)), cx);
            });
            return;
        }
        let next = (self.preview_scroll.offset().y + delta).clamp(-max, px(0.));
        let ratio = -next / max;
        self.sync_echo = Some(ratio);
        let est_max = self.editor_est_max_scroll(cx);
        self.editor.update(cx, |state, cx| {
            state.set_scroll_offset(point(px(0.), -(ratio * est_max)), cx);
        });
        cx.notify();
    }
}

impl Render for Step1 {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 观察器攒下的名称回填：模型名与输入框不同步时用 `set_value` 对齐
        // （不发 Change 事件；打字路径是输入框 → 模型，不会回声）。
        if std::mem::take(&mut self.name_sync_pending) {
            let name = self.model.read(cx).name.clone();
            if name != self.name.read(cx).value().as_ref() {
                self.name.update(cx, |state, cx| {
                    state.set_value(name, window, cx);
                });
            }
        }
        let c = cx.studio().colors;
        let model = self.model.read(cx);
        let md = model.md_content.clone();
        let chars = model.md_len();
        let md_filename = model.md_filename.clone();
        let draft_text = model.draft_save_state.text();
        let draft_state = model.draft_save_state;
        let draft_error = model.draft_save_error.clone();
        let aspect = model.aspect;
        let granularity = model.granularity;
        let detected = model.detected_granularity();
        let estimated = model.estimated_slides();
        let has_name = !model.name.trim().is_empty();
        let has_md = !md.trim().is_empty();
        // 原版 `canGenerate`：文稿与名称非空、且没有生成 / 重试在跑（全局条件，见 `store.busy`）。
        let can_generate = model.can_generate_local(self.handle.generation().read(cx).busy());
        let media: Vec<(i64, String, String, bool, Option<i64>, Option<i64>)> = model
            .media
            .iter()
            .map(|a| {
                (
                    a.id,
                    a.original_name.clone(),
                    a.url.clone(),
                    a.media_type == "image",
                    a.width,
                    a.height,
                )
            })
            .collect();
        let h2 = markdown_tools::count_exact_headings(&md, 2);
        let h3 = markdown_tools::count_exact_headings(&md, 3);

        let optimizing = self.optimizing;
        let optimize_error = self.optimize_error.clone();
        let tools_disabled = optimizing || !has_md;

        let top_bar = div()
            .flex_none()
            .h(px(64.0))
            .px(px(18.0))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(14.0))
            .bg(c.panel)
            .border_b_1()
            .border_color(c.border)
            .child(
                div()
                    .text_size(px(14.0))
                    .text_color(c.muted)
                    .child("准备素材"),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .max_w(px(420.0))
                    .child(Input::new(&self.name)),
            )
            .child(div().flex_1())
            .when(!md_filename.is_empty(), |bar| {
                bar.child(widgets::chip_icon(
                    "document",
                    md_filename.clone(),
                    Tone2::Muted,
                    cx,
                ))
            })
            .when(!draft_text.is_empty(), |bar| {
                bar.child(widgets::chip(draft_text, Tone2::Muted, cx))
            })
            .child(widgets::chip(format!("{chars} 字"), Tone2::Primary, cx))
            .child(
                widgets::ghost_button(
                    "ai-optimize",
                    if optimizing {
                        "优化中..."
                    } else {
                        "AI 优化"
                    },
                )
                .icon(carbon("magic-wand"))
                .disabled(optimizing || !has_md)
                .on_click(cx.listener(|this, _event, window, cx| this.optimize(window, cx))),
            )
            .child(
                widgets::ghost_button("upload", "上传文件")
                    .icon(carbon("upload"))
                    .disabled(optimizing)
                    .on_click(cx.listener(|this, _event, window, cx| this.open_file(window, cx))),
            );

        let tool_labels = [
            "自动标题",
            "清除标题加粗",
            "删除序号",
            "`##` 一、二、",
            "`###` （一）（二）",
            "`####` 1. 2.",
        ];
        let tools = div()
            .flex_none()
            .h(px(44.0))
            .px(px(18.0))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(4.0))
            .border_b_1()
            .border_color(c.border)
            .child(
                div()
                    .pr(px(10.0))
                    .text_size(px(13.0))
                    .text_color(c.muted)
                    .child("格式工具"),
            )
            .children(tool_labels.iter().enumerate().map(|(i, label)| {
                widgets::small(widgets::ghost_button(("fmt", i), *label))
                    .disabled(tools_disabled)
                    .on_click(cx.listener(move |this, _event, window, cx| {
                        this.run_format_tool(i, window, cx);
                    }))
            }))
            .when(
                matches!(draft_state, super::DraftSaveState::Error) && !draft_error.is_empty(),
                |row| {
                    row.child(
                        div()
                            .pl(px(10.0))
                            .child(widgets::inline_error(draft_error, cx)),
                    )
                },
            )
            .when(!optimize_error.is_empty(), |row| {
                row.child(
                    div()
                        .pl(px(10.0))
                        .child(widgets::inline_error(optimize_error, cx)),
                )
            });

        let pane_header = |icon: &'static str, label: &'static str| {
            div()
                .flex_none()
                .h(px(32.0))
                .px(px(14.0))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .text_size(px(12.5))
                .text_color(c.muted)
                .border_b_1()
                .border_color(c.border)
                .child(carbon(icon))
                .child(label)
        };

        // 预览页卡片（原版 `MarkdownWorkbench.vue` 的 `md-preview-slide`）：
        // 空文稿也是一张「P1」卡片 + 空态文案；否则按粒度切页，每页 `TextView::markdown`。
        let preview_card = |index: usize, content: Option<String>| {
            div()
                .relative()
                .rounded(px(14.0))
                .border_1()
                .border_color(c.border)
                .bg(c.surface)
                .child(
                    div()
                        .absolute()
                        .top(px(10.0))
                        .right(px(12.0))
                        .px(px(7.0))
                        .py(px(2.0))
                        .rounded(px(999.0))
                        .border_1()
                        .border_color(c.border)
                        .bg(c.panel)
                        .text_size(px(11.0))
                        .font_weight(gpui_kit::FontWeight::BOLD)
                        .text_color(c.muted)
                        .child(format!("P{}", index + 1)),
                )
                .child(match content {
                    Some(chunk) => div()
                        .p(px(18.0))
                        .child(TextView::markdown(("md-preview", index), chunk))
                        .into_any_element(),
                    None => div()
                        .p(px(18.0))
                        .text_size(px(13.0))
                        .text_color(c.muted)
                        .child("预览区将实时显示 Markdown 渲染效果")
                        .into_any_element(),
                })
        };
        let preview_cards: Vec<gpui_kit::AnyElement> = if !has_md {
            vec![preview_card(0, None).into_any_element()]
        } else {
            let resolved = match granularity {
                Granularity::Auto => detected,
                other => other,
            };
            markdown_tools::split_preview_pages(md.trim(), resolved)
                .into_iter()
                .enumerate()
                .map(|(i, chunk)| preview_card(i, Some(chunk)).into_any_element())
                .collect()
        };

        let workbench = div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_row()
            .child(
                div()
                    .id("step1-editor-col")
                    .relative()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .border_r_1()
                    .border_color(c.border)
                    // 拖入文稿（原版 `onDrop`；悬停覆盖层见 `drag_over` 的已知偏差说明）。
                    .on_drop(cx.listener(|this, paths: &ExternalPaths, window, cx| {
                        this.drag_over = false;
                        let Some(path) =
                            paths.paths().iter().find(|p| is_markdown_path(p)).cloned()
                        else {
                            return;
                        };
                        this.load_path(path, window, cx);
                    }))
                    .on_file_drop_exit(cx.listener(|this, _event, _window, cx| {
                        this.drag_over = false;
                        cx.notify();
                    }))
                    .child(pane_header("edit", "编辑"))
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .child(Editor::new(&self.editor).size_full()),
                    )
                    // 原版拖放悬停时的覆盖层：「释放以加载 Markdown 文件」。
                    // GPUI Kit 没有文件拖入悬停钩子，`drag_over` 恒为 false，此层暂不可达。
                    .when(self.drag_over, |col| {
                        col.child(
                            div()
                                .absolute()
                                .size_full()
                                .flex()
                                .flex_col()
                                .items_center()
                                .justify_center()
                                .gap(px(8.0))
                                .bg(c.primary_bg)
                                .child(carbon("document-import"))
                                .child(
                                    div()
                                        .text_size(px(14.0))
                                        .text_color(c.primary)
                                        .child("释放以加载 Markdown 文件"),
                                ),
                        )
                    }),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(pane_header("view", "预览"))
                    .child(
                        div()
                            .id("md-preview")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .track_scroll(&self.preview_scroll)
                            .on_scroll_wheel(cx.listener(
                                |this, event: &gpui_kit::ScrollWheelEvent, window, cx| {
                                    this.sync_preview_wheel(event, window, cx);
                                },
                            ))
                            .p(px(16.0))
                            .flex()
                            .flex_col()
                            .gap(px(14.0))
                            .children(preview_cards),
                    ),
            );

        // ── 分页粒度（原版「分页粒度」卡片） ──
        let gran_option = |g: Granularity, title: &'static str, cx: &mut Context<Self>| {
            let selected = granularity == g;
            let (heading_badge, pages) = match g {
                Granularity::Auto => (
                    match detected {
                        Granularity::H2 => "按二级标题".to_string(),
                        _ => "按三级标题".to_string(),
                    },
                    markdown_tools::estimate_slides(&md, g),
                ),
                Granularity::H2 => (
                    format!("{h2} 个标题"),
                    markdown_tools::estimate_slides(&md, g),
                ),
                Granularity::H3 => (
                    format!("{h3} 个标题"),
                    markdown_tools::estimate_slides(&md, g),
                ),
            };
            div()
                .id(SharedString::from(format!("gran-{title}")))
                .w_full()
                .px(px(14.0))
                .py(px(10.0))
                .flex()
                .flex_col()
                .gap(px(6.0))
                .rounded(px(8.0))
                .border_1()
                .border_color(if selected { c.primary_border } else { c.border })
                .bg(if selected { c.primary_bg } else { c.surface })
                .cursor_pointer()
                .child(div().text_size(px(14.0)).child(title))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(6.0))
                        .child(widgets::chip(heading_badge, Tone2::Muted, cx))
                        .child(widgets::chip(format!("{pages} 页"), Tone2::Muted, cx)),
                )
                .on_click(cx.listener(move |this, _event, _window, cx| {
                    this.set_granularity(g, cx);
                }))
        };
        let granularity_card = div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(widgets::section_label("分页粒度", cx))
                    .child(widgets::chip(
                        format!("预计 {estimated} 页"),
                        Tone2::Primary,
                        cx,
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(gran_option(Granularity::Auto, "自动推荐", cx))
                    .child(gran_option(Granularity::H2, "按二级标题", cx))
                    .child(gran_option(Granularity::H3, "按三级标题", cx)),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(c.muted)
                    .child(format!("二级标题 {h2} 个，三级标题 {h3} 个")),
            );

        let aspect_card = |ratio: AspectRatio,
                           title: &'static str,
                           desc: &'static str,
                           cx: &mut Context<Self>| {
            let selected = aspect == ratio;
            div()
                .id(SharedString::from(format!("aspect-{title}")))
                .flex_1()
                .py(px(14.0))
                .flex()
                .flex_col()
                .items_center()
                .gap(px(4.0))
                .rounded(px(8.0))
                .border_1()
                .border_color(if selected { c.primary_border } else { c.border })
                .bg(if selected { c.primary_bg } else { c.surface })
                .cursor_pointer()
                .child(div().text_size(px(16.0)).child(title))
                .child(div().text_size(px(12.0)).text_color(c.muted).child(desc))
                .on_click(cx.listener(move |this, _event, _window, cx| this.set_aspect(ratio, cx)))
        };

        // ── 素材库（原版「素材库」卡片 + `useMediaLibrary.ts`） ──
        let media_thumb = |(id, name, url, is_image, width, height): &(
            i64,
            String,
            String,
            bool,
            Option<i64>,
            Option<i64>,
        ),
                           cx: &mut Context<Self>| {
            let (id, name, url, is_image, width, height) =
                (*id, name.clone(), url.clone(), *is_image, *width, *height);
            div()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .min_w_0()
                .child(
                    div()
                        .relative()
                        .w_full()
                        .h(px(64.0))
                        .rounded(px(8.0))
                        .border_1()
                        .border_color(c.border)
                        .bg(c.surface)
                        .overflow_hidden()
                        .child(if is_image {
                            img(PathBuf::from(url)).size_full().into_any_element()
                        } else {
                            div()
                                .size_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_color(c.muted)
                                .child(carbon("video-filled"))
                                .into_any_element()
                        })
                        .child(
                            div().absolute().top(px(4.0)).right(px(4.0)).child(
                                widgets::icon_button(
                                    SharedString::from(format!("media-del-{id}")),
                                    "close",
                                    "移除",
                                )
                                .on_click(cx.listener(
                                    move |this, _event, window, cx| {
                                        this.remove_media(id, window, cx);
                                    },
                                )),
                            ),
                        ),
                )
                .child(
                    div()
                        .id(SharedString::from(format!("media-name-{id}")))
                        .truncate()
                        .text_size(px(11.5))
                        .text_color(c.text)
                        .tooltip_text(name.clone())
                        .child(name),
                )
                .when(is_image && width.is_some() && height.is_some(), |col| {
                    col.child(div().text_size(px(11.0)).text_color(c.muted).child(format!(
                        "{}×{}",
                        width.unwrap(),
                        height.unwrap()
                    )))
                })
        };
        let media_card = div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(widgets::section_label("素材库", cx))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.0))
                            .child(widgets::chip(
                                format!("{} 个文件", media.len()),
                                Tone2::Muted,
                                cx,
                            ))
                            .child(
                                widgets::small(widgets::ghost_button("add-media", "添加素材"))
                                    .icon(carbon("add-alt"))
                                    .on_click(cx.listener(|this, _event, window, cx| {
                                        let Some(paths) = rfd::FileDialog::new()
                                            .add_filter(
                                                "图片 / 视频",
                                                &[
                                                    "png", "jpg", "jpeg", "gif", "webp", "svg",
                                                    "mp4", "webm", "mov",
                                                ],
                                            )
                                            .pick_files()
                                        else {
                                            return;
                                        };
                                        this.add_media_paths(paths, window, cx);
                                    })),
                            ),
                    ),
            )
            .child(
                div()
                    .id("media-drop-zone")
                    .w_full()
                    .min_h(px(120.0))
                    .when(media.is_empty(), |zone| {
                        // 素材拖放悬停高亮同 `drag_over`，框架拿不到拖入事件（见 parity.md）。
                        zone.border_1()
                            .border_dashed()
                            .border_color(if self.media_drop_over {
                                c.primary_border
                            } else {
                                c.border
                            })
                            .rounded(px(8.0))
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .gap(px(8.0))
                            .child(carbon("image-copy").size(px(28.0)).text_color(c.muted))
                            .child(
                                div()
                                    .text_size(px(12.5))
                                    .text_color(c.muted)
                                    .child("拖拽图片 / 视频到此处"),
                            )
                    })
                    .when(!media.is_empty(), |zone| {
                        zone.child(
                            div().flex().flex_col().gap(px(10.0)).children(
                                media
                                    .iter()
                                    .map(|asset| media_thumb(asset, cx).into_any_element()),
                            ),
                        )
                    })
                    .on_drop(cx.listener(|this, paths: &ExternalPaths, window, cx| {
                        this.media_drop_over = false;
                        this.add_media_paths(paths.paths().to_vec(), window, cx);
                    }))
                    .on_file_drop_exit(cx.listener(|this, _event, _window, cx| {
                        this.media_drop_over = false;
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(c.muted)
                    .child("素材生成时自动参与图文匹配。"),
            );

        let sidebar = div()
            .id("step1-sidebar")
            .flex_none()
            .w(px(288.0))
            .h_full()
            .overflow_y_scroll()
            .p(px(16.0))
            .flex()
            .flex_col()
            .gap(px(22.0))
            .bg(c.panel)
            .border_l_1()
            .border_color(c.border)
            .child(granularity_card)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    .child(widgets::section_label("幻灯片比例", cx))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(8.0))
                            .child(aspect_card(AspectRatio::Ratio16x9, "16:9", "标准宽屏", cx))
                            .child(aspect_card(AspectRatio::Ratio32x9, "32:9", "超宽屏幕", cx))
                            .child(aspect_card(AspectRatio::Ratio48x9, "48:9", "全景屏幕", cx)),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(c.muted)
                            .child("32:9 / 48:9 在 16:9 屏幕上播放时会横向压缩"),
                    ),
            )
            .child(media_card);

        // 底栏提示（原版三态）与「开始 AI 生成」的可用条件。
        let bottom_hint = if !has_name {
            "请填写演示名称".to_string()
        } else if !has_md {
            "请上传或粘贴文稿内容".to_string()
        } else {
            format!("当前预计生成 {estimated} 张幻灯片")
        };
        let bottom = div()
            .flex_none()
            .h(px(60.0))
            .px(px(18.0))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(12.0))
            .bg(c.panel)
            .border_t_1()
            .border_color(c.border)
            .child(
                widgets::primary_button("generate", "开始 AI 生成")
                    .icon(carbon("magic-wand"))
                    // 原版 `canGenerate`（含「没有页在重试」与全局的生成中）+ `optimizing`。
                    .disabled(!can_generate || optimizing)
                    .on_click(cx.listener(|this, _event, window, cx| {
                        let _ = this.page.update(cx, |page, cx| {
                            page.start_generation(window, cx);
                        });
                    })),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(c.muted)
                    .child(bottom_hint),
            );

        div()
            .size_full()
            .flex()
            .flex_col()
            .child(top_bar)
            .child(tools)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_row()
                    .child(workbench)
                    .child(sidebar),
            )
            .child(bottom)
    }
}
