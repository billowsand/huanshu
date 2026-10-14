# GPUI Kit 原生界面验证报告（第六步 spike）

- 分支：`spike/gpui-kit`（从 `feat/typst-only` 起）
- 程序：`spikes/gpui-kit/`（独立 cargo 工程，自带 `[workspace]`），`cargo run --release` 启动
- 机器：Windows 11、MSVC 14.44（VS 2022 BuildTools）、rustc 1.97.1、RTX 4090 Laptop + Intel UHD、
  **显示器 2560×1600 @150%**（`window.scale_factor() == 1.5`）
- 依赖：`gpui-kit = "=0.7.3"`（`Cargo.lock`：gpui-kit / gpui-base / gpui-component 均 0.7.3，
  引擎是 `gpui-pre 0.3.9`，即 zed@93d7456 的快照）
- 截图：本目录下的 `board-dark-0.png`、`board-light-0.png`、`board-darkblue-0.png`、`editor-0.png`、
  `controls-0.png`、`controls-dialog-0.png`、`presentation-0.png`（全屏演示）、`presentation-1.png`（背后的主窗口）
- 日志：本次所有数字都是 **release** 构建、`--quit-after` 自动退出、stderr 重定向到文件后读出来的

---

## 1. 结论

**有条件能。**

GPUI Kit 0.7.3 能把幻述现在的外壳（无边框窗口 + 自绘标题栏 + 深浅主题 + 看板 + 演示 +
多行编辑 + 表单/对话框/轻提示/文件对话框）搭起来，幻灯片位图交接也很快。剩下**一个条件**必须先满足：

> **审阅更正（内存）**：初稿把「连续重渲内存不收敛（5 分钟 353 → 2103 MiB）」列为第一阻塞项，并归到 GPUI 侧。
> 审阅时查明是 spike 自己的泄漏：`soak_tick` / `type_bench_char` 每次改标题都 `self.thumbs.remove(&self.selected)`，
> 丢掉旧缩略图却没有 `cx.drop_image`，GPUI 图集里的 tile 永远不释放（384 px 缩略图约 0.32 MiB × 每秒 20 次
> ≈ 6.3 MiB/s，正好是初稿测到的「带 `drop_image` 仍 ~6 MiB/s」）。补上释放（`Board::forget_selected_thumb`，
> 换牌组时也释放全部缩略图与大图）后重测：连续改标题 **120 s 稳定在 299–301 MiB，300 s 在 282–306 MiB**，无增长。
> 结论：GPUI 不会自动回收 `RenderImage` 的图集 tile（第 4 节第 2 条），**宿主每丢一张图都必须 `drop_image`**——
> 这是迁移时的编码规则，不是阻塞项。

1. ~~连续重渲下的内存不收敛~~（已更正，见上）。
2. **中文输入法必须真机验收**（本文第 6 节）。源码层面 IME 接口是齐全的、Windows 平台也实现了
   组字与候选窗定位，但候选框跟光标、多屏、150% 缩放这些只能人来看——**我没有验证过**。

另外有两条不算阻塞、但要提前决策的：

3. Windows 上 `Window::start_window_move()` 是空实现，标题栏拖动/双击最大化/贴靠全部交给系统
   （`WindowControlArea::Drag` → `HTCAPTION`）；`TitleBar::on_close_window` 只在 Linux 生效。
   现在跑得通，但自绘标题栏能做多少、要人配合验证多少，得先接受这个边界。
4. Step1 的 Markdown **预览**（现在 `MarkdownWorkbench.vue` 里 markdown-it + KaTeX 渲染、`typography.css` 的 `.md-preview`）
   在 GPUI Kit 里**没有对应组件**：库里有 tree-sitter 高亮的代码编辑器，但没有 Markdown → 富文本
   的渲染器。要么把 Step1 预览也做成 Typst（多一条渲染链），要么自己写（工作量大）。

## 2. 逐项结果表

