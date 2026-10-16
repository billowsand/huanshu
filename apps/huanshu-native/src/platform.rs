//! 平台差异集中在这里：应用退出与菜单、快捷键的显示文字、文件对话框、默认字体。
//!
//! 页面里不要再写 `cfg!(target_os = ...)` 去处理这几件事，一律经本模块。
//! （标题栏的平台差异在 `widgets::title_bar`，它和布局绑得太紧，留在那里。）

use std::path::{Path, PathBuf};

use gpui_kit::component::notification::Notification;
use gpui_kit::component::WindowExt as _;
use gpui_kit::{App, Context, Modifiers, QuitMode, Window};

// ── 应用生命周期 ─────────────────────────────────────────────────────────────

// 菜单栏的动作只在 macOS 用到（Windows / Linux 没有应用菜单）。
#[cfg(target_os = "macos")]
gpui_kit::actions!(huanshu, [Quit, Hide, HideOthers, ShowAll, Minimize, Zoom]);

/// 应用级初始化，在 `gpui_kit::init` 之后、开窗口之前调一次。
///
/// - 关掉最后一个窗口就退出。GPUI 在 Windows / Linux 上本来就这样；macOS 的默认是留在
///   Dock 里（`QuitMode::Explicit`），本应用只有一个窗口，留着进程没有意义。
/// - macOS 的菜单栏与 ⌘Q / ⌘H / ⌘M：GPUI Kit 不替应用做（"Applications own quit/close
///   actions"），不做的话 ⌘Q 没反应、菜单栏是空的。Windows / Linux 不加菜单、不加快捷键，
///   行为与之前完全一样。
pub fn init(cx: &mut App) {
    cx.set_quit_mode(QuitMode::LastWindowClosed);
    #[cfg(target_os = "macos")]
    init_macos_menus(cx);
}

#[cfg(target_os = "macos")]
fn init_macos_menus(cx: &mut App) {
    use gpui_kit::{KeyBinding, Menu, MenuItem};

    cx.on_action(|_: &Quit, cx| cx.quit());
    cx.on_action(|_: &Hide, cx| cx.hide());
    cx.on_action(|_: &HideOthers, cx| cx.hide_other_apps());
    cx.on_action(|_: &ShowAll, cx| cx.unhide_other_apps());
    cx.on_action(|_: &Minimize, cx| {
        if let Some(window) = cx.active_window() {
            let _ = window.update(cx, |_, window, _| window.minimize_window());
        }
    });
    cx.on_action(|_: &Zoom, cx| {
        if let Some(window) = cx.active_window() {
            let _ = window.update(cx, |_, window, _| window.zoom_window());
        }
    });
    cx.bind_keys([
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-h", Hide, None),
        KeyBinding::new("alt-cmd-h", HideOthers, None),
        KeyBinding::new("cmd-m", Minimize, None),
    ]);
    // 第一个菜单的标题由系统取应用名（.app 的 CFBundleName），这里的 name 只在裸二进制时出现。
    cx.set_menus([
        Menu::new("幻述").items([
            MenuItem::action("隐藏幻述", Hide),
            MenuItem::action("隐藏其他", HideOthers),
            MenuItem::action("全部显示", ShowAll),
            MenuItem::separator(),
            MenuItem::action("退出幻述", Quit),
        ]),
        Menu::new("窗口").items([
            MenuItem::action("最小化", Minimize),
            MenuItem::action("缩放", Zoom),
        ]),
    ]);
    // 从终端 `cargo run` 启动时窗口不会自己到前台。
    cx.activate(true);
}

// ── 快捷键 ───────────────────────────────────────────────────────────────────

/// 「主修饰键」是否按下：macOS 是 ⌘，其余平台是 Ctrl（GPUI 的 `Modifiers::secondary`）。
/// 页面里判断「Ctrl/⌘ + 某键」一律用它，不要直接读 `control` / `platform`。
pub fn primary_modifier(modifiers: &Modifiers) -> bool {
    modifiers.secondary()
}

/// 「主修饰键 + key」的显示文字：macOS `⌘S`，其余平台 `Ctrl + S`。
pub fn shortcut_label(key: &str) -> String {
    if cfg!(target_os = "macos") {
        format!("⌘{}", key.to_uppercase())
    } else {
        format!("Ctrl + {}", key.to_uppercase())
    }
}

// ── 文件对话框 ───────────────────────────────────────────────────────────────

/// 文件类型过滤（显示名 + 不带点的扩展名）。
#[derive(Debug, Clone, Copy)]
pub struct FileFilter {
    pub name: &'static str,
    pub extensions: &'static [&'static str],
}

impl FileFilter {
    fn accepts(&self, path: &Path) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| self.extensions.iter().any(|e| e.eq_ignore_ascii_case(ext)))
    }
}

