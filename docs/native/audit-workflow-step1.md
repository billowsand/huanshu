# 逐页审计：工作流框架 + 第一步 + 应用级生成 store

审计轮次：2026-10（分支 `feat/native-audit`）。范围：`src/views/WorkflowView.vue`（工作流框架：侧栏 / 路由 / 项目模型）、
`src/views/steps/Step1Prepare.vue`、`src/components/MarkdownWorkbench.vue`、`src/composables/useMediaLibrary.ts`（第一步素材的副作用）、
`src/stores/generation.ts`（应用级生成 store）↔ 原生 `apps/huanshu-native/src/pages/workflow/mod.rs`、`src/pages/workflow/step1.rs`、`src/generation.rs`。

做法：原版每个文件按模板从上到下逐项列出**用户能看到或操作的东西**（文字、按钮、输入、提示、禁用/显示条件、快捷键、悬停/拖放、确认框、轻提示），
再在 `<script>` / store 里列出**副作用**（调用了哪个命令、写了什么存储、何时触发）。状态：

- ✅ 一致
- 🔧 本轮已修（只改了 `pages/workflow/mod.rs` / `pages/workflow/step1.rs`，未碰其他文件）
- ⚠️ 有差距未修（附估计工作量）
- ➖ 有意不做（附原因与原版行号依据）

主题相关的纯视觉细节（阴影、渐变、毛玻璃、动画、字号/宽度的细微差）不逐条列，每个文件一行「视觉细节」。
对照截图：`docs/native/reference/project-step1.png` / `new-step1.png` ↔ `docs/native/shell/project-step1-0.png` / `new-step1-0.png`（同一份沙箱数据、同一主题，肉眼已对齐）。

本轮累计（正文第 1–5 节的逐项表，共 123 行）：**✅ 94 / 🔧 11 / ⚠️ 3 / ➖ 15**（另见 B 节两个调查项；统计明细见文末）。
第十五轮只做记号归并：原「预览渲染语义差异（breaks/linkify/html）」的 ⚠️ 按新原则归入 ➖「表现层，按 GPUI 原生做法」；
KaTeX 公式保持 ⚠️（另起一轮，需 KaTeX 等价排版/位图管线）。

---

## 1. `src/views/WorkflowView.vue` → `pages/workflow/mod.rs`（工作流框架）

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 加载中：「加载项目中...」 | `WorkflowView.vue:395-398` | `mod.rs::render`（`Load::Loading`，`:969-978`） | ✅ | 文案逐字；原版还有转圈图标，归「视觉细节」 |
| 加载失败：错误文本 + 「返回首页」按钮 | `WorkflowView.vue:400-403` | `mod.rs::render`（`Load::Failed`，`:979-993`） | ✅ | |
| 侧栏：返回按钮、项目名、三步、主题圆点、设置 | `WorkflowView.vue:408-466`（模板）、`:598-606`（`.sidebar` 宽 220px） | `mod.rs::render`（`:1204-1258`）| ✅ | 原生 204px 宽（视觉细节） |
| 返回按钮 → 首页（`arrow-left`） | `WorkflowView.vue:410-412`、`:616-628` | `mod.rs::render`（`widgets::icon_button("back", …)`，`:1216-1219`） | ✅ | 原生多一个「返回首页」tooltip（原版无） |
| 项目名：空名显示「未命名项目」、超长省略号、悬停显示全名、点击进改名 | `WorkflowView.vue:425-432`（`:427` 是 `:title`） | `mod.rs::render` name_area（`:1184-1197`）、`start_rename`（`:370-379`） | 🔧 | **本轮补上 `tooltip_text`**（原版 `:title="projectName \|\| '未命名项目'"`） |
| 改名输入框：Enter 保存 / 失焦保存 / Esc 取消 / `autofocus` | `WorkflowView.vue:414-423`（`:418-420`） | `mod.rs::start_rename`（`:370-379`，聚焦）、订阅 Enter/Blur（`:305-319`）、Esc（`:1174-1182`） | ✅ | |
| 改名保存：空名不保存且停在编辑态 | `WorkflowView.vue:366-374`（`:367`） | `mod.rs::save_name`（`:386-389`） | ✅ | |
| 改名落库：`updateContent(id, 新名, 文稿)` | `WorkflowView.vue:368-371` | `mod.rs::save_name`（`run_bg` → `projects::update_project_content`，`:398-422`） | ✅ | 原版无错误分支（unhandled），原生记 stderr 并留在编辑态 |
| 步骤一「准备素材」：说明=文件名 / `N 字符` / 「上传或粘贴文稿」；总是可点 | `WorkflowView.vue:313-327`（`:317-321`）、`:436-457` | `mod.rs::render` descs[0]（`:1005-1012`）、`clickable`（`:913`） | ✅ | 字数用 UTF-16 码元数（`ProjectModel::md_len`） |
| 步骤二「AI 生成」：说明四态（生成出错 / `stageLabel` / 已生成 N 张 / 等待开始） | `WorkflowView.vue:328-341` | `mod.rs::render` descs[1]（`:1013-1022`） | ✅ | |
| 步骤二：生成中圆点里是转圈；出错时圆点是 warning 图标 | `WorkflowView.vue:446-449` | `mod.rs::render`（`:1069-1080`） | ✅ | |
| 步骤二可点条件：`currentStep >= 2` | `WorkflowView.vue:340` | `mod.rs::clickable`（`:915`） | ✅ | |
| 步骤三「编辑完善」：说明=`N 张幻灯片` / 「生成后可编辑」 | `WorkflowView.vue:342-351` | `mod.rs::render` descs[2]（`:1023-1027`） | ✅ | |
| 步骤三可点条件：有蓝图且没有页在重试 | `WorkflowView.vue:350` | `mod.rs::clickable`（`:917`） | ✅ | |
| 步骤状态配色：active=primary、done=success（绿）、error=error | `WorkflowView.vue:703-720` | `mod.rs::render`（`:1044-1096`） | 🔧 | **本轮修**：原版 done 用 `--studio-success`/`-bg`、error 用 `--studio-error-bg`；原生原来一律 primary/panel |
| 步骤间连线（前两步下），走过的连线是绿色 | `WorkflowView.vue:451`（模板）、`:730-759` | `mod.rs::render`（`:1114-1124`） | 🔧 | **本轮修**：done 时 `c.primary` → `c.success` |
| 悬停可点步骤 → 标签变亮（text 色） | `WorkflowView.vue:680` | `mod.rs::render`（`group` + `group_hover`，`:1134-1142`） | 🔧 | **本轮修**（照 `step3.rs` 的写法） |
| 不可点步骤：半透明、点了没反应 | `WorkflowView.vue:441`、`:679`（opacity 0.45） | `mod.rs::render`（`:1109-1119`） | ✅ | 原生 0.5，差 0.05（视觉细节） |
| 步骤说明：最宽 130px + 省略号 | `WorkflowView.vue:775-783` | `mod.rs::render`（`:1147-1153`） | 🔧 | **本轮修**：补 `max_w(130)` + `truncate` + 文本列 `min_w_0`（长文件名原来会撑破侧栏） |
| 点击某一步切页（第一步总是可点） | `WorkflowView.vue:354-357`、`:472-549` | `mod.rs::show_step`（`:921-938`） | ✅ | 原版是 `v-if`，离开第一步就卸载（撤销历史丢失）；原生同样重建页面 |
| 侧栏底部：主题圆点（点即换主题并落库）+「设置」 | `WorkflowView.vue:460-465` | `mod.rs::render`（`:1224-1258`，`widgets::theme_dots` / `ghost_button("settings")`） | ✅ | |
| 初始步骤：生成中 → 2、有蓝图 → 3、否则 → 1 | `WorkflowView.vue:134` | `mod.rs::finish_open`（`:892-898`） | ✅ | |
| 打开项目副作用：`loadLatestRun` →（没有进行中生成时）`loadFromJson` / `reset` → 再 `loadLatestRun` → 读素材 | `WorkflowView.vue:106-139` | `mod.rs::load_project`（`:798-877`）+ `generation.rs::open_project`（`:805-850`） | ✅ | **有意偏离**：store 正为别的项目跑时不清空（原版 `gen.reset()` 会抹掉在途生成，`:126-132`）→ parity.md 第 85 行 |
| 生成完成 800ms 后自动进第三步（条件不变才计时） | `WorkflowView.vue:152-162` | `mod.rs::sync_advance`（`:724-754`） | ✅ | 依赖一次成功生成，未真机验证（需要 LM Studio） |
| 打开项目即预热图标向量 `ensure_icon_embeddings`（结果忽略） | `WorkflowView.vue:104` | `mod.rs::new`（`:286-295`） | ✅ | |
| 离开页面清掉草稿防抖计时 | `WorkflowView.vue:147-149` | `mod.rs`（`autosave_task: Option<Task>` 随页面 drop；`:440-450`） | ✅ | |
| 草稿 deep watch 800ms → `saveStep1Draft`（新建项目 / 更新内容 / `router.replace` / 状态三态） | `WorkflowView.vue:276-289`、`:235-274` | `mod.rs::schedule_autosave` / `flush_draft` / `persist_draft`（`:437-584`） | ✅ | 原生多了「期间内容又变了就再等一轮」，防止把旧稿写回 |
| `persistStep1Meta`：文件名 + 粒度 + 比例，并 `updateAspectRatio(id, 比例)` | `WorkflowView.vue:186-199`（`:197`） | `mod.rs::persist_draft`（`:483-498` 组装、`:531-539` 落库） | ✅ | |
| `loadStep1Meta`：三个字段白名单回落 | `WorkflowView.vue:201-219` | `mod.rs::load_project`（`:826-838`，`Step1ProjectMeta::resolve`） | ✅ | 存储位置改成 core（`app_settings.project_meta:<id>`），有意偏离 → parity.md 第 82 行 |
| 没有「有意义的草稿」且项目未落库 → 不保存、状态回 idle | `WorkflowView.vue:237-241` | `mod.rs::persist_draft`（`:500-510`）+ `has_meaningful_draft`（`:213-217`） | ✅ | |
| 草稿兜底名：名称 → 去扩展名文件名 → 「未命名演示」 | `WorkflowView.vue:229-233`、`:243`、`:263` | `mod.rs::draft_project_name`（`:200-210`） | ✅ | |
| 「开始 AI 生成」：校验 → 取消防抖 → 立即保存 → 清选中页 → 切第 2 步 → `gen.generate(...)` | `WorkflowView.vue:291-310` | `mod.rs::start_generation`（`:758-794`） | ✅ | 生成的牌组由 core 落库，页面不再二次存（原版注释 `:308-309`） |
| 「开始 AI 生成」可用条件：文稿非空 + 名称非空 + 没有生成/重试在跑 | `WorkflowView.vue:359-363` | `mod.rs::can_generate_local`（`:133-135`）+ `generation.rs::busy`（`:263-265`） | ✅ | 全应用同一时刻只有一次生成 → parity.md 第 103 行 |
| 第三步 Ctrl+S / Esc（window 级 keydown） | `WorkflowView.vue:377-391` | 第三步自己的 `track_focus` + `on_key_down` | ➖ | 本轮范围外（`step3.rs` 审计）；原版是**全局** window 监听（`:389-391`），原生改成页面焦点内（有意改进）→ parity.md 第 143 行 |
| 演示覆盖层 `<PresentationOverlay>` | `WorkflowView.vue:553-562` | `present.rs` | ➖ | 别人的负责范围（parity.md「演示」一节） |
| `PdfExportDialog` / `exportPdf()` | `WorkflowView.vue:553`、`:78-90` | `export.rs` + 第三步栏头 | ➖ | 别人的负责范围 |
| 确认对话框 / 轻提示 | 原版工作流页：**没有**（那四个文件里既没有 `window.confirm`，也没有 `alert` 或 toast 调用） | 原生同样没有 | ➖ | 依据：对 `WorkflowView.vue`、`Step1Prepare.vue`、`MarkdownWorkbench.vue`、`useMediaLibrary.ts` 检索 confirm / alert / toast → 0 命中 |
| 视觉细节 | 侧栏 220px vs 原生 204px；步骤圆圈 32px vs 原生 40px；字号（0.8125/0.7rem vs 15/12.5px）；加载转圈、`transition`、`.step-circle` 的 spring 曲线；`.pres-*` 覆盖层样式（`:2671-3087`）与 `@keyframes`（`:3089-3099`） | — | ➖ | 纯视觉，GPUI 下不逐项对齐 |

