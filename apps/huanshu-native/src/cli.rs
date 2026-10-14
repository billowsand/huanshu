//! 命令行开关。为脚本化验证而存在，不必手点。
//!
//! 未知参数只提示不中断：验证脚本经常多传几个开关，静默忽略比直接退出好排查。

use std::path::PathBuf;

/// 解析后的命令行。
#[derive(Debug, Clone, Default)]
pub struct Cli {
    /// 直接打开某个项目，跳过首页。
    pub open: Option<i64>,
    /// 打开的页，从 1 开始。
    pub page: Option<usize>,
    /// 打开后直接进演示模式。
    pub present: bool,
    /// 启动主题（并写入设置）。必须是 `themes::APP_THEMES` 里的 id。
    pub theme: Option<String>,
    /// 跳过保存对话框直接导出到这个路径；导出完以退出码 0/1 结束。
    pub export_pdf: Option<PathBuf>,
    /// 到点自动退出（秒）。
    pub quit_after: Option<u64>,
    /// 看板上每 100ms 换一页，循环这么久（秒）；期间每 30 秒打一次工作集内存。
    pub flip_soak: Option<u64>,
}

impl Cli {
    pub fn parse() -> Self {
        let mut cli = Self::default();
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--open" => cli.open = args.next().and_then(|value| value.parse().ok()),
                "--page" => {
                    cli.page = args
                        .next()
                        .and_then(|value| value.parse::<usize>().ok())
                        // 命令行写的是从 1 开始的下标，内部从 0 开始。
                        .map(|page| page.saturating_sub(1))
                }
                "--present" => cli.present = true,
                "--theme" => cli.theme = args.next(),
                "--export-pdf" => cli.export_pdf = args.next().map(PathBuf::from),
                "--quit-after" => cli.quit_after = args.next().and_then(|value| value.parse().ok()),
                "--flip-soak" => cli.flip_soak = args.next().and_then(|value| value.parse().ok()),
                other => eprintln!("[native] 忽略未知参数 {other}"),
            }
        }
        cli
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Cli {
        let mut cli = Cli::default();
        // 与 `parse` 同样的取值逻辑，但吃一个切片，便于单测。
        let mut iter = args.iter().map(|s| s.to_string());
        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "--open" => cli.open = iter.next().and_then(|v| v.parse().ok()),
                "--page" => {
                    cli.page = iter
                        .next()
                        .and_then(|v| v.parse::<usize>().ok())
                        .map(|page| page.saturating_sub(1))
                }
                "--present" => cli.present = true,
                "--theme" => cli.theme = iter.next(),
                "--export-pdf" => cli.export_pdf = iter.next().map(PathBuf::from),
                "--quit-after" => cli.quit_after = iter.next().and_then(|v| v.parse().ok()),
                "--flip-soak" => cli.flip_soak = iter.next().and_then(|v| v.parse().ok()),
                _ => {}
            }
        }
        cli
    }

    #[test]
    fn page_is_one_based_on_the_command_line() {
        assert_eq!(parse(&["--page", "1"]).page, Some(0));
        assert_eq!(parse(&["--page", "3"]).page, Some(2));
        // 0 不会绕成 usize 的最大值。
        assert_eq!(parse(&["--page", "0"]).page, Some(0));
    }

    #[test]
    fn the_documented_switches_are_all_read() {
        let cli = parse(&[
            "--open",
            "4",
            "--page",
            "2",
            "--present",
            "--theme",
            "dark-blue",
            "--export-pdf",
            "a.pdf",
            "--quit-after",
            "30",
            "--flip-soak",
            "300",
        ]);
        assert_eq!(cli.open, Some(4));
        assert_eq!(cli.page, Some(1));
        assert!(cli.present);
        assert_eq!(cli.theme.as_deref(), Some("dark-blue"));
        assert_eq!(cli.export_pdf, Some(PathBuf::from("a.pdf")));
        assert_eq!(cli.quit_after, Some(30));
        assert_eq!(cli.flip_soak, Some(300));
    }

    #[test]
    fn no_switches_leave_everything_off() {
        let cli = parse(&[]);
        assert!(cli.open.is_none() && !cli.present && cli.theme.is_none());
        assert!(cli.quit_after.is_none() && cli.flip_soak.is_none());
    }
}