| 项 | 结果 | 依据 |
|---|---|---|
| **A1** 骨架 / bootstrap | **通过** | `gpui_kit::application().with_assets(gpui_kit::assets::Assets).run(..)` + `gpui_kit::init` + `gpui_kit::open_window`，见 `src/main.rs:300-330`。首次完整编译：release 184.3 s（587 个 crate，从零；含桩 main.rs），debug 首次在 `stacker` 上以 D8050 失败（见第 7 节），加 profile 覆盖后补完 48.3 s。release 可执行 **62.3 MiB** |
| **A2** 无边框窗口 + 自绘标题栏 | **部分通过（点击/拖动/贴靠需人工）** | `titlebar: None` → `gpui-pre-windows/src/window.rs:478-482` 的 `hide_title_bar` 驱动 `WM_NCCALCSIZE`（`events.rs:770`），窗口确实没有系统外框；`TitleBar::new()` 渲染出「最小化/最大化/关闭」三个按钮（`title_bar.rs:254-296`，截图 `board-dark-0.png` 右上角）。**Windows 上这三个按钮是系统命中测试区**（`HTMINBUTTON`/`HTMAXBUTTON`/`HTCLOSE`，`gpui-pre-windows/src/events.rs:968-976`），拖动/双击最大化同样是 `HTCAPTION` 原生行为；`start_window_move()` 在 Windows 是空实现（`platform.rs:1325` 默认体）。我没有脚本化点击/拖动/贴靠 |
| **A3** 深浅两套主题、运行时切换 | **通过** | `Theme::change(ThemeMode::Dark/Light, Some(window), cx)`（`theme/mod.rs:389`）；标题栏的开关即时切换（截图 `board-dark-0.png` ↔ `board-light-0.png`）。样式全部取 `cx.theme()` 的 token，没有硬编码颜色 |
| **B1** 看板（虚拟列表 + 大图 + 换牌组 + 主题下拉） | **通过** | 18 页 `fixtures/all-layout-blueprints.json`；`v_virtual_list` 单列虚拟列表 + 右侧大图（`board.rs`）；`rfd` 打开对话框已接（`board.rs:open_deck`，系统模态窗需人工点）；下拉框 6 个 `slide_typst::SUPPORTED_THEMES`，`--typst-theme dark-blue` 的截图 `board-darkblue-0.png` 证明真的换了 |
| **B1** 渲染不阻塞 UI 线程、Renderer 只建一次 | **通过** | 光栅化全部在 `cx.background_spawn` 里（`board.rs:pump`、`present.rs:pump`）；`deck::renderer()` 用 `OnceLock`，字体扫描一次性 154–162 ms，日志只打一次 |
| **B1** 位图宽度 = 显示宽度 × 缩放、64 px 档位 | **通过** | 缩略图 248 pt × 1.5 → **384 px**；大图 (1440−300−48) pt × 1.5 → **1664 px**（状态栏实时显示这两个数字，截图可见）。`deck::bucket_width` 照抄 `useTypstThumbnails.ts:132` 的 `max(64, ceil(w/64)*64)` |
| **B2-1** `render_png` + 解码 | **通过** | 开关切到「render_png + 解码」后仍能显示（内部把 PNG 解成 RGBA 再换 BGRA，与 GPUI 自己的解码路径一致） |
| **B2-2** 新增 `Renderer::render_pixels` + 单测 | **通过** | `crates/slide-typst/src/lib.rs`（返回未编码的未预乘 RGBA8；`only`、宽度钳制、溢出诊断与 `render_png` 一致）；单测 3 个：与同一次编译的 PNG 解码逐像素相同、宽度规则、去预乘（22 个 lib 单测全过） |
| **B2-3** 两种方式各 5 次中位数 | **通过** | 见第 3 节。整副 18 页：PNG 54.4 ms vs 裸像素 35.6 ms；单页大图：31.4 ms vs 27.7 ms |
| **B3** 实时编辑预览（serial + 只编译当前页） | **通过** | 「同一时刻只有一个请求在途、期间变化合并成尾部一次、只有键匹配的响应才落到画面」都在 `board.rs:pump/finish`；只发 `[当前页]`。10 次中位数 **51.6 ms**（34.9–56.4，见第 3 节） |
| **C1** 多行 Markdown 编辑 | **通过（撤销/复制等靠组件预绑定，未逐键手测）** | 1 万字文档（20146 字符 / 395 行 / 11138 汉字，`assets/sample.md`）；`Editor` + `EditorState::language("markdown")` 打开 tree-sitter 高亮（截图 `editor-0.png` 可见标题/行内代码/粗体分色）；软换行默认开（`gpui-base/src/input/base/state.rs:812`）、滚动条内置；状态栏显示字符数与行列。撤销/重做/全选/复制剪切粘贴/Ctrl+←→/Home/End 是 `state.rs:136-308` 在 `"Input"` 上下文里预绑定的（由 `gpui_component::init` 注册），**我是读源码确认的，没有逐键手测** |
| **C2** 中文输入法 | **接口层面通过，实机效果需人工** | 见第 6 节。所有三种输入控件都走同一个 `EntityInputHandler` 实现；Windows 平台处理 `WM_IME_*` 并用 `ImmSetCompositionWindow`/`ImmSetCandidateWindow` 定位候选框。**真机行为我无法验证** |
| **D** 控件（下拉/按钮/开关/滑块/数字） | **通过** | `Select`+`SearchableVec`、`Button`、`Switch`、`Slider`+`SliderState`（订阅 `SliderEvent`）、`NumberInput`、`Checkbox`、`RadioGroup`、`Toggle`、`Label`——截图 `controls-0.png`，每项都是活的 |
| **D** 模态对话框 | **通过** | 确认删除用 `AlertDialog`（`window.open_alert_dialog`），密码框用 `Input(masked)` + `Progress` 强度条 + 自组合 `DialogFooter`（截图 `controls-dialog-0.png`）。注意 `Dialog` 的 builder 是**每次重建都会再跑一遍的渲染闭包**，实体要在打开前建好 |
| **D** 轻提示 / toast | **通过** | `window.push_notification(Notification::info/success/warning/error(..), cx)`，默认 5 秒自动消失 |
| **D** 文件打开/保存对话框 | **通过（合成为代码，模态需人工点）** | `rfd::AsyncFileDialog` + `background_spawn` 读文件，不阻塞 UI |
| **D** 可拖拽排序列表 | **部分通过（组件不支持 → 自行实现；真实拖动未验证）** | 列表/表格**没有**行级拖拽排序：`list/` 里的拖拽代码只在 `#[cfg(test)]`；`table/` 只有**列**排序（`move_column`）。我用公开的 `on_drag` / `drag_over::<T>` / `on_drop` 手写了一个 6 行列表（`controls.rs`），能编译、能渲染，**但真实拖动交互没有脚本化验证**（人工清单里有） |
| **D** 演示模式 | **通过** | 无边框全屏窗口（`WindowBounds::Fullscreen`），`cx.observe_keystrokes` 处理左右/空格/Home/End/Esc，鼠标左右键翻页，`ObjectFit::Fill` 非等比铺满，当前页无图时只请求当前页、落图后预取下一页（截图 `presentation-0.png` 是 2560×1600 屏上的 16:9 页面，明显被拉伸） |
| **E** 冷启动 | **通过** | 见第 3 节（本程序 190.7 ms 到首帧、283.5 ms 到窗口可见；幻述 106.5 ms 到窗口、870.4 ms 到出内容） |
| **E** 内存 | **通过（审阅更正）** | 空闲 74–78 MiB；18 页缩略图 + 大图就绪 321–369 MiB；补上缩略图的 `drop_image` 后连续改标题 5 分钟稳定在 **282–306 MiB**（初稿的 353 → 2103 MiB 是 spike 漏释放缩略图，见第 1 节） |
| **E** 窗口缩放时按档位重渲 | **通过** | 1440×900 → 1200×767（逻辑）后「大图」从 `@1664px` 变 `@1280px` 并重渲（`resize-before-0.png` / `resize-after-0.png`） |
| **E** panic / 渲染错误 / 闪烁 | **未发现 panic；无错误弹窗** | 全部运行里没有 panic、没有 Typst 编译错误（日志里只有 fixture 自带的两条 warning：`carbon:target` 等图标不在图标集里、三个数学字体缺失，这与 Tauri 版一致）。切换节点/主题时用截图看不出闪烁，**但「闪烁」这种主观项我只看了静态截图，不算验证过**。缩放时的撕裂也没测（静态截图看不出来） |