---

## 2. `src/views/steps/Step1Prepare.vue` → `pages/workflow/step1.rs`

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 顶栏「准备素材」页标签 | `Step1Prepare.vue:390` | `step1.rs::render` top_bar（`:667-682`） | ✅ | 原版还大写/加字距，中文无效果（视觉细节） |
| 演示名称输入框 + 占位符「演示名称（从 H1 自动提取或手动输入）...」 | `Step1Prepare.vue:392-396`（`:395`） | `step1.rs::new`（`InputState…placeholder`，`:133-137`）+ render（`:667-692`） | ✅ | 订阅 `InputEvent::Change` 写回模型（`:145-154`） |
| 文件名标签（有文件才显示，`document` 图标） | `Step1Prepare.vue:400-403` | `step1.rs::render`（`:692-701`） | 🔧 | **本轮修色调**：原版 `.filename-tag` 是 primary 系（`WorkflowView.vue:1030-1044`），原生原来用 Muted |
| 草稿状态徽章：`草稿保存中...` / `草稿已保存` / `草稿保存失败`（idle 不显示） | `Step1Prepare.vue:83-88`、`:404` | `DraftSaveState::text`（`mod.rs:99-107`）+ `step1.rs::render`（`:702-704`） | ✅ | 由 `mod.rs` 的防抖保存置位 |
| 字数徽章 `{n} 字`（UTF-16 码元数） | `Step1Prepare.vue:405`（`mdContent.length`） | `step1.rs::render`（`:705`）+ `ProjectModel::md_len`（`mod.rs:116-118`） | 🔧 | **本轮修色调**：原版 `.char-pill` 是 muted（`WorkflowView.vue:1046-1056`），不是 `.accent-pill` |
| 「AI 优化」按钮：文案 `优化中...` / `AI 优化`，禁用条件 `optimizing \|\| !文稿` | `Step1Prepare.vue:406-413`（`:408`、`:412`） | `step1.rs::render`（`:707-725`）+ `optimize`（`:276-335`） | ✅ | 调 `optimize_markdown_headings(raw, titleHint)`，`titleHint` 空则不传（`:277-285`） |
| 「上传文件」按钮：禁用条件只有 `optimizing` | `Step1Prepare.vue:414-417` | `step1.rs::render`（`:720-724`）+ `open_file`（`:249-257`） | ✅ | `rfd` 过滤 `.md`/`.txt`（原版 `:194`） |
| 格式工具行：标签「格式工具」 | `Step1Prepare.vue:423` | `step1.rs::render`（`:734-752`） | ✅ | |
| 六个格式按钮文案：`自动标题` / `清除标题加粗` / `删除序号` / `` `##` 一、二、 `` / `` `###` （一）（二） `` / `` `####` 1. 2. ``（逐字，含反引号） | `Step1Prepare.vue:424-465` | `step1.rs::render` tool_labels（`:726-733`）+ `apply_format_tool`（`:80-89`） | ✅ | 六个变换都在 core `markdown_tools`，有快照单测 |
| 六个格式按钮禁用条件：`optimizing \|\| !文稿` | `Step1Prepare.vue:426/432/438/444/453/460` | `step1.rs::render`（`tools_disabled`，`:665`） | ✅ | `run_format_tool` 里再挡一次（`:340-342`） |
| 草稿保存失败行内错误（`draftSaveState==='error'` 且原因非空） | `Step1Prepare.vue:466` | `step1.rs::render`（`:761-767`，`widgets::inline_error`） | ✅ | |
| AI 优化失败行内错误（非空即显示） | `Step1Prepare.vue:467` | `step1.rs::render`（`:769-776`）+ `optimize` 的失败分支（`:328-330`） | ✅ | 每次发起前清空（`:290`），同原版（`:364`） |
| 编辑器列拖入文稿：`drop` 读文件、`dragover` 高亮 + 覆盖层「释放以加载 Markdown 文件」 | `Step1Prepare.vue:474-484`、`:213-234` | `step1.rs::render`（`on_drop` `:864-871`、`on_file_drop_exit` `:873-877`、覆盖层 `:884-907`） | ⚠️ | **框架缺文件拖入悬停钩子**：`gpui-kit 0.7.3` 只有 `on_drop` / `on_file_drop_exit`，`drag_over` 恒为 false → 覆盖层与虚线高亮显示不出来。工作量：**大**（要上游支持 drag-enter 事件）。parity.md 第 97 行已记 |
| 拖入文稿的扩展名过滤 | `Step1Prepare.vue:222-234`：**不校验扩展名**，任何文件都当文本读 | `step1.rs::is_markdown_path`（`:67-69`）+ `on_drop`（`:865-870`）只认 `.md`/`.txt` | ➖ | 有意收紧：拖入 `.png` 等二进制在原生里会变成乱码进编辑器（原版就是这样，原版行号见左栏）。要逐字对齐只需去掉 `:865-870` 的过滤 |
| 编辑器 + 预览（`MarkdownWorkbench` 嵌入） | `Step1Prepare.vue:485-499` | `step1.rs::render` workbench（`:846-940`） | ✅ | 见第 3 节 |
| 分页粒度卡片：「分页粒度」+「预计 N 页」徽章（accent 色） | `Step1Prepare.vue:506-510` | `step1.rs::render`（`:987-1012`，`Tone2::Primary`） | ✅ | `estimate_slides` 调 core |
| 三个粒度选项：`自动推荐` / `按二级标题` / `按三级标题`，各自徽章（自动=检测结果、其余=`N 个标题`）与 `N 页`，选中态描边+底色，点击切换 | `Step1Prepare.vue:511-532` | `step1.rs::render` gran_option（`:937-982`）+ `set_granularity`（`:208-214`） | ✅ | |
| 粒度说明：「二级标题 N 个，三级标题 M 个」 | `Step1Prepare.vue:534-536` | `step1.rs::render`（`:1010-1014`） | ✅ | `count_exact_headings`（core） |
| 幻灯片比例卡片：三选项 `16:9 标准宽屏` / `32:9 超宽屏幕` / `48:9 全景屏幕`，选中态、点击写回 | `Step1Prepare.vue:541-555`、`:69-73` | `step1.rs::render` aspect_card（`:1016-1037`）+ `set_aspect`（`:192-206`） | ✅ | 写回 `update_project_aspect_ratio`（仅在有 id 时） |
| 比例说明：「32:9 / 48:9 在 16:9 屏幕上播放时会横向压缩」 | `Step1Prepare.vue:556-558` | `step1.rs::render`（`:1264-1270`） | ✅ | |
| 素材库头部：「素材库」+「N 个文件」徽章 +「添加素材」按钮（多选，过滤图片/视频） | `Step1Prepare.vue:563-570` | `step1.rs::render`（`:1123-1172`）+ `add_media_paths`（`:357-372`） | ✅ | `pick_files` + 同一份扩展名表（`:35-36`） |
| 素材拖放区：空态图标 + 「拖拽图片 / 视频到此处」 | `Step1Prepare.vue:572-582` | `step1.rs::render`（`:1170-1210`） | ✅ | 悬停高亮同「拖入文稿」的框架限制（见上一行 ⚠️；这里靠 `media_drop_over`，恒 false） |
| 素材区拖入图片/视频（按扩展名过滤，可多文件） | `Step1Prepare.vue:245-252`、`useMediaLibrary.ts:170-177` | `step1.rs::render` `on_drop`（`:1220-1223`）+ `add_media_paths`（`:363-372`） | ✅ | |
| 素材网格：缩略图（图片 / 视频图标）、名字（悬停显示全名）、图片显示 `W×H` | `Step1Prepare.vue:583-597`；样式 `WorkflowView.vue:1188-1268` | `step1.rs::render` media_thumb（`:1044-1130`） | 🔧 | **本轮修排布**：原版是 68×51 小卡横排换行、名字 muted 居中；原生原来是整行宽 64px 高的竖排列表 |
| 素材删除按钮：`title="移除"`，**只在悬停该缩略图时**出现 | `Step1Prepare.vue:590-592`；样式 `WorkflowView.vue:1229-1249`（`opacity: 0` → `:hover` 1） | `step1.rs::render` media_thumb（`:1085-1112`，`group` + `group_hover`） | 🔧 | **本轮修**：原来一直显示 |
| 素材区说明：「素材生成时自动参与图文匹配。」 | `Step1Prepare.vue:599` | `step1.rs::render`（`:1231-1235`） | ✅ | |
| 底栏：「开始 AI 生成」按钮（`magic-wand` 图标），禁用条件 `!canGenerate \|\| optimizing` | `Step1Prepare.vue:606-614` | `step1.rs::render`（`:1294-1308`） | ✅ | |
| 底栏三态提示：「请填写演示名称」/「请上传或粘贴文稿内容」/「当前预计生成 N 张幻灯片」 | `Step1Prepare.vue:615-617` | `step1.rs::render`（`:1275-1280`、`:1310-1312`） | ✅ | |
| AI 优化成功：整体替换文稿（trim）、从 H1 更新名称（没有 H1 保持原名）、保留撤销历史 | `Step1Prepare.vue:361-381`（`:370-375`） | `step1.rs::optimize`（`:307-327`，`extract_h1` + `replace_all`） | ✅ | 身份比对：模型实体 + 项目 id（迟到结果作废） |
| 上传 / 拖入文稿后：文稿整体替换、记文件名、名称取 H1（无 H1 时保留现值或取去扩展名的文件名） | `Step1Prepare.vue:202-211`、`:222-234` | `step1.rs::load_markdown`（`:218-246`） | ✅ | |
| 分页计数与预计页数上报（`detectedGranularityLocal` → 父组件） | `Step1Prepare.vue:171-190` | `step1.rs`（`ProjectModel::detected_granularity` `mod.rs:121-123`，预览分页直接用同一函数） | ✅ | 规则在 core `markdown_tools`，快照单测 |
| 素材新增前的 `ensureProjectForMedia`：无 id 时先建项目（名称兜底「未命名演示」+ 当前文稿）→ 写 meta → 路由 `replace` | `WorkflowView.vue:59-69`、`:243-254` | `step1.rs::ensure_project_for_media`（`:375-435`） | 🔧 | **本轮修**：原来 meta 里写死空文件名 / `auto` / 用 match 拼比例，且**漏写项目的比例列**；现在照原版 `persistStep1Meta` 写当前草稿三字段 + `update_project_aspect_ratio`（`WorkflowView.vue:190-199`） |
| 素材逐个导入（串行、一次一个），失败只 `console.error` | `useMediaLibrary.ts:82-117`（`:114-116`）、`:119-155` | `step1.rs::import_media`（`:438-507`） | ✅ | 原生失败 eprintln，无 UI（同原版） |
| 删除素材：调 core 删除，**失败也从列表移除** | `useMediaLibrary.ts:157-166`（`:165`） | `step1.rs::remove_media`（`:502-527`） | ✅ | |
| 素材元信息：`mime_type` / `media_type`(image\|video) / `slide_index: null` | `useMediaLibrary.ts:36-43`、`:88-100` | `step1.rs::media_mime_type`（`:39-51`）+ `import_media`（`:438-507`） | ✅ | |
| 确认对话框 / 轻提示 | 原版第一步：**没有**（全文件无 `confirm(` / toast；错误只有行内错误与 `console.error`） | 原生同样没有 | ➖ | 见证据行：`Step1Prepare.vue` 全文无对话框调用；`useMediaLibrary.ts:79/116/153/163` 都是 `console.error` |
| 视觉细节 | 顶栏 `flex-wrap`、`.prepare-tools` 字重/字距、输入框 focus 光晕、`.gran-choice`/`.aspect-ratio-choice` 的 `transform: translateY(-1px)` 与 hover 底色、`.media-del-btn` 的黑色圆底 | — | ➖ | 其中 `.gran-choice:hover { background: var(--studio-hover) }`（`WorkflowView.vue:1090-1093`）在暗色主题下**本就没有效果**：`--studio-hover` 只在 `src/themes/notes.css:83` 定义，其余主题未定义 → 声明无效。故原生不做 hover 底色是等价的（只有便签主题有细微差别） |

