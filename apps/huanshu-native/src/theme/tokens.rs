//! Studio 外壳的配色 token：从 Tauri 版 `src/style.css` 与 `src/themes/*.css` 的 `--studio-*` 变量
//! **在运行中的应用里读取计算值**生成（不是手抄），每个主题一份。改 CSS 时同步改这里。
//! `warning*` 只有便签主题定义了，其余主题用同色系的补值（标注 `filled`）。
//! 生成于 2026-10-14；颜色是 0xRRGGBBAA。

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenTable {
    pub bg: u32,
    pub panel: u32,
    pub panel_2: u32,
    pub surface: u32,
    pub border: u32,
    pub border_hover: u32,
    pub text: u32,
    pub muted: u32,
    pub primary: u32,
    pub primary_light: u32,
    pub primary_contrast: u32,
    pub primary_bg: u32,
    pub primary_border: u32,
    pub success: u32,
    pub success_bg: u32,
    pub error: u32,
    pub error_bg: u32,
    pub error_border: u32,
    pub warning: u32,
    pub warning_bg: u32,
    pub warning_border: u32,
}

pub fn table(theme_id: &str) -> TokenTable {
    match theme_id {
        "dark-yellow" => TokenTable {
            bg: 0x0b0c11ff,
            panel: 0x111318ff,
            panel_2: 0x16181fff,
            surface: 0x1c1f2bff,
            border: 0xffffff12,
            border_hover: 0xffffff24,
            text: 0xe0e2ecff,
            muted: 0xe0e2ec61,
            primary: 0xf59e0bff,
            primary_light: 0xfcd34dff,
            primary_contrast: 0x140d02ff,
            primary_bg: 0xf59e0b1a,
            primary_border: 0xf59e0b47,
            success: 0x10b981ff,
            success_bg: 0x10b9811a,
            error: 0xf43f5eff,
            error_bg: 0xf43f5e1a,
            error_border: 0xf43f5e47,
            warning: 0xfbbf24ff,        // filled
            warning_bg: 0xfbbf241a,     // filled
            warning_border: 0xfbbf2447, // filled
        },
        "dark-blue" => TokenTable {
            bg: 0x070c17ff,
            panel: 0x0d1422ff,
            panel_2: 0x11192cff,
            surface: 0x172036ff,
            border: 0x38bdf817,
            border_hover: 0x38bdf833,
            text: 0xd4e8f8ff,
            muted: 0xd4e8f861,
            primary: 0x38bdf8ff,
            primary_light: 0x7dd3fcff,
            primary_contrast: 0x031521ff,
            primary_bg: 0x38bdf81a,
            primary_border: 0x38bdf847,
            success: 0x34d399ff,
            success_bg: 0x34d3991a,
            error: 0xfb7185ff,
            error_bg: 0xfb71851a,
            error_border: 0xfb718547,
            warning: 0xfbbf24ff,        // filled
            warning_bg: 0xfbbf241a,     // filled
            warning_border: 0xfbbf2447, // filled
        },
        "dark-terminal" => TokenTable {
            bg: 0x151614ff,
            panel: 0x1b1d19ff,
            panel_2: 0x1b1d19ff,
            surface: 0x242720ff,
            border: 0x393d33ff,
            border_hover: 0x68705dff,
            text: 0xeeeedeff,
            muted: 0xa1a697ff,
            primary: 0xed9b62ff,
            primary_light: 0xf4b787ff,
            primary_contrast: 0x20170fff,
            primary_bg: 0x30251eff,
            primary_border: 0x76543bff,
            success: 0xa9c58cff,
            success_bg: 0x252e20ff,
            error: 0xe99388ff,
            error_bg: 0x322321ff,
            error_border: 0x754b44ff,
            warning: 0xfbbf24ff,        // filled
            warning_bg: 0xfbbf241a,     // filled
            warning_border: 0xfbbf2447, // filled
        },
        "light-terminal" => TokenTable {
            bg: 0xf4f3ebff,
            panel: 0xfaf9f3ff,
            panel_2: 0xfaf9f3ff,
            surface: 0xeaeadeff,
            border: 0xc9cebeff,
            border_hover: 0x8b947eff,
            text: 0x25291fff,
            muted: 0x656b5cff,
            primary: 0x9d4a1cff,
            primary_light: 0x803b16ff,
            primary_contrast: 0xfff9eeff,
            primary_bg: 0xeee0d1ff,
            primary_border: 0xc4a383ff,
            success: 0x49682cff,
            success_bg: 0xe7eddeff,
            error: 0xa64239ff,
            error_bg: 0xf2e2dcff,
            error_border: 0xc6a29aff,
            warning: 0xa16207ff,        // filled
            warning_bg: 0xfef3c7ff,     // filled
            warning_border: 0xe7c77bff, // filled
        },
        "light-editorial" => TokenTable {
            bg: 0xf6f5f0ff,
            panel: 0xffffffff,
            panel_2: 0xfaf9f6ff,
            surface: 0xeeede7ff,
            border: 0xd8d6cfff,
            border_hover: 0x989890ff,
            text: 0x141414ff,
            muted: 0x73736fff,
            primary: 0x2549d8ff,
            primary_light: 0x1d3db8ff,
            primary_contrast: 0xffffffff,
            primary_bg: 0xe9edfcff,
            primary_border: 0xbdc9f1ff,
            success: 0x2e6e4fff,
            success_bg: 0xe2ede5ff,
            error: 0xa8322aff,
            error_bg: 0xf4e1dcff,
            error_border: 0xd7a9a1ff,
            warning: 0xa16207ff,        // filled
            warning_bg: 0xfef3c7ff,     // filled
            warning_border: 0xe7c77bff, // filled
        },
        "light-notes" => TokenTable {
            bg: 0xf3e3c5ff,
            panel: 0xfaf7efff,
            panel_2: 0xfdfaf4ff,
            surface: 0xeee8dcff,
            border: 0xded5c7ff,
            border_hover: 0xa38767ff,
            text: 0x786051ff,
            muted: 0x89786bff,
            primary: 0x746254ff,
            primary_light: 0x655547ff,
            primary_contrast: 0xfbf7ecff,
            primary_bg: 0xeee7daff,
            primary_border: 0xb6a492ff,
            success: 0x54633dff,
            success_bg: 0xedf0e2ff,
            error: 0x9b4e43ff,
            error_bg: 0xf7e9e3ff,
            error_border: 0xd4b0a5ff,
            warning: 0x826039ff,
            warning_bg: 0xf6ebd8ff,
            warning_border: 0xcfb489ff,
        },
        _ => table("dark-yellow"),
    }
}