## 3. 性能数字

全部为 release 构建。测法：`spikes/gpui-kit/target/release/gpui-kit-spike.exe` 加下方参数运行，
stderr 里的 `[time]` / `[mem]` / `[coldstart]` 行即原始数据（重定向到文件后读取）。

### 3.1 编译与产物

| 项 | 数字 | 测法 |
|---|---|---|
| **release 首次完整编译**（**最终代码** `cargo clean && cargo build --release`） | **216.4 s**（3m36s） | `cargo clean` 删掉整个 10.6 GiB 的 `target/`，再 `cargo build --release`；`Finished release profile [optimized] target(s) in 3m 36s` |
| release 增量（改一个 .rs） | **1.2 – 7.7 s** | 同上，第一次完整构建之后 |
| **debug 首次完整编译**（`cargo clean --profile dev && cargo build`） | **145.3 s**（2m25s） | `cargo clean --profile dev` 然后 `cargo build`；`Finished dev profile [unoptimized] target(s) in 2m 25s` |
| **release 可执行文件** | **65 320 448 B = 62.3 MiB** | clean 构建后 `Get-Item target/release/gpui-kit-spike.exe` |
| **debug 可执行文件** | **129 607 168 B = 123.6 MiB** | 同上，`target/debug/gpui-kit-spike.exe` |
| 对照：幻述 release 可执行文件 | 70 211 584 B = 67.0 MiB | `target/release/auto-slidev-studio.exe` |

> 编译时间的小坑：本机 `stacker` / `tree-sitter` / `tree-sitter-md` 的 C 构建脚本在带调试信息
> 的 profile 下会因 `cl : D8050` 失败（见第 4 节第 1 条），所以 `Cargo.toml` 里加了
> `[profile.dev] debug = false`；上面 145.3 s 的「debug 首次」是绕开这个之后测出来的，没有它
> 就跑不到 `Finished`。release 本来就不传 `-Z7`，不受影响。

### 3.2 B2：两种位图交接方式（各 5 次，交替进行以共享编译缓存）

| 场景 | `render_png` + PNG 解码 | `render_pixels` + 通道交换 |
|---|---|---|
| 整副 18 页缩略图 @384 px | **54.4 ms**（54.3 / 54.4 / 55.5 / 52.6；另有 1 次 425.2 ms 被中位数排除，那一轮与看板自己的首批渲染并发） | **35.6 ms**（35.6 / 34.9 / 36.1 / 35.5 / 36.4） |
| 单页大图 @1664 px | **31.4 ms**（31.4 / 29.8 / 32.1 / 31.7 / 30.4） | **27.7 ms**（27.6 / 27.7 / 28.5 / 28.0 / 27.2） |

- 计时口径：`[time] B2 ...` 是「发起请求 → 位图已装进 UI 状态并 `notify`」，即画面就绪；再往后
  还有一帧的 GPU 呈现（≤16 ms）。