---

## 3. `src/components/MarkdownWorkbench.vue` → `pages/workflow/step1.rs`（编辑器 + 预览）

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 两个 pane 头：「编辑」（`edit` 图标）/「预览」（`view` 图标） | `MarkdownWorkbench.vue:324-336`、样式 `:411-424` | `step1.rs::render` pane_header（`:776-791`，`:877`、`:915`） | ✅ | |
| 编辑器：行号、Markdown 语法高亮 | `MarkdownWorkbench.vue:182-199` | `step1.rs::new`（`EditorState::new().language("markdown")`，`:138-143`） | ✅ | 高亮配色体系不同 → 视觉细节 |
| 编辑器占位符（多行示例文稿） | `MarkdownWorkbench.vue:490-498` | `step1.rs::EDITOR_PLACEHOLDER`（`:32`） | ✅ | 逐字一致（含空行） |
| 编辑器软换行（`EditorView.lineWrapping`） | `MarkdownWorkbench.vue:193` | GPUI 编辑器默认 soft-wrap（`gpui-base-0.7.3/src/input/base/state.rs:812`） | ✅ | 这一条同时是滚动同步偏差的来源 |
| 编辑器快捷键：Tab 缩进 / Shift-Tab / Ctrl+Z / Ctrl+Y / Ctrl+A / Ctrl+C/X/V | `MarkdownWorkbench.vue:195-199`（`indentWithTab` + `defaultKeymap` + `historyKeymap`） | GPUI 内建键位（`gpui-base-0.7.3/src/input/base/state.rs:136-300`：`tab`=IndentInline、`ctrl-z`=Undo、`ctrl-y`=Redo…） | ✅ | Windows 上与原版 Mod-z / Mod-y 一致 |
| 编辑器多光标 / 列选择 | `MarkdownWorkbench.vue:189-190`（`allowMultipleSelections` + `rectangularSelection`） | GPUI 内建：`ctrl-alt-↑/↓` 加光标（`state.rs:200-210`）、列选择（`column_select_start`） | ✅ | |
| 预览：每页一张卡片 + 右上角 `P{n}` 页码徽章 | `MarkdownWorkbench.vue:337-347`（`:344`）、样式 `:442-474` | `step1.rs::render` preview_card（`:795-829`，`P{index+1}` `:815`） | ✅ | |
| 预览分页规则（`splitSlides`：H2/H3 切页；`$$` 显示公式块内不切） | `MarkdownWorkbench.vue:64-115` | core `markdown_tools::split_preview_pages`（快照单测 `tests/markdown_tools.rs`） | ✅ | 注释互相指认（`MarkdownWorkbench.vue:61-63`） |
| 预览空态：「预览区将实时显示 Markdown 渲染效果」（空文稿也是一张 P1 卡） | `MarkdownWorkbench.vue:54-59`、`:101-104` | `step1.rs::render`（`:826`、空文稿分支 `:830-831`） | ✅ | |
| 预览渲染：**数学公式（KaTeX）** | `MarkdownWorkbench.vue:4`（`@vscode/markdown-it-katex`）、`:47`（`md.use(markdownItKatex)`） | `TextView::markdown`（`step1.rs:820`） | ⚠️ | **公式不渲染**：gpui 的 markdown 对未被插件认领的公式回落成原文（`gpui-base-0.7.3/src/text/format/markdown.rs:869-880`），块级公式当代码块（`:1056-1060`）。工作量：**大**（要接一个 KaTeX 等价的排版/位图管线；框架只给了 inline plugin 钩子，见 `src/text/text_view.rs:1179` 的测试）——**另起一轮**（需 KaTeX 等价排版/位图管线），本轮不动 |
| 预览渲染：软换行 `breaks: true` / `linkify` / `typographer` / `html: false` | `MarkdownWorkbench.vue:41-46` | `TextView::markdown`（标准 CommonMark） | ➖ | 表现层，按 GPUI 原生做法（原版单换行成 `<br>`、裸链接自动成链、`html:false` 让 HTML 当纯文本；gpui 侧软换行按空格合并、`Node::Html` 会被解析渲染——`TextView` 自带的 CommonMark 行为即可） |
| 编辑区 ↔ 预览区滚动同步（按比例、80ms 回声守卫） | `MarkdownWorkbench.vue:219-262` | `step1.rs::sync_editor_to_preview`（`:563-570`）、`sync_preview_wheel`（`:592-620`）、`editor_est_max_scroll`（`:523-549`） | ⚠️ | 详情见 B-2：编辑器真实最大滚动量不可得（估计值）、滚动条拖拽挂不上。parity.md 第 95 行 🟡 |
| 文稿变化 → 编辑器整体替换后同步一次预览 | `MarkdownWorkbench.vue:275-289`（`nextTick` + `syncEditorToPreview`） | `step1.rs` 订阅编辑器 observe（`:155-165`，每次状态通知都同步一次） | ✅ | 原生同步更频繁（打字也触发），有回声守卫，行为等价 |
| 窄屏（≤980px）编辑 / 预览 tab 切换 + 单栏 | `MarkdownWorkbench.vue:303-320`、`:575-587` | 未做 | ➖ | 原生窗口最小 1080×680（`apps/huanshu-native/src/main.rs:74`），980px 断点**不可达**；1100px 断点（`WorkflowView.vue:1291-1297` 侧栏下移）只在 1080–1100 的窄带内可达，桌面版不做响应式栈叠 |
| 视觉细节 | 卡片渐变 + `box-shadow` + `rgba(255,255,255,0.12)` 边（`:449-459`）；预览标题配色 h2 `#f5d76e` / h3 `#8fd6ff` / h4 `#b9c5dd`（`:503-519`，注意是硬编码色、非主题 token）；`.md-preview` 字号 0.875rem / 行高 1.8（`:476-481`）；pane 头 0.65rem 大写 + 字距（`:411-424`）；编辑器主题（字号 13px、行高 1.8、content padding、行号栏底色、选中色 `rgba(120,167,255,0.28)`、tok-* 高亮色，`:117-179`） | — | ➖ | 原生走 GPUI Kit 主题（等宽字体与行高由组件定），字号/留白略有差别；预览标题配色随 Kit 主题，**未逐字对齐原版硬编码色** |

