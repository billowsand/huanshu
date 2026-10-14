# 任务：用 GPUI Kit 做原生界面技术验证（第六步，spike）

你在仓库 `E:\huanshu`（幻述 / Auto-Slidev Studio，Tauri v2 + Vue 3 + Rust）里工作。
开始前先读 `CLAUDE.md` 的「Typst 原生 PDF 导出」整节（尤其「Typst 缩略图」「编辑器实时预览与 Step2 详情」），
以及 `docs/tasks/` 下前五步的说明，了解幻灯片现在怎么由 Typst 出图。

## 背景与目标

幻灯片渲染已经全部在 Rust 里（`crates/slide-typst`，Typst 编译 + 光栅化），WebView 现在只负责界面外壳。
长期目标是把外壳换成原生 Rust 界面、去掉 WebView。候选框架是 **GPUI Kit**（https://gpui-kit.com/ ，crate `gpui-kit`，
Longbridge 出品，Apache-2.0，把 Zed 的 GPUI、`gpui-base`、`gpui-component` 和默认资源打包成一个依赖）。

**这一步不迁移任何功能。** 只做一个独立的小程序，回答一个问题：**GPUI Kit 在本机（Windows 11、MSVC、
中文输入法、150% 缩放）上能不能撑起幻述的界面？** 最大的未知数是中文输入法（IME）和多行编辑器，其次是
Typst 位图的显示性能和无边框窗口。

结论可以是「不行」。如实报告比硬凑出一个能跑的演示更重要。

## 前几轮审查发现的问题（不要再犯）

1. **报告与事实不符**：没做的写「未做」，没验证的写「未验证」；「看起来正常」不等于验证过。
2. **性能结论必须是 release 构建测出来的**，并注明测法；debug 下的耗时只能说明 debug。
3. **注释、提交信息与代码不一致**：写下的每条行为都要和代码相符。
4. 你**无法替用户操作输入法**：凡是需要人手输入中文、拖窗口、看候选框位置的项，写进「人工验收清单」交给用户，
   不要自己宣称通过。

## 范围

- 新建独立 cargo 工程 `spikes/gpui-kit/`（**自带 `[workspace]` 表**，不加入根工作区：不能让 `cargo test --workspace`
  去编译 GPUI）。它通过 path 依赖 `../../crates/slide-typst`。构建产物在 `spikes/gpui-kit/target/`（已被 `.gitignore` 的 `target` 规则忽略）。
- 依赖固定精确版本：`gpui-kit = "=0.7.3"`（它自己固定了配套的 GPUI 版本，**不要**另外直接依赖 `gpui` 或 git 上的 Zed）。
  如果 0.7.3 在本机编不过，退到 `=0.7.1` 并在报告里写明原因。文件对话框用 `rfd`。
- 允许改 `crates/slide-typst`：只能**新增**一个公开方法（见 B2），不改现有行为，要带单测。
- **不许改**：`src-tauri/`、`src/`（Vue 前端）、Typst 模板、根 `Cargo.toml`、`CLAUDE.md` / `README` / `CHANGELOG`。
- 本机工具链：`rustc 1.97.1`。若 GPUI Kit 要求更高版本，停下来在报告里写明，不要自行升级工具链。

## A. 骨架

1. 按 GPUI Kit README 的方式起一个应用：`gpui_kit::application().with_assets(gpui_kit::assets::Assets)`、
   `gpui_kit::init(cx)`、`gpui_kit::open_window(...)`。先写个最小窗口确认能编译运行，记录**首次**完整编译耗时
   （release 与 debug 各一次）和 release 可执行文件大小。
2. **无边框窗口**（幻述现在是 `decorations: false` 的自绘标题栏）：自绘标题栏可拖动，最小化/最大化/关闭按钮可用，
   双击标题栏最大化、拖到屏幕边缘的 Windows 贴靠能用（贴靠交给人工验收）。
3. 主题：用 GPUI Kit 的主题机制做一套深色、一套浅色，能在运行时切换。不要求和现有 Studio 配色一致。

## B. 显示 Typst 幻灯片

### B1. 看板与大图

