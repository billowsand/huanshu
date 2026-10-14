# GPUI Kit spike（幻述原生界面技术验证）

这个目录回答一个问题：**GPUI Kit（crate `gpui-kit`，Longbridge 出品，把 Zed 的 GPUI 打包成
一个依赖）能不能在本机（Windows 11、MSVC、中文输入法、150% 缩放）上撑起幻述的界面？**

它不迁移任何幻述功能：幻灯片仍是 `crates/slide-typst` 用 Typst 编译 + 光栅化，这里只是把
Rust 进程里已经有的像素交给 GPUI 显示，并逐项验证外壳能力（无边框窗口、主题、看板、编辑器、
文件对话框、对话框、轻提示、演示模式、拖拽排序、输入法）。

验证结论、逐项结果、性能数字与人工验收清单见 `../../../docs/spikes/gpui-kit.md`。

## 运行

```powershell
cd spikes/gpui-kit
cargo run --release
```

独立 cargo 工程（自带 `[workspace]`），所以 `cargo test --workspace` 不会连带编译 GPUI。
它通过 path 依赖 `../../crates/slide-typst`，构建产物在 `spikes/gpui-kit/target/`。

> **debug 构建的坑（本机）**：`stacker`、`tree-sitter`、`tree-sitter-md` 的 C 构建脚本会以
> `cl : Command line error D8050 : 无法将命令行放入调试记录中` 失败，因为 cc-rs 在有调试信息
> 的 profile 下会传 MSVC 的 `-Z7`。同一份命令在 `target/` 下能过、在 `spikes/` 下不行（与命令行
> 长度无关，用一个三行 crate 即可复现）。`Cargo.toml` 里用 `[profile.dev] debug = false` 绕开；
> release 不传 `-Z7`，不受影响。详见报告的 A1 一节。

## 命令行开关

截图和数字都靠这些开关复现（不必手点界面）：

| 开关 | 作用 |
|---|---|
| `--page board\|editor\|controls` | 启动时停在哪个标签页 |
| `--theme dark\|light` | GPUI Kit 的深色 / 浅色主题（运行时可切换） |
| `--typst-theme <id>` | 幻灯片主题，取值见 `slide_typst::SUPPORTED_THEMES`（默认 `light-notes`） |
| `--deck <path>` | 启动时载入的牌组 JSON（默认 `fixtures/all-layout-blueprints.json`） |
| `--present` | 同时打开演示窗口（无边框全屏） |
| `--dialog` | 启动时打开「导出加密牌组」密码对话框 |
| `--bench` | 跑 B2 位图交接基准（两种模式各 5 轮，交替） |
| `--bench-edit` | 跑 B3 标题输入延迟基准（10 轮） |
| `--soak` | 连续编辑标题 2 分钟，每 30 秒报一次内存 |
| `--quit-after <秒>` | 到点自动退出（便于脚本化） |

## 每个页面验证什么

### 幻灯片看板（`--page board`，默认）

- 左侧是 GPUI Kit 的虚拟列表（`v_virtual_list`），窗口尺寸变化时按 64 px 档位重算光栅宽度；
- 右侧是选中页的大图，宽度按「显示宽度 × 缩放比例（本机 1.5）」光栅化；
- 顶部下拉框切换 Typst 主题；开关在 `render_png + 解码` 与 `render_pixels` 之间切换；
- 大图下方是单行输入框，改选中页 `title` 会实时重渲：同一时刻只有一个请求在途，期间的变化
  合并成尾部一次（与 `useTypstThumbnails` 的 `serial` 相同），且只有键匹配的响应才会落到画面上。

### Markdown 编辑器（`--page editor`）

- 默认用 `Editor` + `EditorState`（tree-sitter markdown 高亮），可切到 `Textarea` 对照，切换时
  文档在两者之间搬运，不会丢；
- 打开 `assets/sample.md`（20146 字符 / 395 行，其中 11138 个汉字）；
- 状态栏显示字符数与光标行列；支持打开 / 另存为 / 恢复示例文档（`rfd`）。

### 外壳组件（`--page controls`）

按钮（primary/ghost/danger/disabled/Toggle）、开关、复选框、单选组、滑块、数字输入、
下拉框、确认对话框（`AlertDialog`）、密码对话框（`Input` 掩码 + 强度条）、四种轻提示、
文件打开/保存对话框、Popover、以及手写的拖拽排序列表（`on_drag` / `drag_over` / `on_drop`）。

### 演示模式（`--present` 或标题栏的「演示模式」）

无边框全屏窗口，`←/→/空格/PageUp/PageDown/Home…`、左右鼠标键翻页，`Esc` 关闭；位图按屏幕
非等比铺满（32:9 牌组在 16:9 屏上会被压扁，与 `PresentationOverlay` 的拉伸模式一致）；
当前页没有图时只请求当前页，落图后再预取下一页。

## 工具

| 脚本 | 作用 |
|---|---|
| `tools/make_sample.py` | 生成 `assets/sample.md`（需要 Python） |
| `tools/screenshot.ps1` | 截取进程的所有可见顶层窗口；先 `SetProcessDPIAware`，否则 150% 缩放下会得到糊图 |
| `tools/coldstart.ps1` | 冷启动测量：窗口可见、首个非空画面（像素代理）、App 自报的首帧 |

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tools/screenshot.ps1 -Prefix board-dark
powershell -NoProfile -ExecutionPolicy Bypass -File tools/coldstart.ps1 -Runs 5
powershell -NoProfile -ExecutionPolicy Bypass -File tools/coldstart.ps1 -Exe ..\..\..\target\release\auto-slidev-studio.exe
```

## 与 Tauri 版的对应关系

| 这里 | 幻述里的对应物 |
|---|---|
| `src/deck.rs` | `commands/thumbnails.rs` + `composables/useTypstThumbnails.ts` |
| `src/image_bridge.rs` | WebView 的 PNG/base64 解码 → `<img>` |
| `src/board.rs` | `Step2Generate.vue` 的看板与详情 |
| `src/editor.rs` | `components/MarkdownWorkbench.vue`（Step1 的 CodeMirror） |
| `src/controls.rs` | `PasswordDialog.vue`、`ExportDialog.vue` 等对话框与设置项 |
| `src/present.rs` | `PresentationOverlay.vue` |