---

## 4. `src/stores/generation.ts` → `src/generation.rs`（应用级生成 store）

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 状态字段：`stage` / `message` / `progress` / `blueprints` / `running` / `lastError` / `logs` / `currentRun` / `activeProjectId` / `pageStatuses` / `retryingPages`（+ 非响应式 `queuedRetries`、`generationVersion`） | `stores/generation.ts:91-104` | `generation.rs::GenerationStore`（`:185-210`） | ✅ | `progress` 保留；`lastError` 未单独存（界面无消费者，见下行） |
| `stageLabel`（八态中文表）/ `slideCount` | `stores/generation.ts:78-88`、`:106-107` | `GenState::stage_label`（`:120-122` → core `generation_progress::stage_label`）、`slide_count`（`:252-254`） | ✅ | 文案表在 core（快照单测） |
| `startListening`：`gen:progress` → stage/message/currentRun/progress | `stores/generation.ts:119-134` | `generation.rs::on_event` → `on_progress`（`:402-462`） | ✅ | 通道常驻（`tasks::listen_events_in`，`:214-216`），由 `receiving` 开合 |
| `gen:log` → 追加日志；`currentRun` 不存在或 run_id 不同就新建一条 | `stores/generation.ts:136-160` | `generation.rs::on_log`（`:465-488`） | ✅ | |
| `ensureBlueprintSlot`（按下标补空位） | `stores/generation.ts:163-167` | `generation.rs::ensure_slot`（`:60-65`） | ✅ | |
| `gen:slide_ready` → 按下标写蓝图（+ `completedSlides`） | `stores/generation.ts:169-177` | `generation.rs::on_event`（`:408-416`） | ✅ | `completedSlides` 见下文 |
| `gen:page_status` → 逐页状态（连续两次 error 保留更早的 previousStage）+ 带蓝图的按下标落位 | `stores/generation.ts:179-195`（`:186`） | `generation.rs::on_event`（`:417-428`）→ core `generation_progress::apply_page_status_event` | ✅ | 优先级规则在 core，有单测 |
| `gen:error` → stage=error、message、lastError、running=false、currentRun 状态改 error | `stores/generation.ts:197-210` | `generation.rs::on_event`（`:429-439`） | ✅ | 这条路正是**图表围栏报错**到达界面的路径（见 B-1） |
| `stopListening` | `stores/generation.ts:213-224` | `receiving = false`（`generation.rs:603`、`:931`） | ✅ | 通道不销毁，只关闸（有意偏离，模块文档） |
| `loadRunDeck`：`/new` 未落库 → `get_blueprints`，否则 `get_generation_deck(projectId)` | `stores/generation.ts:226-237` | `generation.rs::generate` 的收尾读牌组（`:546-557`） | ✅ | |
| `generate()`：忙则直接返回；重置 state/blueprints/logs/pageStatuses/currentRun、`activeProjectId`、stage=start | `stores/generation.ts:239-254` | `generation.rs::generate`（`:493-520`） | ✅ | 全应用一次生成（`busy()`，`:263-265`） |
| `generate()` 成功：载入本次运行的牌组；`activeProjectId` 变了就整条放弃；`pageStatuses` 非 error 改 done；stage=done、progress=1、message=`生成完成，共 N 张幻灯片` | `stores/generation.ts:266-277` | `generation.rs::finish_generate`（`:576-599`） | ✅ | 原生用版本号判定「期间换过项目」（`:576`） |
| `generate()` 失败：`lastError`/stage/message=错误；再尝试取回已组装的牌组（首张是 cover 才采用）供单页重试 | `stores/generation.ts:278-292` | `generation.rs::finish_generate`（`:590-598`，`first_is_cover`） | ✅ | |
| `generate()` finally：`running=false`、`stopListening`、依次执行排队中的重试 | `stores/generation.ts:293-300` | `generation.rs::finish_generate`（`:602-605`）+ `drain_queued_retries`（`:642-651`） | ✅ | 原生用 `retry_in_flight` 区分「排队中」与「正在跑」（`:198-199`），否则排队的重试永远轮不到 |
| `executePageRetry`：状态置 content「正在重试该页」；没有 `currentRun` → error「缺少生成记录，请重新生成」 | `stores/generation.ts:303-308` | `generation.rs::execute_page_retry`（`:660-693`） | ✅ | |
| `executePageRetry` 成功：写 `blueprints[index+2]`、状态 done「重试完成」；`deck_ready` → stage=done/progress=1/message「该页重试完成，可继续编辑」，否则把 `invalid_pages` 标 error「未通过最终校验」、message=`该页已更新；{remaining}` | `stores/generation.ts:312-334` | `generation.rs::finish_page_retry`（`:727-774`） | ✅ | 版本号不符 → 整条作废（`:727`） |
| `executePageRetry` 失败：状态 error（previousStage 取旧值或 content）、`message=String(error)` | `stores/generation.ts:335-339` | `generation.rs::finish_page_retry`（`:775-788`） | ✅ | |
| `retryPage()`：三个前置条件（已在重试 / 非运行中有重试 / 该页不是 error）→ 加入 `retryingPages`，运行中则排队 | `stores/generation.ts:345-354` | `generation.rs::retry_page`（`:611-635`） | ✅ | |
| `repairSlide()` | `stores/generation.ts:356-360` | 未移植 | ➖ | 原版**没有任何调用方**（`grep -rn "repairSlide" src/` 只命中定义本身），界面到不了 → 不移植（`generation.rs:28` 已注明） |
| `reset()` | `stores/generation.ts:362-378` | `generation.rs::reset` / `reset_inner`（`:924-944`） | ✅ | |
| `loadFromJson(json)`：先 `stopListening`；解析失败静默；`version++`、清排队重试与重试集合、铺蓝图为 done（首张是 cover 时切掉前后）、stage=done、message=`已加载 N 张幻灯片` | `stores/generation.ts:380-399` | `generation.rs::load_from_deck`（`:901-921`）+ `content_slots`（`:78-90`） | ✅ | 原生没有在 `load_from_deck` 里动 `receiving`：调用它的前提（`open_project` 里 store 不在忙）决定了此刻 `receiving` 已是 false |
| `loadLatestRun(projectId)`：设 `activeProjectId`、读 run 与日志、从日志恢复失败/修好的页状态 | `stores/generation.ts:401-420` | `generation.rs::apply_run`（`:853-871`）+ `read_run_and_logs`（`:948-958`） | ✅ | 日志→状态规则在 core `statuses_from_logs`（单测） |
| `loadLatestRun`：`run.status==='running'` → `running=true`、stage=current_stage、message 兜底「正在恢复生成进度...」、重新 `startListening` | `stores/generation.ts:421-425` | `generation.rs::apply_run`（`:872-882`） | ✅ | 只在 store 本来就为**本项目**在跑时走这条路 |
| `loadLatestRun`：否则（不 running 时）按 error / done+有牌组 改 stage | `stores/generation.ts:426-433` | `generation.rs::apply_run`（`:889-897`）+ `stored_stage`（`:161-168`） | ✅ | **有意偏离**：库里停在 `running` 但进程没在跑 → 按「已中断」显示（`:883-888`），parity.md 第 85 行 |
| `completedSlides` | `stores/generation.ts:95`、`:175`、`:192`、`:245`、`:271`、`:288`、`:314`、`:369`、`:390` | 未存 | ➖ | 全仓库无消费者：`grep -rn "completedSlides" src/` 只命中 store 自身（`generation.ts:95,175,192,245,271,288,314,369,390`）→ 界面看不到 → 不移植（`generation.rs:27-28` 已注明） |
| `progress` / `lastError` 的界面消费 | `stores/generation.ts:93`、`:97` | `GenState.progress` 保留、`lastError` 未存 | ➖ | 第一步/第二步都不读它们：检索 `gen.progress` / `lastError` 在 `src/` 下无命中（第二步的进度条走 `utils/generationProgress.ts`） |
| 视觉/文案 | `stageLabel` 八态文案 | core `generation_progress::stage_label` | ✅ | 第二步的看板属于另一轮审计（`step2.rs`） |