窗口左侧是缩略图列表（用 GPUI Kit 的虚拟列表），右侧是选中页的大图。牌组从 JSON 文件加载，默认
`fixtures/all-layout-blueprints.json`，`rfd` 的打开对话框可以换成别的文件（例如 `crates/slide-typst/tests/fixtures/charts.json`）。
主题下拉框切换 Typst 主题（`slide_typst::SUPPORTED_THEMES`）。

- 渲染一律在后台线程（GPUI 的后台执行器）做，**绝不能阻塞 UI 线程**；`slide_typst::Renderer::with_system_fonts()`
  进程内只建一次（参考 `src-tauri/src/commands/pdf.rs` 的 `renderer()`）。
- 缩略图宽度 = 显示宽度 × 缩放比例（150% 屏幕要乘 1.5），大图同理。宽度变化时按 64px 档位取整再重渲，
  不要每像素都重渲（与 `useTypstThumbnails` 的 `WIDTH_BUCKET` 同理）。

### B2. 位图怎么交给 GPUI

现在的应用是「Typst 光栅化 → PNG 编码 → base64 → WebView 解码」。原生界面应该直接把像素交给 GPUI：

1. 先用现有的 `Renderer::render_png`，在界面里解码 PNG 显示（最快跑通）。
2. 再在 `slide-typst` 里**新增** `Renderer::render_pixels(slides, options, width_px, only) -> Result<RenderedPixels, RenderError>`：
   与 `render_png` 同一次编译、同样的宽度规则，但返回未编码的 RGBA8（tiny-skia 的 `Pixmap` 是预乘 alpha，要去预乘；
   幻灯片页面不透明时两者相同，但不要依赖这一点），并加单测（像素尺寸、`only`、与 `render_png` 解码后逐像素相同）。
   界面改用它，按 GPUI 要求的像素格式（查 GPUI Kit 重导出的 GPUI 里 `RenderImage` / `ImageSource` 的要求，
   很可能是 BGRA）转换后显示。
3. 用 release 构建测两种方式：整副 18 页缩略图从「请求」到「全部显示」的耗时、单页大图的耗时，各测 5 次取中位数。

### B3. 实时编辑预览

右侧大图下方放一个**单行**输入框编辑选中页的 `title`，输入时大图实时更新：
- 规则与 Step3 编辑器相同：同一时刻只有一个渲染在途，期间的变化合并成尾部的一次渲染（`useTypstThumbnails` 的
  `serial`）；只编译当前这一页（`slides = [当前页]`）。
- 测：release 下连续快速输入时，最后一次按键到画面落在最新内容的延迟（取 10 次中位数）。

## C. 文本编辑（最关键）

### C1. 多行 Markdown 编辑器

单独一个页面（标签页或侧边切换都行）：用 GPUI Kit 的多行输入 / 代码编辑器组件编辑 Markdown，替代现在 Step1 的
CodeMirror（`src/components/MarkdownWorkbench.vue`）。要求：
- 打开一个约 1 万字的中文 Markdown 文件（自己生成一份放在 `spikes/gpui-kit/assets/sample.md`，含标题、列表、代码块、
  中英混排、长段落）；
- 软换行、滚动、选择、复制/剪切/粘贴、撤销/重做、全选、Home/End、Ctrl+←/→ 按词移动；
- 如果组件支持 Markdown 语法高亮（Tree-sitter），打开它；
- 状态栏显示字数、光标所在行列。

### C2. 中文输入法

单行输入框（B3）和多行编辑器（C1）都要支持 Windows 的中文输入法（微软拼音、搜狗等）。查清楚 GPUI Kit 的输入组件
是否实现了 GPUI 的 IME 接口（`EntityInputHandler` / `InputHandler` 一类），在报告里写明依据（源码位置）。
实际效果你无法验证，写进人工验收清单：
- 拼音组字时有下划线的未上屏文本；
- 候选框跟着光标走（包括编辑器滚动后、窗口挪到第二块屏幕后、150% 缩放下）；
- 回车上屏英文、Shift 切中英、标点全半角；
- 在 1 万字文档中间连续输入一整段，没有明显卡顿。

## D. 其他外壳能力

