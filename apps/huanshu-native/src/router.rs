//! 页面路由：与原版 `src/main.ts` 的五条路由一一对应。
//!
//! | 原版                | 这里                                   |
//! |---------------------|----------------------------------------|
//! | `/`                 | [`Route::Home`]                        |
//! | `/new`              | [`Route::Project`] `{ id: None, .. }`  |
//! | `/project/:id`      | [`Route::Project`] `{ id: Some(id) }`  |
//! | `/settings`         | [`Route::Settings`]                    |
//! | `/wizard`           | [`Route::Wizard`]                      |
//!
//! 演示（`PresentationOverlay.vue`）不是路由，是盖在当前页上的一层，见 `present`。

/// 设置页的五个分区（原版 `SettingsView.vue` 左侧导航）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsTab {
    #[default]
    Appearance,
    Models,
    Generation,
    Storage,
    About,
}

impl SettingsTab {
    pub const ALL: [SettingsTab; 5] = [
        SettingsTab::Appearance,
        SettingsTab::Models,
        SettingsTab::Generation,
        SettingsTab::Storage,
        SettingsTab::About,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SettingsTab::Appearance => "外观与字体",
            SettingsTab::Models => "模型服务",
            SettingsTab::Generation => "生成设置",
            SettingsTab::Storage => "存储与缓存",
            SettingsTab::About => "关于与帮助",
        }
    }

    /// 原版导航项的 Carbon 图标。
    pub fn icon(self) -> &'static str {
        match self {
            SettingsTab::Appearance => "screen",
            SettingsTab::Models => "cube",
            SettingsTab::Generation => "settings-adjust",
            SettingsTab::Storage => "data-base",
            SettingsTab::About => "help",
        }
    }
}

/// 工作流的三步（原版 `WorkflowView.vue` 的 `currentStep`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Step {
    Prepare = 1,
    Generate = 2,
    Edit = 3,
}

impl Step {
    pub fn number(self) -> u8 {
        self as u8
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    Wizard,
    Home,
    Settings(SettingsTab),
    /// `id` 为 `None` 是「新建演示」。`step` 为 `None` 时按原版规则自动选：
    /// 生成进行中 → 2，有蓝图 → 3，否则 → 1。
    Project {
        id: Option<i64>,
        step: Option<Step>,
    },
}

impl Route {
    /// 命令行 `--route` 的写法：`home`、`wizard`、`settings[:tab]`、`new`、`project:<id>[:<step>]`。
    pub fn parse(text: &str) -> Option<Route> {
        let mut parts = text.split(':');
        let route = match parts.next()? {
            "home" => Route::Home,
            "wizard" => Route::Wizard,
            "new" => Route::Project {
                id: None,
                step: None,
            },
            "settings" => {
                let tab = match parts.next() {
                    None | Some("appearance") => SettingsTab::Appearance,
                    Some("models") => SettingsTab::Models,
                    Some("generation") => SettingsTab::Generation,
                    Some("storage") => SettingsTab::Storage,
                    Some("about") => SettingsTab::About,
                    Some(_) => return None,
                };
                Route::Settings(tab)
            }
            "project" => {
                let id = parts.next()?.parse().ok()?;
                let step = match parts.next() {
                    None => None,
                    Some("1") => Some(Step::Prepare),
                    Some("2") => Some(Step::Generate),
                    Some("3") => Some(Step::Edit),
                    Some(_) => return None,
                };
                Route::Project { id: Some(id), step }
            }
            _ => return None,
        };
        Some(route)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_parse_from_the_command_line() {
        assert_eq!(Route::parse("home"), Some(Route::Home));
        assert_eq!(
            Route::parse("settings:models"),
            Some(Route::Settings(SettingsTab::Models))
        );
        assert_eq!(
            Route::parse("project:3:2"),
            Some(Route::Project {
                id: Some(3),
                step: Some(Step::Generate)
            })
        );
        assert_eq!(
            Route::parse("new"),
            Some(Route::Project {
                id: None,
                step: None
            })
        );
        assert_eq!(Route::parse("project:x"), None);
        assert_eq!(Route::parse("nowhere"), None);
    }
}
