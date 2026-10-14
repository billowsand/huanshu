//! 导出 PDF：保存对话框 + 结果摘要。
//!
//! 渲染是同步的重活，在后台线程上跑（见 `app::export_pdf`）；UI 线程只改按钮状态。

use std::path::PathBuf;

use huanshu_core::services::export::PdfExportReport;

/// 弹原生保存对话框。默认文件名 = 项目名 + `.pdf`。用户取消时返回 `None`。
pub fn ask_save_path(default_name: &str) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_file_name(default_name)
        .add_filter("PDF 文档", &["pdf"])
        .save_file()
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
