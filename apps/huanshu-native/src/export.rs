//! 导出：PDF / Typst 工程的进行中 / 结果 / 失败对话框（原版 `PdfExportDialog.vue`）。
//!
//! 渲染 / 打包是同步的重活，在后台线程上跑（见 `app::export_pdf` / `app::export_typst_project`）；
//! UI 线程只改按钮状态。三个对话框都挂在**窗口**上、状态是进程级的一份（原版 `usePdfExport` 的
//! 全局 `exporting` / `report` / `error`）：导出前 [`begin_export`]（PDF）或
//! [`begin_typst_export`]（Typst 工程）开「进行中」，结束时 [`show_report`] /
//! [`show_project_report`] 先收起它，再按结果开结果框（成功）或失败框（错误全文在里面）。
//! 首页（`pages/home.rs::export_pdf` / `export_typst`）与第三步
//! （`workflow/step3.rs::export_pdf`）共用其中一部分；进行中对话框由调用方在导出前各调一句。

use std::sync::atomic::{AtomicU8, Ordering};

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dialog::{DialogClose, DialogFooter};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, WindowExt as _};
use gpui_kit::prelude::*;
use gpui_kit::{div, px, AnyElement, App, Div, SharedString, Window};
use huanshu_core::editor_schema::{self, EditorSchema};
use huanshu_core::services::export::{
    PdfExportReport, PlaceholderSlide, TypstProjectReport, FALLBACK_THEME,
};
use huanshu_core::themes;

use crate::icons::carbon;
use crate::theme::{StudioColors, StudioExt as _};

// ── 「正在生成 PDF…」 ────────────────────────────────────────────────────────

/// 进行中对话框的三态：没开 / 开着 / 被用户关掉了。
///
/// 原版是「导出中不可关」的模态；原生允许关（✕ / Esc / 点遮罩），后台渲染不受影响，
/// 结果框照样会出来——`tasks::run_bg` 的结果回调在页面被换掉后不会执行（界面规矩 4），
/// 一个关不掉的模态会永远挡在窗口上。状态记住「开着」还是「被关掉」，是为了
/// [`show_report`] 不去关别人打开的对话框（比如用户自己的删除确认）。
static PROGRESS: AtomicU8 = AtomicU8::new(PROGRESS_IDLE);
const PROGRESS_IDLE: u8 = 0;
const PROGRESS_OPEN: u8 = 1;
const PROGRESS_DISMISSED: u8 = 2;

/// 打开「正在生成 PDF…」（原版 `PdfExportDialog.vue:49-55`）。**导出前**调用，
/// 结束后由 [`show_report`] 收起；重复调用只开一个。
///
/// 首次导出要扫系统字体、要几秒，这一步让用户看得见「在跑」；对话框盖住页面，
/// 导出期间点不到「导出」按钮（调用方自己的进行中标志是第二道闸）。
pub fn begin_export(window: &mut Window, cx: &mut App) {
    begin_export_with(
        window,
        cx,
        "正在生成 PDF…",
        "首次导出需要扫描系统字体，会稍慢一些。",
    );
}

/// 打开「正在打包 Typst 工程…」。与 [`begin_export`] 共用同一份进程级状态
/// （PDF 与工程导出互斥，见 `pages/home.rs` 的两个 `*_exporting` 标志）。
pub fn begin_typst_export(window: &mut Window, cx: &mut App) {
    begin_export_with(
        window,
        cx,
        "正在打包 Typst 工程…",
        "会一并收集模板、图片素材与图标，首次导出需要扫描系统字体，稍慢一些。",
    );
}