- 差值：整副 18 页省约 **19 ms**（1.5 倍），单页大图省约 **3.7 ms**（1.13 倍）。省下的是
  PNG 编码（`tiny-skia` 还要为去预乘多拷一份缓冲区）+ 应用侧解码；`render_pixels` 只做一次
  去预乘。
- 两种方式最终都交给 GPUI 同一张 BGRA 位图，所以差异只在编码/解码，不在显示路径。

### 3.3 B3：标题输入延迟（10 次中位数）

| 项 | 数字 |
|---|---|
| 末端按键 → 该文本对应的大图就绪 | **中位数 51.6 ms**（35.1 / 55.4 / 53.8 / 56.4 / 34.9 / 36.1 / 35.7 / 49.5 / 55.6 / 53.8），两簇 35 ms 与 55 ms 交替出现，对应「这一轮是否刚好卡在上一轮在途请求上」 |

测法：`--bench-edit`，10 轮 × 每轮 6 次改动、间隔 30 ms，每轮测最后一次改动到对应画面就绪。
**这是程序化驱动同一个 change 处理器，不是合成键盘事件**；真机打字（含输入法）另见人工清单。

### 3.4 冷启动（各 5 次，中位数）

| 进程 | 窗口可见 | 首个非空画面 | 程序自报首帧 |
|---|---|---|---|
| GPUI Kit spike | **283.5 ms**（262–305） | **445.0 ms**（419–816，两簇） | **190.7 ms**（173–198，从 `main` 入口算） |
| 幻述（Tauri + WebView2） | **106.5 ms**（69–116） | **870.4 ms**（704 / 726 / 870，另有 3925 与 6884 两次异常慢） | —（程序不自报） |

- 「窗口可见」是 PowerShell `Start-Process` 到能枚举到该进程的可见窗口，**两边都含 `Start-Process`
  自身约 50–80 ms 的开销**。
- 「首个非空画面」是截窗口位图直到亮度方差不小于 10 且均值相对第一帧偏移大于 6（只看窗口内侧
  88%，避免把窗口阴影/桌面算成内容）。这个代理**有约 ±50–100 ms 的粒度**，而且幻述的两次
  3.9 s / 6.9 s 说明同一台机器上波动很大（当时有别的编译在跑）。
- 结论只能这么说：**GPUI 的窗口出现比 Tauri 慢（283 vs 107 ms），但画面（真内容）比 WebView2
  快**——GPUI 自报首帧 190.7 ms，而幻述要等 WebView2 + Vue 起来，代理值 870 ms 量级。

### 3.5 内存（`GetProcessMemoryInfo.WorkingSetSize`，程序自己在各阶段打印）

| 阶段 | 工作集 |
|---|---|
| 启动前（`main` 第一行） | 10.2 MiB |
| 三个页面视图构造完（含 2 万字 Markdown 状态） | 61.5 – 64.5 MiB |
| 主窗口首帧 | 65.0 MiB |
| 窗口创建后 | 74.3 – 77.8 MiB |
| 18 页缩略图 + 大图就绪 | **341.5 – 368.8 MiB** |
| 再开一个演示窗口（18 页 + 全屏位图） | 579.1 MiB |
| 位图基准（10 轮 × 19 张）之后 | 418.6 MiB |
| 连续编辑标题 120 s（无 `drop_image`） | 893 → 1481 → 2050 → **2649 MiB**（~19 MiB/s，线性） |
| 连续编辑标题 120 s（每次替换都 `cx.drop_image`） | 465 → 664 → 897 → **1058 MiB**（~6 MiB/s，仍线性） |
| 连续编辑标题 **300 s**（带 `drop_image`，后台跑的那一次） | 353 → 498 → 651 → 842 → 1090 → 1291 → 1454 → 1632 → 1789 → 1952 → **2103 MiB**（每 30 秒约 +150–250 MiB，**5 分钟里没有任何平台化迹象**） |
| **审阅重测**：连续编辑标题 120 s（缩略图也 `drop_image`） | 300.3 → 299.7 → 299.2 → **300.5 MiB**（平稳） |
| **审阅重测**：连续编辑标题 300 s（同上） | 305.7 → 302.3 → 292.1 → 292.6 → 291.7 → 287.1 → 296.0 → 282.4 → 285.1 → **287.4 MiB**（平稳） |
| 只画不发请求（GPUI 独占：2400 张 1664×936，每张都释放） | 332 → 312 → 331 → **362 MiB**（平稳） |
| Typst 独占（`raster_loop`，240 次编译 + 光栅化） | 215 → 304 → 326 → 315 → 324 → 309 → 313 → 314 → **315 MiB**（平稳） |

「341–369 MiB 就绪」比 Tauri 版高得多，原因是 Typst 的记忆化缓存（`comemo::evict(50)`，
与 Tauri 版同一套代码）+ 字体 + GPUI 图集。上表前三行「连续编辑」的增长来自 spike 漏释放缩略图
（见第 1 节审阅更正），不是 GPUI 或 Typst 的问题。

`raster_loop` 还顺带给出：单页 @1664 px 的「改一页 → 整副编译 → 光栅化」约 **53 ms/次**
（240 次 12.6 s）。

