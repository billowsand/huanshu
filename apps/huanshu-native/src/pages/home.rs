//! 首页（原版 `HomeView.vue`）——**样板页**：后续页面照它的结构写。
//!
//! 已实现：操作栏（项目 · 主题浮层 · 导入 · 设置 · 新建演示）、标题区、加载骨架、空状态、
//! 项目卡片（名称、比例徽章、页数、更新时间、导出、删除）、删除确认、导出对话框
//! （普通 `.keynn`、加密 `.keynn` 与 PDF）、导入 `.keynn`（加密文件弹密码对话框，
//! 密码错误不关闭可重试，照原版 `handlePasswordSubmit`）。

use gpui_kit::component::button::ButtonVariant;
use gpui_kit::component::dialog::{AlertDialog, Dialog, DialogClose, DialogFooter};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::{Disableable as _, Selectable as _};
use gpui_kit::prelude::*;
use gpui_kit::{div, px, Context, Entity, Render, SharedString, Window};
use huanshu_core::db::ProjectSummary;
use huanshu_core::services::generation;
use huanshu_core::services::{projects, storage};
use huanshu_core::types::AspectRatio;

use crate::export;
use crate::icons::carbon;
use crate::password_dialog::{PasswordDialog, PasswordMode};
use crate::router::{Route, SettingsTab};
use crate::shell::ShellHandle;
use crate::tasks::{run_async, run_bg};
use crate::theme::StudioExt as _;
use crate::widgets::{self, Tone2};

/// 内容列的最大宽度（原版 `.home-body` 的 `max-width: 860px`，`box-sizing: border-box`
/// 所以 1.5rem 的内边距算在里面）。
const BODY_MAX_W: f32 = 860.0;

/// 原版 `usePdfExport.ts` 的 `safeFileName`：PDF 的默认文件名去掉 Windows 不允许的字符
/// （连续的非法字符合成一个 `_`，名字为空时用「未命名演示」）。`.keynn` 照原版不做处理
/// （`HomeView.vue` 的 `doExport` 直接用 `${p.name}.keynn`）。
fn safe_file_name(name: &str) -> String {
    const INVALID: [char; 9] = ['\\', '/', ':', '*', '?', '"', '<', '>', '|'];
    let name = name.trim();
    if name.is_empty() {
        return "未命名演示".to_string();
    }
    let mut out = String::with_capacity(name.len());
    let mut in_invalid = false;
    for character in name.chars() {
        if INVALID.contains(&character) {
            if !in_invalid {
                out.push('_');
                in_invalid = true;
            }
        } else {
            out.push(character);
            in_invalid = false;
        }
    }
    out
}

/// 导入读文件的两种走向（原版 `handleImport` 的分支）。
enum ImportRead {
    /// 未加密：已直接导入。
    Imported,
    /// 加密：等用户在密码对话框里输密码。
    Encrypted(Vec<u8>),
}

pub struct HomePage {
    handle: ShellHandle,
    /// `None` = 正在加载（显示骨架卡片）。
    projects: Option<Vec<ProjectSummary>>,
    load_error: Option<String>,
    show_theme_picker: bool,
    /// 正在导出的项目（按钮显示转圈，禁止重复点击）。
    exporting: Option<i64>,
    /// PDF 导出互斥（原版 `usePdfExport.ts` 的全局 `exporting`：同一时刻只跑一次）。
    pdf_exporting: bool,
    importing: bool,
    /// 加密导入：已读入内存、等待密码的 `.keynn` 字节（原版 `pendingImportData`）。
    pending_import: Option<Vec<u8>>,
    /// 当前打开的密码对话框（外部错误经它更新，实体也由此保活）。
    password_dialog: Option<Entity<PasswordDialog>>,
}

impl HomePage {
    pub fn new(handle: ShellHandle, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut page = Self {
            handle,
            projects: None,
            load_error: None,
            show_theme_picker: false,
            exporting: None,
            pdf_exporting: false,
            importing: false,
            pending_import: None,
            password_dialog: None,
        };
        page.reload(window, cx);
        // 原版 `onMounted` 的另一半：`invoke("ensure_icon_embeddings").catch(() => {})`
        // （`HomeView.vue:35`）。结果与错误都忽略——没配向量库时让它失败就好，不挡界面。
        let preheat = page.handle.shared().clone();
        run_async(
            window,
            cx,
            async move { generation::ensure_icon_embeddings(&preheat).await },
            |_page: &mut Self, _result: Result<(), String>, _window, _cx| {},
        );
        page
    }