/// 应用里用到的几种文件。
pub const PDF: FileFilter = FileFilter {
    name: "PDF",
    extensions: &["pdf"],
};
pub const KEYNN: FileFilter = FileFilter {
    name: "幻述项目",
    extensions: &["keynn"],
};
pub const MARKDOWN: FileFilter = FileFilter {
    name: "Markdown",
    extensions: &["md", "txt"],
};
pub const MEDIA: FileFilter = FileFilter {
    name: "图片 / 视频",
    extensions: &[
        "png", "jpg", "jpeg", "gif", "webp", "svg", "mp4", "webm", "mov",
    ],
};

/// 要选什么。
#[derive(Debug, Clone, Copy)]
pub enum Pick {
    Folder,
    File(FileFilter),
    Files(FileFilter),
}

/// 弹「打开」对话框；用户选了东西才调用 `done`（取消则什么都不做）。
///
/// - **Windows / Linux**：照旧同步弹 rfd 对话框（带类型过滤），行为与之前逐字相同。
/// - **macOS**：走 GPUI 的 `prompt_for_paths`（`NSOpenPanel` + completion handler，异步）。
///   不用同步的 rfd：它在 GPUI 的事件回调里再跑一层 AppKit 模态循环，期间 GPUI 收到的
///   重绘 / 窗口事件会重入正被借用的 App。GPUI 的面板没有类型过滤，所以选完再按扩展名筛，
///   全被筛掉时提示一句。
pub fn pick_paths<T: 'static>(
    this: &mut T,
    pick: Pick,
    window: &mut Window,
    cx: &mut Context<T>,
    done: impl FnOnce(&mut T, Vec<PathBuf>, &mut Window, &mut Context<T>) + 'static,
) {
    #[cfg(target_os = "macos")]
    {
        let _ = this;
        let (files, directories, multiple) = match pick {
            Pick::Folder => (false, true, false),
            Pick::File(_) => (true, false, false),
            Pick::Files(_) => (true, false, true),
        };
        let receiver = cx.prompt_for_paths(gpui_kit::PathPromptOptions {
            files,
            directories,
            multiple,
            prompt: None,
        });
        cx.spawn_in(window, async move |view, cx| {
            let Ok(Ok(Some(paths))) = receiver.await else {
                return;
            };
            let _ = view.update_in(cx, move |view, window, cx| {
                if let Some(paths) = accept_picked(pick, paths, window, cx) {
                    done(view, paths, window, cx);
                }
            });
        })
        .detach();
    }
    #[cfg(not(target_os = "macos"))]
    {
        let picked = match pick {
            Pick::Folder => rfd::FileDialog::new().pick_folder().map(|path| vec![path]),
            Pick::File(filter) => rfd::FileDialog::new()
                .add_filter(filter.name, filter.extensions)
                .pick_file()
                .map(|path| vec![path]),
            Pick::Files(filter) => rfd::FileDialog::new()
                .add_filter(filter.name, filter.extensions)
                .pick_files(),
        };
        if let Some(paths) = picked {
            if let Some(paths) = accept_picked(pick, paths, window, cx) {
                done(this, paths, window, cx);
            }
        }
    }
}

/// 按过滤器筛掉不认识的文件；一个都不剩时提示并返回 `None`。
fn accept_picked<T>(
    pick: Pick,
    paths: Vec<PathBuf>,
    window: &mut Window,
    cx: &mut Context<T>,
) -> Option<Vec<PathBuf>> {
    let filter = match pick {
        Pick::Folder => return (!paths.is_empty()).then_some(paths),
        Pick::File(filter) | Pick::Files(filter) => filter,
    };
    let accepted: Vec<PathBuf> = paths.into_iter().filter(|p| filter.accepts(p)).collect();
    if accepted.is_empty() {
        let kinds = filter
            .extensions
            .iter()
            .map(|ext| format!(".{ext}"))
            .collect::<Vec<_>>()
            .join(" / ");
        window.push_notification(
            Notification::error(format!("请选择{}文件（{kinds}）", filter.name)),
            cx,
        );
        return None;
    }
    Some(accepted)
}

