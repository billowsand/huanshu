# 任务：原生界面第二轮——第一步「准备素材」与工作流（第十步）

你在仓库 `E:\huanshu`（幻述）里工作。原生界面 `apps/huanshu-native` 的框架和第一轮（设置、向导、加密导入导出）已经完成并审过。
本轮把**第一步「准备素材」**和它周边的**工作流功能**补到与原版一致，并把「开始 AI 生成」接到生成流水线上。开始前**必须**先读完：

1. `apps/huanshu-native/README.md`——框架与「写页面的规矩」（逐条遵守）；
2. `docs/native/parity.md`——本轮要把「工作流」「第一步」两节全部做成 ✅（「生成完成后自动跳到第 3 步」也在本轮）；
3. 样板：`pages/home.rs`、上一轮的 `pages/settings/`（表单、异步、草稿的写法照它们）；
4. `CLAUDE.md` 开头三节（代码结构、隔离数据目录、原生界面）与「布局选择」「图表页」「素材库图片与生成链路」三节；
5. 原版源码：`src/views/steps/Step1Prepare.vue`、`src/components/MarkdownWorkbench.vue`、`src/views/WorkflowView.vue`
   （`saveStep1Draft` / `persistStep1Meta` / `onStep1StartGenerate` / `saveName` / `ensureProjectForMedia` / 自动跳转的 `watch`）、
   `src/composables/useMediaLibrary.ts`、`src/stores/generation.ts`（只看 `generate` 与事件监听部分）；
6. 原版截图：`docs/native/reference/new-step1.png`、`project-step1.png`。

## 原则（同上一轮）

- **复刻**：布局、文案逐字、交互、规则以原版为准；可按 GPUI 特点调整控件形态，但**不能少功能**。拿不准时读原版代码。
- **不重写框架**：缺部件往 `widgets.rs` **加**；改框架接口要在汇报第 8 节说明理由。
- **数据只经 `huanshu_core::services`**；新逻辑放 core（带单测），界面里不写规则。

## 前几轮审查发现的问题（不要再犯）

1. **真机验证碰了真实数据**：上一轮有一次没设 `HUANSHU_HOME` 就启动了原生版，在用户的真实数据库里写进了 `app_theme`。
   本轮会**新建项目、写文稿、导入素材、发起生成**——每次启动前确认 `HUANSHU_HOME=E:\huanshu\target\sandbox-home`，
   **用 `tools/drive.ps1` / `tools/shots.ps1`（它们自己设置沙箱）**，不要直接双击 exe 或 `cargo run` 不带环境变量。
   动手前备份沙箱（`target/sandbox-home` → `target/sandbox-home.bak2`）。
2. **报告与事实不符**：没验证的写「未验证」；截图逐张亲眼看过再写「一致」。上一轮没有交汇报——本轮**必须**按文末格式交汇报。
3. **异步结果落错地方**：AI 优化、素材导入、生成都是异步的，结果回来时用户可能已换了项目或离开页面——带上发起时的身份比对。
4. **全应用按键观察者**：快捷键用 `track_focus` + `on_key_down`。
5. **文案漏字**：上一轮离开确认框漏了一个「会」。所有文案从原版复制，不要凭记忆打。

## A. core：文稿工具（规则搬进 Rust）

原版 `Step1Prepare.vue` / `MarkdownWorkbench.vue` 里有一批纯文本规则，原生版需要同一套。放进 core 新模块
`huanshu_core::markdown_tools`（纯函数，不碰状态），**逐条照抄原版的正则与边界行为**：

- `count_exact_headings(md, level)`（`countExactHeadings`）、`parse_sections`（`parseSections`）、
  `estimate_slides(md, mode)`（`estimateSlides`，`mode` 为 auto/h2/h3，auto 按「二级标题数 > 三级标题数」取 h2，否则 h3）；