    /// 重新读项目列表（原版 `projects.refresh()`）。
    fn reload(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let shared = self.handle.shared().clone();
        run_bg(
            window,
            cx,
            move || projects::list_projects(&shared),
            |page: &mut Self, result, _window, cx| {
                match result {
                    Ok(list) => {
                        page.projects = Some(list);
                        page.load_error = None;
                    }
                    Err(error) => {
                        page.projects = Some(Vec::new());
                        page.load_error = Some(error);
                    }
                }
                cx.notify();
            },
        );
    }

    fn confirm_delete(&mut self, id: i64, window: &mut Window, cx: &mut Context<Self>) {
        let page = cx.weak_entity();
        // 文案与原版一致。
        window.open_alert_dialog(cx, move |alert: AlertDialog, _window, _cx| {
            let page = page.clone();
            alert
                .title("确认删除")
                .description("删除后无法恢复，确定要删除这个项目吗？")
                .confirm()
                .ok_text("删除")
                .ok_variant(ButtonVariant::Danger)
                .cancel_text("取消")
                .on_ok(move |_event, window, cx| {
                    let _ = page.update(cx, |page, cx| page.delete(id, window, cx));
                    true
                })
        });
    }

    fn delete(&mut self, id: i64, window: &mut Window, cx: &mut Context<Self>) {
        let shared = self.handle.shared().clone();
        run_bg(
            window,
            cx,
            move || projects::delete_project(&shared, id),
            |page: &mut Self, result, window, cx| {
                if let Err(error) = result {
                    window.push_notification(Notification::error(format!("删除失败：{error}")), cx);
                }
                page.reload(window, cx);
            },
        );
    }

