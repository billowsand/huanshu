# Auto-Slidev Studio — 启动指南

## 环境要求

| 工具 | 版本 | 说明 |
|------|------|------|
| Rust | ≥ 1.77 | https://rustup.rs |
| Bun | ≥ 1.0 | https://bun.sh |
| Tauri CLI v2 | 最新 | `cargo install tauri-cli --version "^2"` |
| LM Studio | 任意 | https://lmstudio.ai |

---

## Windows 环境准备

Tauri v2 在 Windows 上使用 WebView2，通常已预装。如未安装：

```powershell
# 安装 Visual Studio Build Tools（C++ 构建工具链）
# https://visualstudio.microsoft.com/visual-cpp-build-tools/
# 勾选：Desktop development with C++

# 验证 Rust 工具链
rustup target add x86_64-pc-windows-msvc
```

WebView2 Runtime：Windows 11 已内置；Windows 10 需手动安装：
https://developer.microsoft.com/microsoft-edge/webview2/

---

## 首次安装

```bash
# 在项目根目录（PowerShell 或 Git Bash）
bun install
```

---

## 开发模式

```bash
cargo tauri dev
```

Tauri 会自动：
1. 运行 `bun dev`（Vite on :5173）
2. 编译 Rust 后端
3. 打开 Auto-Slidev Studio 窗口

> **如果 `vite` 启动失败**，先单独检查：
> ```bash
> bun dev
> ```
> 确认 :5173 能正常访问后再跑 `cargo tauri dev`。

---

## 构建发行版（生成 .exe 安装包）

```bash
# 先构建前端资产
bun build

# 构建 Tauri .exe
cargo tauri build
```

构建产物：`src-tauri/target/release/bundle/msi/` 或 `nsis/`

---

## 图标（首次构建前必须）

```bash
# 用任意 512×512 PNG 生成所有尺寸图标
cargo tauri icon icon.png
```

临时跳过：把 `src-tauri/tauri.conf.json` 中 `bundle.icon` 改为 `[]`

---

## 目录结构

```
auto-slidev-studio/
├── src-tauri/             # Tauri Rust 后端
│   ├── src/
│   │   ├── lib.rs         # Tauri 入口 + command 注册
│   │   ├── config.rs      # GenerationConfig + LlmSettings
│   │   ├── commands/      # generate / settings commands
│   │   └── generator/     # 生成器核心
│   └── tauri.conf.json
├── src/                   # Studio 前端（Vue3 + Vite）
│   ├── views/             # StudioView（三栏）+ SettingsView
│   ├── components/         # PipelineProgress, SlideCard, SlideRenderer, Keynote*.vue
│   └── stores/            # generation.ts, config.ts
├── dist-studio/            # 构建产物（构建后生成）
└── index.html
```

---

## 使用流程

1. **启动** → `cargo tauri dev`
2. **设置 LLM** → 右上角「设置」，填写 LM Studio 地址和模型名
3. **上传 Markdown** → 点击「上传文件」或粘贴内容
4. **生成** → 点击「生成 Keynote」，六阶段进度实时可见
5. **调试** → 幻灯片卡片点 🔍 查看/修改 Blueprint JSON，支持单页修复
