//! 命令行开关。为脚本化验证（截图、导出比对）而存在，不必手点。
//!
//! 未知参数只提示不中断：验证脚本经常多传几个开关，静默忽略比直接退出好排查。

use std::path::PathBuf;

use crate::router::Route;

/// 解析后的命令行。
#[derive(Debug, Clone, Default)]
pub struct Cli {
    /// 启动后直接进入的页面（`home`、`wizard`、`settings[:tab]`、`new`、`project:<id>[:<step>]`）。
    /// 首次启动（`is_first_run`）时仍然先进向导。
    pub route: Option<Route>,
    /// 与 `--export-pdf` 一起用：导出哪个项目。
    pub open: Option<i64>,
    /// 启动主题（并写入设置）。必须是 `themes::APP_THEMES` 里的 id。
    pub theme: Option<String>,
    /// 跳过窗口直接导出到这个路径；导出完以退出码 0/1 结束。需要 `--open`。
    pub export_pdf: Option<PathBuf>,
    /// 到点自动退出（秒）。
    pub quit_after: Option<u64>,
}

impl Cli {
    pub fn parse() -> Self {
        Self::from_args(std::env::args().skip(1))
    }

    fn from_args(args: impl IntoIterator<Item = String>) -> Self {
        let mut cli = Self::default();
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--route" => {
                    let value = args.next().unwrap_or_default();
                    cli.route = Route::parse(&value);
                    if cli.route.is_none() {
                        eprintln!("[native] 无法识别的 --route {value}");
                    }
                }
                "--open" => cli.open = args.next().and_then(|value| value.parse().ok()),
                "--theme" => cli.theme = args.next(),
                "--export-pdf" => cli.export_pdf = args.next().map(PathBuf::from),
                "--quit-after" => cli.quit_after = args.next().and_then(|value| value.parse().ok()),
                other => eprintln!("[native] 忽略未知参数 {other}"),
            }
        }
        cli
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::router::{SettingsTab, Step};

    fn parse(args: &[&str]) -> Cli {
        Cli::from_args(args.iter().map(|arg| arg.to_string()))
    }

    #[test]
    fn the_documented_switches_are_all_read() {
        let cli = parse(&[
            "--route",
            "project:4:3",
            "--theme",
            "dark-blue",
            "--quit-after",
            "30",
        ]);
        assert_eq!(
            cli.route,
            Some(Route::Project {
                id: Some(4),
                step: Some(Step::Edit)
            })
        );
        assert_eq!(cli.theme.as_deref(), Some("dark-blue"));
        assert_eq!(cli.quit_after, Some(30));

        let cli = parse(&[
            "--open",
            "4",
            "--export-pdf",
            "a.pdf",
            "--route",
            "settings:storage",
        ]);
        assert_eq!(cli.open, Some(4));
        assert_eq!(cli.export_pdf, Some(PathBuf::from("a.pdf")));
        assert_eq!(cli.route, Some(Route::Settings(SettingsTab::Storage)));
    }

    #[test]
    fn no_switches_leave_everything_off() {
        let cli = parse(&[]);
        assert!(cli.route.is_none() && cli.open.is_none() && cli.theme.is_none());
        assert!(cli.quit_after.is_none() && cli.export_pdf.is_none());
    }
}
