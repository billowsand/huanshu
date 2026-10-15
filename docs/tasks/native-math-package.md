# 任务：原生界面第八轮——预览里的数学公式、原生版打包（第十六步）

你在仓库 `E:\huanshu`（幻述）里工作。原生界面的功能性差距只剩审计里的一条大项：**第一步 Markdown 预览不渲染数学公式**（原版用 KaTeX）。
另外原生版还没有打包流程。本轮两件事：

- **A. 预览里的公式**：用 GPUI Kit `TextView` 的 Markdown 插件，把 `$…$` / `$$…$$` 渲染成图，排版走我们自己的 Typst 公式管线；
- **B. 打包**：原生版的发布构建、程序图标、不弹控制台窗口、安装包 / 免安装版、打包脚本与文档。

开始前先读：`apps/huanshu-native/README.md`；`CLAUDE.md` 开头三节、「Typst 原生 PDF 导出」里**数学公式**那一段、「打包与发布（Tauri）」整节；
`crates/slide-typst/src/math.rs` 与 `lib.rs` 里公式相关的部分（`failing_math`、哨兵段）；`apps/huanshu-native/src/pages/workflow/step1.rs` 的预览；
原版 `src/components/MarkdownWorkbench.vue`（`markdown-it-katex` 的用法）；`scripts/package.ps1`。

## 原则（`CLAUDE.md`「原生界面」）

关键功能不能丢，但**贴合 GPUI、用好它的能力，不逐像素复制**。公式长得像 Typst 而不像 KaTeX 没关系——它和幻灯片、PDF 里的公式是同一套排版，反而一致。

## 前几轮的教训（不要再犯）

1. **真机验证只在沙箱**：只用 `tools/drive.ps1` / `tools/shots.ps1`，动手前备份沙箱（→ `target/sandbox-home.bak13`）。**任何一版应用、包括装好的安装版，
   都不要不带 `HUANSHU_HOME` 启动**——安装版默认就指向用户的真实数据目录。
2. **界面延时用 GPUI 计时器**；「进行中」标志每条出口都复位；回调可能在调用方的 `update` 里同步执行时用 `cx.defer`。
3. **模型 / 用户写的字符串绝不拼成 Typst 标记**（`CLAUDE.md` 有注入单测的说明）；一个坏公式不能让别的东西失败。
4. **汇报要诚实**：没真机验证的写「未验证」。

## A. 第一步预览里的公式

- **slide-typst 加一个公式出图的接口**，例如 `Renderer::render_formula(latex, display, color, size)`，返回 SVG 或位图（选 GPUI 显示起来更合适的那种）。
  复用 `math.rs` 的 mitex 转换与哨兵规则，不要另写一套 LaTeX 处理；坏公式返回错误、不 panic、不影响其他公式；同一个进程里复用已经建好的 `Renderer`
  （字体扫描只做一次）。单测：正常行内 / 块级公式出图、坏公式报错、注入字符串（含 `\u{1}` / `\u{2}` 与 Typst 标记）不会被执行。
- **原生预览接上插件**：`TextView` 的 `MarkdownExtensions::plugin`（见 `gpui-base` 的 `text/markdown_ext.rs`）认领行内公式与块级公式节点
  （现在没人认领时，行内公式原样显示、块级公式被当成代码块：`text/format/markdown.rs` 的 `InlineMath` / `Math` 分支）。
  插件的 `parse` 在后台、`render` 每帧都会调：**出图放在后台做并缓存**（键 = 公式文本 + 行内/块级 + 颜色 + 字号），`render` 只读缓存；
  还没出图时先显示公式原文，出图后让预览刷新一次；坏公式显示原文并用错误色（原版 KaTeX 默认 `throwOnError: false`，出错时显示红色原文）。
  公式颜色跟随当前主题的正文色，主题切换后要重新出图（缓存键里有颜色就行）。缓存要有上限，不要无限增长。