    /// 导出对话框（原版 `ExportDialog.vue`）：普通导出 / 加密导出 / PDF 文档。
    fn open_export(
        &mut self,
        project: ProjectSummary,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let page = cx.weak_entity();
        window.open_dialog(cx, move |dialog: Dialog, _window, cx| {
            let c = cx.studio().colors;
            let option =
                |id: &'static str, icon: &'static str, title: &'static str, desc: &'static str| {
                    div()
                        .id(id)
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(12.0))
                        .p(px(12.0))
                        .rounded(px(8.0))
                        .border_1()
                        .border_color(c.border)
                        .bg(c.surface)
                        .cursor_pointer()
                        .hover(|style| style.border_color(c.primary_border))
                        .child(carbon(icon))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .child(div().text_size(px(14.0)).child(title))
                                .child(div().text_size(px(12.0)).text_color(c.muted).child(desc)),
                        )
                };
            let (plain_page, pdf_page, encrypted_page) = (page.clone(), page.clone(), page.clone());
            let (plain_project, pdf_project, encrypted_project) =
                (project.clone(), project.clone(), project.clone());
            dialog
                .title("导出演示文稿")
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(10.0))
                        .child(
                            div()
                                .text_size(px(13.0))
                                .text_color(c.muted)
                                .child(format!("准备导出「{}」", project.name)),
                        )
                        .child(
                            option(
                                "export-plain",
                                "document-export",
                                "普通导出",
                                "直接生成 `.keynn` 文件",
                            )
                            .on_click(move |_event, window, cx| {
                                window.close_dialog(cx);
                                let _ = plain_page.update(cx, |page, cx| {
                                    page.export_keynn(plain_project.clone(), None, window, cx)
                                });
                            }),
                        )
                        .child(
                            option(
                                "export-encrypted",
                                "locked",
                                "加密导出",
                                "设置密码后再生成导出文件",
                            )
                            .on_click(move |_event, window, cx| {
                                window.close_dialog(cx);
                                let _ = encrypted_page.update(cx, |page, cx| {
                                    page.open_encrypt_dialog(encrypted_project.clone(), window, cx)
                                });
                            }),
                        )
                        .child(
                            option(
                                "export-pdf",
                                "document-pdf",
                                "PDF 文档",
                                "矢量排版，适合阅读与打印",
                            )
                            .on_click(move |_event, window, cx| {
                                window.close_dialog(cx);
                                let _ = pdf_page.update(cx, |page, cx| {
                                    page.export_pdf(pdf_project.clone(), window, cx)
                                });
                            }),
                        ),
                )
                .footer(
                    DialogFooter::new()
                        .child(DialogClose::new().trigger(|button| button.label("取消"))),
                )
        });
    }

    /// 普通导出（`.keynn`）。`password` 为 `Some` 时走加密导出（原版 `doExport`）。
    fn export_keynn(
        &mut self,
        project: ProjectSummary,
        password: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(path) =
            export::ask_save_path(&format!("{}.keynn", project.name), "幻述项目", "keynn")
        else {
            return;
        };
        self.exporting = Some(project.id);
        cx.notify();
        let shared = self.handle.shared().clone();
        run_bg(
            window,
            cx,
            move || {
                let encrypted = Some(password.is_some());
                let bytes = storage::export_project(&shared, project.id, encrypted, password)?;
                std::fs::write(&path, bytes).map_err(|error| format!("写入失败：{error}"))?;
                Ok::<_, String>(path)
            },
            |page: &mut Self, result, window, cx| {
                page.exporting = None;
                page.password_dialog = None;
                let note = match result {
                    Ok(path) => Notification::success(format!("已导出：{}", path.display())),
                    Err(error) => Notification::error(format!("导出失败：{error}")),
                };
                window.push_notification(note, cx);
                cx.notify();
            },
        );
    }

    /// 加密导出：先弹保存路径，再带密码导出（原版 `doExport`，与 `export_keynn` 同序）。
    fn open_encrypt_dialog(
        &mut self,
        project: ProjectSummary,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let page = cx.weak_entity();
        let dialog = PasswordDialog::open(window, cx, move |_window, cx| {
            let submit_page = page.clone();
            let cancel_page = page.clone();
            let project = project.clone();
            PasswordDialog::new(
                PasswordMode::Encrypt,
                Some(project.name.clone()),
                move |password, window, cx| {
                    window.close_dialog(cx);
                    let _ = submit_page.update(cx, |page, cx| {
                        page.export_keynn(project.clone(), Some(password), window, cx)
                    });
                },
                move |_window, cx| {
                    let _ = cancel_page.update(cx, |page, cx| {
                        page.password_dialog = None;
                        cx.notify();
                    });
                },
                _window,
                cx,
            )
        });
        self.password_dialog = Some(dialog);
        cx.notify();
    }

    fn export_pdf(&mut self, project: ProjectSummary, window: &mut Window, cx: &mut Context<Self>) {
        // 原版 `usePdfExport.exportPdf` 开头就是 `if (exporting.value) return;`：同一时刻
        // 只跑一次 PDF 导出，第二次静默忽略（连保存对话框都不弹）。
        if self.pdf_exporting {
            return;
        }
        let default_name = format!("{}.pdf", safe_file_name(&project.name));
        let Some(path) = export::ask_save_path(&default_name, "PDF", "pdf") else {
            return;
        };
        self.pdf_exporting = true;
        self.exporting = Some(project.id);
        cx.notify();
        // 「正在生成 PDF…」（首次导出要扫系统字体，要几秒）。结果 / 失败对话框由
        // `show_report` 接着开，它顺手收起这个框，这里不用再管。
        export::begin_export(window, cx);
        let shared = self.handle.shared().clone();
        let theme = cx.studio().theme.id.to_string();
        run_bg(
            window,
            cx,
            move || crate::app::export_pdf(&shared, project.id, None, path, theme),
            |page: &mut Self, result, window, cx| {
                page.pdf_exporting = false;
                page.exporting = None;
                export::show_report(result, None, window, cx);
                cx.notify();
            },
        );
    }

    /// 导入 `.keynn`（原版 `handleImport` / `handlePasswordSubmit`）。
    /// 加密文件先读入内存，弹 decrypt 密码对话框；密码错误不关闭对话框，可重试。
    fn import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("幻述项目", &["keynn"])
            .pick_file()
        else {
            return;
        };
        self.importing = true;
        cx.notify();
        let shared = self.handle.shared().clone();
        run_bg(
            window,
            cx,
            move || {
                let bytes = std::fs::read(&path).map_err(|error| format!("读取失败：{error}"))?;
                if storage::is_encrypted_file(&bytes) {
                    // 加密文件：回界面弹密码对话框。
                    Ok(ImportRead::Encrypted(bytes))
                } else {
                    storage::import_project(&shared, bytes, None)?;
                    Ok::<ImportRead, String>(ImportRead::Imported)
                }
            },
            |page: &mut Self, result, window, cx| {
                page.importing = false;
                match result {
                    Ok(ImportRead::Imported) => {
                        window.push_notification(Notification::success("导入完成"), cx);
                        page.reload(window, cx);
                    }
                    Ok(ImportRead::Encrypted(bytes)) => page.open_decrypt_dialog(bytes, window, cx),
                    Err(error) => window
                        .push_notification(Notification::error(format!("导入失败：{error}")), cx),
                }
                cx.notify();
            },
        );
    }

    /// 加密导入的密码对话框（原版 decrypt 模式的 `PasswordDialog`）。
    fn open_decrypt_dialog(&mut self, bytes: Vec<u8>, window: &mut Window, cx: &mut Context<Self>) {
        self.pending_import = Some(bytes);
        let page = cx.weak_entity();
        let dialog = PasswordDialog::open(window, cx, move |_window, cx| {
            let submit_page = page.clone();
            let cancel_page = page.clone();
            PasswordDialog::new(
                PasswordMode::Decrypt,
                None,
                move |password, window, cx| {
                    let _ = submit_page.update(cx, |page, cx| {
                        page.submit_import_password(password, window, cx)
                    });
                },
                move |_window, cx| {
                    let _ = cancel_page.update(cx, |page, cx| {
                        page.pending_import = None;
                        page.password_dialog = None;
                        cx.notify();
                    });
                },
                _window,
                cx,
            )
        });
        self.password_dialog = Some(dialog);
        cx.notify();
    }

    /// 原版 `handlePasswordSubmit`：解密并导入；失败不关闭对话框，错误显示在框内。
    fn submit_import_password(
        &mut self,
        password: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(bytes) = self.pending_import.clone() else {
            window.close_dialog(cx);
            self.password_dialog = None;
            cx.notify();
            return;
        };
        self.importing = true;
        cx.notify();
        let shared = self.handle.shared().clone();
        run_bg(
            window,
            cx,
            move || storage::import_project(&shared, bytes, Some(password)),
            |page: &mut Self, result, window, cx| {
                page.importing = false;
                match result {
                    Ok(_) => {
                        window.close_dialog(cx);
                        page.pending_import = None;
                        page.password_dialog = None;
                        window.push_notification(Notification::success("导入完成"), cx);
                        page.reload(window, cx);
                    }
                    // 密码错误等：保持对话框打开，把错误显示在框里（原版 externalError）。
                    Err(error) => {
                        if let Some(dialog) = page.password_dialog.clone() {
                            let _ = dialog
                                .update(cx, |dialog, cx| dialog.set_external_error(error, cx));
                        }
                    }
                }
                cx.notify();
            },
        );
    }
}