/// 弹「保存」对话框；用户确定了路径才调用 `done`。路径缺扩展名时补上 `filter` 的第一个。
///
/// 平台分工同 [`pick_paths`]：macOS 走 GPUI 的 `prompt_for_new_path`（`NSSavePanel`，异步），
/// 初始目录是「文稿」；其余平台照旧同步弹 rfd。
pub fn save_path<T: 'static>(
    this: &mut T,
    default_name: &str,
    filter: FileFilter,
    window: &mut Window,
    cx: &mut Context<T>,
    done: impl FnOnce(&mut T, PathBuf, &mut Window, &mut Context<T>) + 'static,
) {
    #[cfg(target_os = "macos")]
    {
        let _ = this;
        let directory = default_save_dir();
        let receiver = cx.prompt_for_new_path(&directory, Some(default_name));
        cx.spawn_in(window, async move |view, cx| {
            let Ok(Ok(Some(path))) = receiver.await else {
                return;
            };
            let path = with_extension(path, filter);
            let _ = view.update_in(cx, move |view, window, cx| done(view, path, window, cx));
        })
        .detach();
    }
    #[cfg(not(target_os = "macos"))]
    {
        let picked = rfd::FileDialog::new()
            .set_file_name(default_name)
            .add_filter(filter.name, filter.extensions)
            .save_file();
        if let Some(path) = picked {
            done(this, with_extension(path, filter), window, cx);
        }
    }
}

/// 保存面板的初始目录：`~/Documents`，没有就是主目录，再没有就是当前目录。
#[cfg(target_os = "macos")]
fn default_save_dir() -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    match home {
        Some(home) if home.join("Documents").is_dir() => home.join("Documents"),
        Some(home) => home,
        None => PathBuf::from("."),
    }
}

/// 用户把扩展名删掉了（macOS 的保存面板不强制类型）就补回去；已有正确扩展名则原样返回。
fn with_extension(path: PathBuf, filter: FileFilter) -> PathBuf {
    if filter.accepts(&path) {
        return path;
    }
    let Some(ext) = filter.extensions.first() else {
        return path;
    };
    let mut name = path
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    name.push(".");
    name.push(ext);
    path.with_file_name(name)
}

// ── 默认字体 ─────────────────────────────────────────────────────────────────

/// 本平台默认中文字体（与 core 的 `FontSettings::default` 同一个值）。
pub const DEFAULT_FONT: &str = slide_typst::DEFAULT_FONT;

/// [`DEFAULT_FONT`] 的中文显示名（字体列表还没读出来时用）。
pub const DEFAULT_FONT_LABEL: &str = if cfg!(target_os = "macos") {
    "苹方"
} else {
    "微软雅黑"
};

/// 界面字体解析的最后一道兜底（都没装时 GPUI 再落到系统 UI 字体）。
pub const UI_FALLBACK_FONT: &str = if cfg!(target_os = "macos") {
    "Hiragino Sans GB"
} else {
    "Segoe UI"
};

/// 设置页字体选择器无搜索词时置顶的常用字体（原版 `commonNames`）。
pub fn common_font_names(code: bool) -> &'static [&'static str] {
    #[cfg(target_os = "macos")]
    {
        if code {
            &["Menlo", "SF Mono", "Monaco", "JetBrains Mono", DEFAULT_FONT]
        } else {
            &[
                DEFAULT_FONT,
                "Songti SC",
                "Kaiti SC",
                "Heiti SC",
                "Hiragino Sans GB",
                "Arial",
                "Georgia",
            ]
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        if code {
            &["Consolas", "Cascadia Code", "JetBrains Mono", DEFAULT_FONT]
        } else {
            &[
                DEFAULT_FONT,
                "Microsoft YaHei UI",
                "SimSun",
                "SimHei",
                "KaiTi",
                "DengXian",
                "MiSans",
                "Arial",
                "Georgia",
            ]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_extension_is_appended() {
        assert_eq!(
            with_extension(PathBuf::from("/tmp/报告"), PDF),
            PathBuf::from("/tmp/报告.pdf")
        );
        assert_eq!(
            with_extension(PathBuf::from("/tmp/a.PDF"), PDF),
            PathBuf::from("/tmp/a.PDF")
        );
        // 文件名里带点但扩展名不对：照样补，不替换。
        assert_eq!(
            with_extension(PathBuf::from("/tmp/v1.2"), PDF),
            PathBuf::from("/tmp/v1.2.pdf")
        );
    }

    #[test]
    fn filter_matches_extensions_case_insensitively() {
        let markdown = MARKDOWN;
        assert!(markdown.accepts(Path::new("a.MD")));
        assert!(markdown.accepts(Path::new("dir/b.txt")));
        assert!(!markdown.accepts(Path::new("c.docx")));
        assert!(!markdown.accepts(Path::new("md")));
    }

    #[test]
    fn shortcut_label_follows_the_platform() {
        let label = shortcut_label("s");
        if cfg!(target_os = "macos") {
            assert_eq!(label, "⌘S");
        } else {
            assert_eq!(label, "Ctrl + S");
        }
    }

    #[test]
    fn default_font_leads_the_common_list() {
        assert_eq!(common_font_names(false)[0], DEFAULT_FONT);
        assert!(common_font_names(true).contains(&DEFAULT_FONT));
    }
}