---

## 5. `src/composables/useMediaLibrary.ts`（第一步素材的副作用，配合第 2 节）

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| `loadProjectMedia`：打开项目时读 `get_project_media`，`url` 走 `convertFileSrc` | `useMediaLibrary.ts:49-68` | `mod.rs::load_project`（`get_project_media`，`mod.rs:806`）+ `step1.rs` 的 `media_thumb`（`img(PathBuf::from(url))`） | ✅ | 读失败只 `console.error`（原版 `:65-67`）→ 原生忽略 |
| `openMediaFiles`：多选 + 「图片 / 视频」过滤器 | `useMediaLibrary.ts:70-80` | `step1.rs::render`（`rfd::pick_files` + 同一份过滤器名与扩展名表，`:1140-1165`） | ✅ | |
| `addMediaFromPath`：先 `ensureProjectForMedia`，再读文件字节 → `import_media_asset` → push 进列表 | `useMediaLibrary.ts:82-117` | `step1.rs::add_media_paths`（`:357-372`）/ `ensure_project_for_media`（`:375-435`）/ `import_media`（`:438-507`） | 🔧 | 本轮修了 `ensureProjectForMedia` 的 meta 与比例落库（见第 2 节「素材新增前…」行） |
| `addMediaFromFile`（拖入路径走这条：`arrayBuffer` → 同上） | `useMediaLibrary.ts:119-155`、`:170-177` | `step1.rs::add_media_paths`（拖入给的是路径，读文件字节） | ✅ | 原生只有路径一条路（GPUI 拖入给 `ExternalPaths`） |
| `removeMedia`：`delete_media_asset`，失败也从列表移除 | `useMediaLibrary.ts:157-166` | `step1.rs::remove_media`（`:502-527`） | ✅ | |
| `insertMediaUrl` | `useMediaLibrary.ts:179-190` | 未移植（第三步范围） | ➖ | 原版的触发入口「插入素材引用」在 `Step3Editor.vue` 里**没有可点的地方**（parity.md 第 139 行已记），界面到不了 |