- **分页规则不变**：`$$` 块内不切页已经在 core 的 `split_preview_pages` 里（有快照单测）。
- **注意美元符号**：普通文本里的 `$5 和 $10` 会被 Markdown 解析器当成行内公式（`markdown.rs` 里有注释）。对照原版 `markdown-it-katex` 的定界规则
  （例如 `$` 后紧跟空白、或结束的 `$` 前是空白、后面紧跟数字时不算公式），把规则写进 core 并带单测，保证价格之类的文字不被吞掉；
  这条规则要与 `CLAUDE.md` 说的「三处定界规则必须一致」对齐（屏幕 `richText.ts`、PDF `math.rs`、生成侧 `generator/utils.rs`），不一致的地方在汇报里列出来。
- 验证：沙箱里新建一个项目，文稿里放行内公式、块级公式、坏公式、带价格的普通句子、`$$` 跨行块，截图；切换一次主题再截图；连续编辑时预览不卡
  （量一下打字时的帧耗时）。完成后审计里那条 ⚠️ 改成 ✅，parity「Markdown 预览」行同步。

## B. 原生版打包

- **不弹控制台窗口**：发布构建加 `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]`。确认 `--export` 这类命令行模式与 `drive.ps1`
  重定向 stderr 读 `[coldstart]` 的用法仍然能拿到输出（拿不到就说明怎么改，不要静默失效）。
- **程序图标与版本信息**：exe 嵌入 `src-tauri/icons/icon.ico` 与版本号（`build.rs` 里用 `winresource` / `embed-resource` 之类的小依赖；
  `build.rs` 现有的 8 MiB 主栈设置保留）。窗口与任务栏图标也要对。版本号与 `src-tauri/tauri.conf.json` 保持一致（现在都是 0.4.0）。
- **打包脚本** `scripts/package-native.ps1`（ASCII-only，原因见 `CLAUDE.md` 打包一节）：`cargo build --release -p huanshu-native` →
  产物汇总到 `dist-package/`：免安装单文件 `huanshu-native_windows_x64.exe`，以及 NSIS 安装包（本机 Tauri 已带 NSIS：`%LOCALAPPDATA%\tauri\NSIS`；
  找不到 makensis 时只出免安装版并提示）。安装包要与 Tauri 版的安装**分开**（不同的安装目录、产品名与卸载项，例如「幻述（原生版）」），
  不能覆盖或卸掉用户已装的 Tauri 版。两版共用同一个数据目录，这是有意的（原生版要接替它），在文档里写清楚。
- 检查运行时依赖：原生版不需要 WebView2；确认它还需要什么（VC 运行库？字体？）并写进文档。
- **验证**：打包脚本跑通；免安装 exe 带 `HUANSHU_HOME=<沙箱>` 冒烟（`--quit-after`，看窗口标题与退出码）；安装包静默安装到用户目录 → **带沙箱环境变量**启动一次 →
  静默卸载 → 确认没有残留（安装目录、开始菜单快捷方式、卸载注册表项）。安装 / 卸载会改用户机器上的东西：只做这一轮装一次卸一次，前后列出改动清单。
- **文档**：`CLAUDE.md` 加「打包与发布（原生版）」一节（命令、产物、坑），`apps/huanshu-native/README.md` 补一段「怎么出包」。

## 分支与提交

从 `feat/native-gaps` 开 `feat/native-math-package`：

1. `feat(slide-typst): render a single formula for the editor preview`
2. `feat(core): math delimiter rules shared by preview and slides`（如有）
3. `feat(native): formulas in the step 1 preview`
4. `build(native): no console window, icon and version resource`
5. `build: native packaging script and installer`
6. `docs: native packaging, audit and parity`

## 验证命令

```bash
cargo fmt --all
cargo test --workspace
cargo build -p huanshu-native --release
cargo build -p huanshu-native
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/package-native.ps1
```

## 最终汇报

1. 提交的 hash 与一句话说明
2. 验证命令的结果（贴输出末尾几行）
3. 公式：接口、缓存与失效、坏公式与美元符号规则、三处定界规则的对照、截图与帧耗时
4. 打包：产物清单与大小、安装目录 / 产品名 / 卸载项、运行时依赖、安装与卸载前后的改动清单
5. 新增 / 修改的 core 与 slide-typst 函数和单测；有意修改的测试期望值及原因（没有就写「无」）
6. 改动了框架接口的地方；「进行中」标志的复位出口；给用户的人工验收清单