/// 进行中对话框的实际实现：标题与说明由调用方给（PDF / Typst 工程文案不同）。
/// 闭包要 `'static`，所以先把 `&str` 收成 `String` 再移动进去。
pub fn begin_export_with(window: &mut Window, cx: &mut App, title: &str, detail: &str) {
    if PROGRESS.swap(PROGRESS_OPEN, Ordering::SeqCst) == PROGRESS_OPEN {
        return;
    }
    let title = title.to_string();
    let detail = detail.to_string();
    window.open_dialog(cx, move |dialog, _window, cx| {
        let c = cx.studio().colors;
        dialog
            .overlay(true)
            .w(px(520.0))
            .on_close(|_event, _window, _cx| {
                PROGRESS.store(PROGRESS_DISMISSED, Ordering::SeqCst);
            })
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(10.0))
                    .py(px(16.0))
                    .child(Spinner::new().large().color(c.primary))
                    .child(div().text_size(px(18.0)).child(title.clone()))
                    .child(
                        div()
                            .text_size(px(14.0))
                            .text_color(c.muted)
                            .child(detail.clone()),
                    ),
            )
    });
}

/// 收起进行中对话框（如果还开着）。
fn finish_export(window: &mut Window, cx: &mut App) {
    if PROGRESS.swap(PROGRESS_IDLE, Ordering::SeqCst) == PROGRESS_OPEN {
        window.close_dialog(cx);
    }
}

// ── 结果 / 失败 ─────────────────────────────────────────────────────────────

/// PDF 导出完成后的结果对话框：页数、占位页、裁切页、缺图（每处一条）、主题回落。
/// **失败时开一个失败对话框、错误全文放在里面**（轻提示会截断长错误，看不到原因）。
///
/// `note` 是调用方要额外说的一句话（第三步有未应用的修改时会传「编辑器里还有未应用的
/// 修改，PDF 按应用前的内容导出。」，原版 `PdfExportDialog` 的第一条说明），没有就传 `None`。
pub fn show_report(
    result: Result<PdfExportReport, String>,
    note: Option<&str>,
    window: &mut Window,
    cx: &mut App,
) {
    finish_export(window, cx);
    match result {
        Ok(report) => open_report_dialog(report, note, window, cx),
        Err(error) => open_error_dialog(error, window, cx),
    }
}

/// 完整 Typst 工程导出完成后的结果对话框：文件、页数 / 文件数、字体、占位页、裁切页、
/// 缺图、主题回落，以及「解压后怎么编译」的常驻提示。**失败时复用 PDF 的失败对话框**
/// （错误全文在里面，长错误不会被截断）。
pub fn show_project_report(
    result: Result<TypstProjectReport, String>,
    note: Option<&str>,
    window: &mut Window,
    cx: &mut App,
) {
    finish_export(window, cx);
    match result {
        Ok(report) => open_project_report_dialog(report, note, window, cx),
        Err(error) => open_error_dialog(error, window, cx),
    }
}

fn open_project_report_dialog(
    report: TypstProjectReport,
    note: Option<&str>,
    window: &mut Window,
    cx: &mut App,
) {
    let path = report.path.clone();
    // 结果框只显示文件名：完整路径太长，要看具体位置用「在文件夹中显示」。
    let file_name = last_segment(&path).to_string();
    let summary = format!(
        "{} 页 · {} 个文件 · {} 秒",
        report.pages,
        report.files,
        seconds(report.elapsed_ms)
    );
    let notes = project_report_notes(&report, note);
    // 字体缺失、占位页、裁切、缺图、主题回落都算「要留意的几处」；调用方的 note 不算。
    let has_warnings = !report.missing_fonts.is_empty()
        || !report.placeholder_slides.is_empty()
        || !report.overflow_slides.is_empty()
        || !report.missing_assets.is_empty()
        || report.theme_fallback.is_some();
    window.open_dialog(cx, move |dialog, _window, cx| {
        dialog
            .w(px(520.0))
            .title(if has_warnings {
                "Typst 工程已导出，有几处需要留意"
            } else {
                "Typst 工程已导出"
            })
            .child(report_body(&file_name, &summary, &notes, None, "摘要", cx))
            .footer(reveal_footer(path.clone()))
    });
}