---

## 6. B 节：parity 两项的证据调查

### B-1 图表围栏（```chart）的校验提示

**结论：原版第一步没有任何围栏校验 UI；围栏错误只在「开始 AI 生成」后由 core 抛出，显示在第二步的头部消息里。**
parity.md 第 107 行（现已改成「图表围栏的校验提示 ✅ · 原版无单独提示」并记为已真机验证）与本文档结论一致；第十四轮曾把它记成「⬜ `input.rs` 的解析报错」挂在第一步，那行当时不实，本轮确认已经修正。

查证过程与依据：

1. 第一步的原版文件里没有任何针对 ```chart 的即时校验：
   - `grep -rn "chart|围栏|fence" src/views/steps/Step1Prepare.vue` → **0 命中**（在该文件里唯一相关的是 `estimateSlides` / 编号工具，与图表无关）；
   - `src/components/MarkdownWorkbench.vue` 只有 `$$`（数学公式）的围栏状态机（`:79-85`），**没有 chart 围栏**；
   - `src/views/WorkflowView.vue` 只有 `.ej-chart-json`（第三步图表 JSON 框的一条样式，`:2180-2181`），step1 侧无逻辑；
   - `src/utils/chartSpec.ts` 只是类型定义（`:1-9` 明确写「校验规则在 Rust 侧」「编辑器通过 `chart_problems` 命令取中文报错」）——那是**第三步**的框；
   - `src/stores/`、`src/composables/` 里 `chart` 只有 `chart_focus` 这类版式名，没有围栏校验。
2. core 的围栏错误在解析 ```` ```chart ```` 块时抛出，解析发生在**生成流水线里**：
   - 解析入口 `crates/huanshu-core/src/input.rs::parse_markdown`：闭合时 JSON 解析失败 → `:103-104`；`ChartSpec::problems()` 非空 → `:106-113`；不在 H2/H3 小节内 → `:119`；未闭合 → `:220-222`。文件末尾还要求有 H1（`:236`）与至少一个 H2（`:239`）。
   - 调用点：`crates/huanshu-core/src/services/generation.rs:1072`（`generate_slides` 的流水线里、`init` 阶段之后），`?` 直接冒泡到 `:329-352` 的 Err 分支：写库 error → `events.emit(CoreEvent::Error{ message })` → 返回 `Err`。
3. 原版第二步怎么显示它：
   - `src/stores/generation.ts:197-210` 的 `gen:error` 监听器把 `stage='error'`、`message=错误文本`；
   - `src/views/steps/Step2Generate.vue:415` 标题变「生成遇到问题」，`:418` 第二行直接显示 `props.message`；底栏出现「返回上一步」/「重新生成全部」（`:671-677`）。
   - 同理，`invoke` 的 Err 也走 `generation.ts:278-282` 的 catch，把同一段文本写进 `message`。
