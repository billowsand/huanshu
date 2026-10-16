//! 第一步「准备素材」（原版 `steps/Step1Prepare.vue` + `MarkdownWorkbench.vue`）。
//!
//! 顶栏（名称、文件名标签、字数、草稿状态、AI 优化、上传文件）、格式工具行、
//! Markdown 编辑器 + 按页预览（P1/P2… 卡片，滚动同步）、分页粒度、幻灯片比例、
//! 素材库、底栏「开始 AI 生成」。文稿规则全部用 `huanshu_core::markdown_tools`
//! （与原版 TS 逐字对照、快照测试守住）。

use std::path::PathBuf;

use gpui_kit::component::input::{Editor, EditorState};
use gpui_kit::component::text::TextView;
use gpui_kit::component::Disableable as _;
use gpui_kit::prelude::*;
use gpui_kit::{
    canvas, div, fill, img, point, px, size, Bounds, Context, Entity, ExternalPaths, Hsla, Render,
    ScrollHandle, SharedString, Subscription, WeakEntity, Window,
};
use huanshu_core::markdown_tools::{self, Granularity, HeadingNumberStyle};
use huanshu_core::services::projects::Step1ProjectMeta;
use huanshu_core::services::{generation, media, projects};
use huanshu_core::types::AspectRatio;