## 4. GPUI Kit 本身的风险

**版本节奏（本机能核对的）**：本机 crates.io index 缓存里 `gpui-kit` 共 11 个版本
（0.1.0、0.6.0、0.6.1、0.6.2、0.6.4、0.6.5、0.6.6、0.7.0、0.7.1、0.7.2、0.7.3），
**`0.6.5` 是 yanked**，`0.6.0 → 0.7.3` 一共 10 个版本。发布日期在 index 缓存里没有，无法离线核对
「一个多月」这个说法。

**它固定的 GPUI**：`Cargo.lock` 里是 `gpui-pre / gpui-base / gpui-component = 0.7.3`，引擎是
`gpui-pre 0.3.9`，crate 描述自称是 **zed@93d7456 的快照**——也就是说它绑的是 Zed 的一个切片，
不是官方发布的 GPUI。0.7.3 还带 `gpui-fast` 这个可选后端（换一套 `gpui_fast_*`）。

**API 稳定性**：`gpui-kit` 0.7.3 的 tests 目录里有一批 `legacy_*_compat.rs`（按钮、控件、语言配置、
主题），说明作者在为改名/改签名保留兼容层；`src/lib.rs:83-103` 有一段「公开 facade 决策」注释，
讨论 `gpui_kit::*` 与 `gpui::` 路径的取舍。我这次踩到的具体差异见下。

**文档**：crate 只带 README（指向 gpui-kit.com），没有 examples 目录。本机 `web_fetch` 拿不到
crates.io / docs.rs（DNS 策略），所以我**全部靠读 vendored 源码 + crate 自带的集成测试**摸 API
（`tests/input/*.rs`、`tests/controls.rs`、`tests/window.rs`、`tests/components.rs` 等是能编译的
真实用例，帮助很大）。换句话说：文档不完整，但源码和测试够用。

**这次遇到的 bug / 坑（都能复现）**：

1. **debug 构建在这台机器上编译不过（阻塞 15 分钟）**：`stacker`、`tree-sitter`、`tree-sitter-md`
   的 C 构建脚本报 `cl : Command line error D8050 : 无法将命令行放入调试记录中`。原因是 cc-rs 在
   带调试信息的 profile 下给 MSVC 传 `-Z7`。**与命令行长度无关**：我用一个三行 crate
   （只有 `stacker = "=0.1.25"`）复现——同一个 `-Z7` 调用在 `E:\huanshu\target\...` 下能过，
   在 `E:\huanshu\spikes\...`、`E:\huanshu\acltest\...` 下失败（`E:\huanshu\target` 对当前用户
   有显式 full-control ACE，新目录只有继承来的沙箱 ACE；我试着给 `spikes` 显式 grant ACE 也没用，
   所以只能说「环境相关的路径/ACL 差异」，没有定位到 MSVC 内部原因）。绕过：spike 的 `Cargo.toml`
   里 `[profile.dev] debug = false`（release 本来就不传 `-Z7`）。
2. **`RenderImage` 不会自动回收 GPU 图集**：GPUI 只在图集缓存被清理时调 `drop_image`
   （`gpui-pre/src/elements/image_cache.rs:240/269/280`），而 `ImageSource::Render` 走的是
   「直接返回这个 Arc」的路径（`elements/img.rs:551`）。所以**每张新的 `RenderImage` 都会在图集里
   留下一个 tile**，除非宿主自己 `cx.drop_image(old, Some(window))`。这条算不上 GPUI 的 bug
   （Zed 里图片是少量长期存在的），但对「每次编辑都换一张大图」的应用是必须知道的。
3. **Windows 上 `start_window_move()` 是空实现**，自绘标题栏拖动只能靠 `.window_control_area(Drag)`
   → `HTCAPTION`；`TitleBar::on_close_window` 在 Windows 不生效；双击最大化也只有 Linux/macOS
   在 `TitleBar` 里接了（Windows 靠原生）。
4. **`RenderImage::scale_factor` 是 `pub(crate)`**：从外部建出来的位图按 1 逻辑 px = 1 像素算，
   150% 屏上要自己按 `window.scale_factor()` 乘宽度光栅化（我这么做了，所以清晰）。
5. **列表/表格没有行级拖拽排序**（见结果表 D 那一行）。
6. **没有 Markdown 富文本渲染组件**（见结论 4）。

## 5. 人工验收清单（我做不了，请按顺序操作）

> 启动：`cd spikes/gpui-kit; cargo run --release`（或直接跑 `target/release/gpui-kit-spike.exe`）

**一、中文输入法（最关键）**

1. 看板页 → 点底部「选中页标题」输入框 → 切到微软拼音：打 `huan`，应看到**带下划线的未上屏文本**
   （不是先落进框里再变），候选框应贴在**光标右下**、不遮住光标。
2. 滚一下输入框所在的窗口/把窗口拖到副屏 → 再打拼音：候选框应**跟着光标**（副屏那台若缩放不同，
   候选框不应偏移到别处）。
3. 回车/空格上屏后，下划线消失、"渲染中"→ 新的大图；再按 `Shift` 切英文打 `abc`，标点试 `,` 与 `，`
   （应分别为半角/全角）。
