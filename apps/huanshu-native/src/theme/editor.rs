//! 第一步 Markdown 编辑器与预览的主题外观。
//!
//! 原版的编辑器是 CodeMirror：`MarkdownWorkbench.vue` 的 `editorTheme`（透明底、等宽 13px、行高 1.8、
//! `tok-*` 一组暗色主题用的高亮色），便签主题再由 `notes.css` / `notes-studio.css` 覆盖成稿纸
//! （纸色、32px 横线、页边、墨色标题）。GPUI Kit 的编辑器不认这些：它读 `Theme.highlight_theme`，
//! 不设就是自带的「Default Light / Dark」（白底、蓝色标题），所以每个主题都要在这里给一份。
//!
//! 便签主题按锤子便签的稿纸做：米白纸、贯通全宽的浅棕横线（行距 32px，与原版 `--notes-rule` 一致）、
//! 左侧略深的页边、宋体正文、深棕墨色；预览区是木纹桌面上的一叠便签卡。

use gpui_kit::component::highlighter::HighlightTheme;
use gpui_kit::{rgba, Hsla, SharedString};
use std::sync::Arc;

/// Markdown 语法的几类颜色（对应原版 `.tok-*`）。
#[derive(Debug, Clone, Copy)]
struct SyntaxTable {
    /// 标题文字（`.tok-heading`）。
    heading: u32,
    /// 粗体（`.tok-strong`）。界面一律常规体（README 规矩 12），粗体只靠颜色区分。
    strong: u32,
    /// 斜体（`.tok-emphasis`）。
    emphasis: u32,
    /// 链接文字（`.tok-link`）。
    link: u32,
    /// 行内代码与代码块（`.tok-monospace`）。
    code: u32,
    /// 标记符号：`#`、`>`、`*`、反引号、链接地址（`.tok-meta`）。
    meta: u32,
    /// 列表符号与分隔线（`.tok-list`）。
    list: u32,
}

#[derive(Debug, Clone, Copy)]
struct EditorTable {
    /// 编辑区底色；`None` 透明（原版 `backgroundColor: transparent`，落在工作区底色上）。
    background: Option<u32>,
    foreground: u32,
    /// 行号栏（原版 `.cm-gutters`）。
    gutter: u32,
    /// 当前行（`.cm-activeLine`）。
    active_line: u32,
    /// 选区（`.cm-selectionBackground`）。
    selection: u32,
    syntax: SyntaxTable,
}

fn table(theme_id: &str) -> EditorTable {
    // 原版 `editorTheme` 的 `tok-*`：为暗色主题写的，两套暗色主题照用。
    const ORIGINAL_DARK: SyntaxTable = SyntaxTable {
        heading: 0xf5d76eff,
        strong: 0xffd78aff,
        emphasis: 0x8fd6ffff,
        link: 0x87c3ffff,
        code: 0xffb37aff,
        meta: 0x7f8ca6ff,
        list: 0x7fe3c4ff,
    };
    match theme_id {
        "dark-yellow" => EditorTable {
            background: None,
            foreground: 0xe0e2ecff,
            gutter: 0xffffff05,
            active_line: 0xffffff08,
            selection: 0x78a7ff47,
            syntax: ORIGINAL_DARK,
        },
        "dark-blue" => EditorTable {
            background: None,
            foreground: 0xd4e8f8ff,
            gutter: 0xffffff05,
            active_line: 0xffffff08,
            selection: 0x78a7ff47,
            syntax: ORIGINAL_DARK,
        },
        // 终端两套：橙色主色当标题，绿色当代码（终端的「输出」色），其余退到灰阶。
        "dark-terminal" => EditorTable {
            background: None,
            foreground: 0xeeeedeff,
            gutter: 0xffffff05,
            active_line: 0xffffff08,
            selection: 0xed9b6240,
            syntax: SyntaxTable {
                heading: 0xed9b62ff,
                strong: 0xf4b787ff,
                emphasis: 0xd8d2b4ff,
                link: 0x8fb8c9ff,
                code: 0xa9c58cff,
                meta: 0x7d846fff,
                list: 0xd9b56eff,
            },
        },
        "light-terminal" => EditorTable {
            background: None,
            foreground: 0x25291fff,
            gutter: 0x25291f08,
            active_line: 0x25291f0a,
            selection: 0x9d4a1c2e,
            syntax: SyntaxTable {
                heading: 0x9d4a1cff,
                strong: 0x803b16ff,
                emphasis: 0x4f6d7aff,
                link: 0x2f6f8fff,
                code: 0x49682cff,
                meta: 0x8b947eff,
                list: 0xa16207ff,
            },
        },
        // 报刊：墨色标题，唯一强调色钴蓝给符号与链接，代码用报刊的印章红。
        "light-editorial" => EditorTable {
            background: None,
            foreground: 0x1c2723ff,
            gutter: 0x1c272308,
            active_line: 0x1d4ed80d,
            selection: 0x1d4ed82e,
            syntax: SyntaxTable {
                heading: 0x0f1714ff,
                strong: 0x0f1714ff,
                emphasis: 0x1a43bfff,
                link: 0x1d4ed8ff,
                code: 0xa8322aff,
                meta: 0xa39b8bff,
                list: 0x1d4ed8ff,
            },
        },
        // 便签·稿纸（锤子便签）：纸色、深棕墨、页边略深；强调色取自原版 notes.css 的覆盖。
        "light-notes" => EditorTable {
            background: Some(0xfbf7ecff),
            foreground: 0x5c4637ff,
            gutter: 0x8a6a4a0f,
            active_line: 0xc8a97a1a,
            selection: 0xdcc39c80,
            syntax: SyntaxTable {
                heading: 0x47321fff,
                strong: 0x47321fff,
                emphasis: 0x526773ff,
                link: 0x526773ff,
                code: 0x865137ff,
                meta: 0xb39d86ff,
                list: 0x54633dff,
            },
        },
        _ => table("dark-yellow"),
    }
}

