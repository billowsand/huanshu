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

交互验证：`tools/drive.ps1 -Route settings -Steps "click:0.21,0.29;wait:1.5;shot:a;key:^s" -Out target/drive`
（按窗口比例点击、SendKeys、截图；同样只在沙箱里跑）。页面刚打开时字体等数据还在加载、布局会挪动，第一次点击前多等几秒（`-Boot 12`）。

## 框架

```
src/
  main.rs        启动：读状态、应用主题、开窗口
  shell.rs       外壳：标题栏 + 当前页 + 演示覆盖层；ShellHandle（换页、换主题、开演示，
                 并记住进设置前的路由，供设置页「返回」回来源页）
  router.rs      路由（对应原版 main.ts 的五条路由）与 --route 解析
  theme/         Studio 配色 token（tokens.rs 由旧应用的计算值生成）→ cx.studio()
  widgets.rs     共用部件：按钮、徽章、面板、标题栏、主题圆点、todo_block 占位
  icons.rs       Carbon 图标（与原版同一套）+ 品牌图标的资源源
  slides.rs      Typst 位图：SlideImages（调度、落图、释放）+ slide_frame（加载/失败/重试）
  formulas.rs    预览里的数学公式：TextView 的 Markdown 插件（`$…$` / `$$…$$` → Typst 出图）
                 + FormulaCache（上限 512 条，淘汰的图由 Step1 render 顺路 drop_all）
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
9. **表单输入框是实体，不能每帧新建**（会丢焦点、丢光标、漏内存）：按**完整路径**（`cards[2].title`）
   缓存 `Entity<InputState>` / `Entity<TextareaState>` / `Entity<SelectState<_>>`，各自订阅；订阅与缓存同生共死。
   列表增删移、换页、换版式、应用成功这些**结构变化**用「表单代次」整组重建（下标变了路径就指向别的项）；
   字段里打字**不**重建。render 里同步值时跳过**正被编辑**的那个框（`focus_handle.is_focused(window)`），
   `InputState::set_value` 不发 `Change`，不会回环（样板见 `pages/workflow/step3.rs`）。
10. **位图换目标（换页、换牌组）用 `SlideImages::reset`**，不要重建 `SlideImages`（图集不会自己回收）；
    `release` 只在页面离开时调。要显示「渲染中…」用 `SlideImages::pending()` + 自己计时（原版 150ms）。
11. **第三步的牌组编辑只走 `WorkflowPage::commit_deck`**（唯一写入路径）：它写 `ProjectModel` → 应用级
    store（`GenerationStore::set_deck`）→ 按需落库。不要在页面里直接改 `ProjectModel.deck` 或 store 的牌组
    （store 的镜像会盖回去，或者污染别的项目）；动手前先问 `WorkflowPage::editing_locked`
    （只有**本项目**在生成 / 重试时才只读；别的项目在生成时本项目的第三步照常可编辑）。
12. **界面文字一律常规体**：原版 `src/themes/typography.css` 的 `#app * { font-weight: normal !important; }`
    把 Studio 外壳的字重全拍平了，要粗体必须**选粗体字族**。原生版照做——页面里不要写
    `.font_weight(...)`（`grep -rn "FontWeight::" src` 应当没有任何匹配），强调靠**颜色**
    （`cx.studio().colors`）与**字族 / 字号**，不要靠字重。唯一的例外是 GPUI Kit 组件内部
    改不掉的两处与主题里的语法高亮（见下）；另外原版被 `Teleport to="body"` 挪出 `#app` 的弹窗
    （密码、导出、字体选择、首页删除确认、第三步的版式选择）**不在** `#app *` 的范围内，原版里
    它们仍是 500/600——原生版为了统一已经拍平，属于有意偏离。

### 已知的 GPUI Kit 细节

- 组件读的是 `Theme.tokens`（由 `colors` 派生），改完颜色要 `ThemeTokens::from(&colors)` 重新派生（`theme::apply` 已处理）。
- 按钮的 `disabled` / `selected` 来自 trait：`use gpui_kit::component::{Disableable as _, Selectable as _};`
- `img("brand/mark.svg")` 走资源源（彩色 SVG）；`Icon` 走单色遮罩，按文字颜色着色。
- Markdown 渲染：`gpui_kit::component::text::TextView::markdown(id, text)`。
  预览里的数学公式挂 `formulas.rs` 的插件（`markdown_extensions`）：`parse` 在后台只判定并带上
  latex，`render_inline` / `render` 每帧只读 `FormulaCache`（出图在后台，键 = 公式 + 行内/块级 +
  正文色 + 0.5pt 字号档）。**淘汰下来的公式图必须 `drop_all`**（`Step1::render` 顺路做），
  与幻灯片位图同一条规矩。定界规则用 `huanshu_core::markdown_tools`（「价格 $5 和 $10」不被吞）。
- 代码编辑器：`EditorState::new(window, cx).language("markdown")` + `Editor::new(&state)`。
- **GPUI Kit 里改不掉的字重**（规矩 12 的例外，共两处）：`TextView::markdown` 的表头
  （`gpui-component` 的 `table_head` 固定 `FontWeight::BOLD`）；语法高亮里的 `emphasis.strong`（700）
  与 `title`（600），来自 `gpui-component` 的 `theme/default-theme.json`。按钮、输入框、
  对话框标题与标签都**没有**内置字重，跟随页面设置。