4. 原生侧走的是同一条链路：`generation.rs::on_event` 的 `CoreEvent::Error`（`:429-439`）→ `step2.rs:318`「生成遇到问题」+ `:371-379` 显示 `snap.message`。**没有额外工作要做**。
5. 顺带记录：第一步的**预览**对 ```chart 围栏也不会高亮/报错（它就是普通代码块），原版亦然（`md.render` 的默认行为，`MarkdownWorkbench.vue:54-59`）。

**给 parity 的建议行**：`图表围栏的校验提示 | Step1Prepare/MarkdownWorkbench 无此 UI | ➖ | 原版第一步没有即时校验：围栏错误由 core 在生成时抛（input.rs:103-119,220-222 → services/generation.rs:1072 → CoreEvent::Error），第二步显示为「生成遇到问题」+ message（Step2Generate.vue:415,418）`

### B-2 编辑区 ↔ 预览区滚动同步（parity 现为 🟡）

**结论：① 拿不到编辑器真实最大滚动量（`text_bounds()` 是视口矩形，不是内容边界）；② 两个方法都能访问（`EditorState` 就是 `InputBaseState<EditorMode>` 的别名，方法都是固有公开方法）；③ 滚动条拖拽挂不上——预览侧根本没有可拖的滚动条，编辑器内建滚动条的拖拽只改句柄、不通知实体。因此本轮**没有**把 `editor_est_max_scroll` 改成真值（保持 ⚠️），只在代码里写清了查证结果。**

逐项证据：

**① `text_bounds()` 是视口大小的矩形，`text_bounds.h − input_bounds.h` 恒为 0**

- `gpui-base-0.7.3/src/input/base/state.rs:462`：字段 `pub(super) last_bounds: Option<Bounds<Pixels>>`（注释写 "The text bounds"）；`:644-646` `pub fn text_bounds(&self) -> Option<Bounds<Pixels>> { self.last_bounds }`。
- 它只在 paint 里被赋值，值就是**元素自身**的 bounds：
  - `base/element.rs:2860` `let input_bounds = bounds;`（prepaint 拿到的元素 bounds，尺寸=视口）
  - `base/element.rs:2868-2869` `layout_cursors(&last_layout, &mut bounds, scroll_size, …)`
  - `base/element.rs:717`（`layout_cursors` 末尾）`bounds.origin = bounds.origin + scroll_offset;` —— **只有 origin 被滚动平移，size 不变**
  - `base/element.rs:2983` `PrepaintState { …, bounds, … }`；`:3303` `let bounds = prepaint.bounds;`；`:3311` `state.last_bounds = Some(bounds);`
- `input_bounds()`（`state.rs:640-642`）在 paint 里由 `:3313` `state.set_input_bounds(input_bounds, cx)` 写入，也是同一个元素 bounds。
  → 两者**尺寸相同**（都是视口），所以任务书里 `max = max(0, text_bounds.h − input_bounds.h)` 这一类算法得到恒 0，不能用。
- 真正的内容高度是私有字段 `scroll_size`：`state.rs:494-495`（`pub(crate) scroll_size: gpui::Size<Pixels>`），由 prepaint 写进 `element.rs:3315`，clamp 在 `update_scroll_offset`（`state.rs:2607-2624`：`safe_y_range = (-scroll_size.height + input_bounds.size.height).min(0)..0`）。`DisplayMap::wrap_row_count()`（`editor/display_map/display_map.rs:375`）虽然 `pub`，但它挂在私有的 `display_map` 字段上（`state.rs:410` `pub(super) display_map`）。
- 公开可用的只有：`text_bounds()`、`input_bounds()`、`scroll_offset()`（`:3111`）、`set_scroll_offset()`（`:3118`）、`line_height()`（`:3124`）、`visible_row_range()`（`:3106`）、`range_to_bounds()`（`:3796`，只对**可见**范围有效，见 `line_and_position_for_offset` `:1006-1028` 只遍历 `last_layout.lines`＝可见行）。126 个 `pub fn` 里没有 max-scroll / content-size 的 getter（`grep -n "pub fn " base/state.rs` 全表核对）。
- 唯一能读到真值的办法是**探针**：`set_scroll_offset(point(0, -1e6))` 的 deferred 值会在下一帧 prepaint 被 `clamp_auto_grow_vertical_scroll_offset`（`element.rs:310-321`）+ paint 的 clamp 夹到 `safe_y_range.start`（正好是 `-max_scroll`），下一帧 `scroll_offset()` 就是真值（`prepaint_caret_geometry().scroll_offset()` `state.rs:386-390` 也能在 prepaint 里读到夹过的值）。代价：那一帧文本原点被画在 -1e6，**编辑器会可见地闪一下**，且会打乱用户当时的滚动位置 → 不采用。

**② `EditorState` 能不能访问这两个方法：能**

- `gpui-base-0.7.3/src/input/editor/mod.rs:11`：`pub type EditorState = InputBaseState<EditorMode>;`（另见 `src/input/base/kind.rs:9` 的注释：「pub type EditorState = InputBaseState<EditorMode>」）。`InputBaseState` 本身是 `pub struct`（`state.rs:403`），`text_bounds()` / `input_bounds()` 是它的**固有公开方法**（`:640`/`:644`），不需要 Deref。`gpui-component-0.7.3/src/input/mod.rs:28-39` 又把 `EditorState` 原样重导出，原生页正是从这里拿到它（`step1.rs:10`）——现有代码已经在调 `input_bounds()`（`step1.rs:543`）、`line_height()`、`scroll_offset()`、`set_scroll_offset()`，说明这条路是通的。

**③ 滚动条拖拽有没有事件可挂：没有**

- 预览侧容器只是 `overflow_y_scroll()` 的 div + `.track_scroll(&preview_scroll)`（`step1.rs:918-926`）。这版 gpui 的 div **不画滚动条**：`gpui-pre-0.3.9/src/elements/div.rs` 里没有任何 scrollbar 相关代码（`grep -rni "scrollbar\|thumb" src/elements/div.rs` 只命中 `overflow-*` 的样式设置，没有绘制/命中框），只有 `src/styled.rs:64-70` 的 `scrollbar_width`（只在布局里**预留**宽度）与 `src/style.rs:191-192/1200-1220` 的说明。→ 界面上没有可拖的滚动条，想同步也无从挂起。（顺带：这也意味着第一步预览区目前**看不到滚动条滑块**，与原版 WebView 的 `overflow-y: auto` 有可见差别，归视觉细节。）
- 编辑器**有**内建滚动条：`gpui-base-0.7.3/src/input/base/element.rs:194`（`pub(super) struct EditorScrollbar`，`:225-260` 的 prepaint 用 `state.scroll_handle.clone()` 造 `Scrollbar::vertical(&handle)` + `viewport_bounds` + `scroll_size`）。但：
  - 该元素是 `pub(super)`、`InputBaseState::scroll_handle` 是 `pub(crate)`（`state.rs:494`）→ 从 `huanshu-native` 拿不到；
  - `Scrollbar` 拖拽最终调 `ScrollbarHandle::set_offset`（`gpui-base-0.7.3/src/scrollbar.rs:69-100`，`impl ScrollbarHandle for ScrollHandle` → `gpui-pre-0.3.9/src/elements/div.rs:4400` 的 `ScrollHandle::set_offset`），**只改 `Rc<RefCell<…>>` 里的值、不通知任何实体**，所以 `cx.observe(&editor, …)` 不会响；
  - 而且它不会顺带触发通知：paint 里的 `update_scroll_offset` 只在「夹过的偏移 ≠ 句柄当前值」时才 `cx.notify()`（`state.rs:2616-2624`），拖拽本身就在合法范围内 → 不通知。（滚轮则**会**通知：`InputBaseState::on_scroll_wheel` `state.rs:2578-2597` → `update_scroll_offset` → `cx.notify()`，这是现在「编辑器滚→预览跟」能工作的原因。）
- 唯一可行的挂法：不依赖内建滚动条，自己放一个 `Scrollbar` 元素并把句柄包成自己的 `ScrollbarHandle`（trait 提供 `start_drag` / `end_drag` / `set_offset` 回调，`scrollbar.rs:69-82`；`viewport_bounds` / `scroll_size` 也可自定，见 `:836-857`）。这会同时解决「预览区没有滚动条」这件事，但要动布局与句柄包装（约 40-60 行 + 视觉回归），**本轮不做**，记 ⚠️。编辑器侧则无论如何都拿不到它的内建滚动条，拖它同步在框架层面无解。
- 结论一句话（建议写进 parity 第 95 行）：
  「编辑器最大滚动量无公开 API（`text_bounds()` 是视口矩形；内容尺寸 `scroll_size` 是 `pub(crate)`），按「行数 × 行高 − 视口」估计，soft-wrap 折行时中段有偏差；
  编辑器内建滚动条的拖拽只改 `ScrollHandle`、不通知实体，预览侧又根本没有可拖的滚动条（`overflow_y_scroll` 的 div 不画滚动条），故双向的滚动条拖拽都不触发同步；只挂了滚轮，双向均有回声守卫。」

---

## 统计

正文（第 1–5 节）逐项表共 **123** 行：

| 状态 | 条数 | 分布（框架 / 第一步 / 编辑器·预览 / store / media composable） |
|---|---|---|
| ✅ 一致 | 94 | 28 / 28 / 10 / 24 / 4 |
| 🔧 本轮已修 | 11 | 5 / 5 / 0 / 0 / 1。见下表 |
| ⚠️ 有差距未修 | 3 | 0 / 1 / 2 / 0 / 0。见下表 |
| ➖ 有意不做 | 15 | 5 / 3 / 3 / 3 / 1。见下表（含 5 条「视觉细节」行） |

### 本轮 🔧 清单

| # | 位置 | 内容 |
|---|---|---|
| 1 | `mod.rs::render`（步骤圆点） | done 用 `c.success` / `c.success_bg`、error 用 `c.error_bg`（原版 `WorkflowView.vue:703-720`） |
| 2 | `mod.rs::render`（步骤连线） | 走过的连线 `c.primary` → `c.success`（原版 `:730-759`） |
| 3 | `mod.rs::render`（步骤标签） | done 也亮成 text 色；可点步骤悬停变亮（`group` + `group_hover`，原版 `:680`、`:772-773`） |
| 4 | `mod.rs::render`（步骤说明） | `max_w(130)` + `truncate` + 文本列 `min_w_0`（原版 `:775-783`） |
| 5 | `mod.rs::render`（侧栏项目名） | 补 `tooltip_text`（原版 `:title`，`:426`） |
| 6 | `mod.rs::aspect_str` | `fn` → `pub(super) fn`（供 step1 复用存储字符串） |
| 7 | `step1.rs::render`（文件名标签） | Muted → Primary（原版 `.filename-tag`，`WorkflowView.vue:1030-1044`） |
| 8 | `step1.rs::render`（字数徽章） | Primary → Muted（原版 `.char-pill`，`WorkflowView.vue:1046-1056`） |
| 9 | `step1.rs::render`（素材缩略图 + 删除按钮 + 拖放区） | 68×51 小卡横排换行、名字 muted 居中省略、删除按钮只悬停显示、拖放区虚线框常显（原版 `WorkflowView.vue:1157-1268`） |
| 10 | `step1.rs::render`（编辑/预览两栏） | 宽度比改成原版 `.md-panes` 的 1.15 : 0.85、预览最小 260px（`MarkdownWorkbench.vue:395`） |
| 11 | `step1.rs::ensure_project_for_media` | 写当前草稿的 meta（文件名/粒度/比例）并补 `update_project_aspect_ratio`（原版 `persistStep1Meta`，`WorkflowView.vue:65,190-199`） |

### 本轮 ⚠️ 清单（附估计工作量）

（与上面正文的 3 条 ⚠️ 行一一对应；第 2 条在正文里是一行，实际含两个子问题。）

| # | 差距 | 位置 | 工作量 |
|---|---|---|---|
| 1 | 文稿/素材拖入的**悬停**高亮与覆盖层显示不出来（框架无 drag-enter 钩子，只能收到 drop 与 exit） | 原版 `Step1Prepare.vue:213-220,236-243,474-484`；原生 `step1.rs:105,107`（两个标志位）、`:864-877`、`:1220-1226` | 大（要上游 gpui-kit 支持拖入悬停事件） |
| 2 | 编辑区↔预览区滚动同步（正文一行，含两件事）：(a) 编辑器最大滚动量只能估计（soft-wrap 下中段有偏差）；(b) 双向的**滚动条拖拽**同步（编辑器内建滚动条拖拽不通知实体；预览侧干脆没有滚动条元素） | 原版 `MarkdownWorkbench.vue:219-262`；原生 `step1.rs:523-549`（估计）、`:592-620`（滚轮回调） | (a) 中（见 B-2：唯一办法是越界探针，会闪一帧，不建议；除非上游公开 `scroll_size`）；(b) 中-大（要自建 `Scrollbar` 元素 + 包装 `ScrollbarHandle`，约 40-60 行） |
| 3 | 预览**不渲染数学公式**（原版 KaTeX；gpui 对未认领公式回落原文、块级公式当代码块） | 原版 `MarkdownWorkbench.vue:4,47`；gpui `text/format/markdown.rs:869-880,1056-1060` | 大（需要 KaTeX 等价排版；框架只给了 inline plugin 钩子）。**另起一轮**（需 KaTeX 等价排版/位图管线），本轮不动 |

> 另有两条**归入 ➖ 的「视觉细节」行**、未计入 ⚠️：预览标题配色未逐字对齐原版硬编码色（h2 `#f5d76e` / h3 `#8fd6ff` / h4 `#b9c5dd`，`MarkdownWorkbench.vue:503-519`，跟随 GPUI Kit 主题）；第一步预览区没有可见滚动条滑块（原版 WebView 的 `overflow-y: auto`）。

