//! Tauri 命令外壳：枚举系统字体的实现见 `huanshu_core::fonts`。

use huanshu_core::fonts::SystemFont;

#[tauri::command]
pub async fn list_system_fonts() -> Result<Vec<SystemFont>, String> {
    tauri::async_runtime::spawn_blocking(huanshu_core::fonts::enumerate_system_fonts)
        .await
        .map_err(|error| format!("读取系统字体失败：{error}"))?
}