4. 切到「Markdown 编辑器」页，把光标点到 1 万字文档中间（例如第 200 行），连续输入一整段中文
   （30–50 字）：不应有明显卡顿，不应丢字/重复，滚动条不应乱跳。
5. 同样在编辑器里用输入法打「日本」：应显示为「日本」而不是乱码或 `ri ben`。
6. 状态栏「第 N 行，第 M 列」在移动光标（方向键、Home/End、Ctrl+←→）时应跟着变；用输入法组字时
   光标位置不应突然跳到行首。

**二、无边框窗口与系统行为**

7. 拖标题栏空白处移动窗口；拖到屏幕左/右/上边缘看**贴靠**（Snap）是否出现；
8. 双击标题栏空白处应最大化/还原；点右上角「–」「□」「×」应分别为最小化/最大化/关闭；
9. 拉伸窗口四边/角，看缩放时是否**撕裂**、缩略图是否按 64 px 档位重渲（状态栏的「缩略图 xxx px」
   会跳档）、大图是否随之变化。

**三、看板与演示**

10. 点左侧缩略图切换页面（大图和标题框应一起换）；点「打开牌组…」选
    `crates/slide-typst/tests/fixtures/charts.json`（5 张图表页）看是否正常；
11. 顶部下拉框逐个试 6 个 Typst 主题（`light-notes` / `light-terminal` / `light-editorial` /
    `dark-terminal` / `dark-yellow` / `dark-blue`），切换时**不应闪成空白**（旧图应留到新图出来）；
12. 点「演示模式」：应开一个全屏窗口；`→`/空格/左键翻页，`←`/右键回退，`Home`/`End` 到底，
    `Esc` 关闭；翻页时**不应出现空白帧**（当前页无图时应先出当前页）；在 16:9 屏上放 32:9 牌组
    应明显被横向压扁（拉伸模式）。

**四、其他外壳能力**

13. 「外壳组件」页：滑块拖动时数字跟着变；数字输入框用 `+`/`-` 与键盘上下键步进；「确认删除…」弹出
    确认框（Esc / 点遮罩应可取消）；「密码（带强度）…」里打字时**强度条应实时变化**（我是靠
    `cx.notify` 触发的，如果不动请记下来）；四种轻提示各点一次（默认 5 秒消失）；
14. 手写拖拽列表：按住「0 封面 · cover」往下拖到第 3 行松开，顺序应改变，底部提示「拖动第 1 行到
    第 2 行（第 N 次）」；
15. 「打开文件…/保存文件…」应弹出系统对话框（保存对话框选个路径，底部提示会显示路径）。

## 6. IME 支持的源码依据

- 接口：`gpui-pre-0.3.9/src/input.rs:13` `pub trait EntityInputHandler`（`text_for_range` /
  `selected_text_range` / `marked_text_range` / `unmark_text` / `replace_text_in_range` /
  `replace_and_mark_text_in_range` / `bounds_for_range` / `character_index_for_point`）。
- **三种输入控件共用同一个实现**：`gpui-base-0.7.3/src/input/base/state.rs:4094`
  `impl<M: InputModeKind> EntityInputHandler for InputBaseState<M>`；`InputState`、`TextareaState`、
  `EditorState` 都是它的别名（`input/input/mod.rs:10`、`input/textarea/mod.rs:10`、
  `input/editor/mod.rs:11`）。paint 时通过
  `window.handle_input(&focus_handle, ElementInputHandler::new(bounds, state.clone()), cx)` 装上
  （`gpui-base/src/input/base/element.rs:3028-3032`）。
- Windows 平台：`gpui-pre-windows-0.3.9/src/events.rs:154-155` 处理
  `WM_IME_STARTCOMPOSITION` → `handle_ime_position`、`WM_IME_COMPOSITION` → `handle_ime_composition`；
  `retrieve_caret_position()`（`:634-645`）用 `selected_text_range` + `bounds_for_range` 拿光标矩形，
  并用窗口的 `scale_factor` 从逻辑坐标换成物理坐标；`update_ime_position()`（`:654-681`）调
  `ImmSetCompositionWindow`（`:659`）与 `ImmSetCandidateWindow`（`:670`）——**这正是候选框跟光标走的地方**。
  `update_ime_enabled()`（`:683`）按焦点控件是否接受文本开关输入法。
- 协议级测试：`gpui-kit-0.7.3/tests/input/composition.rs:33-77`（UTF-16 下标下的预编辑文本替换、
  提交、取消、`unmark`、选区方向）与 `tests/input/lifecycle.rs:30-38`（三种控件都装上了
  `ElementInputHandler`）。**这些测试自己声明不驱动真实 OS 输入法、候选框和平台事件翻译**
  （文件头注释），crate 的 `tests/input/README.md` 也是这个口径。
- 运行时旁证：spike 跑起来后，进程里出现了 `MSCTFIME UI` 和 `IME` 两个窗口类（Text Framework /
  输入法已挂到该窗口上），说明平台侧确实建立了 IME 上下文。

