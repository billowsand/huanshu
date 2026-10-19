# 幻述 — 启动指南

幻述是**纯 Rust 项目**：一个 Cargo workspace，三个 crate，不需要 Node.js / bun，也不需要 WebView2。

| Crate | 作用 |
|---|---|
| `crates/huanshu-core` | 业务内核：数据库、生成流水线、导入导出、设置（不依赖任何界面框架） |
| `crates/slide-typst` | 进程内嵌的 Typst 编译器：幻灯片位图、公式位图与 PDF 全走它 |
| `apps/huanshu-native` | 界面（GPUI Kit），唯一的可执行程序 |

---

## 环境要求

| 工具 | 版本 | 说明 |
|------|------|------|
| Rust | ≥ 1.85 | https://rustup.rs |
| LM Studio | 任意 | https://lmstudio.ai（生成流程需要，可选） |

Windows 另需 MSVC 工具链（Visual Studio Build Tools 的「使用 C++ 的桌面开发」）：

```powershell
rustup target add x86_64-pc-windows-msvc
```

macOS 只需 Xcode Command Line Tools（GPUI Kit 开了 `runtime_shaders`，不需要完整 Xcode 的 Metal 编译器）：

```bash
xcode-select --install
```

出 Windows 安装包时另需 NSIS（`choco install nsis`，或装独立版 NSIS）；只在 `package-native.ps1`
找不到 `makensis` 时才需要。

---

## 运行

```bash
cargo run -p huanshu-native --release
```

> 调试构建里第三步的编辑器表单会栈溢出——`apps/huanshu-native/build.rs` 已把 Windows 主线程栈提到 8 MiB，
> 但 release 更快，推荐直接 `--release`。

### 沙箱数据目录（验证用，绝不碰真实数据）

设 `HUANSHU_HOME` 后，数据库、`settings.json`、素材与 `bootstrap.json` 全部落在它下面：

```powershell
$env:HUANSHU_HOME = 'E:\huanshu\target\sandbox-home'
cargo run -p huanshu-core --example seed_demo -- $env:HUANSHU_HOME   # 灌一批演示项目（已有库时加 --force）
cargo run -p huanshu-native --release
```

---

## 构建发行版

Windows（免安装单文件 + NSIS 安装包，汇总到 `dist-package/`）：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/package-native.ps1
```

macOS（`.app`，加 `--universal` 出 arm64 + x86_64 通用版）：

```bash
scripts/package-native-macos.sh --universal
```

只想编译、不出包：`cargo build --release -p huanshu-native` → `target/release/huanshu-native.exe`。

打包脚本与发版的坑全在根 `CLAUDE.md` 的「打包与发布」章节。验收一律在沙箱里做。

---

## 图标与素材

图标与 Carbon 图标集都**已提交在仓库里**，不需要额外的生成步骤：

| 资源 | 位置 | 谁用它 |
|---|---|---|
| Carbon 图标集（Apache-2.0） | `assets/carbon/icons.json` | 两个 `build.rs` 编译期内嵌；缺文件直接编译失败 |
| exe / `.app` 图标 | `assets/app-icon/` | `apps/huanshu-native/build.rs`（ico 进 exe）、`package-native-macos.sh`（icns 进 `.app`） |

改了品牌图标后重新生成：

```bash
python scripts/generate_brand_assets.py
```

---

## 目录结构

```
huanshu/
├── crates/
│   ├── huanshu-core/       # 业务内核（无界面依赖）
│   └── slide-typst/        # Typst 渲染器（typst/ 下是版式与主题）
├── apps/
│   └── huanshu-native/     # 界面（GPUI Kit）
│       ├── src/            #   shell / router / theme / pages / platform
│       └── tools/          #   截图与交互验证脚本（shots.ps1 / drive.ps1）
├── assets/                 # Carbon 图标集与应用图标
├── docs/                   # 设计文档、原生界面对照表、截图
└── scripts/                # 打包脚本与品牌素材生成
```

---

## 使用流程

1. **启动** → `cargo run -p huanshu-native --release`（首次会进向导）
2. **设置 LLM** → 「设置 → 模型服务」填写 LM Studio 地址与模型名
3. **上传 Markdown** → 第一步「上传文件」或直接粘贴
4. **生成** → 「开始 AI 生成」，第二步看板逐页显示状态
5. **编辑与导出** → 第三步改版式与内容，「导出 PDF」或「导出 Typst 工程」
