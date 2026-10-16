## 下载

### 原生版（Rust + GPUI，推荐 Windows 用户使用）

- `HuanshuNative_*_x64-setup.exe`：NSIS 安装包（currentUser 静默 `/S`，卸载 `uninstall.exe /S`）
- `huanshu-native_windows_x64.exe`：免安装单文件，双击即跑
- 不需要 WebView2，也不需要 VC++ 运行库；与 Tauri 版共用同一份数据目录，安装目录独立、互不覆盖
- Linux 暂时只有下面的 Tauri 版

### Tauri + Vue 版

- **Windows**：`Huanshu_*_x64-setup.exe`（NSIS 安装包）
- **Linux**：`Huanshu_*.AppImage` 或 `Huanshu_*.deb`
- 也可使用 portable 二进制：`auto-slidev-studio_windows_x64.exe` / `auto-slidev-studio_linux_x64`

两版界面同源、共用同一个数据目录（原生版的目标是接替 Tauri 版），功能对照表见
[docs/native/parity.md](https://github.com/billowsand/huanshu/blob/master/docs/native/parity.md)。

查看 [CHANGELOG](https://github.com/billowsand/huanshu/blob/master/CHANGELOG.md) 了解详细更新内容。