/// 稿纸主题的纸张与桌面（便签主题专有；别的主题为 `None`）。
#[derive(Debug, Clone, Copy)]
pub struct PaperLook {
    /// 卡片纸，比纸再亮一点（`--notes-card`）。
    pub card: Hsla,
    /// 纸边（`--notes-edge`）。
    pub edge: Hsla,
    /// 横线（锤子便签的格线，比原版 `--notes-grid` 略深，32px 行距下才看得清）。
    pub rule: Hsla,
    /// 页边色带（锤子便签左侧那一栏）。
    pub margin: Hsla,
    /// 页边分隔线（`--notes-margin`）。
    pub margin_line: Hsla,
    /// 纸张阴影的近 / 远两层（`--notes-paper-shadow`）。
    pub shadow_near: Hsla,
    pub shadow_far: Hsla,
    /// 木纹桌面底色（`--notes-backing`），纹理是 `notes/wood.svg`。
    pub backing: Hsla,
    /// 稿纸上的表头条（原版 `.prepare-tools` 等的纸色渐变 `#fbf7ec → #f1ebdf`）。
    pub bar_top: Hsla,
    pub bar_bottom: Hsla,
    /// 墨色（标题、卡片里的强调文字；比外壳文字更深，长时间读写不费眼）。
    pub ink: Hsla,
    /// 深棕标题栏（锤子便签的导航条；原版 `.titlebar` 的 `--notes-chrome-light → --notes-chrome`
    /// 渐变、`#655547` 底线、纸色文字）。
    pub chrome_top: Hsla,
    pub chrome_bottom: Hsla,
    pub chrome_border: Hsla,
    pub chrome_text: Hsla,
    /// 标题栏按钮悬停（原版 `.ctrl-btn:hover { background: #ffffff1c }`）。
    pub chrome_hover: Hsla,
}

fn paper(theme_id: &str) -> Option<PaperLook> {
    let c = |value: u32| -> Hsla { rgba(value).into() };
    (theme_id == "light-notes").then(|| PaperLook {
        card: c(0xfefbf5ff),
        edge: c(0xded5c7ff),
        rule: c(0xaa927d38),
        margin: c(0x8a6a4a0f),
        margin_line: c(0x7462522e),
        shadow_near: c(0x7462541c),
        shadow_far: c(0x74625438),
        backing: c(0xf3e3c5ff),
        bar_top: c(0xfbf7ecff),
        bar_bottom: c(0xf1ebdfff),
        ink: c(0x5c4637ff),
        chrome_top: c(0x7f6f60ff),
        chrome_bottom: c(0x746254ff),
        chrome_border: c(0x655547ff),
        chrome_text: c(0xfbf7ecff),
        chrome_hover: c(0xffffff1c),
    })
}

