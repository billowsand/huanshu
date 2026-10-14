//! 导出：保存对话框、PDF 结果摘要与结果对话框（原版 `PdfExportDialog.vue`）。
//!
//! 渲染是同步的重活，在后台线程上跑（见 `app::export_pdf`）；UI 线程只改按钮状态。

use std::path::PathBuf;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dialog::{Dialog, DialogClose, DialogFooter};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::WindowExt as _;
use gpui_kit::prelude::*;
use gpui_kit::{div, px, App, Window};
use huanshu_core::services::export::PdfExportReport;

use crate::theme::StudioExt as _;

/// 弹原生保存对话框。用户取消时返回 `None`。
pub fn ask_save_path(default_name: &str, filter: &str, extension: &str) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_file_name(default_name)
        .add_filter(filter, &[extension])
        .save_file()
}

/// PDF 导出完成后的结果对话框：页数、占位页、裁切页、缺图、主题回落，带「在文件夹中显示」。
/// 失败时是一条错误轻提示。
pub fn show_report(result: Result<PdfExportReport, String>, window: &mut Window, cx: &mut App) {
    let report = match result {
        Ok(report) => report,
        Err(error) => {
            window.push_notification(Notification::error(format!("导出失败：{error}")), cx);
            return;
        }
    };
    window.open_dialog(cx, move |dialog: Dialog, _window, cx| {
        let c = cx.studio().colors;
        let path = report.path.clone();
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
            .gap(px(6.0))
            .child(line("文件", report.path.clone()))
            .child(line(
                "页数",
                format!("{} 页 · {} ms", report.pages, report.elapsed_ms),
            ));
        if !report.placeholder_slides.is_empty() {
            body = body.child(line(
                "占位页",
                report
                    .placeholder_slides
                    .iter()
                    .map(|slide| format!("第 {} 页（{}）", slide.index + 1, slide.kind))
                    .collect::<Vec<_>>()
                    .join("、"),
            ));
        }
        if !report.overflow_slides.is_empty() {
            body = body.child(line(
                "裁切页",
                report
                    .overflow_slides
                    .iter()
                    .map(|index| format!("第 {} 页", index + 1))
                    .collect::<Vec<_>>()
                    .join("、"),
            ));
        }
        if !report.missing_assets.is_empty() {
            body = body.child(line(
                "缺失图片",
                format!("{} 处", report.missing_assets.len()),
            ));
        }
        if let Some(theme) = &report.theme_fallback {
            body = body.child(line("主题", format!("「{theme}」不可用，已回落到便签")));
        }
        dialog.title("PDF 导出完成").child(body).footer(
            DialogFooter::new()
                .child(DialogClose::new().trigger(|button| button.label("关闭")))
                .child(
                    Button::new("reveal")
                        .primary()
                        .label("在文件夹中显示")
                        .on_click(move |_event, window, cx| {
                            if let Err(error) = reveal(&path) {
                                window.push_notification(
                                    Notification::error(format!("打开文件夹失败：{error}")),
                                    cx,
                                );
                            }
                        }),
                ),
        )
    });
}

/// 一行摘要，够看出有没有页出问题。
pub fn summarize(report: &PdfExportReport) -> String {
    let mut parts = vec![format!("{} 页", report.pages)];
    if !report.placeholder_slides.is_empty() {
        let kinds: Vec<String> = report
            .placeholder_slides
            .iter()
            .map(|slide| slide.kind.clone())
            .collect();
        parts.push(format!("占位页 {}", kinds.join("、")));
    }
    if !report.overflow_slides.is_empty() {
        parts.push(format!("裁切页 {:?}", report.overflow_slides));
    }
    if !report.missing_assets.is_empty() {
        parts.push(format!("缺图 {}", report.missing_assets.len()));
    }
    if let Some(theme) = &report.theme_fallback {
        parts.push(format!("主题回落到 {theme}"));
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

    #[test]
    fn a_clean_export_only_reports_the_page_count() {
        assert!(summarize(&report(29)).contains("29 页"));
    }

    #[test]
    fn problems_are_listed_instead_of_hidden() {
        let mut value = report(4);
        value.placeholder_slides = vec![huanshu_core::services::export::PlaceholderSlide {
            index: 2,
            kind: "future_kind".into(),
        }];
        value.overflow_slides = vec![3];
        value.theme_fallback = Some("nope".into());
        let text = summarize(&value);
        assert!(text.contains("占位页 future_kind"), "{text}");
        assert!(text.contains("裁切页"), "{text}");
        assert!(text.contains("主题回落到 nope"), "{text}");
    }

    #[test]
    fn revealing_a_missing_file_reports_it() {
        assert!(reveal("C:/definitely-not-here.pdf").is_err());
    }
}