**但我没有、也无法验证**：真实微软拼音/搜狗下的组字显示、候选框位置（多屏、150% 缩放、编辑器
滚动之后）、回车与 Shift 行为、全半角标点、以及万字文档中间连续输入的流畅度。这些都在第 5 节
的人工清单里。

## 7. 与本说明不一致的实现及原因

1. **`render_pixels` 的返回类型**：说明里写 `Result<RenderedPixels, RenderError>`；实现是
   `Result<RenderedFrames, RenderError>`，其中 `RenderedFrames { pages: Vec<RenderedPixels>, overflows,
   missing_assets }`——与 `render_png` / `RenderedPages` 对称（一次调用可以覆盖多页），
   `RenderedPixels` 仍是单页结构（`index/width/height/pixels`）。
2. **多做了三件说明之外的事**，都是为了把「内存不收敛」这件事查清楚：
   `--paint-soak`（只造图不请求 Typst 的 2 分钟循环）、`src/bin/raster_loop.rs`（不经过 GPUI 的
   Typst 重复编译，带工作集打印）、以及 `pump/finish` 改成带 `Window` 以便调用 `cx.drop_image`。
3. **`[profile.dev] debug = false`**：见第 4 节第 1 条，本机 debug 构建的环境绕过，写进了
   `spikes/gpui-kit/Cargo.toml` 的注释里；release 不受影响。
4. **牌组比例固定 `ratio_16x9`**：`fixtures/all-layout-blueprints.json` 没有 `aspect_ratio` 字段，
   spike 也不做比例选择器；大图外框按**实际位图**的宽高比算（`board.rs:stage_box`），
   所以它不会因为 32:9 牌组而显示错，但 32:9/48:9 我**没有实测**。
5. **给 spike 加了命令行开关**（`--page/--theme/--typst-theme/--deck/--present/--dialog/--bench/
   --bench-edit/--soak/--soak-seconds/--quit-after`）：说明里没要求，但没有它们就无法脚本化地
   复现截图和数字。
6. **B2 的「PNG 解码」是我自己用 `image` crate 解的**（`deck.rs:rasterize`），不是交给 GPUI 的
   资产管线解；这样两种方式都落在同一张 BGRA 上，差值才只反映「编码/解码」。GPUI 自己的
   解码路径（`ImageSource::Image` → 内部 `into_rgba8()` 再 `swap(0,2)`，`platform.rs:3249-3254`）
   做的是同一件事。
7. **B3 用程序化改动而不是合成按键**（见 3.3 的说明）。

## 8. 没做完 / 不确定的点

- ~~内存增长没有定位到根因~~：审阅时已定位并修复（spike 漏释放缩略图，见第 1 节）。
- **窗口缩放**：`observe_window_bounds` → 重算 64 px 档位 → 重渲这条链路**验证过**：
  把窗口从 1440×900 拉到 1200×767（逻辑），状态栏的「大图」从 **@1664px** 变成 **@1280px**，
  画面同时更新（`resize-before-0.png` / `resize-after-0.png`）。**撕裂没有验证**（静态截图看不出来）。
- **窄窗口的排版**：1200 逻辑 px 宽时工具栏一行放不下，右侧「Typst 主题 … / 缩略图 … px」会被切掉
  （`resize-after-0.png`）。功能不受影响，但说明工具栏需要自己做瘦身或换行。
- **拖拽排序的真实交互**没有脚本化验证（只有渲染与编译）。
- **六个主题 × 所有页面**没有全走（看板试了 `light-notes` / `dark-blue` + 深浅两套外壳，
  编辑器与组件页只在深色外壳下看过）。
- **没有 release 之外的真机验证**，也没有在第二块屏 / 别的缩放比例下跑过。
- **Step1 的 Markdown 预览与 CodeMirror 特性**（图表围栏编辑、贴图、lint）在 spike 里完全没有对应物，
  这一块的成本我**没有**估进去（见第 9 节最后一行）。

## 9. 如果继续走 GPUI：按页面的迁移工作量（粗估）

本 spike 一共写了 **2540 行 Rust**（含 3 个页面、演示窗口、渲染服务、3 个测量脚本/二进制），
覆盖了下面「已基本打通」的部分。行数是实测，人日是按「一个熟悉 Rust、不熟悉 GPUI 的工程师」粗估，
**不包含** IME 真机验收不通过时的攻关时间。