### 本轮 ➖ 清单（附原版行号依据）

| # | 项 | 依据 / 原因 |
|---|---|---|
| 1 | 第三步 Ctrl+S / Esc（WorkflowView 的 window 级 keydown） | `WorkflowView.vue:377-391`：原版是全局 window 监听；原生按 README 规矩 6 用页面 `track_focus`（parity.md 第 143 行已记为有意改进），属第三步审计 |
| 2 | 演示覆盖层 / PDF 导出对话框 | `WorkflowView.vue:553-562`、`:78-90`；present.rs / export.rs 是别人的范围 |
| 3 | 工作流页的确认对话框 / 轻提示 | 那四个文件里既没有 `window.confirm`，也没有 `alert` 或 toast 调用（候选 0 命中） |
| 4 | 拖入文稿的扩展名过滤（原生比原版**严**） | 原版 `Step1Prepare.vue:222-234` 不校验扩展名（任何文件当文本读）；原生 `step1.rs:67-69,865-870` 只认 `.md`/`.txt`——有意收紧，避免二进制进编辑器 |
| 5 | 窄屏响应式（≤980px 编辑/预览 tab、≤1100px 侧栏下移） | 原版 `MarkdownWorkbench.vue:303-320,575-587`、`WorkflowView.vue:1291-1297`；原生窗口最小 1080×680（`apps/huanshu-native/src/main.rs:74`）→ 980px 断点不可达，1100px 断点只在 1080–1100 窄带可达，桌面版不做栈叠 |
| 6 | `repairSlide` | `stores/generation.ts:356-360`；全仓库无调用方（检索 `repairSlide` 只命中定义本身） |
| 7 | `completedSlides` | `stores/generation.ts:95,175,192,245,271,288,314,369,390`；界面无消费者（只有 store 内部读写） |
| 8 | `progress` / `lastError` 的界面消费 | `stores/generation.ts:93,97`；检索 `gen.progress` / `lastError` 在界面侧 0 命中 |
| 9 | 预览**渲染语义差异**：原版 `breaks:true`（单换行成 `<br>`）/`linkify`/`typographer`/`html:false`，gpui 是标准 CommonMark 且会渲染内联 HTML | 表现层，按 GPUI 原生做法（原版 `MarkdownWorkbench.vue:41-46`；gpui `text/format/markdown.rs:1063-1068`；`TextView` 自带 CommonMark 行为即可）。第十五轮由 ⚠️ 改判 |
| 10 | 各节末尾的「视觉细节」行 | 共 5 行：框架（`WorkflowView.vue:598-606,689-759` 等）、第一步（`Step1Prepare.vue` 顶栏/工具行/素材卡样式）、编辑器预览（`MarkdownWorkbench.vue:449-519,117-179`）、store 无、media composable 无 | 阴影/渐变/动画/字号/宽度，逐条对齐无意义 |

---

## 附：本轮改了哪些文件

只改了两个（都在允许范围内），`generation.rs` **未改**（审计后未发现需要当场修的项；store 的三个差异都是既有有意偏离，已记在 parity.md 与 `generation.rs:15-28` 的模块文档里）；`docs/native/parity.md` 与 `src/` 下的原版前端**一字未动**：

- `apps/huanshu-native/src/pages/workflow/mod.rs`
- `apps/huanshu-native/src/pages/workflow/step1.rs`（另有两条纯注释更新：`editor_est_max_scroll` 与 `sync_preview_wheel` 的文档注释，写成 B-2 的查证结论，便于下一个人接手）

（`rustfmt --edition 2021` 已跑过两个文件，`--check` 干净。未跑 `cargo build/check/test`：共享 target 目录，由集成方统一验证。）