- `strip_heading_number_prefix`、`to_chinese_number`、`add_heading_numbers(md, level, style)`、`remove_heading_numbers`、
  `auto_detect_headings`、`clear_heading_bold`；
- `split_preview_pages(md, granularity)`（`MarkdownWorkbench.vue` 的 `splitSlides`，含 `$$` 公式块内不切页）；
- `extract_h1(md)`（上传文件、AI 优化后从 H1 取项目名的那条正则）。

**对照测试必须用原版代码生成期望值**（与上一轮编辑器快照同样做法）：写一个一次性 `bun` 脚本（放 `target/` 下，不提交），把上面这些
TypeScript 函数**原样**拷进去，对一组输入文稿（至少覆盖：中文一二三编号、（一）（二）、1. 2.、`**加粗**` 标题、`#` 到 `######`、
`---` 分隔、空行、`$$` 跨行公式里的 `##`、CRLF 换行、没有任何标题）跑一遍，结果写成
`crates/huanshu-core/tests/fixtures/markdown-tools.json`；Rust 单测逐条比对。汇报里写清脚本怎么生成的。

**允许的重复（三）**：Tauri 版的 TypeScript 继续保留，两边文件头互相注释「改一边必须改另一边」，快照测试守住一致性。

## B. 第一步「准备素材」（`pages/workflow/step1.rs`）

把 `todo_block` 与 `TODO(parity)` 全部替换成实现，parity 表「第一步」一节全部 ✅：

1. **顶栏**：名称输入（占位文案照原版「演示名称（从 H1 自动提取或手动输入）...」）、上传后的文件名标签、字数、草稿保存状态
   （草稿保存中... / 草稿已保存 / 草稿保存失败）、AI 优化（`services::generation::optimize_markdown_headings`，async；进行中禁用，
   成功后整体替换文稿并从 H1 更新名称，失败显示原版的行内错误）、上传文件（`rfd`，`.md`/`.txt`，从 H1 取名）。
2. **格式工具**：六个按钮全部可用（A 节的函数），改写编辑器内容并进入撤销历史（用 `EditorState` 的 API 设值，确认 Ctrl+Z 能撤回）。
3. **拖入文稿**：把 `.md` / `.txt` 拖到编辑区载入（GPUI 的文件拖放：`on_drop` + `ExternalPaths`），拖入时显示原版的覆盖层提示。
4. **预览**：按分页粒度切成 P1、P2… 卡片（`split_preview_pages`），每张卡片里用 `TextView::markdown` 渲染；编辑区与预览区**滚动同步**
   （原版 `syncEditorToPreview` / `syncPreviewToEditor`，按比例同步即可，写清实现方式与已知偏差）。
5. **分页粒度**：自动推荐（显示推荐的是哪一级）/ 按二级标题 / 按三级标题，各显示标题数与页数；「预计 N 页」徽章；
   下方「二级标题 N 个，三级标题 N 个」。
6. **素材库**：文件数、添加素材（`rfd` 多选图片/视频，扩展名照原版 `MEDIA_EXTS`）、拖入图片/视频、列表（缩略图、删除、图注）、
   「素材生成时自动参与图文匹配。」。新建项目还没有 id 时，添加素材先建项目（原版 `ensureProjectForMedia`）。
   数据走 `services::media::{import_media_asset, delete_media_asset, get_project_media, update_media_caption}`。
7. **底栏**：「开始 AI 生成」的可用条件照原版 `canGenerate`；三种提示文案（请填写演示名称 / 请上传或粘贴文稿内容 / 当前预计生成 N 张幻灯片）。

## C. 工作流（`pages/workflow/mod.rs`）