fn open_report_dialog(
    report: PdfExportReport,
    note: Option<&str>,
    window: &mut Window,
    cx: &mut App,
) {
    let path = report.path.clone();
    let summary = format!("{} 页 · {} 秒", report.pages, seconds(report.elapsed_ms));
    let notes = report_notes(&report, note);
    // 原版 `hasWarnings`：调用方的 note 不算「要留意的几处」。
    let has_warnings = !report.placeholder_slides.is_empty()
        || !report.overflow_slides.is_empty()
        || !report.missing_assets.is_empty()
        || report.theme_fallback.is_some();
    let theme_hint = report
        .theme_fallback
        .is_none()
        .then(|| "PDF 版式跟随当前主题。".to_string());
    window.open_dialog(cx, move |dialog, _window, cx| {
        dialog
            .w(px(520.0))
            .title(if has_warnings {
                "PDF 已导出，有几处需要留意"
            } else {
                "PDF 已导出"
            })
            .child(report_body(
                &path,
                &summary,
                &notes,
                theme_hint.as_deref(),
                "页数",
                cx,
            ))
            .footer(reveal_footer(path.clone()))
    });
}

/// 「在文件夹中显示」+「完成」这一排按钮：PDF 与 Typst 工程的结果框共用
/// （都要在文件管理器里定位刚写出的文件）。
fn reveal_footer(path: String) -> DialogFooter {
    DialogFooter::new()
        .child(
            Button::new("reveal")
                .ghost()
                .label("在文件夹中显示")
                .on_click(move |_event, window, cx| {
                    if let Err(error) = reveal(&path) {
                        window.push_notification(
                            Notification::error(format!("打开文件夹失败：{error}")),
                            cx,
                        );
                    }
                }),
        )
        .child(DialogClose::new().trigger(|button| button.primary().label("完成")))
}

/// 失败对话框（原版 `PdfExportDialog.vue:57-69`）：标题 + **错误全文** + 「知道了」。
/// 错误可能很长（Typst 的诊断、写文件的系统错误），所以给一块可滚动的区域，
/// 不让它在对话框底部被切掉。
pub(crate) fn open_error_dialog(error: String, window: &mut Window, cx: &mut App) {
    window.open_dialog(cx, move |dialog, _window, cx| {
        let c = cx.studio().colors;
        dialog
            .w(px(520.0))
            .title("导出失败")
            .child(
                div()
                    .id("pdf-error")
                    .w_full()
                    .max_h(px(320.0))
                    .overflow_y_scroll()
                    .p(px(12.0))
                    .rounded(px(10.0))
                    .bg(c.error_bg)
                    .text_size(px(13.0))
                    .text_color(c.error)
                    .child(error.clone()),
            )
            .footer(DialogClose::new().trigger(|button| button.primary().label("知道了")))
    });
}

/// 结果对话框的正文：文件、摘要（PDF 是页数，工程是页数 / 文件数 / 耗时），
/// 然后是说明列表（有的话）与常驻的版式提示。
fn report_body(
    path: &str,
    summary: &str,
    notes: &[ReportNote],
    theme_hint: Option<&str>,
    summary_label: &str,
    cx: &App,
) -> Div {
    let c = cx.studio().colors;
    let line = |label: &str, value: String| {
        div()
            .flex()
            .flex_row()
            .gap(px(8.0))
            .text_size(px(13.0))
            .child(
                div()
                    .w(px(72.0))
                    .text_color(c.muted)
                    .child(label.to_string()),
            )
            .child(div().flex_1().child(value))
    };
    let mut body = div()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(line("文件", path.to_string()))
        .child(line(summary_label, summary.to_string()));
    if !notes.is_empty() {
        let mono = cx.theme().mono_font_family.clone();
        body = body.child(
            div()
                .id("pdf-notes")
                .w_full()
                .max_h(px(220.0))
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .p(px(12.0))
                .rounded(px(10.0))
                .border_1()
                .border_color(c.border)
                .bg(c.surface)
                .children(notes.iter().map(|note| note_row(note, c, mono.clone()))),
        );
    }
    if let Some(hint) = theme_hint {
        body = body.child(
            div()
                .text_size(px(12.0))
                .text_color(c.muted)
                .child(hint.to_string()),
        );
    }
    body
}