/// 编辑器外观：`theme::apply` 写进 [`super::Studio`]，第一步读。
#[derive(Debug, Clone)]
pub struct EditorLook {
    /// 编辑区底色；`None` 透明。
    pub background: Option<Hsla>,
    /// 字体；`None` 用 GPUI Kit 的等宽字体（原版 `--font-mono`）。
    pub font: Option<SharedString>,
    pub text_size: f32,
    /// 行高（px）。稿纸横线按它画，所以写成绝对值。
    pub line_height: f32,
    /// 稿纸横线；`None` 不画。
    pub rule: Option<Hsla>,
}

pub(super) fn look(theme_id: &str) -> (EditorLook, Option<PaperLook>) {
    let t = table(theme_id);
    let paper = paper(theme_id);
    let c = |value: u32| -> Hsla { rgba(value).into() };
    let look = match paper {
        Some(paper) => EditorLook {
            background: t.background.map(c),
            font: None,
            text_size: 15.0,
            line_height: 32.0,
            rule: Some(paper.rule),
        },
        // 原版 `editorTheme`：13px、行高 1.8。
        None => EditorLook {
            background: t.background.map(c),
            font: None,
            text_size: 13.0,
            line_height: 13.0 * 1.8,
            rule: None,
        },
    };
    (look, paper)
}

/// 选区色：GPUI Kit 的选区是全局一个颜色（输入框与编辑器共用）。
pub(super) fn selection(theme_id: &str) -> Hsla {
    rgba(table(theme_id).selection).into()
}

fn hex(value: u32) -> String {
    format!("#{value:08x}")
}

/// 给 GPUI Kit 的高亮主题（Zed 主题格式的 JSON 反序列化：`ThemeStyle` 的字段是私有的）。
///
/// 只给 Markdown 语法（`gpui-component` 的 `languages/markdown*/highlights.scm`）用到的捕获名；
/// **不设任何字重**（README 规矩 12）：默认主题的 `emphasis.strong` 700 / `title` 600 到此为止。
/// 行号颜色不在这里：编辑器读的是 `Theme.muted_foreground`（外壳的 muted）。
pub(super) fn highlight_theme(theme_id: &str, dark: bool) -> Arc<HighlightTheme> {
    let t = table(theme_id);
    let s = t.syntax;
    let color = |value: u32| serde_json::json!({ "color": hex(value) });
    let editor_background = t.background.unwrap_or(0x00000000);
    let style = serde_json::json!({
        "editor.background": hex(editor_background),
        "editor.foreground": hex(t.foreground),
        "editor.active_line.background": hex(t.active_line),
        "editor.gutter.background": hex(t.gutter),
        "editor.invisible": hex(t.syntax.meta),
        "syntax": {
            "title": color(s.heading),
            "emphasis.strong": color(s.strong),
            "emphasis": { "color": hex(s.emphasis), "font_style": "italic" },
            "link_text": color(s.link),
            "link_uri": color(s.meta),
            "text.literal": color(s.code),
            "text.code.span": color(s.code),
            "punctuation.special": color(s.meta),
            "punctuation.delimiter": color(s.meta),
            "punctuation.list_marker": color(s.list),
            "string.escape": color(s.meta),
        }
    });
    let theme = serde_json::json!({
        "name": format!("huanshu-{theme_id}"),
        "appearance": if dark { "dark" } else { "light" },
        "style": style,
    });
    let parsed: HighlightTheme = serde_json::from_value(theme).unwrap_or_else(|err| {
        eprintln!("[theme] highlight theme for {theme_id} rejected: {err}");
        let fallback = if dark {
            HighlightTheme::default_dark()
        } else {
            HighlightTheme::default_light()
        };
        (*fallback).clone()
    });
    Arc::new(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_theme_builds_a_highlight_theme() {
        for theme in huanshu_core::themes::APP_THEMES {
            let parsed = highlight_theme(theme.id, theme.tone.is_dark());
            assert_eq!(parsed.name, format!("huanshu-{}", theme.id));
            assert!(parsed.style.editor_background.is_some());
            assert!(parsed.style.syntax.title.is_some(), "{}", theme.id);
        }
    }
}