逐项做一个最小可用的例子，并记录用的组件名：
- 下拉选择、按钮、开关、滑块、数字输入（Step1 的生成设置会用到）；
- 模态对话框（确认删除、密码输入框带强度提示——参考 `src/components/PasswordDialog.vue`）；
- 轻提示 / 通知（toast）；
- 文件打开 / 保存对话框（`rfd`）；
- 可拖拽排序的列表（编辑器里的幻灯片顺序，做不到就写「组件不支持」）；
- 演示模式：一个无边框全屏窗口，左右方向键 / 空格 / 鼠标点击翻页，Esc 退出，位图按屏幕尺寸非等比铺满
  （与 `PresentationOverlay.vue` 的拉伸模式一致），下一页预取。

## E. 资源与稳定性

release 构建下测量并记录：
- 冷启动到窗口出现的时间（5 次中位数）；
- 空闲内存、载入 18 页牌组并显示全部缩略图后的内存、连续编辑标题 2 分钟后的内存（看 Typst comemo 缓存是否失控，
  `slide-typst` 已在每次编译后 `evict(50)`）；
- 对比：同一台机器上现在的幻述（`target/release/auto-slidev-studio.exe`，如果没有就写「未对比」）的冷启动和空闲内存；
- 运行期间的 panic、渲染错误、闪烁、窗口缩放时的撕裂。

## 交付物

1. `spikes/gpui-kit/`：可运行的验证程序，`cargo run --release` 即可启动；README 写清楚每个页面验证什么、怎么操作。
2. `slide-typst` 的 `render_pixels`（B2）与单测。
3. `docs/spikes/gpui-kit.md`：验证报告，结构如下：
   - **结论**（先写）：能 / 不能 / 有条件能；如果「有条件」，条件是什么；
   - 逐项结果表：A1–E 每一项「通过 / 不通过 / 部分 / 未验证 / 需人工」，附依据（数字、源码位置、截图路径）；
   - B2 两种位图交接方式的耗时对比，B3 的输入延迟；
   - GPUI Kit 本身的风险：版本发布节奏（0.6.x→0.7.3 一个多月发了十个版本，0.6.5 被撤回）、它固定的 GPUI 版本、
     API 是否稳定、文档完整度、遇到的 bug（附复现方法）；
   - **人工验收清单**：给用户逐条操作的步骤与预期结果（输入法、贴靠、多屏、缩放等）；
   - 如果继续走 GPUI：按页面估计迁移工作量（首页、设置、首次向导、Step1/2/3、演示、各对话框），以及哪些逻辑应先从
     TypeScript 搬进 Rust（例如 `useSlideEditor.ts` 的 `KIND_META` / `KIND_FIELDS` / `KIND_DEFAULTS`、`chartProblems`）。
4. 截图存到 `docs/spikes/gpui-kit/`（PowerShell 截窗口即可，至少：看板 + 大图、编辑器、演示、深浅两套主题）。

## 分支与提交

从 `feat/typst-only` 新建分支 `spike/gpui-kit`，提交：

1. `feat(slide-typst): render_pixels for native hosts`（B2 的新方法与单测）
2. `spike: GPUI Kit shell prototype`（`spikes/gpui-kit/`）
3. `docs: GPUI Kit spike report`（报告与截图）

## 验证

```bash
cargo test -p slide-typst
cargo test --workspace
cd spikes/gpui-kit && cargo build --release
```

前两条必须仍然全过（根工作区不应因为 spike 有任何变化，除了 `slide-typst` 的新方法）。

## 最终汇报（按此格式，不要省略）

1. 三个提交的 hash 与一句话说明
2. 每条验证命令的结果（贴输出末尾几行）
3. 结论（能 / 不能 / 有条件能）与最关键的三条依据
4. 逐项结果表（同报告）
5. 性能数字：编译耗时、可执行文件大小、冷启动、内存、B2 对比、B3 延迟（都注明 release / debug 与测法）
6. IME 支持的源码依据，以及交给用户的人工验收清单
7. 遇到的 GPUI Kit 问题与绕过办法
8. 与本说明不一致的实现及原因；没做完或不确定的点
