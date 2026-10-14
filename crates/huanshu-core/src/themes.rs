//! 应用主题清单：id、中文名、明暗。
//!
//! 与 `src/themes/catalog.ts` 一一对应（两份并存到 Tauri 界面下线为止，**改一边必须改另一边**）。
//! id 必须与 `slide_typst::SUPPORTED_THEMES` 一致——那是 Typst 真正有版式的主题集合，
//! 对不上的 id 导出时会静默回落到便签。

/// 明暗。外壳的配色与 `ThemeMode` 都跟着它走。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Light,
    Dark,
}

impl Tone {
    pub fn is_dark(self) -> bool {
        self == Tone::Dark
    }
}

/// 一个可选主题。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppTheme {
    pub id: &'static str,
    pub name: &'static str,
    pub tone: Tone,
}

/// 全部主题，顺序与前端的主题切换器一致。
pub const APP_THEMES: &[AppTheme] = &[
    AppTheme {
        id: "dark-yellow",
        name: "暗·琥珀",
        tone: Tone::Dark,
    },
    AppTheme {
        id: "dark-blue",
        name: "暗·星图",
        tone: Tone::Dark,
    },
    AppTheme {
        id: "dark-terminal",
        name: "终端·墨",
        tone: Tone::Dark,
    },
    AppTheme {
        id: "light-terminal",
        name: "终端·纸",
        tone: Tone::Light,
    },
    AppTheme {
        id: "light-editorial",
        name: "报刊·钴蓝",
        tone: Tone::Light,
    },
    AppTheme {
        id: "light-notes",
        name: "便签·稿纸",
        tone: Tone::Light,
    },
];

/// 没有设置过主题时用它。
pub const DEFAULT_THEME: &str = "dark-yellow";

/// 主题 id 是否已知（带版本回落的检查）。
pub fn is_known_theme(id: &str) -> bool {
    APP_THEMES.iter().any(|theme| theme.id == id)
}

/// 按 id 取主题；未知 id 回落到 [`DEFAULT_THEME`]，第二个返回值说明有没有回落
/// （与 `export_pdf` 报告里的 `theme_fallback` 同义）。
pub fn theme_or_default(id: Option<&str>) -> (&'static AppTheme, Option<String>) {
    let requested = id.unwrap_or(DEFAULT_THEME);
    APP_THEMES
        .iter()
        .find(|theme| theme.id == requested)
        .map(|theme| (theme, None))
        .unwrap_or_else(|| {
            let fallback = APP_THEMES
                .iter()
                .find(|theme| theme.id == DEFAULT_THEME)
                .expect("DEFAULT_THEME is in the list");
            (fallback, Some(requested.to_string()))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 主题 id 与 Typst 的版式集合必须完全一致：多一个 id 导出时静默回落，少一个则有人选不到。
    #[test]
    fn theme_ids_match_the_typst_layouts() {
        let ours: Vec<&str> = APP_THEMES.iter().map(|theme| theme.id).collect();
        let mut typst: Vec<&str> = slide_typst::SUPPORTED_THEMES.to_vec();
        let mut sorted_ours = ours.clone();
        sorted_ours.sort_unstable();
        typst.sort_unstable();
        assert_eq!(sorted_ours, typst);
        assert_eq!(ours.len(), APP_THEMES.len(), "no duplicate ids");
    }

    #[test]
    fn every_theme_has_a_chinese_name_and_a_tone() {
        for theme in APP_THEMES {
            assert!(!theme.name.is_empty(), "{} has no name", theme.id);
            assert!(is_known_theme(theme.id));
        }
        assert!(APP_THEMES.iter().any(|t| t.tone.is_dark()));
        assert!(APP_THEMES.iter().any(|t| !t.tone.is_dark()));
    }

    #[test]
    fn an_unknown_theme_falls_back_to_the_default_and_says_so() {
        let (theme, fallback) = theme_or_default(None);
        assert_eq!(theme.id, DEFAULT_THEME);
        assert_eq!(fallback, None);

        let (theme, fallback) = theme_or_default(Some("light-notes"));
        assert_eq!(theme.id, "light-notes");
        assert_eq!(fallback, None);

        let (theme, fallback) = theme_or_default(Some("no-such-theme"));
        assert_eq!(theme.id, DEFAULT_THEME);
        assert_eq!(fallback.as_deref(), Some("no-such-theme"));
    }
}