use super::{ProjectModel, WorkflowPage};
use crate::formulas;
use crate::icons::carbon;
use crate::platform;
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
    /// 预览里的公式图（`formulas.rs`）：`TextView` 的插件只读它，出图在后台。
    formulas: Entity<formulas::FormulaCache>,
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
        // 文稿名不在这一步编辑：它显示在标题栏正中，点它改名（`WorkflowPage::title_element`）。
        let md = model.read(cx).md_content.clone();
        let editor = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("markdown")
                .placeholder(EDITOR_PLACEHOLDER)
                .default_value(md)
        });
        let formulas = cx.new(|_| formulas::FormulaCache::new());
        let subscriptions = vec![
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
            // 公式出图落地（或失败）后重画预览：插件的渲染只读缓存，画什么由这一次通知决定。
            cx.observe(&formulas, |_this: &mut Self, _cache, cx| cx.notify()),
        ];
        Self {
            model,
            handle,
            page,
            editor,
            optimizing: false,
            optimize_error: String::new(),
            drag_over: false,
            media_drop_over: false,
            preview_scroll: ScrollHandle::new(),
            sync_echo: None,
            formulas,
            subscriptions,
        }
    }

    /// 离开工作流 / 换步骤：交还公式图（`WorkflowPage::release` 调，README 规矩 5）。
    /// 这里拿不到 window，`drop_all` 用 `None`（与 `SlideImages::release` 一致）。
    pub fn release(&mut self, cx: &mut Context<Self>) {
        let images = self.formulas.update(cx, |cache, _| cache.drain());
        crate::images::drop_all(images, None, cx);
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
        cx.notify();
    }

    /// 上传文件（原版 `openFile`：选 `.md` / `.txt`）。
    fn open_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        platform::pick_paths(
            self,
            platform::Pick::File(platform::MARKDOWN),
            window,
            cx,
            |this, mut paths, window, cx| this.load_path(paths.remove(0), window, cx),
        );
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
        let (name, md, aspect, meta) = {
            let model = self.model.read(cx);
            (
                if model.name.trim().is_empty() {
                    "未命名演示".to_string()
                } else {
                    model.name.trim().to_string()
                },
                model.md_content.clone(),
                model.aspect,
                // 原版 `persistStep1Meta(projectId.value, step1State.value)`：新建时写的是**当前
                // 草稿**的文件名 / 粒度 / 比例（`WorkflowView.vue:59-69, 190-199`），不是默认值。
                Step1ProjectMeta {
                    md_filename: model.md_filename.clone(),
                    granularity: model.granularity.as_str().to_string(),
                    aspect_ratio: super::aspect_str(model.aspect).to_string(),
                },
            )
        };
        let shared = self.handle.shared().clone();
        run_bg(
            window,
            cx,
            move || {
                let id = projects::create_project(&shared, name, md)?;
                projects::open_project(&shared, id)?;
                projects::set_step1_project_meta(&shared, id, &meta)?;
                // 原版 `persistStep1Meta` 顺带 `updateAspectRatio`（`:196-198`）：项目的比例列
                // 也要写下，否则这一步新建的项目在库里一直是默认比例。
                projects::update_project_aspect_ratio(&shared, id, aspect)?;
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
    /// 已知偏差：编辑器默认 soft-wrap（`gpui-base-0.7.3/src/input/base/state.rs:812`），
    /// 折行会让这里的行数估计偏小、比例偏大，长文稿折行多时会比原版更「激进」地把预览滚到底。
    /// 底部由框架钳住（`update_scroll_offset`，`state.rs:2607-2624`），两端一致，中间可能有偏差。
    ///
    /// **编辑器侧的真实最大滚动量拿不到**（2026-10 逐接口查过，`gpui-kit 0.7.3`）：
    /// `EditorState` 就是 `InputBaseState<EditorMode>`（`gpui-base-0.7.3/src/input/editor/mod.rs:11`），
    /// 公开的滚动接口只有 `text_bounds()` / `input_bounds()` / `scroll_offset()`（`state.rs:640/644/3111`）、
    /// `set_scroll_offset()`（`:3118`）、`visible_row_range()`（`:3106`）。`text_bounds()` 返回的是
    /// **视口大小**的矩形（origin 跟着 scroll_offset 平移，size 是元素尺寸：`base/element.rs:2860`、
    /// `2983`、`3303-3311`），所以 `text_bounds.h − input_bounds.h` 恒为 0；真正的内容尺寸是私有的
    /// `scroll_size`（`pub(crate)`，`state.rs:495`，由 prepaint 写入 `element.rs:3315`），
    /// `DisplayMap::wrap_row_count()`（`editor/display_map/display_map.rs:375`）也隔着私有的 `display_map`。
    /// 唯一能读到真值的办法是「把偏移设到越界、下一帧读回被钳制的值」这种探针，代价是编辑器会可见地
    /// 跳到底部一帧，故不采用。见 `docs/native/audit-workflow-step1.md` B 节与 parity.md。
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
    ///
    /// **只挂了滚轮，滚动条拖拽挂不上**（2026-10 查过 `gpui-kit 0.7.3` 的接口）：
    /// 预览容器只有 `overflow_y_scroll()`，而这版 gpui 的 div **不画滚动条**
    /// （`gpui-pre-0.3.9/src/elements/div.rs` 里没有任何滚动条代码，只有 `styled.rs:64`
    /// 的 `scrollbar_width` 预留宽度），界面上没有可拖的滚动条；编辑器内建的滚动条
    /// （`gpui-base-0.7.3/src/input/base/element.rs:194` 的 `EditorScrollbar`）拖拽时只调
    /// `ScrollHandle::set_offset`（`gpui-pre-0.3.9/src/elements/div.rs:4400`），不通知实体、
    /// 不会触发 `cx.observe`。要挂只能自己放一个 `Scrollbar` 元素、把句柄包成自己的
    /// `ScrollbarHandle`（`gpui-base-0.7.3/src/scrollbar.rs:69-82` 有 `start_drag` /
    /// `end_drag` / `set_offset` 三个回调），本轮不做。见审计表 B 节。
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

/// 稿纸横线（便签主题，锤子便签式）：盖在编辑器上面、贯通全宽（含行号栏），每个显示行的底边一条。
///
/// 画在编辑器**之后**：编辑器在自己的 paint 里才落定本帧的滚动偏移（光标自动滚动），先画会慢一帧。
/// `input_bounds` 是去掉内边距后的文字区（窗口坐标），第一行的顶 = 它的顶 + 滚动偏移；
/// soft-wrap 的折行也是一行一个行高，所以按行高等距画就与文字行对齐。canvas 不注册命中区域，
/// 点击、滚轮照常落到编辑器上。
fn ruled_lines(editor: Entity<EditorState>, rule: Hsla) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, cx| {
            let state = editor.read(cx);
            let Some(line_height) = state.line_height() else {
                return;
            };
            let input = state.input_bounds();
            if line_height <= px(4.0) || input.size.height <= px(0.0) {
                return;
            }
            let first_row_top = input.origin.y + state.scroll_offset().y;
            // 从视口上方最近的一条线开始，免得长文稿滚到底时从第一行一路数下来。
            let skipped = ((bounds.top() - first_row_top) / line_height)
                .floor()
                .max(0.0);
            let mut y = first_row_top + line_height * (skipped + 1.0);
            while y < bounds.bottom() {
                if y > bounds.top() {
                    window.paint_quad(fill(
                        Bounds::new(
                            point(bounds.left(), y - px(1.0)),
                            size(bounds.size.width, px(1.0)),
                        ),
                        rule,
                    ));
                }
                y += line_height;
            }
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

impl Render for Step1 {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.studio().colors;
        // 编辑器外观与稿纸（便签主题才有 `paper`），见 `theme/editor.rs`。
        let look = cx.studio().editor.clone();
        let paper = cx.studio().paper;
        let editor_font = look.font.clone().unwrap_or_else(|| widgets::code_font(cx));
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

        // 公式缓存淘汰下来的位图在这里真正交还 GPUI（`RenderImage` 不回收就漏图集 tile，
        // 见 `images.rs`）。顺路做，不额外占一帧。
        let released = self.formulas.update(cx, |cache, _| cache.take_released());
        crate::images::drop_all(released, Some(window), cx);
        // 插件每帧重建都可以（形状稳定 → 框架不重新解析文档）；实例里只有缓存实体的句柄。
        let md_extensions = formulas::extensions(&self.formulas);

        let optimizing = self.optimizing;
        let optimize_error = self.optimize_error.clone();
        let tools_disabled = optimizing || !has_md;

        let tool_labels = [
            "自动标题",
            "清除标题加粗",
            "删除序号",
            "`##` 一、二、",
            "`###` （一）（二）",
            "`####` 1. 2.",
        ];
        let draft_failed =
            matches!(draft_state, super::DraftSaveState::Error) && !draft_error.is_empty();
        let top_bar = div()
            .flex_none()
            .h(px(super::HEADER_H))
            .px(px(18.0))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(10.0))
            .bg(c.panel)
            .when_some(paper, |bar, p| bar.bg(widgets::paper_bar(&p)))
            .border_b_1()
            .border_color(c.border)
            // 格式工具占据原来「准备素材」标签与名称框的位置（名称挪到了标题栏正中）；
            // 窗口窄时工具组先被裁，右侧的字数与操作按钮始终完整。
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(4.0))
                    .child(
                        div()
                            .flex_none()
                            .pr(px(8.0))
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
                    })),
            )
            .when(!md_filename.is_empty(), |bar| {
                // 原版 `.filename-tag`：primary 色 + primary-bg + primary-border（`WorkflowView.vue:1030-1044`）。
                bar.child(widgets::chip_icon(
                    "document",
                    md_filename.clone(),
                    Tone2::Primary,
                    cx,
                ))
            })
            .when(!draft_text.is_empty(), |bar| {
                bar.child(widgets::chip(draft_text, Tone2::Muted, cx))
            })
            // 原版 `.char-pill`（muted + surface + border，`WorkflowView.vue:1046-1056`），不是 `.accent-pill`（`:1057-1063`）。
            .child(widgets::chip(format!("{chars} 字"), Tone2::Muted, cx))
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

        // 草稿保存失败、AI 优化失败：工具栏下面一条提示（原来挤在工具行里）。
        let errors = (draft_failed || !optimize_error.is_empty()).then(|| {
            div()
                .flex_none()
                .px(px(18.0))
                .py(px(6.0))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(14.0))
                .border_b_1()
                .border_color(c.border)
                .when(draft_failed, |row| {
                    row.child(widgets::inline_error(draft_error, cx))
                })
                .when(!optimize_error.is_empty(), |row| {
                    row.child(widgets::inline_error(optimize_error, cx))
                })
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
                // 便签主题：原版 `.md-pane-label { background: var(--studio-panel-2) }`。
                .when(paper.is_some(), |head| head.bg(c.panel_2))
                .border_b_1()
                .border_color(c.border)
                .child(carbon(icon))
                .child(label)
        };

        // 预览页卡片（原版 `MarkdownWorkbench.vue` 的 `md-preview-slide`）：
        // 空文稿也是一张「P1」卡片 + 空态文案；否则按粒度切页，每页 `TextView::markdown`
        // （挂上公式扩展：`$…$` / `$$…$$` 由 `formulas.rs` 出图）。
        let preview_body = |index: usize, content: Option<String>| match content {
            Some(chunk) => div()
                .font_family(cx.studio().body_font.clone())
                .p(px(18.0))
                .child(
                    TextView::markdown(("md-preview", index), chunk)
                        .markdown_extensions(md_extensions.clone()),
                )
                .into_any_element(),
            None => div()
                .p(px(18.0))
                .text_size(px(13.0))
                .text_color(c.muted)
                .child("预览区将实时显示 Markdown 渲染效果")
                .into_any_element(),
        };
        // 便签主题：锤子便签列表里的一张便签——卡片纸、细纸边、纸张阴影、左侧页边色带，
        // 页码放在顶上的信息行（便签的日期行），下面一条细线再接正文。
        const NOTE_MARGIN: f32 = 26.0;
        let note_card = |p: crate::theme::PaperLook, index: usize, content: Option<String>| {
            div()
                .relative()
                .flex_none()
                .rounded(px(2.0))
                .border_1()
                .border_color(p.edge)
                .bg(p.card)
                .shadow(widgets::paper_shadow(&p))
                .overflow_hidden()
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .bottom_0()
                        .w(px(NOTE_MARGIN))
                        .bg(p.margin)
                        .border_r_1()
                        .border_color(p.margin_line),
                )
                .child(
                    div()
                        .ml(px(NOTE_MARGIN))
                        .h(px(30.0))
                        .px(px(14.0))
                        .flex()
                        .flex_row()
                        .items_center()
                        .border_b_1()
                        .border_color(p.edge)
                        .text_size(px(12.0))
                        .text_color(c.muted)
                        .child(format!("P{}", index + 1)),
                )
                .child(
                    div()
                        .ml(px(NOTE_MARGIN))
                        .text_color(p.ink)
                        .child(preview_body(index, content)),
                )
        };
        let preview_card = |index: usize, content: Option<String>| {
            if let Some(p) = paper {
                return note_card(p, index, content).into_any_element();
            }
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
                        .text_color(c.muted)
                        .child(format!("P{}", index + 1)),
                )
                .child(preview_body(index, content))
                .into_any_element()
        };
        let preview_cards: Vec<gpui_kit::AnyElement> = if !has_md {
            vec![preview_card(0, None)]
        } else {
            let resolved = match granularity {
                Granularity::Auto => detected,
                other => other,
            };
            markdown_tools::split_preview_pages(md.trim(), resolved)
                .into_iter()
                .enumerate()
                .map(|(i, chunk)| preview_card(i, Some(chunk)))
                .collect()
        };

        // 两栏宽度按原版 `.md-panes` 的 `minmax(0, 1.15fr) minmax(260px, 0.85fr)`
        // （`MarkdownWorkbench.vue:395`）：编辑区略宽于预览区，预览区最小 260px。
        let workbench = div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_row()
            .child(
                div()
                    .id("step1-editor-col")
                    .relative()
                    .flex_grow(1.15)
                    .flex_shrink(1.0)
                    .flex_basis(px(0.0))
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
                    // 编辑器不画自己的边框（原版 CodeMirror 铺满窗格）；字体、字号、行高随主题
                    // 字体始终读取「编辑器」设置；便签主题仅改变字号、行高与稿纸。
                    .child(
                        div()
                            .relative()
                            .flex_1()
                            .min_h_0()
                            .when_some(look.background, |area, bg| area.bg(bg))
                            .child(
                                Editor::new(&self.editor)
                                    .bordered(false)
                                    .size_full()
                                    .font_family(editor_font)
                                    .text_size(px(look.text_size))
                                    .line_height(px(look.line_height)),
                            )
                            .when_some(look.rule, |area, rule| {
                                area.child(ruled_lines(self.editor.clone(), rule))
                            }),
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
                    .flex_grow(0.85)
                    .flex_shrink(1.0)
                    .flex_basis(px(0.0))
                    .min_w(px(260.0))
                    .flex()
                    .flex_col()
                    // 不画底色：便签主题下卡片之间露出外壳铺的木纹桌面。
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
        //
        // 原版 `.media-thumb-wrap` / `.media-thumb`（`WorkflowView.vue:1194-1227`）：68px 宽的竖排小卡，
        // 缩略图 68×51、名字居中省略（`.media-name` `:1251-1260`）、删除按钮**只在该缩略图悬停时**
        // 才显示（`.media-del-btn` `:1229-1249`）。
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
            let group = SharedString::from(format!("media-thumb-{id}"));
            div()
                .flex()
                .flex_none()
                .flex_col()
                .items_center()
                .gap(px(2.0))
                .w(px(68.0))
                .child(
                    div()
                        .id(SharedString::from(format!("media-thumb-box-{id}")))
                        .group(group.clone())
                        .relative()
                        .w(px(68.0))
                        .h(px(51.0))
                        .flex_none()
                        .rounded(px(6.0))
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
                            div().absolute().top(px(2.0)).right(px(2.0)).child(
                                widgets::small(widgets::icon_button(
                                    SharedString::from(format!("media-del-{id}")),
                                    "close",
                                    "移除",
                                ))
                                .opacity(0.0)
                                .group_hover(group.clone(), |style| style.opacity(1.0))
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
                        .max_w(px(68.0))
                        .truncate()
                        .text_size(px(10.0))
                        .text_color(c.muted)
                        .tooltip_text(name.clone())
                        .child(name),
                )
                .when(is_image && width.is_some() && height.is_some(), |col| {
                    col.child(div().text_size(px(9.5)).text_color(c.muted).child(format!(
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
                                        platform::pick_paths(
                                            this,
                                            platform::Pick::Files(platform::MEDIA),
                                            window,
                                            cx,
                                            |this, paths, window, cx| {
                                                this.add_media_paths(paths, window, cx)
                                            },
                                        );
                                    })),
                            ),
                    ),
            )
            .child(
                // 原版 `.media-drop-zone`：**始终**是虚线框（空与否都一样，`WorkflowView.vue:1157-1171`），
                // 拖入悬停时变 primary 色（`.dragover` `:1168-1171`）。
                div()
                    .id("media-drop-zone")
                    .w_full()
                    .min_h(px(120.0))
                    .p(px(8.0))
                    .flex()
                    .flex_col()
                    .rounded(px(8.0))
                    .border_1()
                    .border_dashed()
                    // 素材拖放悬停高亮同 `drag_over`，框架拿不到拖入事件（见 parity.md）。
                    .border_color(if self.media_drop_over {
                        c.primary
                    } else {
                        c.border
                    })
                    .when(self.media_drop_over, |zone| zone.bg(c.primary_bg))
                    .when(media.is_empty(), |zone| {
                        zone.child(
                            div()
                                .flex_1()
                                .flex()
                                .flex_col()
                                .items_center()
                                .justify_center()
                                .gap(px(6.0))
                                .child(carbon("image-copy").size(px(28.0)).text_color(c.muted))
                                .child(
                                    div()
                                        .text_size(px(12.0))
                                        .text_color(c.muted)
                                        .child("拖拽图片 / 视频到此处"),
                                ),
                        )
                    })
                    .when(!media.is_empty(), |zone| {
                        zone.child(
                            // 原版 `.media-grid`：flex-wrap 的横排小卡（每张 68px 宽，`WorkflowView.vue:1188-1192`）。
                            div()
                                .w_full()
                                .flex()
                                .flex_row()
                                .flex_wrap()
                                .gap(px(8.0))
                                .children(
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
            .children(errors)
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