1. **项目改名**：点侧栏项目名进入编辑，回车 / 失焦保存，Esc 取消（`saveName`，`services::projects::update_project_content`）。
2. **草稿自动保存**：第一步任何改动 800ms 防抖后保存（原版 `saveStep1Draft`）；新建项目在「有意义的草稿」时才落库
   （`hasMeaningfulStep1Draft`、`getDraftProjectName`），落库后路由换成该项目（`Route::Project { id: Some(..) }`，不要重建页面丢状态——
   需要的话给 `ShellHandle` 加一个只改路由不重建页面的方法，并在汇报里说明）。文件名、粒度、比例三项原版存 localStorage：
   原生版存进 core（新增 `services::projects` 的「项目附加信息」读写，存在 `app_settings` 的 `project_meta:<id>` 键里，JSON），带单测。
3. **开始 AI 生成**（`onStep1StartGenerate`）：取消待保存的防抖、立即保存草稿、切到第 2 步，用 `tasks::run_async` 调
   `services::generation::generate_slides`，事件经 `tasks::listen_events`（`ChannelSink`）回到界面：
   - `ProjectModel` 增加生成状态（`generating`、`generation_failed`、最近一条进度文案与进度值），侧栏第 2 步的说明随之变化；
   - 生成完成后从数据库重读牌组（core 会把结果存进当前项目，先确认 `set_active_project` / `open_project` 已对准这个项目）；
   - **完成且没有失败页时 800ms 后自动跳到第 3 步**（原版 `watch`，用户已离开第 2 步则不跳）；
   - 第 2 步的进度条、阶段、逐页状态**不在本轮**（下一轮做），本轮只要求第 2 步顶部能看出「生成中… / 生成完成 / 生成出错」和最新进度文案。
   - 生成需要 LM Studio。本机没在跑时：确认按钮可点、出错能显示「生成出错」且不崩，汇报写「生成流程未真机验证」。

## 分支与提交

从 `feat/native-settings` 新建分支 `feat/native-step1`，按顺序提交（每步提交前跑完验证，并同步更新 `docs/native/parity.md`）：

1. `feat(core): markdown tools shared with the editor`（A）
2. `feat(native): step 1 — prepare, format tools, preview, granularity, media`（B）
3. `feat(native): workflow — rename, draft autosave, start generation`（C）

## 验证

```bash
cargo fmt --all
cargo test --workspace
cargo build -p huanshu-native --release
```

真机验证（**全部在沙箱**，用 `tools/drive.ps1` 与 `tools/shots.ps1`）：

- 截图与 `docs/native/reference/new-step1.png`、`project-step1.png` 逐张对比，列出差异。
- 新建演示：粘贴（或上传）一份含二级/三级标题与 ```` ```chart ```` 块的文稿 → 名称从 H1 填好 → 草稿自动保存 → 返回首页能看到新项目 → 再打开，
  文稿、文件名、粒度、比例都还在。
- 六个格式工具各点一次，Ctrl+Z 能撤回；分页粒度三个选项的页数与原版对同一文稿的计算一致（用 A 节的快照证明）。
- 素材：添加两张图片 → 列表出现 → 改一条图注 → 删除一张；重新打开项目素材还在。
- 改名：侧栏改名，回车保存；Esc 取消不改。
- 开始生成：LM Studio 在跑就完整生成一次（3–5 节的小文稿），确认自动跳到第 3 步、牌组可预览；没在跑就按 C.3 最后一条验证并如实写。
- 拖放、滚动同步这类脚本做不到的交互，写进人工验收清单。

## 最终汇报（按此格式，不要省略）

1. 三个提交的 hash 与一句话说明
2. 每条验证命令的结果（贴输出末尾几行）
3. parity 表本轮改动的行（改动前 → 改动后，附依据）
4. A 节快照：生成脚本怎么写的、覆盖了哪些输入、Rust 与 TS 有没有不一致（有的话怎么处理的）
5. 截图对比清单与差异
6. 真机验证逐项结果（没做的写「未做」）；给用户的人工验收清单
7. 新增 / 修改的 core 函数与单测；有意修改的测试期望值及原因（没有就写「无」）
8. 与本说明或原版不一致的实现及原因；改动了框架接口的地方；没做完或不确定的点