/// 原版 `projects.formatDate`：`2026/10/14 20:03`（本地时区）。
fn format_date(seconds: i64) -> String {
    use chrono::TimeZone as _;
    chrono::Local
        .timestamp_opt(seconds, 0)
        .single()
        .map(|time| time.format("%Y/%m/%d %H:%M").to_string())
        .unwrap_or_default()
}

impl Render for HomePage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.studio().colors;
        let theme_id = cx.studio().theme.id;
        let handle = self.handle.clone();

        // ── 操作栏 ─────────────────────────────────────────────
        let (settings_handle, new_handle) = (handle.clone(), handle.clone());
        let top_bar = div()
            .flex_none()
            .h(px(48.0))
            .px(px(20.0))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .bg(c.panel)
            // 便签主题：纸色渐变条（原版 `.top-bar { background: linear-gradient(#fbf7ec, #f1ebdf) }`）。
            .when_some(cx.studio().paper, |bar, p| bar.bg(widgets::paper_bar(&p)))
            .border_b_1()
            .border_color(c.border)
            .child(div().text_size(px(13.0)).text_color(c.muted).child("项目"))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .child(
                        div()
                            .relative()
                            // 原版 `.theme-anchor` 的 `@click.stop`（`HomeView.vue:181`）：
                            // 浮层里的点击不再往上冒泡，否则根节点的「点空白关浮层」会在
                            // mouse down 时先把浮层拆掉，主题圆点就点不到了。
                            .on_mouse_down(gpui_kit::MouseButton::Left, |_event, _window, cx| {
                                cx.stop_propagation()
                            })
                            .child(
                                widgets::icon_button("theme", "color-palette", "切换主题")
                                    .selected(self.show_theme_picker)
                                    .on_click(cx.listener(|page, _event, _window, cx| {
                                        page.show_theme_picker = !page.show_theme_picker;
                                        cx.notify();
                                    })),
                            )
                            .when(self.show_theme_picker, |anchor| {
                                let handle = handle.clone();
                                anchor.child(
                                    widgets::panel(cx)
                                        .absolute()
                                        .top(px(38.0))
                                        .right_0()
                                        .p(px(10.0))
                                        .shadow_lg()
                                        .child(
                                            // 原版 `ThemeToggle.vue` 的「主题」标签（`:29-30`）。
                                            div()
                                                .flex()
                                                .flex_row()
                                                .items_center()
                                                .gap(px(8.0))
                                                .child(widgets::section_label("主题", cx))
                                                .child(widgets::theme_dots(
                                                    "home-theme",
                                                    theme_id,
                                                    move |id, window, cx| {
                                                        handle.set_theme(id, true, window, cx)
                                                    },
                                                    cx,
                                                )),
                                        ),
                                )
                            }),
                    )
                    .child(widgets::bar_divider(cx))
                    .child(
                        widgets::ghost_button(
                            "import",
                            if self.importing {
                                "导入中…"
                            } else {
                                "导入"
                            },
                        )
                        .icon(carbon(if self.importing {
                            "renew"
                        } else {
                            "folder-add"
                        }))
                        .disabled(self.importing)
                        .on_click(cx.listener(|page, _event, window, cx| page.import(window, cx))),
                    )
                    .child(
                        widgets::icon_button("settings", "settings", "设置").on_click(
                            move |_event, window, cx| {
                                settings_handle.navigate(
                                    Route::Settings(SettingsTab::default()),
                                    window,
                                    cx,
                                )
                            },
                        ),
                    )
                    .child(
                        widgets::primary_button("new", "新建演示")
                            .icon(carbon("add"))
                            .on_click(move |_event, window, cx| {
                                new_handle.navigate(
                                    Route::Project {
                                        id: None,
                                        step: None,
                                    },
                                    window,
                                    cx,
                                )
                            }),
                    ),
            );

        // ── 标题区 ─────────────────────────────────────────────
        let count = self.projects.as_ref().map(Vec::len);
        let heading = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(12.0))
                    .child(
                        div()
                            .size(px(42.0))
                            .rounded(px(10.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(c.surface)
                            .border_1()
                            .border_color(c.border)
                            .child(widgets::brand_mark(28.0)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(c.primary)
                                    .child("幻述 STUDIO"),
                            )
                            .child(div().text_size(px(18.0)).child("我的演示文稿")),
                    ),
            )
            .when_some(count.filter(|count| *count > 0), |row, count| {
                row.child(
                    div()
                        .text_size(px(12.0))
                        .text_color(c.muted)
                        .child(format!("{count} 个项目")),
                )
            });

        // ── 列表 / 骨架 / 空状态 ──────────────────────────────
        let body: gpui_kit::AnyElement = match &self.projects {
            None => div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                .children((0..4).map(|_| {
                    widgets::panel(cx)
                        .h(px(64.0))
                        .flex()
                        .items_center()
                        .px(px(18.0))
                        .gap(px(12.0))
                        .child(widgets::brand_mark(24.0))
                        .child(
                            div()
                                .w(px(220.0))
                                .h(px(12.0))
                                .rounded(px(4.0))
                                .bg(c.surface),
                        )
                }))
                .into_any_element(),
            Some(list) if list.is_empty() => {
                let new_handle = handle.clone();
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(8.0))
                    .pt(px(80.0))
                    .child(widgets::brand_mark(48.0))
                    .child(div().text_size(px(16.0)).child("还没有任何演示文稿"))
                    .child(
                        div()
                            .text_size(px(13.0))
                            .text_color(c.muted)
                            .child("上传 Markdown 文件，AI 自动生成漂亮的演示文稿"),
                    )
                    .when_some(self.load_error.clone(), |empty, error| {
                        empty.child(div().text_size(px(12.0)).text_color(c.error).child(error))
                    })
                    .child(
                        widgets::primary_button("empty-new", "新建演示")
                            .icon(carbon("add"))
                            .on_click(move |_event, window, cx| {
                                new_handle.navigate(
                                    Route::Project {
                                        id: None,
                                        step: None,
                                    },
                                    window,
                                    cx,
                                )
                            }),
                    )
                    .into_any_element()
            }
            Some(list) => div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                .children(list.iter().map(|project| self.project_card(project, cx)))
                .into_any_element(),
        };

        div()
            .size_full()
            .flex()
            .flex_col()
            .on_mouse_down(
                gpui_kit::MouseButton::Left,
                cx.listener(|page, _event, _window, cx| {
                    if page.show_theme_picker {
                        page.show_theme_picker = false;
                        cx.notify();
                    }
                }),
            )
            .child(top_bar)
            .child(
                div()
                    .id("home-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(
                        div()
                            .mx_auto()
                            .w_full()
                            .max_w(px(BODY_MAX_W))
                            // 原版 `.home-body` 的 `padding: 1.5rem`（含左右）。
                            .px(px(24.0))
                            .py(px(24.0))
                            .flex()
                            .flex_col()
                            .gap(px(20.0))
                            .child(heading)
                            .child(body),
                    ),
            )
    }
}