| 界面 | spike 里已经打通的 | 还缺什么 | 粗估 |
|---|---|---|---|
| 首页（项目列表 + 导入/导出 + 加密） | 列表（虚拟列表）、按钮、确认框、密码框 + 强度条、toast、rfd | 卡片/缩略图墙、导入导出的进度、二维码之类没有 | 3–5 人日 |
| 设置（模型/并发/外观与字体/主题） | 下拉、开关、滑块、数字输入、可选字体列表（`Select` 支持搜索） | 字体预览、目录选择、保存与校验逻辑 | 4–6 人日 |
| 首次向导 | `Stepper`/`TabBar`/`Dialog` 都有 | 向导自身的状态机 | 2–3 人日 |
| Step1（Markdown + 图表校验 + 比例） | 多行编辑 + tree-sitter 高亮 + 字数/行列 | **Markdown 预览没有原生方案**；图表围栏编辑、图片拖入、lint 都要另做 | 8–15 人日（其中预览是大头） |
| Step2（看板 + 详情 + 生成日志） | 看板 + 大图 + 主题切换 + 位图缓存调度；日志本身是文本 | 生成进度流、日志虚拟滚动、错误跳转 | 5–8 人日 |
| Step3（编辑器：字段表单 + 预览 + 高级 JSON） | 实时预览（含 serial 合并）、输入框、按钮 | `KIND_FIELDS` 驱动的表单生成器（字段很多）、JSON 编辑器（`Editor` + tree-sitter json）、图表数据校验 | 10–15 人日 |
| 演示模式 | 全屏无边框 + 键盘/鼠标翻页 + 非等比拉伸 + 预取 | 选显示器、激光笔/黑屏、缩略图导航 | 2–3 人日 |
| 对话框族 | 确认/密码/toast/popover/tooltip 都在 | 批量导出、设置保存的确认链 | 2–3 人日 |
| IME | — | 第 1 节的真机验收 | **先验收，不估** |

**建议先搬进 Rust 的逻辑（按性价比排序）**：

1. `useSlideEditor.ts` 的 `KIND_META` / `KIND_FIELDS` / `KIND_DEFAULTS` 与 Step3 的 `FIELD_DEFS`：
   它们本来就是 `src-tauri/src/types.rs` 的镜像，搬过去以后 Rust 侧就能直接生成表单描述，
   前端只剩渲染。
2. `chartSpec.ts` 的 `chartProblems()`：Rust 里已经有等价的 `ChartSpec::problems()`，现在两份规则
   要保持同步（CLAUDE.md 明确要求），搬走一份就少一处漂移。
3. `useTypstThumbnails` 的调度（serial / progressive / 64 px 档位 / generation 作废）：这次 spike 在
   Rust 里又实现了一遍（`board.rs:pump/finish`），证明它是**纯逻辑**，可以整体下沉成
   `slide-typst` 旁边的一个 service。
4. 主题/字体 token：屏幕侧如果换原生界面，即使用 GPUI 的 `Theme`，也应该把「主题 → token」表放进 Rust 一份
   （Typst 主题自带幻灯片与图表配色，Studio 外壳的配色现在只在 CSS 里）。
   （审阅注：`titleTail.ts`、`chartOptions.ts`、`richText.ts` 已在第五步删除，不在待迁移之列。）

## 10. 复现命令

```powershell
# 构建 / 运行
cd spikes/gpui-kit
cargo build --release
cargo run --release                      # 默认深色，看板页

# 截图（先 SetProcessDPIAware，否则 150% 下是糊图）
powershell -NoProfile -ExecutionPolicy Bypass -File tools/screenshot.ps1 -Prefix board-dark
# 各页面/主题
cargo run --release -- --page editor --quit-after 40
cargo run --release -- --page controls --dialog --quit-after 40
cargo run --release -- --present --typst-theme dark-blue --quit-after 60

# 性能数字（stderr 里的 [time] / [mem] / [coldstart] 行）
cargo run --release -- --bench --quit-after 90          # B2
cargo run --release -- --bench-edit --quit-after 60     # B3
cargo run --release -- --soak --soak-seconds 300 --quit-after 330   # 内存
cargo run --release -- --paint-soak --quit-after 140    # 只画不发请求
cargo run --release --bin raster_loop -- 240 1664       # 只测 Typst

# 冷启动对比
powershell -NoProfile -ExecutionPolicy Bypass -File tools/coldstart.ps1 -Runs 5
powershell -NoProfile -ExecutionPolicy Bypass -File tools/coldstart.ps1 -Exe ..\..\..\target\release\auto-slidev-studio.exe -Runs 5

# 生成的 sample.md（20146 字符）
python tools/make_sample.py
```

## 11. 截图清单

| 文件 | 内容 |
|---|---|
| `board-dark-0.png` | 看板 + 大图，深色外壳，`light-notes` 主题，18 页缩略图，状态栏给出两个位图宽度 |
| `board-light-0.png` | 同一画面，浅色外壳（A3 运行时切换） |
| `board-darkblue-0.png` | Typst 主题切到 `dark-blue`（B1 主题下拉） |
| `editor-0.png` | Markdown 编辑器（tree-sitter 高亮 + 状态栏字数/行列） |
| `controls-0.png` | 外壳组件页：按钮/开关/勾选/单选/滑块/数字输入/对话框入口/文件对话框/Popover/拖拽列表 |
| `controls-dialog-0.png` | 密码对话框（掩码输入 + 强度条 + 自组合按钮） |
| `presentation-0.png` | 演示模式全屏窗口（2560×1600 上的 16:9 页面，被非等比拉伸） |
| `presentation-1.png` | 演示窗口背后的主窗口 |
| `resize-before-0.png` / `resize-after-0.png` | 同一个窗口在 1440×900 → 1200×767（逻辑）前后的对比；状态栏「大图」从 `@1664px` 变 `@1280px`，即 64 px 档位重算 + 重渲生效；同时能看到窄窗口下工具栏被切 |
