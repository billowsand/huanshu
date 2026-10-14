# 幻述原生界面（GPUI Kit）

逐页复刻 Tauri + Vue 版的原生界面。**目标是完整替代原版**：可以按 GPUI 的特点调整细节，但关键功能一项都不能丢。
功能清单与进度见 [`docs/native/parity.md`](../../docs/native/parity.md)，原版截图在 `docs/native/reference/`。

## 运行（一律在沙箱里）

```powershell
$env:HUANSHU_HOME = 'E:\huanshu\target\sandbox-home'
cargo run -p huanshu-core --example seed_demo -- $env:HUANSHU_HOME   # 首次：建演示项目
cargo run -p huanshu-native --release
```

命令行开关（脚本化验证用）：

| 开关 | 作用 |
|---|---|
| `--route <路由>` | 直接进入某页：`home`、`wizard`、`settings[:appearance\|models\|generation\|storage\|about]`、`new`、`project:<id>[:<1\|2\|3>]` |
| `--theme <主题 id>` | 启动主题（并写入设置） |
| `--quit-after <秒>` | 到点退出 |
| `--open <id> --export-pdf <路径>` | 不开窗口，直接导出 PDF，退出码 0/1 |

截图：`powershell -NoProfile -ExecutionPolicy Bypass -File apps/huanshu-native/tools/shots.ps1 [-Only home,project-step3]`
→ `docs/native/shell/`，与 `docs/native/reference/` 同名对照。

## 框架

```
src/
  main.rs        启动：读状态、应用主题、开窗口
  shell.rs       外壳：标题栏 + 当前页 + 演示覆盖层；ShellHandle（换页、换主题、开演示）
  router.rs      路由（对应原版 main.ts 的五条路由）与 --route 解析
  theme/         Studio 配色 token（tokens.rs 由旧应用的计算值生成）→ cx.studio()
  widgets.rs     共用部件：按钮、徽章、面板、标题栏、主题圆点、todo_block 占位
  icons.rs       Carbon 图标（与原版同一套）+ 品牌图标的资源源
  slides.rs      Typst 位图：SlideImages（调度、落图、释放）+ slide_frame（加载/失败/重试）
  tasks.rs       线程：run_bg（同步重活）、run_async（core 的 async，tokio）、listen_events（生成事件）
  present.rs     演示覆盖层
  export.rs      保存对话框、PDF 结果对话框
  pages/         一个页面一个模块（home 是样板）；workflow/ 下是三步
```

### 写页面的规矩

1. **对照原版写**：先看 `docs/native/reference/` 的截图和对应的 `.vue` 文件，文案逐字一致，尺寸按原版 CSS 的逻辑像素。
   不确定的交互以原版代码为准，不要凭感觉。
2. **颜色只用 `cx.studio().colors`**，部件先找 `widgets.rs`；没有就加进 `widgets.rs`，不要在页面里写第二份样式。
3. **数据只经 `huanshu_core::services`**，本工程不碰 SQLite。耗时操作走 `tasks::run_bg` / `run_async`，UI 线程不阻塞。
4. **异步结果回来时，页面、项目、主题都可能已经变了**：需要时带上发起时的身份比对（页码、项目 id），迟到的结果作废。
5. **幻灯片位图只用 `slides::SlideImages`**；页面离开时调 `release`（GPUI 不会回收图集）。失败不自动重试。
6. **按键**：页面内的快捷键用 `track_focus` + `on_key_down`；不要用 `observe_keystrokes`（它是全应用的）。
7. 没做完的区域用 `widgets::todo_block` 占位（写清对照的 Vue 位置与要做的事），代码里的小缺口写 `TODO(parity): …`。
   做完一项就删掉占位，并更新 `docs/native/parity.md`。
8. 真机验证只在沙箱里做（`HUANSHU_HOME`），**绝不碰** `%LOCALAPPDATA%\auto-slidev-studio`。

### 已知的 GPUI Kit 细节

- 组件读的是 `Theme.tokens`（由 `colors` 派生），改完颜色要 `ThemeTokens::from(&colors)` 重新派生（`theme::apply` 已处理）。
- 按钮的 `disabled` / `selected` 来自 trait：`use gpui_kit::component::{Disableable as _, Selectable as _};`
- `img("brand/mark.svg")` 走资源源（彩色 SVG）；`Icon` 走单色遮罩，按文字颜色着色。
- Markdown 渲染：`gpui_kit::component::text::TextView::markdown(id, text)`。
- 代码编辑器：`EditorState::new(window, cx).language("markdown")` + `Editor::new(&state)`。