impl HomePage {
    fn project_card(&self, project: &ProjectSummary, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.studio().colors;
        let id = project.id;
        let exporting = self.exporting == Some(id);
        let handle = self.handle.clone();
        let ratio = match project.aspect_ratio {
            Some(AspectRatio::Ratio32x9) => Some("32:9"),
            Some(AspectRatio::Ratio48x9) => Some("48:9"),
            _ => None,
        };
        let export_project = project.clone();
        // 便签主题下是一张便签卡（纸张阴影、小圆角），卡片之间露出木纹桌面。
        widgets::paper_card(cx)
            .id(SharedString::from(format!("project-{id}")))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(14.0))
            .px(px(18.0))
            .py(px(12.0))
            .cursor_pointer()
            .hover(|style| style.border_color(c.border_hover))
            .on_click(move |_event, window, cx| {
                handle.navigate(
                    Route::Project {
                        id: Some(id),
                        step: None,
                    },
                    window,
                    cx,
                )
            })
            .child(
                div()
                    .flex_none()
                    .size(px(37.0))
                    .rounded(px(9.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(c.surface)
                    .border_1()
                    .border_color(c.border)
                    .child(widgets::brand_mark(24.0)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .child(
                        div()
                            .text_size(px(15.0))
                            .truncate()
                            .child(project.name.clone()),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.0))
                            .when_some(ratio, |meta, ratio| {
                                meta.child(widgets::chip(ratio, Tone2::Warning, cx))
                            })
                            .child(widgets::chip(
                                format!("{} 张幻灯片", project.slide_count),
                                Tone2::Primary,
                                cx,
                            ))
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(c.muted)
                                    .child(format_date(project.updated_at)),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(4.0))
                    // 卡片的点击会打开项目；操作按钮自己处理点击，不往上冒泡。
                    .on_mouse_down(gpui_kit::MouseButton::Left, |_event, _window, cx| {
                        cx.stop_propagation()
                    })
                    .child(
                        widgets::icon_button(
                            SharedString::from(format!("export-{id}")),
                            if exporting { "renew" } else { "download" },
                            if exporting {
                                "导出中…"
                            } else {
                                "导出（.keynn / PDF）"
                            },
                        )
                        .disabled(exporting)
                        .on_click(cx.listener(
                            move |page, _event, window, cx| {
                                cx.stop_propagation();
                                page.open_export(export_project.clone(), window, cx)
                            },
                        )),
                    )
                    .child(
                        widgets::icon_button(
                            SharedString::from(format!("delete-{id}")),
                            "trash-can",
                            "删除",
                        )
                        .on_click(cx.listener(
                            move |page, _event, window, cx| {
                                cx.stop_propagation();
                                page.confirm_delete(id, window, cx)
                            },
                        )),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pdf_file_names_drop_the_characters_windows_refuses() {
        assert_eq!(safe_file_name("图表"), "图表");
        assert_eq!(safe_file_name("a/b:c*d?e"), "a_b_c_d_e");
        assert_eq!(safe_file_name("a::b"), "a_b");
        assert_eq!(safe_file_name("  压力  "), "压力");
        assert_eq!(safe_file_name("   "), "未命名演示");
    }
}