/// 一条说明：左侧图标 + 右侧文字（原版 `.pdf-notes li`）。
fn note_row(note: &ReportNote, c: StudioColors, mono: SharedString) -> AnyElement {
    let row = || {
        div()
            .flex()
            .flex_row()
            .items_start()
            .gap(px(8.0))
            .text_size(px(13.0))
    };
    let icon = |name: &'static str| div().flex_none().text_color(c.muted).child(carbon(name));
    match note {
        ReportNote::Line { icon: name, text } => row()
            .child(icon(name))
            .child(div().flex_1().child(text.clone()))
            .into_any_element(),
        ReportNote::MissingAsset {
            page,
            reference,
            reason,
        } => row()
            .child(icon("image"))
            .child(div().flex_none().child(format!("第 {page} 页图片")))
            .child(
                div()
                    .flex_none()
                    .px(px(4.0))
                    .rounded(px(4.0))
                    .bg(c.primary_bg)
                    .text_color(c.primary)
                    .text_size(px(12.0))
                    .font_family(mono)
                    .child(reference.clone()),
            )
            .child(div().flex_1().child(format!("未导出：{reason}")))
            .into_any_element(),
    }
}

/// 结果对话框里的一条说明（原版 `.pdf-notes` 的 `li`，顺序同 `PdfExportDialog.vue:85-111`）。
#[derive(Debug, PartialEq, Eq)]
enum ReportNote {
    /// 一行说明：图标 + 文案。
    Line { icon: &'static str, text: String },
    /// 缺图：页码、引用与原因各给一份，引用用等宽字显示
    /// （原版「第 N 页图片 `引用` 未导出：原因」）。
    MissingAsset {
        page: usize,
        reference: String,
        reason: String,
    },
}

/// 说明列表：调用方提示 → 占位页（按版式分组）→ 裁切页 → 缺图（每处一条）→ 主题回落。
fn report_notes(report: &PdfExportReport, note: Option<&str>) -> Vec<ReportNote> {
    let mut notes = Vec::new();
    if let Some(note) = note {
        notes.push(ReportNote::Line {
            icon: "information",
            text: note.to_string(),
        });
    }
    let groups = placeholder_groups(&report.placeholder_slides, &editor_schema::editor_schema());
    if !groups.is_empty() {
        notes.push(ReportNote::Line {
            icon: "document-blank",
            text: format!(
                "以下模板还没有 PDF 版式，导出为占位页：{}",
                groups.join("；")
            ),
        });
    }
    if !report.overflow_slides.is_empty() {
        notes.push(ReportNote::Line {
            icon: "fit-to-screen",
            text: format!(
                "{} 内容过多，已缩到最小字号仍放不下，超出部分被裁掉。",
                page_list(&report.overflow_slides)
            ),
        });
    }
    for asset in &report.missing_assets {
        notes.push(ReportNote::MissingAsset {
            page: asset.slide + 1,
            reference: asset.reference.clone(),
            reason: asset.reason.clone(),
        });
    }
    if let Some(theme) = &report.theme_fallback {
        notes.push(ReportNote::Line {
            icon: "color-palette",
            text: format!(
                "主题「{}」还没有 PDF 版式，已回落到「{}」。",
                themes::theme_name(theme),
                themes::theme_name(FALLBACK_THEME)
            ),
        });
    }
    notes
}

/// 工程导出的说明列表，顺序固定：调用方提示 → 缺字体 → 占位页（按版式分组）→ 裁切页 →
/// 缺图（每处一条）→ 主题回落 → 「解压后怎么编译」的常驻提示（这条永远在最后）。
fn project_report_notes(report: &TypstProjectReport, note: Option<&str>) -> Vec<ReportNote> {
    let mut notes = Vec::new();
    if let Some(note) = note {
        notes.push(ReportNote::Line {
            icon: "information",
            text: note.to_string(),
        });
    }
    // 缺字体不阻断导出（工程里保留了字体设置），只是提醒在别的机器上编译时可能回落到默认字体。
    if !report.missing_fonts.is_empty() {
        notes.push(ReportNote::Line {
            icon: "text-font",
            text: format!(
                "本机未安装这些字体：{}。工程按用户字体设置排版；在装有这些字体的系统上用 Typst 编译效果最准确，缺失时 Typst 会回落到默认字体。",
                report.missing_fonts.join("、")
            ),
        });
    }
    let groups = placeholder_groups(&report.placeholder_slides, &editor_schema::editor_schema());
    if !groups.is_empty() {
        notes.push(ReportNote::Line {
            icon: "document-blank",
            text: format!(
                "以下模板还没有 Typst 版式，导出为占位页：{}",
                groups.join("；")
            ),
        });
    }
    if !report.overflow_slides.is_empty() {
        notes.push(ReportNote::Line {
            icon: "fit-to-screen",
            text: format!(
                "{} 内容过多，已缩到最小字号仍放不下，超出部分被裁掉。",
                page_list(&report.overflow_slides)
            ),
        });
    }
    for asset in &report.missing_assets {
        notes.push(ReportNote::MissingAsset {
            page: asset.slide + 1,
            reference: asset.reference.clone(),
            reason: asset.reason.clone(),
        });
    }
    if let Some(theme) = &report.theme_fallback {
        notes.push(ReportNote::Line {
            icon: "color-palette",
            text: format!(
                "主题「{}」还没有 Typst 版式，已回落到「{}」。",
                themes::theme_name(theme),
                themes::theme_name(FALLBACK_THEME)
            ),
        });
    }
    // 常驻提示：工程是源码而不是成品，告诉用户怎么得到 PDF。
    notes.push(ReportNote::Line {
        icon: "terminal",
        text: "解压后在工程目录运行 typst compile main.typ deck.pdf；字体与目录结构见 README.md。"
            .to_string(),
    });
    notes
}

/// 占位页按版式分组：「封面（第 3、5 页）；组织架构（第 7 页）」。
///
/// 版式用 `editor_schema` 的中文名而不是 `kind` 原值（原版 `kindLabel`：
/// `schemaStore.kindMeta[kind]?.label ?? kind`；schema 里没有的 kind 原样显示）。
fn placeholder_groups(
    placeholder_slides: &[PlaceholderSlide],
    schema: &EditorSchema,
) -> Vec<String> {
    let mut kinds: Vec<&str> = Vec::new();
    let mut pages: Vec<Vec<usize>> = Vec::new();
    for slide in placeholder_slides {
        match kinds.iter().position(|kind| *kind == slide.kind) {
            Some(index) => pages[index].push(slide.index),
            None => {
                kinds.push(&slide.kind);
                pages.push(vec![slide.index]);
            }
        }
    }
    kinds
        .iter()
        .zip(pages)
        .map(|(kind, pages)| {
            let label = schema
                .kind(kind)
                .map(|entry| entry.label.clone())
                .unwrap_or_else(|| (*kind).to_string());
            format!("{label}（{}）", page_list(&pages))
        })
        .collect()
}

/// 「第 3、5 页」。下标是 0 基，页码是 1 基（原版 `pageList`）。
fn page_list(indices: &[usize]) -> String {
    let pages: Vec<String> = indices
        .iter()
        .map(|index| (index + 1).to_string())
        .collect();
    format!("第 {} 页", pages.join("、"))
}

/// 导出耗时（原版 `(report.elapsed_ms / 1000).toFixed(1)`）。
fn seconds(elapsed_ms: u128) -> String {
    format!("{:.1}", elapsed_ms as f64 / 1000.0)
}

/// 路径的最后一段（结果框只显示文件名；`/` 与反斜杠都算分隔符，两个平台一致）。
fn last_segment(path: &str) -> &str {
    // Both separators on every platform: the report path comes from the host, but
    // the helper is also exercised with Windows-style paths in its unit test.
    path.rsplit(|ch: char| ch == '/' || ch == '\\')
        .find(|part| !part.is_empty())
        .unwrap_or(path)
}

/// 一行摘要，够看出有没有页出问题（命令行 `--export-pdf` 用）。
pub fn summarize(report: &PdfExportReport) -> String {
    let mut parts = vec![format!("{} 页", report.pages)];
    let groups = placeholder_groups(&report.placeholder_slides, &editor_schema::editor_schema());
    if !groups.is_empty() {
        parts.push(format!("占位页 {}", groups.join("；")));
    }
    if !report.overflow_slides.is_empty() {
        parts.push(format!("裁切页 {}", page_list(&report.overflow_slides)));
    }
    if !report.missing_assets.is_empty() {
        parts.push(format!("缺图 {} 处", report.missing_assets.len()));
    }
    if let Some(theme) = &report.theme_fallback {
        parts.push(format!(
            "主题 {} 回落到 {}",
            themes::theme_name(theme),
            themes::theme_name(FALLBACK_THEME)
        ));
    }
    format!("{} · {}", report.path, parts.join(" · "))
}

/// 一行摘要，够看出工程里有没有要留意的地方（命令行 `--export-typst` 用）。
pub fn summarize_project(report: &TypstProjectReport) -> String {
    let mut parts = vec![
        format!("{} 页", report.pages),
        format!("{} 个文件", report.files),
    ];
    if !report.missing_fonts.is_empty() {
        parts.push(format!("缺字体 {} 个", report.missing_fonts.len()));
    }
    let groups = placeholder_groups(&report.placeholder_slides, &editor_schema::editor_schema());
    if !groups.is_empty() {
        parts.push(format!("占位页 {}", groups.join("；")));
    }
    if !report.overflow_slides.is_empty() {
        parts.push(format!("裁切页 {}", page_list(&report.overflow_slides)));
    }
    if !report.missing_assets.is_empty() {
        parts.push(format!("缺图 {} 处", report.missing_assets.len()));
    }
    if let Some(theme) = &report.theme_fallback {
        parts.push(format!(
            "主题 {} 回落到 {}",
            themes::theme_name(theme),
            themes::theme_name(FALLBACK_THEME)
        ));
    }
    format!("{} · {}", report.path, parts.join(" · "))
}

/// 结果对话框里的「在文件夹中显示」按钮；打不开时把错误还给调用方。
pub fn reveal(path: &str) -> Result<(), String> {
    huanshu_core::services::export::reveal_in_file_manager(std::path::Path::new(path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use huanshu_core::services::export::PlaceholderSlide;
    use slide_typst::MissingAsset;

    fn report(pages: usize) -> PdfExportReport {
        PdfExportReport {
            path: "C:/tmp/deck.pdf".into(),
            pages,
            placeholder_slides: Vec::new(),
            overflow_slides: Vec::new(),
            missing_assets: Vec::new(),
            theme_fallback: None,
            elapsed_ms: 12,
        }
    }

    /// 只看文案，图标名不参与比对（图标是表现层，文案才是信息）。
    fn texts(notes: &[ReportNote]) -> Vec<String> {
        notes
            .iter()
            .map(|note| match note {
                ReportNote::Line { text, .. } => text.clone(),
                ReportNote::MissingAsset {
                    page,
                    reference,
                    reason,
                } => format!("第 {page} 页图片 {reference} 未导出：{reason}"),
            })
            .collect()
    }

    #[test]
    fn a_clean_export_only_reports_the_page_count() {
        assert!(summarize(&report(29)).contains("29 页"));
        assert!(report_notes(&report(29), None).is_empty());
    }

    /// 缺图必须一处一条，带页码、引用与原因（原版这里给的正是这三样）。
    #[test]
    fn every_missing_image_lists_page_reference_and_reason() {
        let mut value = report(4);
        value.missing_assets = vec![
            MissingAsset {
                slide: 1,
                reference: "media:7".into(),
                reason: "素材目录中没有这个引用".into(),
            },
            MissingAsset {
                slide: 3,
                reference: "https://example.com/a.png".into(),
                reason: "导出时不下载远程图片".into(),
            },
        ];
        let notes = report_notes(&value, None);
        assert_eq!(
            notes,
            vec![
                ReportNote::MissingAsset {
                    page: 2,
                    reference: "media:7".into(),
                    reason: "素材目录中没有这个引用".into(),
                },
                ReportNote::MissingAsset {
                    page: 4,
                    reference: "https://example.com/a.png".into(),
                    reason: "导出时不下载远程图片".into(),
                },
            ]
        );
        assert!(texts(&notes)[0].starts_with("第 2 页图片 media:7 未导出："));
        // 一行摘要里至少要有「几处」，明细在对话框里。
        assert!(summarize(&value).contains("缺图 2 处"));
    }

    /// 占位页用版式的中文名（`editor_schema` 的 label），按版式分组给页码；
    /// schema 里没有的 kind 原样显示。
    #[test]
    fn placeholder_pages_group_by_kind_and_use_the_schema_label() {
        let mut value = report(6);
        value.placeholder_slides = vec![
            PlaceholderSlide {
                index: 2,
                kind: "cover".into(),
            },
            PlaceholderSlide {
                index: 4,
                kind: "cover".into(),
            },
            PlaceholderSlide {
                index: 5,
                kind: "future_kind".into(),
            },
        ];
        let label = editor_schema::editor_schema()
            .kind("cover")
            .expect("cover is in the schema")
            .label
            .clone();
        assert_eq!(
            texts(&report_notes(&value, None)),
            vec![format!(
                "以下模板还没有 PDF 版式，导出为占位页：{label}（第 3、5 页）；future_kind（第 6 页）"
            )]
        );
        assert!(
            !texts(&report_notes(&value, None))[0].contains("cover（"),
            "不能给 kind 原值"
        );
    }

    /// 裁切页要有解释，主题回落要给主题名（不是 id），调用方的 note 排在最前面。
    #[test]
    fn clipped_pages_and_the_theme_fallback_explain_themselves() {
        let mut value = report(9);
        value.overflow_slides = vec![2, 7];
        value.theme_fallback = Some("dark-blue".into());
        let notes = report_notes(&value, Some("编辑器里还有未应用的修改。"));
        assert_eq!(
            texts(&notes),
            vec![
                "编辑器里还有未应用的修改。".to_string(),
                "第 3、8 页 内容过多，已缩到最小字号仍放不下，超出部分被裁掉。".to_string(),
                format!(
                    "主题「暗·星图」还没有 PDF 版式，已回落到「{}」。",
                    themes::theme_name(FALLBACK_THEME)
                ),
            ]
        );
    }

    /// 未知主题 id 也不能变成空白：名字查不到就用 id 本身。
    #[test]
    fn an_unknown_fallback_theme_still_reads() {
        let mut value = report(2);
        value.theme_fallback = Some("no-such-theme".into());
        assert_eq!(
            texts(&report_notes(&value, None)),
            vec![format!(
                "主题「no-such-theme」还没有 PDF 版式，已回落到「{}」。",
                themes::theme_name(FALLBACK_THEME)
            )]
        );
    }

    /// 一行摘要（命令行用）也要给中文名，不能只剩 kind 原值。
    #[test]
    fn summarize_uses_schema_labels_and_theme_names() {
        let mut value = report(4);
        value.placeholder_slides = vec![PlaceholderSlide {
            index: 0,
            kind: "future_kind".into(),
        }];
        value.overflow_slides = vec![3];
        value.theme_fallback = Some("nope".into());
        let text = summarize(&value);
        assert!(text.contains("占位页 future_kind（第 1 页）"), "{text}");
        assert!(text.contains("裁切页 第 4 页"), "{text}");
        assert!(text.contains("主题 nope 回落到"), "{text}");
    }

    fn project_report(pages: usize, files: usize) -> TypstProjectReport {
        TypstProjectReport {
            path: "C:/tmp/演示-typst.zip".into(),
            pages,
            files,
            fonts: vec!["微软雅黑".into()],
            missing_fonts: Vec::new(),
            placeholder_slides: Vec::new(),
            overflow_slides: Vec::new(),
            missing_assets: Vec::new(),
            theme_fallback: None,
            elapsed_ms: 2500,
        }
    }

    /// 干净的工程只说明怎么编译：那句常驻提示永远在最后。
    #[test]
    fn a_clean_project_explains_how_to_compile_it() {
        let notes = project_report_notes(&project_report(6, 12), None);
        assert_eq!(
            texts(&notes),
            vec!["解压后在工程目录运行 typst compile main.typ deck.pdf；字体与目录结构见 README.md。"]
        );
        assert_eq!(
            summarize_project(&project_report(6, 12)),
            "C:/tmp/演示-typst.zip · 6 页 · 12 个文件"
        );
    }

    /// 缺字体要一次列全（中文顿号），并解释「工程保留字体、别的机器上可能回落」。
    #[test]
    fn missing_fonts_are_named_together_with_the_fallback_note() {
        let mut value = project_report(3, 9);
        value.missing_fonts = vec!["霞鹜文楷".into(), "Fira Code".into()];
        assert_eq!(
            texts(&project_report_notes(&value, None))[0],
            "本机未安装这些字体：霞鹜文楷、Fira Code。工程按用户字体设置排版；在装有这些字体的系统上用 Typst 编译效果最准确，缺失时 Typst 会回落到默认字体。"
        );
        assert_eq!(
            summarize_project(&value),
            "C:/tmp/演示-typst.zip · 3 页 · 9 个文件 · 缺字体 2 个"
        );
    }

    /// 说明列表的顺序固定：调用方提示 → 缺字体 → 占位页 → 裁切页 → 缺图 → 主题回落 → 编译提示。
    #[test]
    fn project_notes_keep_the_documented_order() {
        let mut value = project_report(8, 20);
        value.missing_fonts = vec!["霞鹜文楷".into()];
        value.placeholder_slides = vec![PlaceholderSlide {
            index: 2,
            kind: "future_kind".into(),
        }];
        value.overflow_slides = vec![4];
        value.missing_assets = vec![MissingAsset {
            slide: 5,
            reference: "media:7".into(),
            reason: "素材目录中没有这个引用".into(),
        }];
        value.theme_fallback = Some("nope".into());
        let notes = texts(&project_report_notes(
            &value,
            Some("工程里包含未应用的修改。"),
        ));
        assert_eq!(notes.len(), 7);
        assert_eq!(notes[0], "工程里包含未应用的修改。");
        assert!(
            notes[1].starts_with("本机未安装这些字体：霞鹜文楷。"),
            "{}",
            notes[1]
        );
        assert_eq!(
            notes[2],
            "以下模板还没有 Typst 版式，导出为占位页：future_kind（第 3 页）"
        );
        assert_eq!(
            notes[3],
            "第 5 页 内容过多，已缩到最小字号仍放不下，超出部分被裁掉。"
        );
        assert_eq!(
            notes[4],
            "第 6 页图片 media:7 未导出：素材目录中没有这个引用"
        );
        assert!(notes[5].contains("已回落到"), "{}", notes[5]);
        assert!(notes[6].starts_with("解压后在工程目录运行"), "{}", notes[6]);
    }

    /// 命令行摘要把每一类问题都点名（缺图只给处数，明细在对话框里）。
    #[test]
    fn summarize_project_names_every_problem() {
        let mut value = project_report(4, 11);
        value.missing_fonts = vec!["A".into()];
        value.placeholder_slides = vec![PlaceholderSlide {
            index: 0,
            kind: "future_kind".into(),
        }];
        value.overflow_slides = vec![3];
        value.missing_assets = vec![MissingAsset {
            slide: 2,
            reference: "x.png".into(),
            reason: "missing".into(),
        }];
        value.theme_fallback = Some("nope".into());
        let text = summarize_project(&value);
        for part in [
            "4 页",
            "11 个文件",
            "缺字体 1 个",
            "占位页 future_kind（第 1 页）",
            "裁切页 第 4 页",
            "缺图 1 处",
            "主题 nope 回落到",
        ] {
            assert!(text.contains(part), "{text} 里缺少 {part}");
        }
    }

    /// 两个平台的分隔符都要认（路径来自 core，可能是任意一种写法）。
    #[test]
    fn the_last_segment_handles_both_separators() {
        assert_eq!(last_segment("C:/tmp/演示-typst.zip"), "演示-typst.zip");
        assert_eq!(last_segment("C:/tmp/"), "tmp");
        assert_eq!(last_segment(""), "");
        assert_eq!(
            last_segment(&format!("C:\\tmp\\演示-typst.zip")),
            "演示-typst.zip"
        );
    }

    #[test]
    fn revealing_a_missing_file_reports_it() {
        assert!(reveal("C:/definitely-not-here.pdf").is_err());
    }
}
