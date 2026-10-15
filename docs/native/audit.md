# 原生界面逐页审计（第十四轮逐页 + 第十五轮修复与归并，对照 Tauri + Vue 原版）

方法：每个原版界面文件按模板从上到下列出每一个用户能看到或操作的东西（文字、按钮、输入、提示、
禁用/显示条件、快捷键、悬停/拖放、确认对话框、轻提示）与 `<script>` 里每一个副作用（调用了哪个
命令、写了什么存储、何时触发），逐项对照原生实现。小差距当场修（状态 🔧），大差距记 ⚠️
（工作量估计见各节），有意不做的记 ➖ 并附原版行号依据。主题相关的纯视觉细节（阴影、渐变、毛玻璃、
动画）不逐条列，每个文件归一行「视觉细节」。

**第十五轮更新**：修掉一批功能性 ⚠️ 并翻成 🔧（首页 PDF 结果对话框整节、第二步的「静态状态」与
「选中页跨步骤保留」、设置页的「返回来源」与「读失败提示」、第三步的「保存按钮固定底栏」）；
纯表现层的 ⚠️ 一次性归档为 ➖「表现层，按 GPUI 原生做法」（理由只写这一句）；向导的模型校验标为
**有意偏离：提前发现模型配置错误**；KaTeX 数学公式保持 ⚠️（另起一轮，需 KaTeX 等价排版/位图管线）。
本总览与七个分节同口径。

分节文件（本文末尾为各节全文）：

- `audit-home.md` — 首页与全局组件（HomeView + ExportDialog/PasswordDialog/PdfExportDialog/SkeletonCard/BrandMark/ThemeToggle/TitleBar）
- `audit-settings.md` — 设置页（SettingsView + settings.css + SystemFontPicker）
- `audit-wizard.md` — 首次向导 + App 外壳/路由（FirstRunWizard + App.vue + main.ts）
- `audit-workflow-step1.md` — 工作流框架 + 第一步（WorkflowView + Step1Prepare + MarkdownWorkbench + stores/generation.ts）
- `audit-step2.md` — 第二步「AI 生成」（Step2Generate + RetryLayoutDialog + stores/generation.ts）
- `audit-step3.md` — 第三步「编辑完善」（Step3Editor + useSlideEditor + SlideFrame）
- `audit-present.md` — 演示覆盖层（PresentationOverlay + `.pres-*` 样式）

## 总览

| 分节 | 条目 | ✅ | 🔧 | ⚠️ | ➖ |
|---|---|---|---|---|---|
| 首页与全局组件 | 114 | 70 | 21 | 0 | 23 |
| 设置页 | 115 | 81 | 21 | 0 | 13 |
| 向导与外壳 | 81 | 63 | 9 | 0 | 9 |
| 工作流框架 + 第一步 | 123 | 94 | 11 | 3 | 15 |
| 第二步 | 126 | 112 | 6 | 0 | 8 |
| 第三步 | 105 | 90 | 5 | 0 | 10 |
| 演示覆盖层 | 22 | 14 | 4 | 0 | 4 |
| **合计** | **686** | **524** | **77** | **3** | **82** |

计数按各分节表格里逐行的状态记号统计（第十五轮重数过一遍：第二步原写 120 行、✅107 有笔误，按逐行实为
126 行、✅112；演示覆盖层原写 24 行，按分节表格实为 22 行；本表已按逐行实计更正）。

🔧 合计 77 条（其中第十五轮把 16 条 ⚠️ 翻成 🔧：首页 PDF 结果对话框 11、设置页 2、第二步 2、第三步 1；
其余 61 条来自第十四轮及更早），清单见各分节末节。
⚠️ 合计 3 条是后续轮次的输入：工作流第一步的「文件拖入悬停（框架缺 drag-enter 钩子）」、
「编辑区↔预览区滚动同步（含滚动条拖拽）」、「预览的 KaTeX 数学公式（另起一轮）」；
本轮另有 20 条表现层 ⚠️ 归档为 ➖「表现层，按 GPUI 原生做法」（首页 7、设置页 5、向导 2、
工作流第一步 1、第二步 1、第三步 4），它们不再占 ⚠️ 计数。


---

## 首页与全局组件逐页审计（原版 Vue → 原生 GPUI）

范围：首页与其用到的全局组件。原版 `src/views/HomeView.vue`、`src/components/{ExportDialog,PasswordDialog,PdfExportDialog,SkeletonCard,BrandMark,ThemeToggle,TitleBar}.vue`
→ 原生 `apps/huanshu-native/src/{pages/home.rs,password_dialog.rs,export.rs,widgets.rs}`。

- 状态：✅ 一致　🔧 本轮已修　⚠️ 有差距未修（附估计工作量）　➖ 有意不做（附原版行号依据）
- 截图：`docs/native/reference/home*.png` 对照 `docs/native/shell/home-0.png`（原版 2160×1350、原生 2182×1361，都按 1.5 倍缩放换算成逻辑像素测量）
- 主题相关的纯视觉细节（阴影、渐变、毛玻璃、动画、字号亚像素）不逐条列，每个文件末尾归一行「视觉细节」。

### 1. `HomeView.vue` → `pages/home.rs`

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 挂载时刷新项目列表 | HomeView.vue:34 | home.rs::HomePage::new → reload | ✅ | `projects::list_projects` |
| 挂载时预热图标向量库 | HomeView.vue:35 | home.rs::HomePage::new | 🔧 | 原版 `invoke("ensure_icon_embeddings").catch(() => {})`；原生漏了这一步（工作流页有、首页没有），本轮照 `run_async` 补上，结果与错误都忽略 |
| 面包屑「项目」 | HomeView.vue:177 | home.rs::render（top_bar） | ✅ | 字号 13 vs 原版 0.75rem=12，见「视觉细节」 |
| 主题按钮（`color-palette`、title 切换主题、展开时高亮） | HomeView.vue:183-189 | home.rs::render + widgets::icon_button | ✅ | `selected(show_theme_picker)` 对应 `.active` |
| 主题浮层里的点击不外冒（`@click.stop`） | HomeView.vue:181 | home.rs::render（`.relative()` 那个 div） | 🔧 | 原生缺这一条：根节点在 **mouse down** 关浮层（对应 `@click`），点浮层里的主题圆点时浮层会先被拆掉、点击落不到圆点上。本轮按卡片操作区的同一手法补 `stop_propagation()` |
| 主题浮层的开关（点图标展开 / 点别处收起） | HomeView.vue:186、173 | home.rs::render | ✅ | 展开：按钮 click 取反；收起：根节点 `on_mouse_down` |
| 主题浮层里的 `ThemeToggle` | HomeView.vue:190-194 | home.rs::render → widgets::theme_dots | ✅ | 见 ThemeToggle.vue 一节（缺的「主题」标签单独列了一条 🔧） |
| 操作栏高度 48px | HomeView.vue:381（`.top-bar{height:48px}`） | home.rs::render（top_bar） | 🔧 | 原生原为 44；截图实测原版 48 逻辑像素（`home.png` y=84-36）、原生 44 |
| 浮层与工具行之间的竖分隔 | HomeView.vue:197 | widgets::bar_divider | ✅ | 1×18px |
| 「导入 / 导入中…」按钮 | HomeView.vue:199-207 | home.rs::render + HomePage::import | ✅ | 文案逐字；`disabled=importing` 对应（原版 `:disabled="importing"`） |
| 导入按钮图标 | HomeView.vue:204-206 | home.rs::render | ➖ | 原版写的是 `i-carbon:folder-up`，而 Carbon 集合里没有这个名字（`iconify.ts:5-13` 的集合直接取自 `@iconify-json/carbon/icons.json`，查无 `folder-up`），原版那一支渲染为空；原生用同族的 `folder-add`，是**有意保留的等效图标** |
| 导入进行中的转圈（`renew spin`） | HomeView.vue:204 | home.rs::render | ✅ | 图标换 `renew`；旋转动画见「视觉细节」 |
| 设置图标按钮 → `/settings` | HomeView.vue:208-214 | home.rs::render（`Route::Settings`） | ✅ | title「设置」 |
| 「新建演示」主按钮 → `/new` | HomeView.vue:215-217 | home.rs::render（`Route::Project{id:None}`） | ✅ | |
| 标题区：品牌标 + 「幻述 Studio」+「我的演示文稿」 | HomeView.vue:225-231 | home.rs::render（heading） | ✅ | `BrandMark :size="28"`；小写 Studio 被 CSS `text-transform: uppercase` 抬成 STUDIO，原生直接写「幻述 STUDIO」 |
| 「N 个项目」（仅加载完且非空） | HomeView.vue:233-238 | home.rs::render（`when_some(count.filter(>0))`） | ✅ | |
| 加载骨架：4 张占位卡 | HomeView.vue:242-251 | home.rs::render（`projects == None`） | ✅ | 见 SkeletonCard.vue 一节（条数简化） |
| 空状态文案「还没有任何演示文稿」/「上传 Markdown 文件，AI 自动生成漂亮的演示文稿」 | HomeView.vue:265-268 | home.rs::render | ✅ | 逐字 |
| 空状态里的「新建演示」 | HomeView.vue:269-271 | home.rs::render（`empty-new`） | ✅ | |
| 空状态插画（三层卡片 + 圆形图标底） | HomeView.vue:256-264 | — | ➖ | 纯装饰（原版 `.slide-stack`/`.empty-icon-circle` 只有旋转与阴影）；原生用一个大号品牌标代替 |
| 点卡片进项目 | HomeView.vue:276-282 | home.rs::project_card | ✅ | `Route::Project{id:Some(id)}` |
| 卡片：名称（超长省略号） | HomeView.vue:288 | home.rs::project_card | ✅ | `.truncate()` |
| 卡片：比例徽章（非 16:9 才显示；32:9 / 48:9） | HomeView.vue:290-292 | home.rs::project_card | ✅ | `AspectRatio` 两个非常规值才给 `ratio` |
| 卡片：「N 张幻灯片」徽章 | HomeView.vue:293-295 | home.rs::project_card + widgets::chip | ✅ | |
| 卡片：更新时间 | HomeView.vue:296-298 + stores/projects.ts:74-79 | home.rs::format_date | ✅ | 原版 `toLocaleString('zh-CN', 2-digit…)` 实得 `2026/10/14 20:03`，原生 `chrono %Y/%m/%d %H:%M` 同形 |
| 卡片：导出按钮（title「导出（.keynn / PDF）」/「导出中…」、导出中禁用、`renew` 转圈） | HomeView.vue:301-318 | home.rs::project_card + open_export | ✅ | tooltip 与禁用一致 |
| 卡片：删除按钮（title「删除」） | HomeView.vue:319-326 | home.rs::project_card | ✅ | |
| 卡片上的操作按钮不触发进项目（`@click.stop`） | HomeView.vue:301 | home.rs::project_card | ✅ | 操作行 `on_mouse_down` + `stop_propagation` |
| 删除确认对话框（警示图标、「确认删除」、「删除后无法恢复，确定要删除这个项目吗？」、取消 / 删除、点遮罩=取消） | HomeView.vue:332-348 | home.rs::confirm_delete | ✅ | 文案逐字、`ok_variant(Danger)`；缺居中的大号红色警示图标（见「视觉细节」）；Esc 可关（GPUI Kit 默认，原版没有 Esc 处理） |
| 删除真的删库并刷新 | HomeView.vue:42-47 + stores/projects.ts:69-72 | home.rs::delete | ✅ | `projects::delete_project` + 重新拉列表；原生多一条失败轻提示（原版静默） |
| 打开导出对话框（带项目名） | HomeView.vue:53-60 | home.rs::open_export | ✅ | |
| 导出提交（普通 / 加密） | HomeView.vue:68-74、90-117 | home.rs::export_keynn | ✅ | 保存默认名 `${p.name}.keynn`（原版不净化，原生也不净化） |
| 导出 PDF（先关对话框；`exportingId` 转圈；主题取 `data-theme`） | HomeView.vue:77-88 + composables/usePdfExport.ts:41-66 | home.rs::export_pdf | 🔧 | 补两处：① PDF 默认名遗漏 `safeFileName`（usePdfExport.ts:30-32、44）→ 加了 `safe_file_name`；② 全局互斥（usePdfExport.ts:42 `if (exporting.value) return;`）→ 加 `pdf_exporting` |
| 内容列宽度与内边距 | HomeView.vue:512-519（`.home-body{max-width:860px;padding:1.5rem}`） | home.rs::{BODY_MAX_W,render} | 🔧 | 原生原为 `max-width` 750 + 左右 4 → 内容宽 742；原版 border-box 860 含 24 内边距 → 内容 812。截图实测（同一行扫卡片边缘）原版 811、原生 741，本轮改成 860/24 |
| 导入：选文件（`.keynn`） | HomeView.vue:120-122 | home.rs::import | ➖ | 原生过滤器名用中文「幻述项目」，原版是 `Keynn Archive`（HomeView.vue:121）；文件对话框里的一行标签，属有意本地化（其它页面同样处理） |
| 导入：读文件 + 判断是否加密 | HomeView.vue:126-131 | home.rs::import（`ImportRead`） | ✅ | `storage::is_encrypted_file` |
| 导入加密文件 → 弹 decrypt 密码框 | HomeView.vue:133-137 | home.rs::open_decrypt_dialog | ✅ | |
| 导入未加密文件 → 直接导入并刷新 | HomeView.vue:139-143 | home.rs::import | ✅ | 原生多一条「导入完成」轻提示（原版静默） |
| 加密导入失败（密码错）不出框、错误显示在框内 | HomeView.vue:152-169 | home.rs::submit_import_password + PasswordDialog::set_external_error | ✅ | 失败保留对话框可重试 |
| 未加密导入失败 | HomeView.vue:145-147 | home.rs::import | ✅ | 原版只 `console.error`，原生是一条错误轻提示（有意改进，不丢信息） |
| 点首页空白关闭主题浮层 | HomeView.vue:173 | home.rs::render（根节点 `on_mouse_down`） | ✅ | 原版用 `@click`，原生用 mouse down（GPUI 里 click 需要按下与抬起落在同一元素） |
| 视觉细节 | — | — | ➖ | 做了：hover 只改边框色、徽章/面板用 `widgets`、骨架与空状态静态元素。按 GPUI 省略：卡片入场动画（`card-in`，HomeView.vue:644-656）、hover 上浮 2px 与阴影、左侧 3px 强调条（:665-679）、骨架微光（SkeletonCard.vue:45-57）、`renew` 旋转（:771-778）、毛玻璃遮罩（:786）。不逐条修的字号亚像素微差：面包屑 13（原 0.75rem=12）、卡片名 15（0.9rem=14.4）、日期 12（0.7rem=11.2）、徽章 11.5（0.65rem=10.4）、标题区字标 11（0.72rem=11.52）；标题区品牌底座 42/圆角 10/灰底（原版 46/14/深色渐变，:464-480）；卡片图标底座 37/9/surface（原版 40/8/primary-bg+primary-border，:681-696）；空状态无插画（:256-264） |

### 2. `ExportDialog.vue` → `home.rs::open_export`（+ `password_dialog.rs`）

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 标题「导出演示文稿」 | ExportDialog.vue:119 | home.rs::open_export | ✅ | |
| 描述「准备导出「X」」 | ExportDialog.vue:120-123 | home.rs::open_export | ✅ | 原版的 else 分支「选择导出方式并确认」（:122）在首页不可达：`handleExport` 总会带上项目名（HomeView.vue:57-59） |
| 右上关闭 ✕ | ExportDialog.vue:111-113 | GPUI Kit Dialog 默认关闭按钮 | ✅ | 原版 12/12，Kit 在 8/8 |
| 点遮罩关闭 | ExportDialog.vue:109 | GPUI Kit Dialog 默认 `overlay_closable` | ✅ | |
| 三个选项：普通导出 / 加密导出 / PDF 文档（标题 + 说明 + 图标） | ExportDialog.vue:126-152 | home.rs::open_export | ✅ | 文案与图标逐字（`document-export` / `locked` / `document-pdf`） |
| 「选中一个选项 + 按主按钮确认」两段式交互、选中态高亮 | ExportDialog.vue:128、137、145 + 210-215 | home.rs::open_export | ➖ | 表现层，按 GPUI 原生做法 |
| 加密面板：`导出密码` + 输入框 + 眼睛 | ExportDialog.vue:157-168 | password_dialog.rs::PasswordDialog（Encrypt） | ➖ | 表现层，按 GPUI 原生做法 |
| 强度条 + 等级文案（太弱…强） | ExportDialog.vue:170-180 | password_dialog.rs::{strength_score,strength_level} | ✅ | 评分规则与 `levels[min(score,4)]` 一致（单测 `strength_scores_follow_the_original_rules`） |
| `确认密码` + 不一致红边 + 「两次输入的密码不一致」 | ExportDialog.vue:183-201 | password_dialog.rs::render | ✅ | 逐字 |
| 提示「密码至少 8 位，需包含大写字母、小写字母和数字。」 | ExportDialog.vue:203-205 | password_dialog.rs::render | 🔧 | 原生两处都没有这句话（`PasswordDialog.vue` 本身没有，只有这里），本轮补在 encrypt 模式的表单尾部 |
| 提交时的强度门槛与错误行 | ExportDialog.vue:93-96、208 | password_dialog.rs::submit | ✅ | `密码必须至少8字符，包含大小写字母和数字`，正则等价（单测 `encrypt_gate_agrees_with_core_password_validation`） |
| 输入框回车提交（`@keyup.enter`） | ExportDialog.vue:164、192 | password_dialog.rs::new/render | 🔧 | 原生原本没有回车提交，本轮补（见 PasswordDialog.vue 一节） |
| 打开时重置表单 | ExportDialog.vue:53-64 | home.rs::open_encrypt_dialog（每次新建对话框实体） | ✅ | 结构上等价 |
| 关掉加密时清空密码 | ExportDialog.vue:66-73 | — | ✅ | 原生没有「同框切格式」这条路径（加密出口固定是新对话框） |
| 「取消」按钮 | ExportDialog.vue:211 | home.rs::open_export（DialogClose） | ✅ | |
| 视觉细节 | — | — | ➖ | 做了：选项行用 `c.panel/surface` + `hover` 边框色、主色图标。按 GPUI 省略：卡片 hover 上浮 1px、`box-shadow: inset` 选中描边、`max-width: 92vw` 与 640px 断点（:475-485）、弹出缩放的进出动画（:454-473） |

### 3. `PasswordDialog.vue` → `password_dialog.rs`

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 标题「设置密码」/「输入密码」 | PasswordDialog.vue:91-93 | password_dialog.rs::render | ✅ | 逐字 |
| 描述「为「X」设置导出密码」/「请输入密码以解密文件」 | PasswordDialog.vue:95-100 | password_dialog.rs::render（`desc`） | ✅ | encrypt 无项目名时不显示描述，与 `v-if` 同 |
| 右上关闭 ✕ | PasswordDialog.vue:82-84 | password_dialog.rs::open + render | 🔧 | 原生原来会**同时出现两个 ✕**（GPUI Kit 自带的 + 内容里自绘的，`step3.rs:628` 是正确写法），本轮 `close_button(false)` 关掉 Kit 那个，保留走 `on_cancel` 的自绘 ✕ |
| 点遮罩关闭（Esc 同理） | PasswordDialog.vue:80 | GPUI Kit Dialog 默认 `overlay_closable` | ➖ | 表现层，按 GPUI 原生做法 |
| 圆形图标（locked；encrypt 绿 / decrypt 蓝） | PasswordDialog.vue:86-89、218-226 | password_dialog.rs::render | ✅ | 颜色字面值与原版一致（0x22c55e / 0x3b82f6 系） |
| `密码` 标签 + 输入框（占位「设置密码/输入密码」）+ 眼睛切换 | PasswordDialog.vue:103-115 | password_dialog.rs::render | ✅ | `masked(true)` + `set_masked` 对应 `type=text/password` |
| 强度条（encrypt 恒定显示，空密码时是空轨道 + 空标签） | PasswordDialog.vue:117-127 | password_dialog.rs::render | ➖ | 表现层，按 GPUI 原生做法 |
| `确认密码` 组（仅 encrypt）+ 不一致红边与错误文案 | PasswordDialog.vue:130-145 | password_dialog.rs::render（`confirm_group`） | ✅ | 逐字 |
| 错误区（`internalError` 优先，其次 `externalError`） | PasswordDialog.vue:21、147 | password_dialog.rs::render（`error`） | ✅ | 底色 `rgba(239,68,68,.1)` 一致 |
| 「取消」按钮 → `close` | PasswordDialog.vue:150 | password_dialog.rs::render | ✅ | 取消回调由调用方给（首页清待导入字节 / 清实体） |
| 主按钮「加密导出」/「解密并导入」，`disabled=!canSubmit` | PasswordDialog.vue:151-157 | password_dialog.rs::render | ✅ | 逐字 + 同一个禁用条件 |
| `canSubmit` 规则（decrypt：非空；encrypt：≥8 且两次一致） | PasswordDialog.vue:43-48 | password_dialog.rs::can_submit | ✅ | |
| encrypt 提交门槛（正则）+ 错误文案 | PasswordDialog.vue:62-66 | password_dialog.rs::submit | ✅ | 与 core `validate_password_strength` 同过同不过（单测） |
| 输入框回车提交（两个框都挂） | PasswordDialog.vue:110、139 | password_dialog.rs::new + render | 🔧 | 原生原本没有：本轮订阅 `InputEvent::PressEnter` 记待办，在 `render` 里用 `spawn_in` 补窗口后提交（提交可能弹原生保存对话框，不能在绘制途中弹） |
| 打开时重置（密码/确认/错误/眼睛） | PasswordDialog.vue:50-57 | home.rs::{open_encrypt_dialog,open_decrypt_dialog}（每次新建实体） | ✅ | 结构等价 |
| `externalError` 由调用方更新（重试不关框） | PasswordDialog.vue:21 + HomeView.vue:164-167 | password_dialog.rs::set_external_error | ✅ | |
| 视觉细节 | — | — | ➖ | 做了：自绘输入框外壳（红边表示不匹配）、4px 强度轨、主按钮图标色。按 GPUI 省略：`transition: width .3s` 的强度条过渡、`.btn { min-width: 100px }`、弹窗缩放进出（:79-161 的 Transition）、`min-width: 360px`（原生固定 400，照 `.password-modal`） |

### 4. `PdfExportDialog.vue` → `export.rs::show_report`

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 「正在生成 PDF…」进行中对话框 + 「首次导出需要扫描系统字体，会稍慢一些。」 | PdfExportDialog.vue:49-55 | export.rs::show_report（进行中框）+ home.rs / step3.rs 的 `begin_export` | 🔧 | 本轮重写：导出前先开「正在生成 PDF…」+ 说明行的框，成功后替换成结果框、失败替换成失败框；首页与第三步都接 `begin_export` |
| 失败态：对话框里「导出失败」+ 错误正文 + 「知道了」 | PdfExportDialog.vue:57-69 | export.rs::show_report（失败框） | 🔧 | 本轮重写：框内「导出失败」+ 错误全文（可滚动）+ 关闭按钮，不再只推轻提示 |
| 「在文件夹中显示」失败时把错误显示在框里 | PdfExportDialog.vue:68-75 | export.rs::show_report | ➖ | 表现层，按 GPUI 原生做法（失败仍走 Kit 轻提示，错误信息不丢） |
| 结果标题「PDF 已导出」/「PDF 已导出，有几处需要留意」 | PdfExportDialog.vue:78-80 | export.rs::show_report | 🔧 | 本轮修：两态标题（有留意项时用「…有几处需要留意」） |
| 结果摘要「{文件名} · {N} 页 · {x.x} 秒」 | PdfExportDialog.vue:81-83 | export.rs::show_report | 🔧 | 本轮修：改成文件名 + `N 页 · x.x 秒`（秒数 `toFixed(1)`） |
| 说明列表：调用方 note（第三步未应用修改） | PdfExportDialog.vue:86-89 | export.rs::show_report（`note` 参数） | ✅ | 原生保留了这条（`information` 图标 + warning 色）；本行登记它对应原版的位置 |
| 说明列表：占位页按版式分组（「{版式}（第 3、5 页）」） | PdfExportDialog.vue:19-25、90-98 | export.rs::show_report | 🔧 | 本轮修：按版式中文名分组（`editor_schema` 的 label），一组一条「{版式}（第 3、5 页）」 |
| 说明列表：裁切页「第 N 页 内容过多，已缩到最小字号仍放不下，超出部分被裁掉。」 | PdfExportDialog.vue:99-102 | export.rs::show_report | 🔧 | 本轮修：补上后半句解释 |
| 说明列表：缺图「第 N 页图片 `引用` 未导出：原因」 | PdfExportDialog.vue:103-106 | export.rs::show_report | 🔧 | 本轮修：一处一条，列页码 / 引用（等宽、主色）/ 原因 |
| 说明列表：主题回落「主题「X」还没有 PDF 版式，已回落到「便签·稿纸」。」 | PdfExportDialog.vue:107-110 + 34-39 | export.rs::show_report | 🔧 | 本轮修：用主题名而非 id（core 新增 `themes::theme_name`、`services::export::FALLBACK_THEME`） |
| 无回落时的常驻提示「PDF 版式跟随当前主题。」 | PdfExportDialog.vue:112 | export.rs::show_report | 🔧 | 本轮补上 |
| 关闭 ✕ / 点遮罩关闭（导出中不可关） | PdfExportDialog.vue:47、58-60、72-74 | GPUI Kit Dialog 默认 | ✅ | 进行中框也能关（原版导出中不可关，见 ➖ 的形态差异）；结果框总能关 |
| 「完成」按钮 | PdfExportDialog.vue:118 | export.rs::show_report | 🔧 | 本轮修：按钮文案改成「完成」（另有「在文件夹中显示」→ core `reveal_in_file_manager`） |
| `dismiss()` 清状态 | PdfExportDialog.vue:77-81 + usePdfExport.ts:77-81 | GPUI Kit 关对话框即回收 | ✅ | 原生没有跨页面的全局 PDF 状态 |
| 视觉细节 | — | — | ➖ | 做了：警示/成功/失败三色图标（`information`/`checkmark`）、等宽显示引用、可滚动列表。按 GPUI 省略：`max-height: 220px` 滚动区、`prefers-reduced-motion` 分支（:292-294）、弹窗淡入淡出、`code` 背景高亮 |
| 整节汇总 | PdfExportDialog.vue 全文件 | export.rs::show_report | 🔧 | 本轮按原版重做：进行中 / 成功 / 失败三框、结果两态标题、摘要「文件名 · N 页 · x.x 秒」、缺图一处一条、占位页按版式中文名分组、裁切页解释、主题回落用主题名、常驻提示、按钮「在文件夹中显示」+「完成」；core 与原生共 8 个新单测。**剩余**：`导出中不可关` 与「在文件夹中显示」失败仍走轻提示（后者已归 ➖） |

### 5. `SkeletonCard.vue` → `home.rs` 内联的骨架

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 骨架行数与形状（首页 `:rows="2"`，末行 60% 宽、行高 13px、行距 10px） | SkeletonCard.vue:8-12、17-26 + HomeView.vue:248 | home.rs::render（`projects == None`） | ➖ | 表现层，按 GPUI 原生做法 |
| 微光动画（`skeleton-shimmer`） | SkeletonCard.vue:45-57 | — | ➖ | 动画细节，GPUI 下不做（与 parity.md 的「卡片入场动画 ➖」同类） |
| 视觉细节 | — | — | ➖ | 骨架底色用 `c.surface`（同原版 `--studio-surface`）、圆角 4px 一致；省略微光 |

### 6. `BrandMark.vue` → `widgets::brand_mark` / `assets/brand-mark.svg`

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 品牌 SVG（3 个渐变 + 7 个图形） | BrandMark.vue:27-120 | icons.rs（`assets/brand-mark.svg`，`img("brand/mark.svg")`） | ✅ | 逐路径核对一致（`#70E1F7…#2090C9`、`#FFD992…#F0A63F`、`#FFF7E9…#F7E4C4`、旋转矩形、45° 方框、`#F4B24B` 圆点、`#59D1F2` 星） |
| `size` 属性（默认 24；各处 16/24/28/30/48） | BrandMark.vue:5-17 | widgets::brand_mark(size) + 各调用点 | ✅ | 标题栏 16、卡片 24、标题区 28、空状态 48（原版 30，见「视觉细节」） |
| `title="幻述"` 悬停提示 | BrandMark.vue:24 | — | ➖ | GPUI 的 `img` 元素没有 tooltip 通道（`widgets::TooltipText` 只给 `Stateful<Div>`）；品牌名在标题栏有文字，信息不丢 |
| `aria-hidden="true"` | BrandMark.vue:25 | — | ➖ | GPUI 无对应的无障碍属性标记，纯装饰图形不参与命中 |
| 视觉细节 | — | — | ➖ | `inline-flex` 居中与 `flex-shrink: 0` 用 `.flex_none()` 表达；`line-height: 0` 在 GPUI 无意义。空状态的品牌标 48（原版 `.empty-icon-circle` 里的 30，HomeView.vue:262） |

### 7. `ThemeToggle.vue` → `widgets::theme_dots`

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 「主题」标签 | ThemeToggle.vue:30 | widgets::theme_dots 的调用方 | 🔧 | 原生 `theme_dots` 只管圆点，标签由调用方给：工作流侧栏有（`workflow/mod.rs:1209`），首页浮层**漏了**（`reference/home-theme-popover.png` 里有「主题」），本轮补上 |
| 六个主题圆点（来自 `APP_THEMES`） | ThemeToggle.vue:5、31-36 | widgets::theme_dots（`themes::APP_THEMES`） | ✅ | 同一份目录 |
| 圆点 title/aria-label = 主题名 | ThemeToggle.vue:37-40 | widgets::theme_dots（`tooltip_text(theme.name)`） | ✅ | aria 部分 ➖（无对应） |
| 点击切换主题并保存 | ThemeToggle.vue:10-14、41 | widgets::theme_dots → `ShellHandle::set_theme(id, true)` | ✅ | 原版写 `data-theme` + localStorage `app-theme`；原生写 `app_settings.app_theme` 并即时换主题（parity.md 全局节 ✅） |
| 当前主题高亮 | ThemeToggle.vue:91-94 | widgets::theme_dots | ✅ | 原版用 `--studio-text` 描边 + 外发光，原生用 `colors.primary` 一圈（视觉细节） |
| 外部改主题时同步（MutationObserver 监听 `data-theme`） | ThemeToggle.vue:16-25 | — | ➖ | 原生没有「DOM 属性」这个变化源：`ShellHandle::set_theme` 自己就是主题的唯一入口，各调用点渲染时都读当前 `cx.studio().theme.id`，等价 |
| 悬停放大 | ThemeToggle.vue:87-89 | — | ➖ | 动画细节（GPUI hover 只改色，缩放要自绘） |
| 视觉细节 | — | — | ➖ | 圆点 20px、内点 8px 一致；原版外层底色用 `theme.swatch`（每个主题一条自定义渐变，`themes/catalog.ts`），原生用该主题的 `tokens.panel` 纯色 + `tokens.primary` 内点，看不到渐变/条纹（如「便签」主题的条纹底） |

### 8. `TitleBar.vue` → `widgets::title_bar`（GPUI Kit `TitleBar`）

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 品牌小底 +「幻述」 | TitleBar.vue:39-42 | widgets::{title_bar,brand_chip} | ✅ | `BrandMark :size="16"` |
| 最小化按钮（title「最小化」） | TitleBar.vue:45-53 | GPUI Kit `TitleBar` | ✅ | 由 Kit 画，走系统命令 |
| 最大化/还原（图标与 title 随状态） | TitleBar.vue:54-95 | GPUI Kit `TitleBar` | ✅ | Kit 用自己的图标与系统 title；原版 `isMaximized` 同步（:9-11、27-30）由 Kit/系统负责 |
| 关闭按钮（hover 红底 `#e81123`） | TitleBar.vue:96-115、200-203 | GPUI Kit `TitleBar` | ✅ | 红底 hover 是 Kit/系统样式 |
| 拖动区、双击最大化、贴靠 | TitleBar.vue:37-38 | GPUI Kit `TitleBar`（Windows 命中测试） | ✅ | parity.md 已注明「贴靠与多屏需人工验收」 |
| 标题栏高度 36px | style.css:2 + TitleBar.vue:126 | GPUI Kit `TitleBar` 默认高度 | ➖ | 表现层，按 GPUI 原生做法（实测原版 36、原生 34，差 2px） |
| 视觉细节 | — | — | ➖ | 品牌底原版是深色渐变 + 内阴影（:147-163），原生 `brand_chip` 用固定深色 `0x111a28` + 细边（同「有意保留的固定深色」思路）；`user-select: none` 在 GPUI 无意义 |

### 统计

| 状态 | 条数 |
|---|---|
| ✅ 一致 | 70 |
| 🔧 本轮已修 | 21 |
| ⚠️ 有差距未修 | 0 |
| ➖ 有意不做 | 23 |
| （合计条目） | 114 |

说明：「回车提交」在 `ExportDialog.vue` 与 `PasswordDialog.vue` 两节各占一行（同一个密码框，实际是一处改动）；「导出 PDF」一行里含两处改动（文件名净化 + 全局互斥），所以 🔧 行为 21 条 = 下方改动表 21 项（第 1–10 项来自第十四轮，第 11–21 项是本轮 PDF 结果对话框重写）。

本页**没有遗留 ⚠️**：第十四轮记的 18 条 ⚠️ 里，11 条（PDF 结果对话框整节）已 🔧；其余 7 条按新原则归入 ➖「表现层，按 GPUI 原生做法」——导出对话框两段式交互与内嵌加密面板（第 2 节 2 条）、遮罩/Esc 关闭走 Kit 原生路径与强度条空轨道（第 3 节 2 条）、骨架行数（第 5 节 1 条）、标题栏高度 34/36px（第 8 节 1 条）、「在文件夹中显示」失败用 Kit 轻提示（第 4 节 1 条）。

### 本轮改动（🔧）

| # | 文件 | 原版位置 | 改了什么 |
|---|---|---|---|
| 1 | pages/home.rs | HomeView.vue:35 | 补挂载时 `ensure_icon_embeddings` 预热（`run_async`，结果忽略） |
| 2 | pages/home.rs | HomeView.vue:181 | 主题浮层容器加 `stop_propagation`（等价 `@click.stop`），否则点主题圆点会被根节点的「点空白关浮层」抢先 |
| 3 | pages/home.rs | ThemeToggle.vue:30 | 主题浮层补「主题」标签 |
| 4 | pages/home.rs | HomeView.vue:381（`.top-bar{height:48px}`） | 操作栏高度 44 → 48 |
| 5 | pages/home.rs | HomeView.vue:512-519（`.home-body{max-width:860px;padding:1.5rem}`） | 内容列 750/内边距 4 → 860/24（截图实测：原版列宽 812、原生 742） |
| 6 | pages/home.rs | usePdfExport.ts:30-32、44 | PDF 默认文件名按 `safeFileName` 过滤 Windows 非法字符（新增 `safe_file_name` + 单测） |
| 7 | pages/home.rs | usePdfExport.ts:42 | PDF 导出全局互斥（新增 `pdf_exporting`） |
| 8 | password_dialog.rs | PasswordDialog.vue:82-84 | `close_button(false)`，去掉重复的 ✕ |
| 9 | password_dialog.rs | ExportDialog.vue:203-205 | encrypt 模式补「密码至少 8 位，需包含大写字母、小写字母和数字。」 |
| 10 | password_dialog.rs | PasswordDialog.vue:110、139 / ExportDialog.vue:164、192 | 两个输入框支持回车提交 |
| 11 | export.rs、pages/home.rs、workflow/step3.rs | PdfExportDialog.vue:49-55 | 进行中框：导出前先开「正在生成 PDF…」+「首次导出需要扫描系统字体，会稍慢一些。」，首页与第三步都接 `begin_export` |
| 12 | export.rs | PdfExportDialog.vue:57-69 | 失败框：「导出失败」+ 错误全文（可滚动）+ 关闭按钮 |
| 13 | export.rs | PdfExportDialog.vue:78-80 | 结果两态标题：「PDF 已导出」/「PDF 已导出，有几处需要留意」 |
| 14 | export.rs | PdfExportDialog.vue:81-83 | 摘要改成「{文件名} · {N} 页 · {x.x} 秒」（秒数 `toFixed(1)`） |
| 15 | export.rs、editor_schema | PdfExportDialog.vue:19-25、90-98 | 占位页按版式中文名分组：「{版式}（第 3、5 页）」 |
| 16 | export.rs | PdfExportDialog.vue:99-102 | 裁切页补后半句「内容过多，已缩到最小字号仍放不下，超出部分被裁掉。」 |
| 17 | export.rs | PdfExportDialog.vue:103-106 | 缺图一处一条：页码 / 引用（等宽、主色）/ 原因 |
| 18 | export.rs、core `themes::theme_name` | PdfExportDialog.vue:107-110 + 34-39 | 主题回落用主题名而非 id（新增 `themes::theme_name`、`services::export::FALLBACK_THEME`） |
| 19 | export.rs | PdfExportDialog.vue:112 | 补常驻提示「PDF 版式跟随当前主题。」 |
| 20 | export.rs | PdfExportDialog.vue:118 | 按钮文案改「完成」（「在文件夹中显示」另接 core `reveal_in_file_manager`） |
| 21 | core + 原生（共 8 个新单测） | PdfExportDialog.vue 全文件 | 上述三框 + 明细的规则单测：core 侧汇总/回落文案、原生侧结果框文案与分组 |

（第 11–21 项是本轮的 PDF 结果对话框重写，明细见第 4 节的逐行表；`export.rs::show_report` 是主要落点。）

---

## 设置页逐页审计（`SettingsView.vue` + `settings.css` + `SystemFontPicker.vue`）

对照对象：原版 `src/views/SettingsView.vue`、`src/views/settings.css`、`src/components/SystemFontPicker.vue`
（外加被它们调用的 `src/stores/systemFonts.ts`、`src/stores/config.ts`、`src/stores/appSettings.ts`、
`src/typography.ts` 的行为）；原生 `apps/huanshu-native/src/pages/settings/{mod,appearance,models,service_form,generation,storage}.rs`。

状态：✅ 一致 / 🔧 本轮已修 / ⚠️ 有差距未修 / ➖ 有意不做（附原版依据）。
「原生位置」一栏写「文件::函数（标识）」；`mod.rs` 等均在 `apps/huanshu-native/src/pages/settings/` 下。
截图对照：`docs/native/reference/settings-appearance.png`、`settings-1..4.png` ↔
`docs/native/shell/settings-{appearance,models,generation,storage,about}-0.png`
（两张截图都是 150 % 缩放，原版窗口 1440×900 逻辑像素、原生 1455×907，可直接比 px）。

**条目统计（115 行）**：✅ 81 / 🔧 21 / ⚠️ 0 / ➖ 13。
分文件：`SettingsView.vue` ✅ 65 / 🔧 13 / ➖ 3（81 行）；
`settings.css` ✅ 4 / 🔧 3 / ➖ 7（14 行）；
`SystemFontPicker.vue` ✅ 12 / 🔧 5 / ➖ 3（20 行）。
第四节把 🔧 归成 **19 条改动**（其中「状态徽章色调」「摘要虚线」各覆盖 2 行）；本轮**没有遗留 ⚠️**
（原 7 条差距：2 条 🔧、5 条按新原则归 ➖「表现层，按 GPUI 原生做法」）；
每个文件末尾的「视觉细节」也是一行（状态按该文件实际情况给）。

主题相关的纯视觉细节（阴影、渐变、毛玻璃、过渡与关键帧动画、被 `#app *` 拍平前后的字重、`aria-*` / `role`）
不逐条列，每个文件末尾一行「视觉细节」说明做了哪些、哪些按 GPUI 省略。

本轮只改了 `apps/huanshu-native/src/pages/settings/` 下的文件，原版前端未动。

---

### 一、`src/views/SettingsView.vue`

#### 模板：侧栏（233–243）

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 「返回工作台」按钮（arrow-left 图标 + 文案，hover 变色） | SettingsView.vue:234；settings.css:46-47 | mod.rs::render（nav 的 `ghost_button`） | ✅ | 文案、图标逐字一致 |
| 「返回工作台」的返回目标 | SettingsView.vue:210-213 | mod.rs::leave + `ShellHandle::leave_settings`（`shell.rs` 的 `Shell.settings_origin`） | 🔧 | 本轮修：`Shell.settings_origin` 记下进设置前的路由，设置页三个出口都走 `leave_settings()` 回来源——从工作流侧栏进来就回**那个项目的那个步骤**（`project:<id>:<step>`，恢复项目与步骤；`show_step` 把步骤同步回路由），没有来源时回 `Home` |
| 「设置」标题 | SettingsView.vue:235；settings.css:48 | mod.rs::render | ✅ | 20px 与 CSS 一致 |
| 五个分区项（图标 + 文案 + 选中态 + hover） | 236-241；settings.css:50-54 | mod.rs::render（遍历 `SettingsTab::ALL`） | ✅ | 顺序、图标（screen / cube / settings-adjust / data-base / help）、文案、选中左侧竖条一致 |
| 版本号 `v{version}` | 242；settings.css:55 | mod.rs::render（`env!("CARGO_PKG_VERSION")`） | ✅ | 两侧都是 v0.4.0 |

#### 模板：主栏与分区头（245–257）

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 分区标题（h1）+ 分区说明 | 248-251；settings.css:61-63 | mod.rs::render + `tab_description` | ✅ | 五段说明逐字；h1 字号本轮改 26→24（见 CSS 一节） |
| 未就绪提示：「正在加载设置…」/「设置加载失败，请重试。」+「重新加载」按钮 | 252-255 | mod.rs::read_settings / reload_settings（失败态）+ core `AppState::{settings_load_error, settings_file_error, reload_settings}` | 🔧 | 本轮修：core 把 `settings.json` 读/解析失败做成真实信号（`state.rs` 的 `settings_load_error` + 磁盘探针 `settings_file_error` + `reload_settings`）；失败态显示「设置加载失败，请重试。」+ 原因 + 「重新加载」按钮，分区不渲染，保存按钮禁用并在 `save()` 里再挡一次；2 个新单测。**loading 窗口仍无**（原生同步读 core，见下一行的 ➖） |
| 未就绪 / 保存中整块禁用（`<fieldset :disabled="!ready \|\| saving">`） | 257 | mod.rs::save / render | ➖ | 原生同步读完即 ready；保存是同一帧内完成的同步 core 调用，没有可观察的 saving 窗口（等价于原版的 transient 禁用） |

#### 模板：外观与字体（259–341）

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 「界面主题」+「点击即可预览，保存后生效」 | 261-264 | appearance.rs::appearance | ✅ | 逐字 |
| 浅色 / 深色两组与组标签、六个主题卡 | 265-283；settings.css:87-93 | mod.rs::theme_group | ✅ | 顺序照 `APP_THEMES`（浅色 终端·纸 / 报刊·钴蓝 / 便签·稿纸；深色 暗·琥珀 / 暗·星图 / 终端·墨），预览用该主题真实 token（`theme::tokens::table`），与原版给预览挂 `data-theme` 同义 |
| 主题卡：点击即预览（不落库） | 269-280 | mod.rs::preview_theme（`set_theme(save=false)`） | ✅ | 只有保存才写 `app_settings` 表 |
| 主题卡：选中勾选徽章 | 277-278；settings.css:108 | mod.rs::theme_card | 🔧 | 本轮补：主色圆 + `carbon("checkmark")`（原来只有主色描边） |
| 「字体」+「已安装 N 种字体」/「系统字体」+「恢复默认」 | 288-292 | appearance.rs::appearance + mod.rs::reset_fonts | ✅ | 标题字号见 CSS 一节（本轮 17→15）；`恢复默认` = 七角色回 `Microsoft YaHei` 且关掉「统一」 |
| 七个字体行：角色名 + 说明 + 触发器；点行切换 previewRole；当前角色高亮 | 293-302 | appearance.rs::appearance | ✅ | 角色名 / 说明 / 顺序照 `roleLabels`、`roleDescriptions` |
| 字体触发器：当前字体名 + 「默认」徽章 + chevron；点击开弹层 | SystemFontPicker.vue:93-98 | appearance.rs::appearance（trigger）+ open_font_picker | ✅ | 打开时把焦点放进搜索框也在本轮补上（见第三、四节） |
| 「此字体未安装，已回退到微软雅黑。」 | SystemFontPicker.vue:99 | appearance.rs::appearance（`missing`） | ✅ | 逐字，条件一致（已加载但查不到该字体） |
| 「统一所有字体」开关 + 副标题「开启后，修改任一用途会同步到全部用途」 | 305-308；settings.css:123-125 | appearance.rs::appearance（unify）+ mod.rs::toggle_unified | ✅ | 开启时用「正文」铺满全部角色（照原版 `toggleUnified`）；单测 `toggling_unified_on_floods_body_value` |
| 预览卡：两个标签「演示预览」/「界面预览」 | 312-315 | appearance.rs::font_preview_card | ✅ | 选中态配色照 CSS 130 |
| 演示预览内容（`01 / IDEAS`、让想法清晰呈现、从灵感到表达、主色分隔线、两行正文、`幻述 Studio · 2026`） | 317-325 | appearance.rs::font_preview_card（slides） | ✅ | 文案逐字；各段用对应角色的字体 |
| 界面预览内容（品牌条 + 幻述 Studio、新的演示文稿、将灵感写下来…、项目名称、我的第一份演示文稿、创建项目） | 326-333 | appearance.rs::font_preview_card（interface） | ✅ | 文案逐字，品牌图 18px |
| specimen 行（角色标签 + `Aa Bb` + `0123456789` + `中文字体示例`） | 334-336；settings.css:144-147 | appearance.rs::font_preview_card（specimen） | 🔧 | 本轮：标签改主色（原版 `--prefs-accent`），`Aa Bb` 16→20px，两个 span 14→13px 且用弱化色（原来全是默认字色） |
| 代码 specimen（`const ideas = 'Hello, 幻述';`） | 337 | appearance.rs::font_preview_card（code_specimen） | ✅ | 用「代码与编辑器」角色字体 |

#### 模板：模型服务（344–450）

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 三类服务卡（图标、名称、副标题、状态徽章、选中态、点击切换） | 345-355；settings.css:164-172 | models.rs::models | ✅ | 三张卡的标题 / 副标题 / 图标照 `services[]`；每个服务各自持有草稿与测试结果 |
| 服务卡状态徽章五态（测试中 / 已连接 / 连接失败 / 待测试 / 未配置） | 353；settings.css:173-180 | models.rs::status_chip + service_form.rs::ServiceState::tone | 🔧 | 本轮照 CSS 改色调：只有 `is-ok` / `is-failed` 换色，`is-testing`（含圆点转主色）/ `is-untested` / `is-unset` 都是默认弱化底（原来给待测试上了警示黄、测试中上了主色底） |
| 表单标题 + 模型名徽章（`model.split('/').pop()`） | 358-361 | models.rs::models | ✅ | 模型为空不显示徽章 |
| 「服务地址」+ placeholder `http://127.0.0.1:1234` + 帮助 | 363-367 | service_form.rs::Render（`url_row`） | ✅ | 逐字 |
| 「API Key」+ 显示 / 隐藏按钮 + 帮助「配置保存在本机。」 | 369-380 | service_form.rs::Render（`key_row` + 眼睛按钮） | ✅ | 按钮提示文案「显示 API Key」/「隐藏 API Key」逐字；按钮图标 `view` / `view-off` 与原版同名 |
| 「模型」：下拉（无候选时「请选择模型」）/ 手动输入切换 | 384-389 | service_form.rs::Render（`manual` / `model_select`） | ✅ | 原版是 `<select>` + 只有一个「请选择模型」`<option>`；原生用 GPUI Kit `Select` 的占位符，等价 |
| 「获取列表」（图标 + 文案；禁用在「测试中」或地址为空；测试中图标转圈） | 390-392 | service_form.rs::Render（`fetch`）+ test_connection | ✅ | 禁用条件逐字对照；转圈动画见视觉细节 |
| 模型提示 +「手动输入模型 ID」/「从列表选择」链接 | 394-397 | service_form.rs::Render（`toggle-manual`） | ✅ | 文案逐字；原版链接是主色 + hover 下划线，原生是 ghost 按钮（见视觉细节） |
| 「网络代理」开关 +「通过 SOCKS5 代理连接」/「直接连接服务」 | 401-407 | service_form.rs::Render（`proxy_switch`） | ✅ | 文案随开关切换 |
| 代理地址 / 用户名 / 密码（仅开关打开时显示；placeholder 与帮助逐字） | 408-424 | service_form.rs::Render（`fields`） | ✅ | 密码框始终掩码（原版同） |
| 连接状态卡：标题 + 状态徽章 | 429-432 | models.rs::models | ✅ | 徽章白名单同上（🔧 行） |
| 摘要三行：服务类型（本地服务 / 远程服务 / 地址未填写）、模型（未选择）、响应耗时（`N ms` / `—`） | 433-437 | models.rs::models + service_form.rs::`service_location` | ✅ | `service_location` 的 host 判定照原版 `new URL(...).hostname`，单测 `service_location_classifies_hosts` |
| 摘要行之间的虚线分隔 | settings.css:206-210 | models.rs::models | 🔧 | 本轮补 `border_t_1 + border_dashed` |
| 「测试连接」按钮（禁用条件、文案「正在测试…」、图标转圈） | 438-440 | models.rs::models | ✅ | 禁用条件 = 测试中或地址为空，与原版一致 |
| 失败时的行内错误（`.prefs-inline-error`） | 441；settings.css:217 | models.rs::models | ✅ | 只在签名匹配且 `ok=false` 时显示 |
| 「通过获取模型列表检查连接，模型能力以服务实际支持为准。」 | 442 | models.rs::models | ✅ | 逐字 |
| 「这个服务用于」+ 三条用途 +「各类服务独立配置，互不覆盖。」 | 443-447 | models.rs::models | ✅ | 用途逐字；原版是 `<ul>` 圆点列表，原生画圆点 + 文本（视觉） |

#### 模板：生成设置（452–475）

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 「生成参数」+「自动修复轮数」（说明、「建议 2–4 轮」、数字输入 0–10） | 456-463 | generation.rs::generation | ✅ | 三行文案逐字；输入框宽度 96 vs 92px（视觉） |
| 「生成并发数」（说明、「本地模型建议 1，最多 8」、数字输入 1–8） | 464-471 | generation.rs::generation | ✅ | 同上 |
| 负载提示块（information 图标 + 文案） | 473；settings.css:228-229 | generation.rs::generation | 🔧 | 本轮：移到面板**外**、主色浅底、图标主色（原来在面板内且无底色） |
| 保存前校验：修复轮数 0–10 整数、并发数 1–8 整数，失败提示并跳到本分区 | 157-162 | mod.rs::save + generation.rs::`generation_valid` | ✅ | 文案逐字；输入框里的非数字文本按失败处理（原版 `Number.isInteger` 对空串/NaN 也是失败）；单测 `generation_ranges_match_the_original_rules` |

#### 模板：存储与缓存（477–507）

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 「文件位置」+ 数据目录 / 媒体目录（只读展示 + 帮助） | 480-489 | storage.rs::storage | ✅ | 原版是 `readonly` `<input>`，原生是同样样式的文本块（只读语义一致） |
| 「图标向量缓存」+ 状态徽章（已初始化 / 模型已变更 / 未初始化） | 492-495 | storage.rs::`embedding_status` + `status_badge` | 🔧 | 本轮：未初始化改用默认弱化底（原版 494 行给的是 `is-untested`，而 settings.css 没有这条配色） |
| 「为图标建立语义索引，提高内容与图标的匹配效果。」 | 496 | storage.rs::storage | ✅ | 逐字 |
| 摘要两行：当前向量模型（未配置）/ 缓存对应模型（—） | 497-500 | storage.rs::storage | ✅ | 「缓存对应模型」的虚线分隔本轮补上（🔧，见模型服务那行） |
| 「初始化缓存」按钮：禁用 = 初始化中 / 已就绪 / 有未保存的模型配置；三态文案 | 501-504 | storage.rs::storage + init_embeddings | ✅ | 禁用条件与文案逐字；`configDirty` 时右侧提示「请先保存模型配置，再初始化缓存。」 |
| 初始化过程中的消息（正在初始化… / 完成 / 错误） | 505 | storage.rs::init_embeddings | ✅ | 三条文案逐字 |

#### 模板：关于与帮助（509–525）

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 品牌图 44px +「幻述 Studio」+「版本 {version}」 | 512-515 | mod.rs::about | ✅ | 逐字 |
| 「将 Markdown 文档自动转化为精美的演示文稿。」 | 516 | mod.rs::about | ✅ | 逐字 |
| 说明与「开始使用」之间的分隔线 | 517；settings.css:240 | mod.rs::about | 🔧 | 本轮补 1px 分隔线 |
| 「开始使用」+ 三步说明 | 518-523 | mod.rs::about | ✅ | 三段逐字；原版是 `<ol>` 数字序号，原生是圆形序号（视觉） |

#### 模板：底部保存条（530–541）

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 状态点 + 四态文案（正在保存… / 更改已保存 / 有未保存的更改 / 所有更改已保存） | 531-533 | mod.rs::render | ✅ | 文案与优先级逐字 |
| 脏且未保存时的「Ctrl + S」提示 | 534；settings.css:254 | mod.rs::render | 🔧 | 本轮补（原来只有文案、没有这个提示块） |
| 状态点颜色：脏时主色、平时成功色 | settings.css:251-252 | mod.rs::render | 🔧 | 本轮 `warning` → `primary` |
| 保存失败行 | 536；settings.css:256 | mod.rs::render | ✅ | 文案 `保存未完成：{error}。未保存的修改仍保留，可重试。` 逐字 |
| 「撤销更改」（禁用 = 不脏或保存中） | 538 | mod.rs::render + revert | ✅ | 禁用条件一致 |
| 「保存更改」（禁用 = 不脏 / 未就绪 / 保存中；保存中显示「保存中…」） | 539 | mod.rs::render + save | ✅ | 原生没有 `config.saving` 这一路（同步保存），其余条件一致 |

#### 模板：离开确认（544–554）

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 遮罩点击 = 继续编辑（`@click.self="answerLeave('stay')"`） | 544 | mod.rs::leave（GPUI Kit `Dialog` 遮罩） | ✅ | 点遮罩关掉对话框即「继续编辑」 |
| 标题「有未保存的更改」+ 说明「离开前要保存吗？放弃后，主题与字体会恢复为上次保存的样式。」 | 546-547 | mod.rs::leave | ✅ | 逐字 |
| 「放弃更改」/「继续编辑」/「保存并离开」（放弃在左、保存并离开为主按钮） | 549-551；settings.css:263-264 | mod.rs::leave | ✅ | 放弃 = `revert` 后离开；保存并离开 = 保存成功（不再脏）才离开，失败留在本页看错误——与原版 `return !dirty.value` 同义 |
| 打开确认框时焦点落在「保存并离开」 | 191-201（`leaveSaveButton.focus()`） | mod.rs::leave | ➖ | 表现层，按 GPUI 原生做法 |

#### 脚本副作用

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 打开页面：`config.load()` → `invoke('get_settings')` | 135-146、220 | mod.rs::new（`services::settings::get_settings`） | ✅ | 原生在构造时同步读；读/解析失败现在会走失败态（见主栏那行的 🔧，core `settings_load_error` / `reload_settings`），不再静默回落默认值 |
| 打开页面：`appSettings.load()` → `invoke('get_app_settings')` | 135-146、220 | mod.rs::new（`services::storage::get_app_settings`） | ✅ | 目录展示、向量缓存状态、字体基线都来自它 |
| 打开页面：`systemFonts.load()` → `invoke('list_system_fonts')` | 221；stores/systemFonts.ts:13-32 | mod.rs::load_fonts（`run_bg`） | ✅ | 同步重活（DirectWrite）走后台线程；**空列表算失败**也在本轮补上（第三、四节） |
| watch(fontDraft) → `applyFonts()`：写 `--font-*` 与 `--font-sans` | 68；typography.ts:51-58 | mod.rs::fonts_changed（`theme::apply`） | ✅ | 原生只实时应用「界面」角色（其余角色只在幻灯片里生效，页面里靠预览卡体现），有意收窄 |
| watch(themeDraft) → `documentElement.dataset.theme`（实时预览） | 69 | mod.rs::preview_theme | 🔧 | 本轮修：`set_theme` 会用**已保存**的界面字体重刷主题，换主题会把字体预览冲掉；现在换完主题补回草稿字体 |
| watch(activeSection) → 关掉字体弹层 | 70 | mod.rs / appearance.rs::open_font_picker | ✅ | 原生弹层是模态对话框，打开时切不了分区，无需关闭 |
| watch(activeService) → 收起 API Key 明文 | 103 | service_form.rs（每个 `ServiceForm` 自带 `show_key`） | ✅ | 一个服务一个表单实体，切换即各自的状态 |
| watch(dirty) → 撤回「更改已保存」 | 71 | mod.rs（各处改动里 `just_saved = false`） | ✅ | |
| `testConnection()` → `invoke('list_models', {target, service})`，记录签名的耗时与结果 | 104-116 | service_form.rs::test_connection（`run_async`） | ✅ | 用草稿的 serde 签名代替 `JSON.stringify`，迟到结果按签名作废；耗时 `Instant` 对应 `performance.now()` |
| 取回列表后替换 `modelLists[target]` | 111 | service_form.rs::test_connection | 🔧 | 本轮：改成整份替换（原来是与上一次的并集，列表只会越滚越多） |
| `initEmbeddings()` → `invoke('ensure_icon_embeddings')` + `appSettings.load()` | 123-134 | storage.rs::init_embeddings（`run_async`） | ✅ | 成功后重读 `app_settings` 刷新状态；`configDirty` / 初始化中直接 return |
| `save()` → `save_settings` / `save_app_settings` / `localStorage['app-theme']` | 155-185 | mod.rs::save（`settings::save_settings` + `storage::save_app_settings` + `ShellHandle::set_theme(save=true)`） | ✅ | 顺序照原版（模型 → 字体 → 主题）；失败文案逐字、草稿保留可重试；主题落 `app_theme` 表（原版落 localStorage，都是「下次启动生效」） |
| `save()` 成功：`saved=true` 并在 3 秒后复位 | 180-182 | mod.rs::save（`background_executor().timer`） | ✅ | 计时用 GPUI 执行器，不是 tokio；页面已销毁时静默丢弃 |
| `revert()`：只改内存（草稿、字体草稿、主题草稿、统一开关、saveError） | 147-154 | mod.rs::revert | ✅ | 同时把数字输入框与三个服务表单实体同步回基线 |
| `onBeforeRouteLeave`：脏 → 拦截并等用户选 | 198-209 | mod.rs::leave | ✅ | 三态语义一致 |
| `onKeydown`：Ctrl/Cmd+S 保存；确认框里 Esc = 继续编辑 | 214-217 | mod.rs::on_key + `Dialog` 的 Esc | ✅ | 原生只认 control（Windows 上与原版等价）；Ctrl+S 在输入框里也生效（事件从焦点冒泡到页面根） |
| `onBeforeUnmount`：恢复已保存的字体与主题 | 223-228 | mod.rs::leave / revert | ✅ | 放弃时 `revert` → `preview_theme(已保存)` → `set_theme` 会读已保存的界面字体，等价于原版的重放 `applyFonts(appSettings.settings.fonts)` |
| 视觉细节 | 232-341 各处 | — | ➖ | 做了：主题卡缩略图（工具栏三点 + 侧栏 + 内容块 + 主按钮）、预览卡标签选中态、字体行 hover / 高亮、服务卡选中描边。按 GPUI 省略：`.prefs-slide-preview` 的右下装饰弧线与 `isolation`、主题卡 hover 上移 1px、字体行高亮的左侧内阴影条、`kbd`/按钮的 transition、页面入场与 `prefs-spin` 转动（原生转圈是静态图标） |

---

### 二、`src/views/settings.css`

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 颜色全部来自 `--studio-*`（含 `--prefs-muted` 的 `color-mix` 提亮、成功 / 危险三件套） | settings.css:6-27 | theme/tokens.rs + `cx.studio().colors` | ✅ | 原生同源；`--prefs-muted`（muted 与 text 混合）用在 12px 帮助文字上，原生直接用 `colors.muted` |
| 侧栏宽 220px（≤1200px 时 192 / 168） | settings.css:30、268、278 | mod.rs::render（`w(px(204.0))`） | ➖ | 表现层，按 GPUI 原生做法 |
| 内容区 `max-width: 1280px; margin: 0 auto`、内边距 28/32 | settings.css:60 | mod.rs::render（`px(30) py(28)`） | ➖ | 参考窗口 1440 逻辑像素、可用宽 1220 < 1280，原版也不会居中；只在 >1500px 窗口下才看得出，按 GPUI 省略 |
| 页头 h1 24px、分区标题 15px、面板 h2 16px | settings.css:62、66、183 | mod.rs::render、appearance.rs::appearance | 🔧 | 本轮 h1 26→24、「字体」/「界面主题」17→15（面板 h2 本来就是 16） |
| 按钮（高 36、内边距 7/16、hover 描边、active 下沉、禁用 45% 透明、主按钮主色） | settings.css:72-80 | widgets.rs::primary_button / ghost_button + GPUI Kit `Button` | ✅ | 形态由 GPUI Kit 决定（同高、同圆角、hover / disabled 由主题映射），active 下沉位移省略 |
| 输入框（高 38、圆角 6、focus 主色描边 + 3px 光环、placeholder 弱化色、readonly 弱化） | settings.css:189-195 | GPUI Kit `Input` / `Select` + theme::apply | ✅ | 尺寸与配色同源；focus 光环由 Kit 画 |
| 开关 38×22（「统一所有字体」与「网络代理」） | settings.css:158-161 | GPUI Kit `Switch` | ✅ | 形态一致（同色映射） |
| 徽章 `.prefs-status` 五态配色 | settings.css:173-180 | models.rs::status_chip / service_form.rs::tone / storage.rs | 🔧 | 本轮照 CSS 修（待测试 / 未配置 / 测试中回默认弱化底，测试中只把圆点画成主色）；`is-testing i` 的 `prefs-pulse` 闪烁省略（视觉） |
| 摘要行 / 设置行的分隔线（虚线 / 实线） | settings.css:208、222 | models.rs::models、storage.rs::storage | 🔧 | 本轮补摘要虚线；「设置行」下实线原生用行间距代替（视觉） |
| 主题预览卡 136×84、hover 上移、选中双环 | settings.css:91-97 | mod.rs::theme_card（124×78 + 2px 主色描边） | ➖ | 表现层，按 GPUI 原生做法（尺寸差 12×6px、选中用单圈描边 + 勾选徽章） |
| 字体表 / 预览卡圆角与底 | settings.css:112-131 | widgets::panel + appearance.rs | ➖ | 原版「字体」区**没有**外层卡片（表自己带边框、`统一所有字体` 自己是张卡）；原生把它们装在一个面板里，见视觉细节 |
| 动画与过渡（transition、`prefs-spin`、`prefs-pulse`、`transform`） | settings.css:46、74、83-84、96、179-180 等 | — | ➖ | 视觉细节，GPUI 下不做（转圈 / 抖动都是静态图标） |
| 遮罩毛玻璃（`backdrop-filter: blur`）与卡片阴影 | settings.css:259-260 | GPUI Kit `Dialog` / `AlertDialog` | ➖ | 视觉细节，交给 GPUI Kit 的默认遮罩与阴影 |
| 响应式断点 1200 / 960 / 640（单列、隐藏图标、底部条换行…） | settings.css:266-311 | — | ➖ | GPUI 没有媒体查询；原生按一个固定布局排（窄窗口下靠 `flex_1` 挤压），窗口最小尺寸由外壳决定 |

---

### 三、`src/components/SystemFontPicker.vue`

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 触发器：当前字体名（找不到时回落默认名 / 原值）+「默认」徽章 + chevron-down | SystemFontPicker.vue:93-98 | appearance.rs::appearance（trigger）、`trigger_label` | ✅ | 「默认」只在值为 `Microsoft YaHei` 时显示（`DEFAULT_FONT` 一致）；触发器禁用（`!ready \|\| saving`）在原生没有可观察窗口（➖，见主栏） |
| 「此字体未安装，已回退到微软雅黑。」 | 99 | appearance.rs::appearance（`missing`） | ✅ | 逐字 |
| 遮罩 + 点击遮罩关闭 | 101 | appearance.rs::open_font_picker（`dialog.overlay(true)`） | ✅ | |
| 头：`选择{label}` +「先预览，再应用到设置」+ 关闭按钮 | 104-106 | appearance.rs::FontPicker::render（header） | ✅ | 七个 label（界面字体 / 标题字体…）与 `fontRoles[].label` 逐字；关闭按钮 aria「关闭字体选择」→ 原生 tooltip 同文案 |
| 搜索框：放大镜 + placeholder「搜索中文或英文名称」+ 有词时的清空按钮 | 110-116 | appearance.rs::FontPicker::render（search 行） | ✅ | 清空按钮 aria「清空搜索」→ tooltip 同文案 |
| 列表 meta：「常用优先 · 全部字体」/「搜索结果」+ 命中数量 | 117 | appearance.rs::FontPicker::render（`meta_label`） | ✅ | 逐字；命中数取 `filtered.len()` |
| 读取失败 / 空列表的错误提示（`systemFonts.error`，主色小字） | 118 | appearance.rs::FontPicker（`error`）+ mod.rs::load_fonts | 🔧 | 本轮补：原来读失败完全没提示（且空列表当成功） |
| 列表空态：「正在读取系统字体…」/「没有找到字体，试试其他名称。」/「未读取到字体，请刷新重试。」 | 120-121 | appearance.rs::FontPicker::render（`list_content`） | 🔧 | 本轮修：读失败（error 有值）不再永远停在「正在读取系统字体…」，而是走空态第三条 |
| 列表项：字体名（溢出省略）+「当前」徽章 + 勾选列 | 122-127 | appearance.rs::FontPicker::render（选项行） | ✅ | 「当前」用 `findSystemFont` 判定（name/label/aliases 全等） |
| 候选列表的排序（无搜索词时「常用优先 + 其余按名称」） | 26-31；stores/systemFonts.ts:22 | appearance.rs::`filtered_font_indices` / `common_names` | ➖ | 表现层，按 GPUI 原生做法（原版按 `label.localeCompare(…,'zh-CN')` 拼音序排尾段，原生按英文名序；要对齐得引入中文 collator） |
| 点列表项：设为高亮项 **并把焦点还给搜索框** | 124（`activeIndex = index; searchInput?.focus()`） | appearance.rs::FontPicker::render（行 `on_click`） | 🔧 | 本轮补焦点回填（原来点完一行要再点一次搜索框才能继续打字） |
| 列表底部：「系统字体」+「刷新」（`systemFonts.load(true)`，读取中禁用且文案变「读取中…」） | 129-131 | appearance.rs::FontPicker::refresh（`run_bg`） | ✅ | 重新枚举后同步回设置页的「已安装 N 种字体」，并清掉错误 |
| 预览区：「效果预览」+ 候选名 + 别名 + specimen（用途示例 + `Aa Bb Cc` / 数字）+ 说明 | 133-138 | appearance.rs::FontPicker::render（preview） | ✅ | specimen 两段都用候选字体；七段用途示例文案逐字（`fontRoles[].sample`） |
| 底部：「所有用途使用此字体」勾选 +「取消」/「应用字体」/「应用到全部」 | 140-144 | appearance.rs::FontPicker::render（footer） | ✅ | 勾选时按钮文案变「应用到全部」；无候选时禁用；「应用到全部」= 一次性铺满七个角色 |
| 打开时：清空搜索词、重置「所有用途」、把高亮定位到当前字体、**聚焦搜索框** | 43-52 | appearance.rs::open_font_picker / FontPicker::new / fonts_changed | 🔧 | 前三条本来就等价（每次打开新建 `FontPicker`）；聚焦搜索框本轮补 |
| 键盘：↑ / ↓ / Home / End 在候选里移动 | 63-69 | appearance.rs::FontPicker::on_key | ➖ | 调查后不改（gpui-kit 0.7.3 源码证据）：① `List` / `Select` / `Combobox` / `Command` 把**点击与回车合并成同一个确认动作**（`list.rs:503-514` 等），点击即应用并关弹层，与原版「点击只高亮、回车才应用」（`SystemFontPicker.vue:124` vs `:70-73`）冲突；② 这些组件的列表上下文都不绑 `home`/`end`（`list.rs:26-35`）；③ 搜索时的高亮重置语义也退化。故不换组件、保持现状——搜索 ✅、回车应用 ✅、Home/End ❌（原生侧原因为 GPUI 单行输入框把这些键绑成光标移动且不 `propagate()`，弹层收不到） |
| 键盘：Enter 选中并应用 | 70-73 | appearance.rs::FontPicker::on_key | 🔧 | 本轮实现（单行输入框对 Enter 会 `cx.propagate()`，事件能冒泡到弹层）；与点击「应用字体」同一条路径 |
| 键盘：Esc 关闭弹层 | 76-79 | GPUI Kit `Dialog` | ✅ | |
| 键盘：Tab / Shift+Tab 在弹层内循环 | 80-87 | — | ➖ | GPUI 是焦点驱动的 Tab 顺序（没有 DOM 的 tabindex 循环），有意不做 |
| 视觉细节 | 151-211 | — | ✅ | 弹层宽 760、左右两栏（目录 / 预览）、列表可滚动 300px、选项主色底 + 勾选、刷新按钮带 renew 图标都做了；省略：`backdrop-filter` 毛玻璃、`box-shadow`、选项 hover 过渡、`@media (max-width:600px)` 的单列窄屏布局（GPUI 无媒体查询） |

---

### 四、🔧 与 ⚠️ 明细

#### 🔧 已修（19 处）

| # | 原版位置 | 改了什么 |
|---|---|---|
| 1 | SettingsView.vue:534 + settings.css:254 | 底部保存条：脏且未保存时补上「Ctrl + S」提示块（`mod.rs::render`） |
| 2 | settings.css:251-252 | 底部状态点：脏时由 `colors.warning` 改为 `colors.primary`（原版 `--prefs-accent`） |
| 3 | settings.css:62 | 分区标题 h1：26 → 24px（`mod.rs::render`） |
| 4 | SettingsView.vue:69 | 换主题预览时补回草稿的「界面」字体：`set_theme` 用的是已保存字体，会把字体预览冲掉；`preview_theme` 现在再 `theme::apply(draft 字体)`一次（`mod.rs::preview_theme`） |
| 5 | settings.css:66 | 「字体」「界面主题」两个小节标题 17 → 15px（`appearance.rs::appearance`） |
| 6 | SettingsView.vue:277-278 + settings.css:108 | 选中主题卡补勾选徽章（绝对定位 18px 主色圆 + `checkmark`）（`mod.rs::theme_card`） |
| 7 | settings.css:144-147 | specimen 行：角色标签改主色，`Aa Bb` 16→20px，两个 span 14→13px 且改弱化色（`appearance.rs::font_preview_card`） |
| 8 | SystemFontPicker.vue:118、120-121；stores/systemFonts.ts:21 | 系统字体读取失败 / 空列表的错误提示与空态（原来失败会永远显示「正在读取系统字体…」且无提示）：`SettingsPage.fonts_error` + 弹层的 `error` 字段、错误小字、空态判定、刷新时清错、空列表按失败处理（`mod.rs::load_fonts`、`appearance.rs::FontPicker::{new,refresh,render}`） |
| 9 | SystemFontPicker.vue:70-73 | Enter 提交：弹层根节点 `on_key_down` → `FontPicker::on_key`（应用当前高亮项并关闭） |
| 10 | SystemFontPicker.vue:124 | 点候选行后把焦点还给搜索框（`state.focus(window, cx)`） |
| 11 | SystemFontPicker.vue:49 | 打开弹层后聚焦搜索框（`open_font_picker` 里 `search.update(…focus…)`） |
| 12 | settings.css:173-180 | 状态徽章色调照 CSS：`ServiceState::tone` 里测试中 / 待测试 / 未配置 → Muted；`models.rs::status_chip` 单独把「测试中」的圆点画成主色；`storage.rs` 的「未初始化」也用默认弱化底 |
| 13 | settings.css:208 | 摘要行之间的虚线分隔（连接状态卡、向量缓存卡）（`models.rs`、`storage.rs`） |
| 14 | SettingsView.vue:111 | 「获取列表」成功后**整份替换**候选列表（原来与旧列表求并集） |
| 15 | settings.css:220 | 生成设置 / 存储与缓存 / 关于三块 `max-width: 880px`（`generation.rs`、`storage.rs`、`mod.rs::about`） |
| 16 | SettingsView.vue:473 + settings.css:228-229 | 负载提示块移出面板、改为主色浅底 + 主色图标（`generation.rs::generation`） |
| 17 | SettingsView.vue:517 + settings.css:240 | 关于卡补 1px 分隔线（`mod.rs::about`） |
| 18 | SettingsView.vue:210-213 | 「返回工作台」回来源：`Shell.settings_origin` 记进设置前的路由（`shell.rs`），三个出口都走 `ShellHandle::leave_settings`——`project:<id>:<step>` 恢复项目与步骤（`show_step` 把步骤同步回路由），无来源回首页（`mod.rs::leave` / `shell.rs`） |
| 19 | SettingsView.vue:252-255 | 设置读失败不再是静默回落：core 把 `settings.json` 读/解析失败做成真实信号（`AppState::{settings_load_error, settings_file_error, reload_settings}` + `services::settings::{read_error, reload_settings}`，磁盘探针负责「文件损坏 / 权限」这类失败），失败态显示「设置加载失败，请重试。」+ 原因 + 「重新加载」按钮、分区不渲染、保存按钮禁用且 `save()` 再挡一次（`mod.rs::{read_settings,reload_settings}` + core）；2 个新单测 |

#### ⚠️ 未修

本轮**没有遗留 ⚠️**：原来的 7 条差距里，2 条已 🔧（「返回工作台」回来源、设置读失败有提示），
其余 5 条按新原则归入 ➖「表现层，按 GPUI 原生做法」：

| # | 项 | 原版依据 | 归 ➖ 的理由 |
|---|---|---|---|
| 1 | 字体弹层的 ↑ / ↓ / Home / End 候选导航 | `SystemFontPicker.vue:63-69` | 表现层，按 GPUI 原生做法（不换 GPUI Kit 列表组件：`List`/`Select`/`Combobox`/`Command` 把点击与回车合并成同一个确认动作、列表上下文不绑 `home`/`end`，换过去反而破坏「点击只高亮、回车才应用」；详见第三节该行） |
| 2 | 离开确认框的初始焦点 | `SettingsView.vue:191-201` | 表现层，按 GPUI 原生做法 |
| 3 | 侧栏宽 204 vs 220px | `settings.css:30` | 表现层，按 GPUI 原生做法 |
| 4 | 主题预览卡 124×78 vs 136×84 | `settings.css:91` | 表现层，按 GPUI 原生做法 |
| 5 | 系统字体的排序规则（拼音序） | `stores/systemFonts.ts:22` | 表现层，按 GPUI 原生做法 |

---

### 五、与 `parity.md` 设置小节的核对

逐条核对 `docs/native/parity.md` 设置一节（42–62 行，17 行，本轮新增「读取设置失败」一行）：

- 「字体：七个角色、可搜索系统字体下拉、统一所有字体、恢复默认 ✅」——实现无误；但「可搜索」这一行**没有提到**
  原版搜索框里的 ↑↓/Home/End 候选导航，本轮调查后判定在 GPUI 里不做（➖ 第 1 条，理由见第四节）。**该行表述偏乐观**，parity.md 已补一句说明。
- 「界面主题卡片：点击预览、保存后生效、撤销 ✅」——本轮发现两处细节当时没做完（选中勾选徽章、换主题会丢字体预览），已 🔧；该行本身不算不实。
- 「底部保存条：状态、撤销、保存 ✅」——同一处：原版还有一个「Ctrl + S」提示块与脏时主色状态点，当时没做，已 🔧。
- 「左侧五个分区、返回工作台、版本号 ✅」——本轮把「返回工作台」改成回来源页（🔧），parity.md 该行已同步。
- 「读取设置失败」是第十五轮新增的一行（✅，parity.md 已加）。
- 其余 12 行（分区标题说明、演示预览与界面预览、三类服务卡片与状态、
  地址与 Key 与模型下拉、SOCKS5、测试连接、生成设置、数据与媒体目录、向量缓存初始化、关于与帮助、
  离开确认、Ctrl+S）与代码一致，未发现「标了 ✅ 其实没做完」的行。
- 需要单独指出的措辞问题：「存储：数据目录、媒体目录展示 ✅」——原版这两个字段是 `readonly` 输入框，原生是只读文本块，
  功能等价，不必改 parity.md。

---

### 六、本轮的验证情况

- **第十四轮只改了** `apps/huanshu-native/src/pages/settings/` 下的 6 个文件（`mod.rs`、`appearance.rs`、`models.rs`、
  `service_form.rs`、`generation.rs`、`storage.rs`）；原版 `src/**` 一行未动，`widgets.rs` / `shell.rs` / `present.rs` /
  `drive.ps1` / `shots.ps1` 未碰。
- 改完对 6 个文件跑了 `rustfmt --edition 2021`（先 `--check` 看差异，只落在我改过的 5 处），随后 `--check` 为 CLEAN；
  rustfmt 能正常解析也说明语法无误。
- **第十五轮的两处 🔧**（返回来源、设置读失败）跨 `pages/settings/mod.rs`、`shell.rs`（`settings_origin` / `leave_settings` /
  `show_step`）与 core（`AppState::{settings_load_error, settings_file_error, reload_settings}`、`services::settings::{read_error, reload_settings}`、
  磁盘探针），共 2 个新单测；由本轮代码负责人验证，审计侧只同步文档。
- 两轮都**没有跑** `cargo build / check / test`（共享 target 目录，按约定由集成方统一验证），也**没有**跑
  `tools/shots.ps1` / `drive.ps1`（会触发编译）。因此 19 处 🔧 都是代码级对齐，**没有真机走查**。
  集成后建议用 `tools/shots.ps1 -Only settings-appearance,settings-models,settings-generation,settings-storage,settings-about`
  重拍对照图，重点看：主题卡勾选徽章、底部「Ctrl + S」提示与状态点颜色、生成设置的负载提示底色、
  存储与关于的 880px 宽度、状态徽章色调。
- 需要人工点的两处：① 打开字体弹层后能否直接打字（第十四轮补的聚焦）；② 弹层里按 Enter 是否应用并关闭（同上）。
  若 Enter 无效，说明该版本 GPUI 在别处吞掉了 Enter，需要退回点击路径。第十五轮新增两处也建议人工走一遍：
  ③ 从工作流侧栏进设置再点「返回工作台」（应回到来源项目的来源步骤）；④ 把沙箱里的 `settings.json` 改成非法 JSON
  再进设置页（应显示「设置加载失败，请重试。」+ 原因 +「重新加载」，保存按钮禁用）。

---

## 逐页审计：首次向导 + App 外壳 / 路由

本轮范围（原版 → 原生，全部逐字逐段读过）：

| 原版 | 原生 | 行数 |
|---|---|---|
| `src/views/FirstRunWizard.vue` | `apps/huanshu-native/src/pages/wizard.rs` | 976 → 901 |
| `src/App.vue` | `apps/huanshu-native/src/shell.rs`（外壳）+ `src/app.rs`（状态） | 48 → 251 / 140 |
| `src/main.ts` | `apps/huanshu-native/src/router.rs` + `src/main.rs` + `src/cli.rs` | 39 → 149 / 112 / 101 |

对照截图：`docs/native/reference/wizard-1..4.png`（原版）、`docs/native/shell/wizard-0.png`（原生，同沙箱、同主题）。

**做法**：每个原版文件从上到下列出用户能看到/能操作的东西（文案、按钮、输入、提示、禁用与显示条件、
快捷键、悬停/拖放、确认框、轻提示），再在 `<script>` 里逐个列出副作用（调了哪个命令、写了什么存储、
何时触发）。「原版没有」与 ➖ 的结论一律贴原版行号。

状态：✅ 一致 ／ 🔧 本轮已修 ／ ⚠️ 有差距未修 ／ ➖ 有意不做（附原因）。
主题相关的纯视觉细节（阴影、渐变、毛玻璃、动画）不逐条列，每个文件归一行「视觉细节」。
下表的「原生位置」行号是改动后的最终状态（`rustfmt` 已跑）。

> 原版截图说明：`reference/wizard-3.png` 与 `wizard-4.png` 都是第 4 步（第 3 步「初始化向量库」没有截图），
> 第 3 步只能按代码逐条对照。

---

### 1. `FirstRunWizard.vue` → `pages/wizard.rs`

#### 1.1 可见项与可操作项

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 卡片（居中、宽 560；第 2 步 960） | FirstRunWizard.vue:218-222, 599-612 | wizard.rs:795-802 `Render` | ✅ | 宽度逐字（560 / 960） |
| 卡片内边距 2.5rem、圆角 16px | :599-609 | wizard.rs:795-799 `Render` | 🔧 | 原为 `widgets::panel` 的 32px / 10px；本轮改 40px / 16px |
| 品牌徽章（56×56、圆角 18、28px 品牌图） | :224-227, 628-644 | wizard.rs:815 `widgets::brand_chip(56,18,28)` | ✅ | 渐变底与阴影见「1.4 视觉细节」 |
| 「HUANSHU STUDIO」小标（主色） | :229, 652-657 | wizard.rs:816-828 | ✅ | 字距按视觉细节省略 |
| 「幻述」品牌名 | :230, 659-664 | wizard.rs:827 | ✅ | |
| 页标题「首次启动配置」 | :233, 666-670 | wizard.rs:830 | ✅ | |
| 副标题「完成以下设置后，即可开始生成和演示你的幻灯片」 | :234-236 | wizard.rs:831-835 | ✅ | 逐字 |
| 四步指示：圆点 1–4（32px、2px 边、数字） | :240-283, 686-699 | wizard.rs:432-487 `Render` | ✅ | 尺寸与编号一致 |
| 圆点 active 态 | :242-245, 701-705 | wizard.rs:457-461 | 🔧 | 原版 CSS 里 `.skipped`（:713）排在 `.active`（:701）/`.done`（:707）之后，同权重后者赢；原生原先让 active 压过 skipped。第 3 步点「跳过（可在设置中后续初始化）」→ 第 4 步 →「上一步」回到第 3 步即可复现（原版该圆点是灰的）。本轮改成 skipped 优先 |
| 圆点 done 态 | :244, 255, 268, 707-711 | wizard.rs:462-466 | ✅ | |
| 圆点 skipped 态（第 2 步跳过 / 第 3 步跳过） | :255, 269, 713-717 | wizard.rs:454-456 | ✅ | 条件 `skipLlm` / `skipEmbedding \|\| skipLlm` 逐条照抄 |
| 步骤连线（48×2）与 done 态 | :249, 260-263, 274-277, 719-728 | wizard.rs:473-485 | ✅ | 三段条件逐条对应：`step>1` / `step>2 \|\| skipLlm` / `step>3 \|\| skipLlm` |
| 第 1 步标题「选择存储目录」 | :288 | wizard.rs:710 | ✅ | 标题值取自 709-720 的匹配表 |
| 第 1 步说明「设置数据库和多媒体文件的存储位置」 | :289 | wizard.rs:710 | ✅ | 逐字 |
| 「数据目录」标签 | :293 | wizard.rs `dir_field`:248 | 🔧 | 原版 `.field label` 不设颜色（:760-763），继承 `--studio-text`（style.css:194-196 的 `[data-theme]`）；原生原为 muted，截图上比原版暗。已改为 `c.text` |
| 数据目录输入框（原版 `readonly`） | :295-299（:296 `readonly`） | wizard.rs `dir_field`:255 | ➖ | 有意：原版只能经「浏览...」修改；原生放开为可编辑（parity.md:68 记为「可编辑」）。多一种输入方式，联动规则不变 |
| 「浏览...」按钮（选目录） | :300-305 | wizard.rs `dir_field`:256-264 → `browse_data_dir`:207 | ✅ | rfd 系统目录对话框 |
| 数据目录提示「存放数据库和项目配置文件」 | :307, 780-784 | wizard.rs `dir_field`:266 | ✅ | muted，与原版一致 |
| 「媒体目录」标签 / 输入框 / 「浏览...」/ 提示 | :310-326 | wizard.rs `dir_field`:248-266 / `browse_media_dir`:222 | ✅ | 与数据目录同构（输入框同样由 readonly 放开为可编辑，见上） |
| 第 2 步标题「配置大模型」 | :332 | wizard.rs:711-714 | ✅ | |
| 第 2 步说明「设置 AI 生成所需的模型服务（可跳过，仅播放幻灯片时无需配置）」 | :333-335 | wizard.rs:711-714 | ✅ | 逐字 |
| 跳过提示条（information 图标 +「跳过此步骤仍可使用幻灯片播放功能，但无法生成新幻灯片」） | :337-340 | wizard.rs:532-552 | ✅ | 逐字 |
| 三个面板标题：大语言模型 / 向量模型 / 多模态模型 | :344, 376, 408 | wizard.rs:515-531（`ModelTarget::ALL`） | ✅ | 顺序一致 |
| 面板字段「API 地址」+ 占位 `http://127.0.0.1:1234` | :345-350, 377-382, 409-414 | `service_form.rs::ServiceForm::compact`（service_form.rs:534-538） | ✅ | 复用设置页表单，占位逐字 |
| 面板字段「API Key (本地留空)」+ 密码框 + 占位 `sk-...` | :352-365, 384-397, 416-429 | service_form.rs:539-550 | ➖ | 表现层，按 GPUI 原生做法（原生是全角「（本地留空）」、且多一个「显示/隐藏 API Key」眼睛按钮，都在 `service_form.rs`） |
| 面板字段「模型」+ 占位（各服务默认模型名） | :366-372, 398-404, 430-436 | service_form.rs:551-557 | ✅ | 占位 = 草稿模型名，清空输入后可见（与原版同） |
| 第 2 步错误提示块（`.error-msg`，`v-if="modelError"`） | :440, 911-919 | wizard.rs:855-868 | ✅ | 文案、配色、圆角一致 |
| 错误提示只在第 2 步渲染 | :440（`v-if` 在第 2 步的 `div` 内，第 3、4 步没有） | wizard.rs:855-868（卡片级） | ➖ | 有意：第 3 步的失败原版另有 `embeddingStatus`（:173）兜底；第 4 步 `finishWizard` 失败原版**完全没有可见反馈**（只有 :212 写 `modelError`）。原生在所有步骤都显示，避免「进入 Studio」点了没反应 |
| 第 3 步标题「初始化向量库」 | :445 | wizard.rs:715-718 | ✅ | |
| 第 3 步说明「正在为图标语义匹配生成向量索引（需要一些时间）」 | :446-448 | wizard.rs:715-718 | ✅ | 逐字 |
| 进度条（8px 高、fill 宽 = 进度%） | :450-456 | wizard.rs:557-572 | ✅ | 进度只有 0/100 两态，与原版相同（原版也只在 :170 置 100） |
| 进度状态文字 | :457 | wizard.rs:573-581 | ✅ | |
| 「开始初始化」按钮（初始化中禁用） | :460-467 | wizard.rs:583-596 | ✅ | 禁用条件 = `isInitializing`，一致 |
| 「开始初始化」按钮的图标：**仅初始化中**显示 `renew` | :465（`v-if="isInitializing"`） | wizard.rs:591 | 🔧 | 原生原先闲置时还画一个 `data-base` 图标（原版没有）→ 改为仅初始化中显示 `renew`（旋转动画见视觉细节） |
| 「跳过（可在设置中后续初始化）」整宽按钮 | :469-477 | wizard.rs:598-605 | ✅ | 文案逐字（含全角括号，:476）；行为 = `skipEmbedding=true; step=4` |
| 第 4 步完成徽章（88×88、圆角 28、52px 品牌图） | :482-486, 864-880 | wizard.rs:644 | ✅ | 渐变与阴影见视觉细节 |
| 「配置完成！」 | :487 | wizard.rs:719 | ✅ | |
| 摘要六行：数据目录 / 媒体目录 / 大模型 / 向量模型 / 多模态模型 / 向量库 | :489-522 | wizard.rs:609-662 | ✅ | 取值规则逐条一致：`skipLlm ? 未配置`、`skipEmbedding \|\| skipLlm ? 未初始化 : 已就绪` |
| 摘要值单行省略、最大宽 280px | :902-909 | wizard.rs:617 | 🔧 | 原生原为 340px → 改 280px |
| 重启警告（warning 图标 +「需要重启应用」+ 说明段），显隐 = `dataDirChanged` | :524-532, 65-69 | wizard.rs:664-702 | ✅ | 与 core 启动时的目录比较 |
| 底栏「上一步」+ `arrow-left`（第 2 步起显示） | :536-542 | wizard.rs:733-737 | ✅ | 图标在文字之前，与原版一致 |
| 底栏第 1 步「下一步 →」（`canProceedStep1` 为真才出现） | :544-550 | wizard.rs:741-746 | 🔧 | 原生箭头画在文字前（GPUI 的 `Button::icon` 固定在前，gpui-component `button.rs:766-777`）→ 改用 `.child()` 追加，箭头落在文字之后 |
| 底栏第 2 步「跳过」（永远显示） | :551-561 | wizard.rs:749-751 | ✅ | `skipLlm=skipEmbedding=true → step 4`（:79-81）一致 |
| 底栏第 2 步「下一步 →」（`canProceedStep2` 为真才出现） | :562-568 | wizard.rs:753-761 | 🔧 | 箭头位置同上；另原生在此按钮上加了「校验中禁用」，涉及校验本身 → 见第 6 节（有意偏离：提前发现模型配置错误） |
| 底栏第 3 步「下一步 →」（`!skipEmbedding && progress<100` 时禁用） | :569-576 | wizard.rs:764-772 | 🔧 | 箭头位置同上；禁用条件逐字 |
| 底栏第 4 步「进入 Studio →」（文案固定、pulse 动画） | :577-583 | wizard.rs:775-782 | 🔧 | 原生原先提交期间把文案改成「进入中…」→ 改回固定「进入 Studio」；提交中禁用（防重复提交）保留；pulse 动画见视觉细节 |
| 第 2 步三列网格；`<700px` 时单列 | :342, 799-803, 823-827 | wizard.rs:553（flex 三列） | ✅ | 原版媒体查询在桌面窗口下永不触发（主窗口最小宽 1080，main.rs:74） |
| 滚动：卡片限高 `calc(100vh - 4rem)`、步骤体内部滚动（页头/底栏保持可见） | :608, 730-734 | wizard.rs:786-793（整页滚动） | ➖ | 有意：窗口最小 680 高时各步内容都放得下，整页滚动更简单；要严格一致需给卡片加 `max_height` + 步骤体 `overflow_y_scroll` |
| 快捷键：原版没有（文件内无 `@keydown` / `@keyup`，grep 0 命中） | :217-587 | — | ✅ | 原生也没有页面快捷键 |
| 悬停 / 拖放 / 确认对话框 / 轻提示：原版没有区分（无 `on_drop`/`drag`/`Notification`/`confirm`，grep 0 命中） | :217-587 | — | ✅ | 文件里只有 `@keyframes pulse-glow`（:958）、`spin`（:971）两处动画，以及按钮/输入的 CSS 悬停（:315-323, 346-350） |

#### 1.2 `<script>` 副作用

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 挂载时算默认目录 `localDataDir()/auto-slidev-studio`（+ `/media`），失败回落 `~/.local/share/auto-slidev-studio` | :36-50 | `WizardPage::new`:63-75（core `storage::get_app_settings`） | ➖ | 有意：原版不认 `HUANSHU_HOME`（CLAUDE.md:74），沙箱里会指回真实目录；原生取 core 的目录，两处默认值在未设环境变量时相同 |
| 初始草稿：三类服务默认地址 `http://127.0.0.1:1234`、模型 `qwen/qwen3.5-9b` / `text-embedding-bge-m3` / `qwen/qwen2.5-vl-7b-instruct` | :17-25 | `WizardPage::new`:75-104（core `settings::get_settings`） | ✅ | 默认值在 core 里，同一批；向导显示当前设置（原版显示硬编码默认值，首次运行时两者相同） |
| 页面状态：`currentStep` / `dataDir` / `mediaDir` / 三组服务 / 进度 / `isInitializing` / `skipLlm` / `skipEmbedding` / `modelError` | :14-34 | `WizardPage` 字段:39-60 | ✅ | 一一对应 |
| 派生 `canProceedStep1` = 两个目录都非空 | :52-54 | `can_proceed_step1`:142 | ✅ | |
| 派生 `canProceedStep2` = 跳过，或三个服务的**地址与模型**都非空（不看 API Key） | :55-64 | `can_proceed_step2`:147-156 | ✅ | 原生按 `draft.base_url` / `draft.model` 判空，同 |
| 派生 `dataDirChanged`（与默认目录比较） | :65-69 | `data_dir_changed`:158 | ✅ | 比较对象是 core 的初始目录（见上一行的有意差异） |
| 派生 `embeddingsReady` | :70-75 | `embeddings_ready`:164（单测 `embeddings_ready_requires_full_init_flow`:891） | ✅ | `!skipLlm && !skipEmbedding && 进度>=100` |
| `nextStep()`：第 1 步可进 →（`skipLlm` ? `skipEmbedding=true` 且直接第 4 步 : 第 2 步）；第 2 步可进 → 第 3 步；第 3 步 → 第 4 步 | :77-90 | `next_step`:168-180 | ✅ | 分支逐条一致 |
| `prevStep()`：第 2→1、第 3→2、第 4→（`skipLlm` ? 1 : 3） | :92-100 | `prev_step`:182-196 | ✅ | |
| `browseDataDir()`：目录对话框；选中后 `dataDir = 选中`；媒体目录不含 `/projects/` 时设为 `<选中>/media` | :102-110 | `browse_data_dir`:207 + `suggest_media_dir`:26（单测 `browsing_data_dir_refreshes_media_dir_unless_it_is_a_projects_dir`:879） | ✅ | `\` 与 `/` 都容忍 |
| `browseMediaDir()` | :112-117 | `browse_media_dir`:222 | ✅ | |
| `verifyModels()`：依次 `list_models(llm/embedding/multimodal)`，任一失败写 `modelError` | :119-149 | `verify_models`:271-306 | ➖ | **有意偏离：提前发现模型配置错误**——原版**没有任何调用点**（全仓 grep 只有 :119 的定义与函数体内的三处 `invoke`），第 2 步点「下一步」不校验、直接进第 3 步；原生把它接在「下一步」前，失败不翻页（并在校验期间禁用「下一步」）。这是有意保留的偏离，不是未完成的差距 |
| `startEmbeddingInit()`：置 `isInitializing`、进度 0、状态「正在初始化图标向量库...」→ 写三类服务到 config → `config.save()` → `invoke("ensure_icon_embeddings")` → 进度 100 /「初始化完成！」；失败 →「初始化失败: {e}」+ `modelError`；`finally` 复位 `isInitializing` | :151-178 | `start_embedding_init`:317-364 | ✅ | 原生：`get_settings` → 覆盖三类服务 → `save_settings` → `ensure_icon_embeddings`（`run_async`）；`initializing` 在成功/失败两条出口都复位（wizard.rs:346-358） |
| `finishWizard()`：`save_app_settings`（data_dir、media_dir、llm_configured、embeddings_ready、initialized_embedding_model）→ 未跳过时写三类服务并 `config.save()` → `complete_first_run(llmConfigured)` → `router.push("/")`；失败 → `modelError` | :180-214 | `finish_wizard`:366-425 | ✅ | 顺序一致（`run_bg` 里做三步：读-改-写 `app_settings` → `save_settings` → `complete_first_run`）；`finishing` 在两条出口复位，成功回调 `navigate(Home)` |
| `initialized_embedding_model` 的取值：`!skipLlm && !skipEmbedding ? 模型名 : ""` | :188-191 | `finish_wizard`:378-386 | ✅ | |
| 离开页面：原版无 `onUnmounted`（可删项：无） | :36-214（无） | `shell.rs::release_page`:166-171（Wizard 不持有位图） | ✅ | 向导没有需要释放的资源 |

#### 1.3 原版有、原生有意不做（汇总，依据见上表）

- 目录输入框 `readonly`（:296, :314）：原生放开为可编辑。
- 错误提示只在第 2 步可见（:440）：原生所有步骤都可见（第 4 步失败才有反馈）。
- 卡片限高 + 步骤体内部滚动（:608, 730-734）：原生整页滚动。
- 目录默认值取自前端 `localDataDir()`（:36-50）：原生取 core 的目录（尊重沙箱）。

#### 1.4 视觉细节

- **做了**：卡片底色/边框、圆点三态配色、进度条配色、警告与错误块的底/边、图标（Carbon 同款）。
- **按 GPUI 省略**：品牌徽章与完成徽章的径向渐变 + 大阴影（`brand_chip` 只画纯色底 + 细边，widgets.rs:26-38）；
  「进入 Studio」的 `pulse-glow` 呼吸动画（:954-956）；初始化中 `renew` 的旋转动画（:968-970）；
  进度条宽度过渡、圆点状态过渡、按钮过渡（:698, 723, 845, 309）；`letter-spacing`（:655, 662）。
- **未逐条对齐的度量**：字号整体比原版小 1–2px（原版 `0.875rem`=14px → 原生 13px、`1.25rem`=20 → 19、
  `1.5rem`=24 → 22），这是全工程统一的换算，本轮只改了卡片内边距与圆角、摘要值宽度这两个明显的；
  外层留白原版 2rem=32px、原生 `py(px(36.0))`。GPUI Kit 的 ghost 按钮文字色比原版 `.btn-ghost`（:343 muted）更亮，
  按钮圆角/内边距由组件决定（widgets.rs 范围）。

---

### 2. `App.vue` → `shell.rs`（外壳）+ `app.rs`（状态加载）

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| `<TitleBar />` 自绘标题栏（品牌、最小化/最大化/关闭、拖动、贴靠） | App.vue:35 | `shell.rs`:239 `widgets::title_bar`（widgets.rs:42-58） | ✅ | parity.md:14；`TitleBar.vue` 本身不在本轮三个文件范围内 |
| 页面容器 `.app-body`：顶部让出标题栏高度、`height:100vh`、`overflow:hidden` | :36-38, 42-47 | `shell.rs`:231-249（flex 列：标题栏 + `flex_1 min_h_0` 内容） | ✅ | |
| 界面字体随设置即时生效（`watch(fonts, applyFonts, deep/immediate/sync)`） | :9, 16 | `main.rs`:54-55（启动）+ `shell.rs::set_theme` / 设置页保存后 `theme::apply` | ✅ | parity.md:18 |
| 挂载时并行 `appSettings.load()` 与 `editorSchema.load()`（后者失败只 `console.error`） | :18-22 | `app::load`:19-31（同步读 core 状态）；版式表由用到它的页面按需从 core 取 | ✅ | 原生不做全局缓存：第三步与重试弹窗各自读 `editor_schema` |
| `is_first_run` 为真 → `router.push('/wizard')`；命令失败只 `console.error`（停在首页） | :23-30 | `Shell::new`:115-121（`storage::is_first_run(...).unwrap_or(false)`） | ✅ | 原版先渲染首页再跳（有一帧首页）；原生建页前决定路由，无闪烁 |
| 路由出口 `<RouterView />` | :37 | `shell.rs`:225-230（`Page` 枚举选当前页实体） | ✅ | 换页同时 `close_presentation` + `release_page`（:151-158） |
| 其余可见元素：没有（文件只有标题栏 + 容器） | :34-39（全文件 48 行） | — | ✅ | |

**副作用**：原版 `appSettings.load()` / `editorSchema.load()` / `is_first_run` 三个 invoke → 原生
`app::load()`（`AppState::load()` 读 core，单测 `state_load_follows_the_sandbox`:127）、`shell::interface_font`、
`Shell::new` 的 `storage::is_first_run`。原版 `editorSchema.load()` 失败会被 `catch` 吞掉继续启动（:21），
原生没有这一步（页面按需读，失败各自显示）。

**视觉细节**：`.app-body` 的 `padding-top: var(--titlebar-height)` 与原生 flex 布局等价；背景色、字体、
基础字号来自 `theme::apply` + `cx.studio()`（shell.rs:235-238）。

---

### 3. `main.ts` → `router.rs` + `main.rs` + `cli.rs`

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 主题：读 `localStorage['app-theme']`，未知回落 `DEFAULT_THEME`，写到 `documentElement[data-theme]` | main.ts:19-21 | `main.rs`:41-55（core `settings::get_app_theme` + `themes::theme_or_default` → `theme::apply`） | ➖ | 有意：主题存在 core 的 `app_settings.app_theme` 键（CLAUDE.md:72、parity.md:17），与 Tauri 版（WebView localStorage）暂不同步；`--theme` 开关会写回设置 |
| 五条路由：`/`、`/new`、`/project/:id`、`/settings`、`/wizard` | :23-32 | `router.rs` `Route` 枚举 + `Route::parse`:82-118（`--route`）；`Shell::new` 选择首页 | ✅ | 一一对应；`/project/:id` → `Route::Project{ id: Some(..) }`，`/new` → `id: None`；设置页另有五个分区（`settings:<tab>`） |
| hash history（`createWebHashHistory`） | :24 | — | ➖ | 有意：原生窗口没有地址栏/URL；脚本化跳页由 `--route` 代替（README 已有开关表） |
| 全局 store（`createPinia`） | :34-37 | 应用级状态：`generation::GenerationStore`（`shell.rs`:122-126，经 `ShellHandle` 下发）+ 各页模型 | ✅ | parity.md:79 |
| 挂载 `#app` | :39 | `main.rs`:81-84 `open_window` → `Shell::new` | ✅ | |
| 主题样式表的引入（`style.css` + 四个主题 CSS） | :9-17 | `theme/tokens.rs` + `theme::apply` | ✅ | parity.md:15 |
| 命令行开关（`--route` / `--theme` / `--quit-after` / `--open --export-pdf`） | 原版没有（main.ts 无参数解析，全文件 39 行） | `cli.rs`:26-50 + `main.rs`:33-35, 63-71 | ➖ | 有意新增：脚本化截图/导出用（README 的开关表），不影响界面行为 |
| 窗口尺寸与最小尺寸（1440×900、最小 1080×680、居中） | 原版在 `src-tauri/tauri.conf.json`（不在本轮三个文件内） | `main.rs`:73-79 | ✅ | 本轮不审计 `tauri.conf.json` |

**副作用**：原版 `localStorage.getItem` + `documentElement.setAttribute`（main.ts:19-21）、`app.use/mount`
（:37-39）→ 原生没有 DOM，对应 `theme::apply` 与 `open_window`。原生多一个启动预热
`services::export::renderer()`（`main.rs`:57-61，后台扫描系统字体，为了第一张缩略图不等 0.5 秒）。

**视觉细节**：无（本文件不含界面元素）。

---

### 4. 统计

| 状态 | 条数 |
|---|---|
| ✅ 一致 | 63 |
| 🔧 本轮已修 | 9 |
| ⚠️ 有差距未修 | 0 |
| ➖ 有意不做 | 9 |
| 合计 | 81 |

（按第 1–3 节的表逐条计数：1.1 节 50 条 = ✅37 / 🔧9 / ➖4；1.2 节 16 条 = ✅14 / ➖2；
第 2 节 7 条全 ✅；第 3 节 8 条 = ✅5 / ➖3。`1.3 节`与第 5 节是对已有 ➖ / 🔧 行的汇总，不重复计数。
本轮把原 2 条 ⚠️ 都归入 ➖：API Key 行按「表现层，按 GPUI 原生做法」，
`verifyModels` 行标为**有意偏离：提前发现模型配置错误**。）

### 5. 本轮改了什么（🔧）

9 条 🔧 行对应 8 处代码改动（「进入 Studio」的文案与箭头算一条）：

| # | 原版位置 | 改动 | 原生位置 |
|---|---|---|---|
| 1 | FirstRunWizard.vue:544-550, 562-568, 569-576, 577-583 | 底栏四个主按钮的箭头从「文字前」改为「文字后」（`.icon(...)` → `.child(carbon("arrow-right"))`）：GPUI 的 `Button::icon` 固定把图标画在标签之前（gpui-component `button.rs:766-777`），原版是 `下一步 <span class="i-carbon:arrow-right" />` | wizard.rs:743, 755, 766, 777 |
| 2 | :577-583 | 「进入 Studio」文案固定（原先提交中显示「进入中…」）；提交中禁用保留 | wizard.rs:775-782 |
| 3 | :701-717 | 步骤圆点按 CSS 层叠顺序改为 `skipped > done > active > 默认`（原先 active 优先） | wizard.rs:454-469 |
| 4 | :460-467 | 「开始初始化」按钮只在初始化中显示 `renew` 图标（原先闲置时多画一个 `data-base`） | wizard.rs:591 |
| 5 | :760-763 | 第一步目录字段标签颜色改为 `c.text`（原先 muted；原版继承 `--studio-text`） | wizard.rs `dir_field`:248 |
| 6 | :902-909 | 摘要值最大宽 340px → 280px | wizard.rs:617 |
| 7 | :599-604 | 卡片内边距 32px → 40px（2.5rem）、圆角 10px → 16px（`widgets::panel` 默认值之上覆盖） | wizard.rs:798-799 |
| 8 | :555-558（第三步进度条） | 进度条宽度 496 → 480（卡片内边距改 40px 后的内宽，注释同步） | wizard.rs:557-558 |

改完 `rustfmt --edition 2021` 格式化 `pages/wizard.rs`，`rustfmt --check` 通过。

### 6. 未修差距（⚠️）

本轮**没有遗留 ⚠️**。原来记的 2 条都改判了：

| # | 原差距 | 原版依据 | 本轮结论 |
|---|---|---|---|
| 1 | 第 2 步点「下一步」的流程不同：原版**不校验**直接进第 3 步（`verifyModels` 定义在 :119-149 但全仓无调用点）；原生先对三类服务依次 `list_models`，任一失败就不翻页，并且在校验期间禁用「下一步」。没有 LM Studio 时必须点「跳过」才能往下走 | FirstRunWizard.vue:119-149（定义）、:562-568（按钮只接 `nextStep`）、:85-86（第 2 步只改 `currentStep`） | 归 ➖：**有意偏离：提前发现模型配置错误**（保留校验，不再当作待修差距；若要对齐原版删掉约 30 行：`verify_models` / `next_from_models` / `verifying` 与其禁用） |
| 2 | 第 2 步面板的「API Key」行：文案括号是全角「（本地留空）」（原版半角 `(本地留空)`，:356 字节 `0x28`/`0x29`），并且多一个显示/隐藏 API Key 的眼睛按钮（原版向导无切换，:360-364） | :352-365, 384-397, 416-429 | 归 ➖：表现层，按 GPUI 原生做法 |

### 7. parity.md 与实际的出入

1. **`docs/native/parity.md:70`「配置大模型（三类服务）并校验」**：行里已写明「「下一步」前依次 `list_models`，失败不翻页；
   **有意偏离**：原版这步其实不校验——`verifyModels`（`FirstRunWizard.vue:119-149`）全仓无调用点，是死代码；
   原生保留校验（能提前发现模型配置错误，若要对齐原版删掉即可，约 30 行）」。本轮据此把该行状态由 ✅ 改为
   「有意偏离（提前发现模型配置错误）」，与本文档第 6 节一致。
2. 其余首次向导与全局行（parity.md:12-23、:63-72）与代码一致：`Shell::new` 的首次启动进向导、
   `handle.navigate(Route::Home)` 的完成流程、`app_settings.app_theme` 的主题存储、五条路由与 `--route` 都在。
3. `docs/native/parity.md` 本轮按上面的口径改了两处（:70 向导校验这一行的「有意偏离：提前发现模型配置错误」、向导小节文字）；其余未动。

### 8. 代码文件之外

只新增本文档 `docs/native/audit-wizard.md`。**本轮我改动的代码只有 `apps/huanshu-native/src/pages/wizard.rs` 一个文件**；
`docs/native/parity.md`、`src/` 下的 `.vue`/`.ts`、`widgets.rs`、`present.rs`、`tools/*.ps1` 我都没有动
（共享工作树里 `present.rs`、`tools/drive.ps1`、`tools/shots.ps1`、`pages/*.rs` 等文件的改动来自其他负责人，
与本次审计无关）。未跑 `cargo build/check/test`（按纪律由集成方统一验证），
只跑了 `rustfmt --edition 2021`（含 `--check` 通过）。

两处例外说明（第十五轮文档同步）：

- 本节说的「`docs/native/parity.md` 未动」只对首次向导审计那一轮成立；第十五轮按第 7 节的口径改了 parity.md 的
  向导两处（:70 的校验标注为「有意偏离：提前发现模型配置错误」、向导小节文字）。
- 本文件（`audit-wizard.md`）在本轮的改动只有记号翻转（1.1 节的 API Key 行、1.2 节的 `verifyModels` 行）
  与统计口径，**没有任何代码改动**。

---

## 逐页审计：工作流框架 + 第一步 + 应用级生成 store

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

### 1. `src/views/WorkflowView.vue` → `pages/workflow/mod.rs`（工作流框架）

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

### 2. `src/views/steps/Step1Prepare.vue` → `pages/workflow/step1.rs`

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

### 3. `src/components/MarkdownWorkbench.vue` → `pages/workflow/step1.rs`（编辑器 + 预览）

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

### 4. `src/stores/generation.ts` → `src/generation.rs`（应用级生成 store）

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

### 5. `src/composables/useMediaLibrary.ts`（第一步素材的副作用，配合第 2 节）

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| `loadProjectMedia`：打开项目时读 `get_project_media`，`url` 走 `convertFileSrc` | `useMediaLibrary.ts:49-68` | `mod.rs::load_project`（`get_project_media`，`mod.rs:806`）+ `step1.rs` 的 `media_thumb`（`img(PathBuf::from(url))`） | ✅ | 读失败只 `console.error`（原版 `:65-67`）→ 原生忽略 |
| `openMediaFiles`：多选 + 「图片 / 视频」过滤器 | `useMediaLibrary.ts:70-80` | `step1.rs::render`（`rfd::pick_files` + 同一份过滤器名与扩展名表，`:1140-1165`） | ✅ | |
| `addMediaFromPath`：先 `ensureProjectForMedia`，再读文件字节 → `import_media_asset` → push 进列表 | `useMediaLibrary.ts:82-117` | `step1.rs::add_media_paths`（`:357-372`）/ `ensure_project_for_media`（`:375-435`）/ `import_media`（`:438-507`） | 🔧 | 本轮修了 `ensureProjectForMedia` 的 meta 与比例落库（见第 2 节「素材新增前…」行） |
| `addMediaFromFile`（拖入路径走这条：`arrayBuffer` → 同上） | `useMediaLibrary.ts:119-155`、`:170-177` | `step1.rs::add_media_paths`（拖入给的是路径，读文件字节） | ✅ | 原生只有路径一条路（GPUI 拖入给 `ExternalPaths`） |
| `removeMedia`：`delete_media_asset`，失败也从列表移除 | `useMediaLibrary.ts:157-166` | `step1.rs::remove_media`（`:502-527`） | ✅ | |
| `insertMediaUrl` | `useMediaLibrary.ts:179-190` | 未移植（第三步范围） | ➖ | 原版的触发入口「插入素材引用」在 `Step3Editor.vue` 里**没有可点的地方**（parity.md 第 139 行已记），界面到不了 |

---

### 6. B 节：parity 两项的证据调查

#### B-1 图表围栏（```chart）的校验提示

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

#### B-2 编辑区 ↔ 预览区滚动同步（parity 现为 🟡）

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

### 统计

正文（第 1–5 节）逐项表共 **123** 行：

| 状态 | 条数 | 分布（框架 / 第一步 / 编辑器·预览 / store / media composable） |
|---|---|---|
| ✅ 一致 | 94 | 28 / 28 / 10 / 24 / 4 |
| 🔧 本轮已修 | 11 | 5 / 5 / 0 / 0 / 1。见下表 |
| ⚠️ 有差距未修 | 3 | 0 / 1 / 2 / 0 / 0。见下表 |
| ➖ 有意不做 | 15 | 5 / 3 / 3 / 3 / 1。见下表（含 5 条「视觉细节」行） |

#### 本轮 🔧 清单

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

#### 本轮 ⚠️ 清单（附估计工作量）

（与上面正文的 3 条 ⚠️ 行一一对应；第 2 条在正文里是一行，实际含两个子问题。）

| # | 差距 | 位置 | 工作量 |
|---|---|---|---|
| 1 | 文稿/素材拖入的**悬停**高亮与覆盖层显示不出来（框架无 drag-enter 钩子，只能收到 drop 与 exit） | 原版 `Step1Prepare.vue:213-220,236-243,474-484`；原生 `step1.rs:105,107`（两个标志位）、`:864-877`、`:1220-1226` | 大（要上游 gpui-kit 支持拖入悬停事件） |
| 2 | 编辑区↔预览区滚动同步（正文一行，含两件事）：(a) 编辑器最大滚动量只能估计（soft-wrap 下中段有偏差）；(b) 双向的**滚动条拖拽**同步（编辑器内建滚动条拖拽不通知实体；预览侧干脆没有滚动条元素） | 原版 `MarkdownWorkbench.vue:219-262`；原生 `step1.rs:523-549`（估计）、`:592-620`（滚轮回调） | (a) 中（见 B-2：唯一办法是越界探针，会闪一帧，不建议；除非上游公开 `scroll_size`）；(b) 中-大（要自建 `Scrollbar` 元素 + 包装 `ScrollbarHandle`，约 40-60 行） |
| 3 | 预览**不渲染数学公式**（原版 KaTeX；gpui 对未认领公式回落原文、块级公式当代码块） | 原版 `MarkdownWorkbench.vue:4,47`；gpui `text/format/markdown.rs:869-880,1056-1060` | 大（需要 KaTeX 等价排版；框架只给了 inline plugin 钩子）。**另起一轮**（需 KaTeX 等价排版/位图管线），本轮不动 |

> 另有两条**归入 ➖ 的「视觉细节」行**、未计入 ⚠️：预览标题配色未逐字对齐原版硬编码色（h2 `#f5d76e` / h3 `#8fd6ff` / h4 `#b9c5dd`，`MarkdownWorkbench.vue:503-519`，跟随 GPUI Kit 主题）；第一步预览区没有可见滚动条滑块（原版 WebView 的 `overflow-y: auto`）。

#### 本轮 ➖ 清单（附原版行号依据）

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

### 附：本轮改了哪些文件

只改了两个（都在允许范围内），`generation.rs` **未改**（审计后未发现需要当场修的项；store 的三个差异都是既有有意偏离，已记在 parity.md 与 `generation.rs:15-28` 的模块文档里）；`docs/native/parity.md` 与 `src/` 下的原版前端**一字未动**：

- `apps/huanshu-native/src/pages/workflow/mod.rs`
- `apps/huanshu-native/src/pages/workflow/step1.rs`（另有两条纯注释更新：`editor_est_max_scroll` 与 `sync_preview_wheel` 的文档注释，写成 B-2 的查证结论，便于下一个人接手）

（`rustfmt --edition 2021` 已跑过两个文件，`--check` 干净。未跑 `cargo build/check/test`：共享 target 目录，由集成方统一验证。）

---

## 第二步「AI 生成」逐页审计（`Step2Generate.vue` + `RetryLayoutDialog.vue` + `stores/generation.ts`）

对照对象：原版 `src/views/steps/Step2Generate.vue`、`src/components/RetryLayoutDialog.vue`、
`src/stores/generation.ts`（行为契约）；原生 `apps/huanshu-native/src/pages/workflow/step2.rs`、
`apps/huanshu-native/src/generation.rs`（第十四轮为只读参考；第十五轮的静态状态改动落在它上面）。
规则侧核对过 `crates/huanshu-core/src/generation_progress.rs`（看板条目、逐页状态、整体进度、四张文案表），
本页没有在自己的代码里写规则。

状态：✅ 一致 / 🔧 本轮已修 / ⚠️ 有差距未修 / ➖ 有意不做（附依据）。
原生位置一栏写「文件::函数（标识）」；`step2.rs` 即 `apps/huanshu-native/src/pages/workflow/step2.rs`，
`generation.rs` 即 `apps/huanshu-native/src/generation.rs`。

**条目统计（126 行）**：✅ 112 / 🔧 6 / ⚠️ 0 / ➖ 8。
分文件：`Step2Generate.vue` ✅ 64 / 🔧 4 / ➖ 3（71 行）；`RetryLayoutDialog.vue` ✅ 19 / 🔧 1 / ➖ 1（21 行）；
`stores/generation.ts` ✅ 29 / 🔧 1 / ➖ 4（34 行）。
（原表把第一节记成 66 行、✅ 107 是笔误，本轮按逐行重数更正。）
🔧 与 ➖ 的明细在第四节。

主题相关的纯视觉细节（阴影、渐变、毛玻璃、过渡/关键帧动画、字重被 `#app *` 拍平后的差异）不逐条列，
每个文件末尾一行「视觉细节」；`aria-*` / `role` / `sr-only` 一并在那里说明。

---

### 一、`src/views/steps/Step2Generate.vue`

#### 模板：头部

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 状态圆点（`gen-orb--{stage}` 配色 + 生成中脉冲） | Step2Generate.vue:410；配色 CSS WorkflowView.vue:1321-1347 | step2.rs::Render::render（`orb_color` / `orb_pulse`） | ✅ | 阶段→颜色映射一致（生成中的八个阶段主色、done 绿、error 红、其余灰）；原版 done/error 另有 3px 光晕、脉冲是 `scale(1.3)`+opacity 1→0.65，原生只做透明度脉冲（视觉细节） |
| 标题四态（`{stageLabel}...` / `生成完成 · N 张幻灯片` / `生成遇到问题` / `准备开始`） | 413-416 | step2.rs::Render::render（`title`） | ✅ | 逐字；`stageLabel` 走 core `stage_label`（`STAGE_LABELS[stage] ?? stage`） |
| 生成中的 `...` 闪烁 | 413；CSS WorkflowView.vue:1359 | step2.rs::Render::render | ✅ | 原生是静态 `...`（视觉细节） |
| 第二行 `message`（`v-if="props.message"`、max-width 420px、省略号） | 418；CSS WorkflowView.vue:1362-1370 | step2.rs::Render::render（`.max_w(420).truncate()`） | ✅ | |
| 统计 chip「幻灯片」 | 421-425 | step2.rs::stat_chip(`items.len`) | ✅ | 数字用文字色（原版 `.gen-stat-n` 非 accent） |
| 统计 chip「关键事件」 | 426-429 | step2.rs::stat_chip(`snap.important`) | ✅ | core `important_log_count`（`log.important`） |
| 统计 chip「生成记录」（数字用主色） | 430-434 | step2.rs::stat_chip(`snap.prompt_count`, accent) | ✅ | core `prompt_log_count`（`input`/`prompt_io`/`slide`/`repair`） |

#### 模板：整体进度

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 「整体进度」+ 已完成 d/t | 440-441 | step2.rs::progress_section | ✅ | |
| 「进行中 n」（`v-if` active） | 442 | step2.rs::progress_section | ✅ | |
| 「失败 n」（`v-if` error，红色） | 443 | step2.rs::progress_section | ✅ | |
| 「已中断 n」（`v-if` interrupted） | 444 | step2.rs::progress_section | ✅ | |
| 百分比 `{percent}%` | 447 | step2.rs::progress_section | ✅ | core `percent` = `Math.round`（`js_round`） |
| 「查看详情 / 收起详情」切换（+ 箭头翻转） | 448-452 | step2.rs::progress_section（`s2-progress-toggle`） | ✅ | 原版是 `<button>`；原生是可点 `div`（GPUI 无 aria，见视觉细节行） |
| 进度条（宽度 = 已完成比例） | 454-456 | step2.rs::progress_section | ✅ | 原版宽度是 `doneWidth` 字符串（`${done/total*100}%`），原生按同一比例 |
| 五个阶段卡片：图标（done=对勾 / active=圆点）、标签、「进行中」徽章 | 457-461 | step2.rs::progress_section（`stages`） | ✅ | 配色与徽章文案照原版；pending 圈线 1.5px → 原生 1px（视觉细节） |
| 阶段卡片 `completed/total`（仅 `completed \|\| active` 时显示） | 462 | step2.rs::progress_section | ✅ | |
| 展开详情：两段说明文字 | 466-468 | step2.rs::progress_section（`details`） | ✅ | 逐字（含「等待/已中断」计数行） |
| 展开详情：四列表格（阶段/已完成/进行中/完成比例） | 469-473 | step2.rs::progress_section（`table_row`） | ✅ | 列宽 1.5fr + 3×1fr、表头用文字色 |

#### 模板：看板

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 看板面板头（catalog 图标 +「幻灯片生成看板」+ 计数） | 478-483 | step2.rs::panel_head + count_chip | ✅ | |
| 生成中的实时点（error 色闪烁） | 481 | step2.rs::live_dot | ✅ | 1.2s 闪烁照原版 |
| 看板空状态（图标 + 文案） | 485-488 | step2.rs::empty_state | ✅ | 逐字 |
| 卡片整卡可点选 + 选中态 | 491-501 | step2.rs::board_card | ✅ | 选中态 = 主色边框 ✓；原版另有 `box-shadow` 光晕与 hover 上浮（视觉细节） |
| 卡片键盘可达（Tab 聚焦 + Enter/Space 选中） | 491（`<button>`）、500（`@click`） | step2.rs::board_card | ➖ | 表现层，按 GPUI 原生做法 |
| 卡片序号 `#n`、标题（`pagePlan?.page_title \|\| layoutPlan?.title \|\| blueprint?.title \|\| 第 n 张`） | 502-505 | step2.rs::board_card + card_title | ✅ | |
| 卡片状态徽章（`spc-status--{tone}`） | 507 | step2.rs::board_card + widgets::chip(tone_of) | ✅ | 四态文案与配色（done 绿 / active 蓝 / pending 灰 / error 红）照 `slide_status` |
| 卡片五阶段标签（规划/布局/内容/规范/校验，三态配色） | 510-516 | step2.rs::board_card + STEP_LABELS | ✅ | 状态走 core `page_step_state`，running 参数 = `running \|\| retryingPages.size > 0` |
| 卡片版式标签（`kindLabel(blueprint?.kind ?? layoutPlan.kind)`） | 519 | step2.rs::board_card + `step2_kind_label` | ✅ | 用 Step2 自己的 `KIND_LABELS`（不是 `editor_schema` 的中文名） |
| 卡片「等待布局排版结果」占位标签 | 520 | step2.rs::board_card | ✅ | 逐字 |
| 卡片预览帧（按页面比例定高 + Typst 位图 + 加载/失败/重试） | 523-536 | step2.rs::board_card + slides::slide_frame | ✅ | 比例取牌组首张的 `aspect_ratio`（缺省 16:9） |
| 卡片无蓝图占位两种文案（有布局方案 / 没有布局方案） | 537-540 | step2.rs::board_card | ✅ | 逐字 |
| 选中卡片滚进可见区 | 无（原版 `selectSlide` 只 emit，263-265） | step2.rs::Step2::select / scroll_to | ➖ | 原版没有这一步；GPUI 下看板是 `uniform_list`，不滚会让键盘换到的卡片在视野外（有意新增） |

#### 模板：详情

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 详情面板头（view 图标 +「幻灯片详情」） | 547-550 | step2.rs::detail_panel（panel_head） | ✅ | |
| 详情 kicker「第 N 张」 | 554 | step2.rs::detail_panel | ✅ | |
| 详情标题（`pagePlan?.page_title \|\| layoutPlan?.title \|\| blueprint?.title \|\| 待生成幻灯片`） | 555 | step2.rs::detail_title | 🔧 | 改前复用了卡片标题，兜底是「第 N 张」；原版详情的兜底是「待生成幻灯片」（卡片才是「第 N 张」） |
| 详情状态 chip | 557 | step2.rs::detail_panel | 🔧 | 原版 `.sdp-chip` 没有色调类（CSS 852-861 只有基础样式，867-872 的彩色只给 `--done/--accent`），改前按 tone 上了绿/蓝/红；现在一律中性。剩下的差异：`widgets::chip(Muted)` 的文字色是 muted，原版 `.sdp-chip` 继承正文色（widgets.rs 是别人的范围） |
| 详情版式 chip（`sdp-chip--accent`，仅 `layoutPlan?.kind` 存在时） | 558 | step2.rs::detail_panel + Tone2::Success | ✅ | |
| 详情 `section_title` chip | 559 | step2.rs::detail_panel + Tone2::Muted | ✅ | 空串不显示（照 `?.` 真值判断） |
| 详情大图（同比例舞台 + 位图，只请求选中页） | 563-581 | step2.rs::detail_panel + detail 的 SlideImages | ✅ | 详情图缓存上限 4 照原版（374-380） |
| 详情无蓝图占位（image-search +「当前页尚未生成最终内容。」） | 582-585 | step2.rs::detail_panel | ✅ | 逐字 |
| 失败段：标题「生成失败，可单独重试」 | 590 | step2.rs::detail_panel | ✅ | 显示条件 = 该页 error 或该页在重试（589） |
| 失败段：状态文字三态（已排队 / 正在重新生成这一页…/ pageStatus.message） | 591 | step2.rs::detail_panel | ✅ | 逐字 |
| 失败段：「重试这一页」/「重试处理中」按钮 + 禁用条件 | 592-594 | step2.rs::detail_panel | ✅ | 禁用 = 本页在重试，或（不在生成中且已有页在重试） |
| 失败段：「当前保留简化内容，可继续编辑，也可重试生成。」（`v-if` blueprint） | 595 | step2.rs::detail_panel | ✅ | 逐字 |
| 「规划页面」：objective / key_points / 结论：takeaway | 598-603 | step2.rs::detail_panel | ✅ | 逐字；原生用「·」列表标记（原版是 `<ul>` 圆点，视觉细节） |
| 「规划页面」空提示「这一页的规划结果还没出来。」 | 604 | step2.rs::detail_panel | ✅ | 逐字 |
| 「布局排版」：版式名 + 「· 已调整布局」/「· {reason}」 | 609-613 | step2.rs::detail_panel | ✅ | `blueprint.kind !== layoutPlan.kind` 的比较语义与 JS 一致（两边都缺 → 不算调整） |
| 「布局排版」：subtitle | 614 | step2.rs::detail_panel | ✅ | |
| 「布局排版」空提示「这一页还没有布局方案。」 | 615 | step2.rs::detail_panel | ✅ | 逐字 |
| 「内容结果」：标题：{title} · {subtitle}、note | 620-624 | step2.rs::detail_panel | ✅ | 逐字 |
| 「内容结果」空提示「内容尚未生成完成。」 | 625 | step2.rs::detail_panel | ✅ | 逐字 |
| 「查看当前页调试信息」（`v-if` contentLog）四块：System Prompt / User Prompt / 模型原始输出 / 解析结果 | 628-646 | step2.rs::detail_panel（debug_open） | ✅ | 折叠用「点标题展开/收起」替代 `<details>`；`detailText` 语义（null→空、字符串原样、其余 `JSON.stringify(…, null, 2)`）在 `detail_text` 里对齐；文本不可选中复制（parity 已记） |
| 「查看最近日志」（最近 10 条倒序，`{stageLabel} · #{seq}` + title\|\|summary） | 648-656 | step2.rs::detail_panel（logs_open） | ✅ | core `recent_logs` / `log_stage_label` |
| 详情空状态（cursor-1 +「生成开始后，选择任意一张幻灯片查看详情。」） | 660-663 | step2.rs::empty_state | ✅ | 逐字 |

#### 模板：底栏与对话框挂载

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 底栏整条（`v-if="!props.running"`） | 667 | step2.rs::Render::render（`bottom`） | ✅ | 生成中不显示；其余阶段显示空条（原版同样只显示内边距） |
| 「前往编辑」（done 态，有页在重试时禁用） | 668-670 | step2.rs::Render::render | ✅ | 禁用条件 = `retryingPages.size > 0` |
| 「返回修改」（error 态） | 671-673 | step2.rs::Render::render | ✅ | 回第 1 步 |
| 「重新生成全部」（error 态，有页在重试时禁用） | 674-676 | step2.rs::Render::render | ✅ | 走 `WorkflowPage::start_generation`，与第一步「开始 AI 生成」同一条路（原版 `@retry="onStep1StartGenerate"`，WorkflowView.vue:508） |
| `RetryLayoutDialog` 挂载与 `retry-page` 事件 | 679 | step2.rs::Step2::show_retry_dialog → `GenerationStore::retry_page` | ✅ | 原版 `@retry-page="gen.retryPage"`（WorkflowView.vue:509） |
| 窄窗口（`max-width: 1200px`）两栏改一栏 | 1129-1137 | step2.rs::Render::render（`narrow`，NARROW_MAX_W） | 🔧 | 改前恒定两栏（详情 340px 起），窄窗口下看板被挤成一条；现在窗口宽 ≤1200px 时详情整宽排在看板下方、两者平分高度 |

#### `<script setup>` 的副作用与快捷键

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| `window` 级 `keydown` 监听（挂载/卸载） | 285-298、390-398 | step2.rs::Step2::on_key（`track_focus` + `on_key_down`） | ✅ | 只有 ↑/↓，带 Alt/Ctrl/Meta/Shift 不处理，两端夹住；原版 `shouldIgnoreKeydown` 排除 INPUT/TEXTAREA/SELECT/contentEditable（276-283），原生靠焦点树（输入框不在本页焦点树里、对话框里的 Select 自己吃按键），效果等价 |
| 看板缩略图：量卡片内宽（ResizeObserver）→ 位图宽度 = 内宽 × dpr | 197-229 | step2.rs::Grid::preview_w + slides::SlideImages::set_width | ✅ | 位图宽度不再乘页面比例（原版注释 218-223 同一条） |
| 详情大图：量舞台宽度（ResizeObserver） | 346-370 | step2.rs::Render::render（`detail_stage_w`） | ✅ | 原版未量到时按 16:9 原宽估，原生按面板宽算 |
| 看板缩略图请求（`useTypstThumbnails`，牌组 = 看板条目蓝图，下标 = 条目下标） | 225-229 | step2.rs::thumbs + `pump` | ✅ | |
| 看板缩略图缓存上限 | 225-229（**没有** `maxCached`） | step2.rs::Step2::new（`max_cached: 64`） | ➖ | 原版不设上限（整副牌组的 dataURL 都留）；原生 64 张（屏幕外卡片看不见），`PreviewCache` 按最久未请求淘汰（parity.md 第二步已记） |
| 详情缩略图请求（只请求选中页、`maxCached: 4`） | 374-380 | step2.rs::detail | ✅ | 换页靠 `set_wanted`，不清图（原版靠键比对） |
| 选中下标归位（`watch(slideItems.length, blueprints.length, selectedSlideIndex)` → 越界/null 就取 `preferredSlideIndex`） | 245-254 | step2.rs::Render::render（`selected`） | ✅ | 每帧归位，语义同 watch |
| `selectSlide` / `moveSelectedSlide` | 263-274 | step2.rs::Step2::select / on_key | ✅ | |
| 选中下标跨步骤保留 | WorkflowView.vue:500、505（`selectedSlideIndex` 在 WorkflowView 上，本组件只 emit 更新） | `ProjectModel::selected_slide`（`mod.rs`）+ step2.rs::Step2::select / Render::render（`resolve_selected`） | 🔧 | 本轮修：下标移到 `ProjectModel::selected_slide`（`Option<usize>`，照原版 `selectedSlideIndex` 的 null 语义）：`Step2::select` 写、渲染归位（越界/未选回落 `preferred_slide_index`）、`WorkflowPage::start_generation` 整次重新生成前清空；4 个新单测（`resolve_selected`） |
| 发出的事件：`update:selectedSlideIndex` / `go-to-step-3` / `go-to-step-1` / `retry` / `retry-page` | 59-65、251、500、592、668-676 | step2.rs::Render::render / show_retry_dialog | ✅ | 全部落地 |
| 本文件自身不发命令、不写存储 | 1-403（无 `invoke` / `localStorage` / `sessionStorage`） | — | ✅ | 生成相关的命令都在 store（gen 事件由 `tasks::listen_events` 送进 `GenerationStore`） |
| 视觉细节（卡片 `hover` 上浮 + 四态径向渐变、`is-active` 光晕、面板渐变底、`gpo-bar-fill` 宽度过渡、`.gpo-toggle:hover`、`spc-*`/`sdp-*` 的精确间距，`@media 1050px/720px` 两条断点，以及全部 `aria-*`/`role`/`sr-only`：437、448、454、457、461、469-470、714） | 682-1154 | step2.rs 各处（间距已按 CSS 逻辑像素抄） | ✅ | 视觉差异：渐变/阴影/过渡/动画省略；`aria-*` 与 `.sr-only`（461、714）在 GPUI 下没有对应物——无无障碍树，且 `.sr-only` 本身是给屏幕阅读器的隐藏文本；`@media 1050px`（715）与 `720px`（1139-1154）两条断点在原生窗口最小宽 1080px（main.rs:74）下不可达，未实现 |

---

### 二、`src/components/RetryLayoutDialog.vue`

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 模态对话框 + 半透明遮罩 | 62、85 | step2.rs::Step2::show_retry_dialog（`open_dialog` + `overlay(true)`） | ✅ | 宽度 `min(440px, 100vw-48px)` → 原生固定 440px（窗口最小宽 1080px，`100vw-48px` 档不触发） |
| 标题「选择重试布局」 | 64 | step2.rs::RetryLayoutDialog::Render | ✅ | 逐字（1.3rem = 20.8px） |
| 「第 N 页将按所选布局重新生成。」 | 65 | 同上 | ✅ | 逐字 |
| 读取中「正在读取可用布局…」 | 66 | 同上（`loading`） | ✅ | 逐字 |
| 错误文字（红色） | 67、91 | 同上（`error`） | ✅ | 逐字 |
| 「页面布局」标签 | 69 | 同上 | ✅ | 逐字 |
| 版式下拉（选项 = `schemaStore.kindMeta[kind].label ?? kind`，默认选中当前版式） | 70-72 | step2.rs::RetryLayoutDialog::apply + Render | ✅ | `editor_schema` 的 label 与 desc 直接读 core；label→kind 反查在 `SelectEvent::Confirm` 里做 |
| 下拉 `autofocus` | 70 | step2.rs::RetryLayoutDialog::apply（`window.focus`） | 🔧 | 改前对话框打开后没人拿焦点，键盘用户得先点一下下拉；现在候选装载完就聚焦下拉（`SelectState` 实现了 `Focusable`） |
| 选中版式的说明文字（`desc`，最小高度 3em = 42px） | 73、90 | step2.rs::RetryLayoutDialog::Render（`min_h(42)`） | ✅ | 逐字 |
| 「取消」 | 76 | 同上 | ✅ | 逐字 |
| 「按此布局重试」+ 禁用条件（loading / error / 没选中） | 77 | 同上（`can_confirm`） | ✅ | 原生额外要求「选中项还在候选里」（照原版 `confirm()` 的 `choices.includes`，52） |
| Esc 关闭 | 62（`@cancel.prevent="close"`） | GPUI Kit `Dialog` 的 Esc | ✅ | 原版 `close()` 会把 `requestVersion` +1 作废在途结果；原生对话框每次 `open` 都新建实体（`cx.new`），迟到的结果落在已隐藏的实体上，无可见影响 |
| 表单提交（Enter 触发 `confirm`） | 63（`<form @submit.prevent="confirm">`） | — | ➖ | GPUI Kit 的 Dialog 没有表单提交；下拉自己的 Enter 是「选中这一项」。按钮点击路径完整（77） |
| `invoke('get_retry_layouts', {runId, pageIndex})` | 37-39 | step2.rs::RetryLayoutDialog::load → `run_bg` → `generation::get_retry_layouts` | ✅ | 同步重活走 `run_bg`，不阻塞 UI 线程 |
| `open()` 重置（清候选/选中/错误、置 loading） | 26-33 | 同上 | ✅ | |
| `requestVersion` 作废迟到结果（关掉后再回来、连开两次） | 19-21、28、40、44、47 | step2.rs::RetryLayoutDialog::version | ✅ | |
| `if (!runId) throw '缺少生成记录，请重新生成'` | 36 | 同上（`None` 分支） | ✅ | 逐字 |
| 候选为空 →「这一页暂无可用布局」 | 42-43 | step2.rs::RetryLayoutDialog::apply | ✅ | 逐字 |
| `confirm()`：选中项必须是候选之一 → `emit('confirm', index, kind)` → 关闭 | 51-55 | step2.rs::RetryLayoutDialog::confirm → `GenerationStore::retry_page` | ✅ | 关掉对话框后才交给 store（`window.close_dialog`） |
| `onBeforeUnmount` → `requestVersion++` | 57 | — | ✅ | 原生没有卸载钩子；实体随下一次 `open` 重建，等价 |
| 视觉细节（对话框底色/边框/圆角/内边距 24px、`::backdrop` 50% 黑、下拉样式、说明的不透明度 .8） | 84-92 | step2.rs::show_retry_dialog / RetryLayoutDialog::Render | ✅ | 外框由 GPUI Kit 的 Dialog 画（`overlay(true)` = 遮罩）；说明用 muted 色替代 `opacity: .8` |

---

### 三、`src/stores/generation.ts`（行为契约 → `generation.rs`）

原版这个 store 是应用级 pinia store；原生是应用级的 `GenerationStore`（Shell 创建、`ShellHandle::generation()` 下发）。
第十四轮 `generation.rs` 不在修改范围（本节只做对照、问题记 ⚠️）；第十五轮的静态缓存改动就落在 `generation.rs`（`StaticCache` / `snapshot_for` 静态分支），本节已同步为 🔧。

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 五类事件监听注册（`gen:progress` / `gen:log` / `gen:slide_ready` / `gen:page_status` / `gen:error`） | 115-211 | generation.rs::GenerationStore::new + `tasks::listen_events_in` | ✅ | 原生事件通道常驻（不随页面来去），由 `receiving` 门控 |
| `gen:progress` → stage/message、currentRun 状态与 `updated_at`、progress（非 null 才写） | 119-134 | generation.rs::on_progress | ✅ | |
| `gen:log` → 追加日志、必要时建立/更新 `currentRun`（含 `project_id` 用 `activeProjectId`） | 136-160 | generation.rs::on_log | ✅ | |
| `gen:slide_ready` → 按下标落位（前面补空位） | 163-177 | generation.rs::on_event(SlideReady) + ensure_slot | ✅ | `completedSlides` 有意不移植（见下） |
| `gen:page_status` → 写逐页状态（连续 error 保留最早的 `previousStage`）+ 带蓝图时落位 | 179-195 | generation.rs::on_event(PageStatus) + core `apply_page_status_event` | ✅ | 规则在 core，有快照单测 |
| `gen:error` → stage=error / message / lastError / running=false / currentRun=error | 197-210 | generation.rs::on_event(Error) | ✅ | |
| `stopListening`（生成结束、切项目时不再收事件） | 213-224、256、295、382 | generation.rs::receiving 标志（finish_generate / reset / load_from_deck） | ✅ | 原生 store 不 reset 时保持接收，语义见「有意偏离」行 |
| `generate` 守卫（`running \|\| retryingPages.size > 0` 直接返回） | 240 | generation.rs::generate（`busy()`） | ✅ | 与 core 的 `generation_running` 同一条限制 |
| `generate` 初始化（version++、清牌组/日志/逐页状态、`activeProjectId = projectId`、state=start） | 242-254 | generation.rs::generate | ✅ | |
| `generate` 调 `generate_slides(mdContent, frontmatterTitle, granularity, aspectRatio)` | 259-264 | generation.rs::generate → `run_async_in` + `generation::generate_slides` | ✅ | 参数与 core 服务同名同义 |
| 生成结束后取「这次运行的牌组」 | 265-271 + 234-237 | generation.rs::generate → `get_generation_deck(project_id)` | ✅ | 有意换掉原版的 `get_blueprints`（当前打开的项目）：core 已按运行所属项目解耦，生成中打开别的项目不会读到别人的牌组（CLAUDE.md「core 生成解耦」） |
| 身份比对（用户中途打开别的项目 → 不写 store、不覆盖 stage） | 269、279 | generation.rs::finish_generate（`version == self.version`） | ✅ | 等价：原生 store 只描述它自己的项目，打开别的项目不会改 version（有意偏离 1） |
| 成功收尾（牌组落地、非 error 的页置 done、stage=done、progress=1、`生成完成，共 N 张幻灯片`） | 272-277 | generation.rs::finish_generate(Ok) | ✅ | |
| 失败收尾（lastError/stage/message；把「已组装出来的牌组」捡回来供单页重试，只要首张是封面） | 278-292 | generation.rs::finish_generate(Err) + first_is_cover | ✅ | |
| `finally`：`running=false` + `stopListening` + 依次执行排队中的重试 | 293-300 | generation.rs::finish_generate + drain_queued_retries | ✅ | 一次跑一个、跑完再 drain（与 `await` 循环等价）。顺序差异：原版 `Map` 是入队序，原生 `BTreeMap` 按页号（1 页 → 3 页 vs 3 页 → 1 页），两者都会跑完，用户可见结果相同 |
| `executePageRetry`：状态置 `content/正在重试该页` → 调 `retry_generation_page({runId, pageIndex, layoutKind})` | 303-313 | generation.rs::execute_page_retry | ✅ | 没有当前运行时走 catch 分支，文案「缺少生成记录，请重新生成」一致 |
| 成功：`blueprints[index+2] = slide`、状态 `done/重试完成` | 313-315 | generation.rs::finish_page_retry(Ok) | ✅ | 原版还有 `completedSlides.add(index+2)`（无人消费） |
| `deck_ready` → stage=done / progress=1 / lastError 清空 / run 置 done + `finished_at` /「该页重试完成，可继续编辑」 | 316-325 | generation.rs::finish_page_retry(Ok, deck_ready) | ✅ | |
| 未 ready → `invalid_pages` 置 error「未通过最终校验」、lastError=remaining、「该页已更新；{remaining}」 | 326-334 | generation.rs::finish_page_retry(Ok, !deck_ready) | ✅ | 已是 error 的页不覆盖（照原版判断） |
| 失败 catch：该页 error，`previousStage = previous?.previousStage ?? 'content'` | 335-339 | generation.rs::finish_page_retry(Err) | ✅ | |
| `finally`：`retryingPages.delete(index)`（version 一致时） | 340-342 | generation.rs::finish_page_retry | ✅ | |
| `retryPage` 三条守卫 + 生成中排队（`queuedRetries`） | 345-354 + 103 | generation.rs::retry_page + queued_retries | ✅ | 守卫：本页已在重试 / 不在生成中但已有页在重试 / 本页状态不是 error |
| `repairSlide` | 356-360 | — | ➖ | 原版自己也没有调用处（`grep -rn "repairSlide" src/` 只有 store 里的定义），原生不移植（generation.rs 模块文档第 3 条） |
| `reset()` | 362-378 | generation.rs::reset_inner | ✅ | 清牌组/日志/运行/逐页状态/`activeProjectId`/重试队列 |
| `loadFromJson`（解析牌组 → 内容页全 done、stage=done、`已加载 N 张幻灯片`） | 380-399 | generation.rs::load_from_deck | ✅ | 切项目时先 `stopListening`（原生 `receiving=false`） |
| `loadLatestRun(projectId)`（读最近一次运行 + 日志、从日志恢复失败/修好的页、running 时续接） | 401-435 | generation.rs::open_project + apply_run + `read_run_and_logs` | ✅ | 顺序照原版 `onMounted`：loadLatestRun → loadFromJson/reset → loadLatestRun |
| 日志恢复页状态（`page_error` → error；`retry` → 看 `retry_succeeded`） | 411-420 | core `statuses_from_logs`（generation.rs::apply_run 调用） | ✅ | 规则在 core，有快照单测 |
| `stageLabel` / `slideCount` computed | 106-107 | generation.rs::GenState::stage_label / slide_count | ✅ | `slideCount` 给工作流侧栏用；第二步模板声明了 `slideCount` prop（Step2Generate.vue:49）但模板里没用 |
| `activeProjectId`（这份状态描述哪个项目） | 100、248、269、279、403 | generation.rs::project_id + live_for | ✅ | 原生界面据此区分「store 正持有本项目（实时）/ 用库里的静态状态」 |
| `completedSlides` | 95、176、188、271、314 | — | ➖ | 原版没有任何界面消费（`grep -rn "completedSlides" src/` 只有 store 内部读写），原生不存（generation.rs 模块文档第 3 条） |
| store 的 `progress` 字段 | 93、131-133 | generation.rs::GenState::progress | ✅ | 第二步模板不读它（原版也不读），侧栏/其他页面用 |
| **静态状态**（store 不持有本项目时的看板/详情） | 401-435（原版总会 `loadLatestRun` + `loadFromJson`，日志与逐页状态一定在） | generation.rs::snapshot_for 的静态分支 + `StaticCache`（`:352-372`） | 🔧 | 本轮修：`GenerationStore` 新增静态缓存（`StaticCache` 单槽，打开项目时经 `run_bg` 读最近运行 + 日志一次，运行变了才失效，`snapshot_for` 每帧只读缓存）；静态分支用 `progress::statuses_from_logs` 恢复失败页、关键事件 / 生成记录统计、最近 10 条日志、看板条目。**失败段与「重试这一页」也出现，但按全局规则禁用**：静态分支下 store 的重试会排到别的项目的运行上，故 `!live` 即禁用（「重新生成全部」仅 `busy_elsewhere` 禁用）；6 个新单测（`generation.rs::tests`） |
| 有意偏离：库里停在 `running` 的旧运行按「已中断」显示 | 421-424（原版见到 running 就 `running=true` 等事件） | generation.rs::apply_run | ➖ | 进程崩过时原版会一直转圈；原生保留阶段文案但 `running=false`，且不改库（generation.rs 模块文档第 2 条） |
| 有意偏离：切到别的项目不 reset / 不 `loadFromJson` | 213-224、380-382（原版会把另一个项目在跑的生成从 store 里抹掉） | generation.rs::open_project | ➖ | store 正为别的项目跑时不碰 store，那个项目按库里的数据显示（generation.rs 模块文档第 1 条） |

---

### 四、本轮的修改（🔧）与遗留（⚠️）

#### 🔧 已修

| 原版位置 | 改了什么 |
|---|---|
| Step2Generate.vue:555 | 详情标题的兜底文案：抽出 `plan_title`，`card_title` 兜底「第 N 张」、新增 `detail_title` 兜底「待生成幻灯片」（改前详情也显示「第 N 张」）（`step2.rs`） |
| Step2Generate.vue:557 + CSS 852-872 | 详情头状态 chip 改回中性色：原版 `.sdp-chip` 没有色调类，改前按 `slide_status` 的 tone 上了绿/蓝/红（与版式 chip 的 `--accent` 抢重）（`step2.rs`） |
| Step2Generate.vue:1129-1137 | 窄窗口单列：新增 `NARROW_MAX_W = 1200.0`，窗口宽 ≤1200px 时工作区改用 `flex_col`（详情整宽排在看板下方、两者 `flex_1` 平分高度），宽窗口仍是 1.5fr / 0.9fr 两栏（`step2.rs`） |
| RetryLayoutDialog.vue:70 | 下拉 `autofocus`：候选装载完成后聚焦下拉——`let focus = select.read(cx).focus_handle(cx); window.focus(&focus, cx);`（`SelectState` 实现了 `Focusable`，见 gpui-component 的 `impl Focusable for SelectState`）（`step2.rs`） |
| WorkflowView.vue:500、505 | 选中下标跨步骤保留：下标移到 `ProjectModel::selected_slide`（`Option<usize>`），`Step2::select` 写、渲染归位（越界/未选回落 `preferred_slide_index`）、`WorkflowPage::start_generation` 整次重新生成前清空；4 个新单测（`resolve_selected`）（`mod.rs` / `step2.rs`） |
| stores/generation.ts:401-435 | store 不持有本项目时的静态状态：`GenerationStore` 新增静态缓存（`StaticCache` 单槽，打开项目时经 `run_bg` 读最近运行 + 日志一次，运行变了才失效，`snapshot_for` 每帧只读缓存）；静态分支用 `progress::statuses_from_logs` 恢复失败页、关键事件 / 生成记录统计、最近 10 条日志、看板条目；失败段与「重试这一页」出现但按全局规则禁用（`!live` 即禁用，因为静态分支下 store 的重试会排到别的项目的运行上；「重新生成全部」仅 `busy_elsewhere` 禁用）；6 个新单测（`generation.rs::tests`）（`generation.rs` / `step2.rs`） |

#### ⚠️ 未修

本轮**没有遗留 ⚠️**：原来的 3 条 ⚠️ 中，「选中页跨步骤保留」与「store 不持有本项目时的静态状态（含连带的重试按钮）」已 🔧；「卡片 Tab 键盘可达」按新原则归入 ➖（表现层，按 GPUI 原生做法）。

#### 与 parity.md 第二步小节的核对

12 行逐条核对（顶部状态与统计、生成流水线与事件、整体进度/五阶段/查看详情、看板卡片、只渲染可见卡片、
键盘 ↑/↓、详情大图/标题/状态/版式、详情三段 + 调试 + 日志、单页与换版式重试、重新生成全部、前往编辑）：
**没有发现「标了 ✅ 但其实没做完」的行**。两点措辞瑕疵（不构成不实）：
- 「顶部状态与统计」行写「数字用主色」，实际只有「生成记录」一个数字用主色（另两个是文字色，Step2Generate.vue:422-433 + CSS WorkflowView.vue:1386-1394）。
- 「详情：大图、标题、状态、版式」行在本轮修复前，兜底标题与状态 chip 配色与原版有差距（已 🔧），该行当时未记这两个差距。

第十五轮同步（`parity.md` 第二步小节）：①「生成状态是应用级 store」那行的**已知偏差**（其他项目的第二步只有静态快照、缺日志 / 逐页状态 / 失败段）已修，该行改为 ✅；② 新增一行「选中页下标跨步骤保留」（`ProjectModel::selected_slide`）。

#### 附带发现（不在我负责范围，供参考）

- `docs/native/reference/project-step2.png` 与 `project-step1.png` **字节完全相同**（都是第 1 步「准备素材」的画面）；第二步的原版对照图实际是 `step2-board.png` / `step2-detail.png`。原生侧 `docs/native/shell/project-step2-0.png`（正常态）与 `project-step2-error-0.png`（失败态）是对的。

#### 本轮的验证情况

- 第十四轮只改了 `apps/huanshu-native/src/pages/workflow/step2.rs`；改完 `rustfmt --edition 2021`（解析通过、无格式差异），并补上了 `Focusable as _` 的导入（原文件没有这个 trait，`focus_handle` 需要它）。
- 第十五轮的三处 🔧 跨 `pages/workflow/mod.rs`（`selected_slide` / `start_generation` 清空 / `show_step`）、`pages/workflow/step2.rs`（`resolve_selected`、静态快照的失败段禁用）与 `generation.rs`（`StaticCache`）三个文件，共 10 个新单测（`resolve_selected` 4 个、`generation.rs::tests` 6 个）；这些改动由本轮负责人验证（`cargo test` 见其汇报），审计侧只同步文档。
- 两轮都**没有跑** `cargo check / build / test`（共享 target 目录，按约定由集成方统一验证），也**没有**跑 `tools/shots.ps1` / `drive.ps1`（会触发编译）；审计侧不改代码。第十四轮改动建议集成后用 `tools/drive.ps1 -Route project:1:2` 走一遍：详情标题（选一张还没规划结果的页应显示「待生成幻灯片」）、详情头状态 chip 是中性色、窗口拉窄到 1200px 及以下时两栏变一栏、「重试这一页」对话框打开后焦点在下拉上。
- 纯逻辑部分（看板条目、逐页状态、整体进度、四张文案表）不在本文件里，由 core 的 `tests/generation_progress.rs` 快照守护。

---

## 第三步「编辑完善」审计（对照 Tauri + Vue 原版）

审计对象（本轮**只读**）：

- `src/views/steps/Step3Editor.vue`（含模板里内联的 KindPicker；`.kp-*` / `.ej-*` / `.edit-*` 的样式都在
  `src/views/WorkflowView.vue` 的全局 `<style>` 里）
- `src/composables/useSlideEditor.ts`（编辑规则契约；Rust 侧 `huanshu_core::slide_edit` 是同一份规则）
- `src/components/SlideFrame.vue`（预览位图容器，Step2 / 演示 / Step3 共用）
- 事件处理在 `src/views/WorkflowView.vue`（`exportPdf` / `presenting` / `currentStep` 等），只在第三步相关处引用

原生对应：`apps/huanshu-native/src/pages/workflow/step3.rs`（本轮唯一改动的文件）；
`slides.rs::slide_frame`、`widgets.rs`、`pages/workflow/mod.rs`（`commit_deck` / `editing_locked`）只读。

方法：

1. 原版三个文件逐字逐段读完；模板里每个可见/可操作项一行，`<script>` 里每个副作用一行。
2. 文案核对用脚本做了一遍：把模板里所有 `>文本<` 与 `placeholder=` / `title=` / `alt=` 抽出来，
   逐个在 `step3.rs` 里找。唯一缺的是「正在读取版式信息…」（见 ➖），
   属性里唯一缺的是 `title="导出为 PDF 文档"`（本轮补上）。
3. 截图对照：`docs/native/reference/project-step3.png`、`step3-kind-picker.png` 与
   `docs/native/shell/project-step3-0.png`、`project-step3-kind-picker-0.png`、`project-step3-images-0.png`。
   原版两张都在 ~1.5x DPR 下取（窗口约 1350 CSS 宽），原生两张也是 ~1.5x（约 1455 CSS 宽），
   所以像素可以直接换算成 CSS px 对比；量到的尺寸都换算过。**两处与当前原版 CSS 冲突的地方**见文末第十节。
4. 需要判定原版 CSS 的真实渲染结果时，用 Edge headless 把同一段 CSS + 同样的 DOM 渲出来量
   （证据文件 `target/audit-flex/probe.html` / `probe.png`）。

状态：✅ 一致 · 🔧 本轮已修 · ⚠️ 有差距未修 · ➖ 有意不做（附原版依据）。
主题相关的纯视觉细节（阴影、渐变、毛玻璃、动画）每个文件归一行「视觉细节」。

**条目统计（105 行）**：✅ 90 / 🔧 5 / ⚠️ 0 / ➖ 10。
分节：`Step3Editor.vue` 模板 ✅ 45 / 🔧 5 / ➖ 9（59 行）；`Step3Editor.vue` script ✅ 26；
`useSlideEditor.ts` ✅ 14 / ➖ 1；`SlideFrame.vue` ✅ 5。
🔧 的明细在第六节；本轮把原 4 条 ⚠️（添加按钮宽度、行号标红、素材小卡悬停底色、左栏行悬停底色）
按新原则归入 ➖「表现层，按 GPUI 原生做法」，另 1 条（应用更改不吸底）已 🔧（见第八节）。

---

### 一、`src/views/steps/Step3Editor.vue` — 模板（用户能看到 / 操作的东西）

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 左栏栏头：`layers` 图标 + 「幻灯片」 + 数量徽章 | Step3Editor.vue:564-566 | step3.rs:render（`col_head` + `widgets::chip(Muted)`） | ✅ | 参考截图里这栏头只有「幻灯片 18」，没有右边的重置按钮（见第十节） |
| 栏头右端「重新生成」按钮（`title="重新生成"`，回第 1 步） | Step3Editor.vue:567-569 | step3.rs:render + `go_to_step_1`(590) | ✅ | 原版点击是 `currentStep = 1`（WorkflowView.vue:545） |
| 左栏列表行：序号 / 标题 / 版式中文名（缺省回落 kind）/ 选中态 | Step3Editor.vue:571-588 | step3.rs:slide_row(2328) | ✅ | 版式名回落逻辑同（587） |
| 行点击选中 | Step3Editor.vue:580 | step3.rs:select(409) | ✅ | 换页丢掉未应用的修改（原版 `initEditorSlide` 如此，无确认框） |
| 行键盘 Enter / Space 选中（`tabindex="0"` + `role="button"`） | Step3Editor.vue:577-582 | — | ➖ | GPUI 列表行没有 Tab 焦点链，行不可聚焦；换页由 ↑/↓（`on_key`）与点击承担。属可达性，不是功能缺失 |
| 行尾垃圾桶：`title="删除"`、只在悬停该行时显示、只剩一页时 disabled + 透明 | Step3Editor.vue:589 + WorkflowView.vue:1931-1948 | step3.rs:slide_row（`group`/`group_hover`、`count<=1` 时 opacity 0） | ✅ | 悬停变红（WorkflowView.vue:1947）是 kit 按钮悬停色改不掉，见「视觉细节」；行本身的悬停底色归 ➖「表现层，按 GPUI 原生做法」 |
| 「新建幻灯片」（`.edit-add-btn` 满宽）→ KindPicker(add) | Step3Editor.vue:594-598 | step3.rs:render + `open_kind_picker(600)` | ✅ | 满宽靠 flex 列的 stretch（原版是 `.edit-add-btn { width: 100% }`）；若实测不成立，加 `.w_full()` 即可 |
| 中栏栏头「实时预览」 | Step3Editor.vue:603-604 | step3.rs:render（`col_head`） | ✅ | |
| 「● 未保存」 | Step3Editor.vue:605 | step3.rs:render（`has_unsaved`） | ✅ | 原版 `.edit-unsaved` 在任何 CSS 里都没有规则（全局 grep 0 命中）→ 继承栏头的 muted 色，原生同 |
| 「渲染中…」（`renderBusy`，150ms） | Step3Editor.vue:606, 488-504 | step3.rs:sync_render_busy(751) | ✅ | 原版有 `role="status"`，GPUI 无此机制 |
| 翻页 `← N / M →`（两端禁用） | Step3Editor.vue:607-615 | step3.rs:render（`preview-prev/next`） | ✅ | 禁用条件逐条一致（`activeSlide === 0` / `>= slideCount-1`） |
| 「导出 PDF」（icon 随 `exportingPdf` 在 renew / document-pdf 间切、`title="导出为 PDF 文档"`） | Step3Editor.vue:616-623 | step3.rs:render + `export_pdf(786)` | 🔧 | **本轮修**：补上 `tooltip("导出为 PDF 文档")`；禁用语（`length===0 \|\| exportingPdf`）本来就一致。导出的是**内存里的牌组**，有未应用修改时结果弹窗里加一行「编辑器里还有未应用的修改，PDF 按应用前的内容导出。」（WorkflowView.vue:83-86 ↔ step3.rs:806-807，文案逐字一致） |
| 「演示」（`btn-primary`，`length===0` 禁用） | Step3Editor.vue:624-626 | step3.rs:present(771) | ✅ | 演示用**内存里的牌组**（含未落库的新建页），从当前页开始 |
| 预览舞台：按当前页 `aspect_ratio` 定框、四周 16px、只放位图 | Step3Editor.vue:628-646 + useSlideEditor.ts:102-120,250-251 | step3.rs:render（`stage_w/h` + `slides::slide_frame`） | ✅ | 原版按 `previewArea` 实测算 `previewFit`，原生按视口算，结果等价（缺省 16:9 回落也一致） |
| 舞台里的加载 / 失败重试（`SlideFrame`） | Step3Editor.vue:638-644 | slides.rs:slide_frame(213) | ✅ | 300ms 延迟「渲染中…」、有旧图时角上「渲染失败 · 重试」都在 `SlideFrame.vue:32-56,73-89` 对照下逐条一致 |
| 裁切提示（`warning-alt` + 文案） | Step3Editor.vue:647-650 | step3.rs:render（`preview.overflow(0)`） | ✅ | 原版颜色是硬编码 `#b45309`（WorkflowView.vue:2018-2020），原生用主题 `warning`，见「视觉细节」 |
| 右栏栏头「编辑 第 N 张」 | Step3Editor.vue:654-656 | step3.rs:render（`col_head`） | ✅ | |
| 栏头上移 / 下移 / 删除（禁用条件） | Step3Editor.vue:657-667 | step3.rs:render（`slide-up/down/del`） | ✅ | 删除另加 `locked`（原版没有生成中只读，见 parity.md:144 的有意偏离） |
| 删除按钮是 error 色 | WorkflowView.vue:2048 | step3.rs:render（`carbon("trash-can").text_color(c.error)`） | 🔧 | **本轮修**：原来用通用 `widgets::icon_button`（默认色），现在贴 `.edit-del-btn { color: var(--studio-error) }` |
| 版式表未就绪两态：「读取版式信息失败：…」/「正在读取版式信息…」 | Step3Editor.vue:670-677 | — | ➖ | 原生 schema 是同步取（step3.rs:244 `editor_schema::editor_schema()`），不存在这两个状态；失败即 panic，不可能出现 |
| 幻灯片类型行：图标 + 中文名 + `kind` + 「更换」 | Step3Editor.vue:680-690 | step3.rs:render_form(1856) | ✅ | 「更换」在 `locked` 时禁用（原版没有锁） |
| 编辑模式行 + 「打开高级 JSON / 收起高级 JSON」+ 说明文案 | Step3Editor.vue:692-703 | step3.rs:render_form（`toggle-advanced`） | ✅ | 文案逐字一致 |
| 基础信息组（标题 + 「优先编辑用户能理解的内容字段」+ 两列网格） | Step3Editor.vue:705-742 | step3.rs:render_form ③ + fields_grid(1512) | ✅ | 原版条件是 `v-if="currentDraft"`，19 个版式都至少有一个简单字段（`title`），所以条件恒真，原生改成 `!simple.is_empty()` 结果相同 |
| 简单字段三种控件：textarea（`rows` 缺省 3）/ select（空值「请选择」）/ text（placeholder） | Step3Editor.vue:713-738 | step3.rs:field_card(1431) | ✅ | `editor_schema` 的 15 个简单字段里没有 `boolean`（快照核对），所以原版简单字段只有这三型 |
| 标签组（`badges` 在 kind.fields 里才显示）：每项输入框 + 删除、末尾满宽「添加标签」 | Step3Editor.vue:744-766 | step3.rs:render_form ④ + string_list(1539) | ✅ | 显示条件：`kindFields` 含 `badges`（cover / closing） |
| 图片素材组：显示条件（`kindFields` 含 `images` **或** 当前页 `image` 非空） | Step3Editor.vue:768 | step3.rs:render_form ⑤ | ✅ | |
| 主图输入框（`placeholder="media:12 或 https://..."`） | Step3Editor.vue:774-783 | step3.rs:render_form ⑤ | ✅ | |
| 图集列表（`showImages` 才有）：每项输入框 + 删除 + 末尾「添加图集图片」 | Step3Editor.vue:785-801 | step3.rs:string_list（label 为空 → 末尾满宽） | ✅ | |
| 素材条（有项目图片素材才显示）：缩略图 + 名字 + 「设为主图」/「加入图集」 | Step3Editor.vue:802-816 | step3.rs:render_form ⑤ + `widgets::media_tile` | ✅ | 「加入图集」只在 `showImages` 时显示、去重逻辑在 `append_gallery_image(499)` |
| 列表区：区标题 + 「添加」、每项卡片「`{label} {n}`」+ 上移/下移/删除 | Step3Editor.vue:819-847 | step3.rs:list_block(1628) + list_card(1675) + list_item_actions(2579) | ✅ | 上移 `index===0`、下移 `index>=len-1`、删除不禁用，逐条一致 |
| 列表区空提示 `emptyText` | Step3Editor.vue:831 | step3.rs:list_block（`count == 0`） | ✅ | |
| `issue_stack` 的 `cards` 区提示（按张数二选一） | Step3Editor.vue:826-830 | step3.rs:issue_stack_hint(2462) | ✅ | 两句逐字一致 |
| 卡片字段网格（textarea 占整行 / select / boolean 自带标签 / text） | Step3Editor.vue:848-887 | step3.rs:fields_grid + field_card | ✅ | 快照里 `boolean` 只出现在列表区与对象区子列表（`focus_example.ranking.muted`、`compare.*.items.highlight`），简单字段里没有——所以原版「简单字段只有 text/textarea/select」这条推断成立 |
| 图标助手（`iconQueryKeys` 有值才有）：按钮「AI 推荐图标 / 推荐中…」+ 候选小片 | Step3Editor.vue:889-912 | step3.rs:icon_box(959) | ✅ | 候选小片 = 图标预览 + 去掉 `i-carbon:` 的名字（1015-1018） |
| 卡片里的字符串列表（每个 stringList 一行标签 + 「添加」+ 每项输入框 + 删除） | Step3Editor.vue:914-937 | step3.rs:string_list（有 label → 标签行） | ✅ | 标签行「添加」的宽度差异归 ➖「表现层，按 GPUI 原生做法」 |
| 嵌套列表（`nestedLists`）：卡片标题 + 删除（**没有**上移/下移）+ 字段 | Step3Editor.vue:939-981 | step3.rs:nested_block(1737) | ✅ | 原版嵌套字段只有 text / textarea（`process.steps`），原生用通用 field_card，多支持的类型用不到 |
| 对象区（`objectSections`）：区标题（无添加按钮）+ 字段网格 + 子列表区 | Step3Editor.vue:985-1149 | step3.rs:object_block(1817) | ✅ | 对象区各 def 的 `stringLists` 在快照里都是空（原生多渲染了一轮，无副作用）；boolean 不显示上方小标签也一致（992-1016） |
| 子列表区：区标题 + 添加 + emptyText + 卡片（上移/下移/删除 + 字段 + 图标助手 + 字符串列表） | Step3Editor.vue:1030-1148 | step3.rs:list_block + list_card | ✅ | |
| 图表数据（JSON）框：标签 + hint 两态（「正在校验…」/「type / data / encoding / series，校验通过才会应用到预览」） | Step3Editor.vue:1151-1158 | step3.rs:render_form ⑧ | ✅ | |
| 图表 JSON 框本体：`rows="12"`、`spellcheck="false"`、占位 JSON、错误时整框变红 + 「JSON 语法错误：…」/「问题；问题」 | Step3Editor.vue:1159-1174 | step3.rs:render_form ⑧ + `TextareaState::rows(12)` + `error_row` | ✅ | 等宽字体（`.ej-chart-json` 的 `--font-code`）与 12.8px 字号都对上了（WorkflowView.vue:2181-2187） |
| 高级 Blueprint JSON：标签 + 「保留给高级用户和特殊字段」+ 「格式化」 | Step3Editor.vue:1177-1184 | step3.rs:render_form ⑨ + `format_json(1220)` | ✅ | |
| 高级 JSON 编辑框：行号栏 + 「第 N 行：{原因}」+ 语义错误块（`warning` 图标 + 每条一行） | Step3Editor.vue:1185-1217 | step3.rs:render_form ⑨ + `json_error_text(2535)` + `semantic_errors_block(2496)` | ✅ | 原版行号栏会把出错的那一行标红（已归 ➖「表现层」） |
| 「应用更改」三态（保存中… / 应用更改 (Ctrl+S) / 已保存）+ 禁用条件 | Step3Editor.vue:1220-1229 | step3.rs:render_form ⑩ | ✅ | 禁用语一致：`jsonError \|\| !hasUnsavedChanges \|\| editorSaving`（原生另加 `locked`） |
| 「应用更改」按钮不吸底（长表单要滚到底才能点） | WorkflowView.vue:2061-2065 | step3.rs:render（`render_actions`：滚动表单区 + 固定底操作区） | 🔧 | **本轮修**：右栏拆成「滚动表单区 + 固定底操作区」（应用更改三态按钮 + locked 提示 + 保存错误行），不模拟 `.ej-save-btn { position: sticky }`；短表单时操作区自然落栏底 |
| 标签行里的「添加」按钮宽度 | WorkflowView.vue:1956-1959 | step3.rs:string_list / list_block / nested_block（紧凑靠右） | ➖ | 表现层，按 GPUI 原生做法 |
| 高级 JSON 出错行的行号标红 | WorkflowView.vue:2352-2356 | — | ➖ | 表现层，按 GPUI 原生做法 |
| 图片素材小卡悬停底色 | WorkflowView.vue:2538-2540 | step3.rs:render_form ⑤（`widgets::media_tile`） | ➖ | 表现层，按 GPUI 原生做法 |
| 左栏行悬停底色（仅 notes 主题下原版有） | WorkflowView.vue:1895 + src/themes/notes.css:83 | step3.rs:slide_row | ➖ | 表现层，按 GPUI 原生做法 |
| 保存失败时按钮下方的错误行 | useSlideEditor.ts:175-176（`jsonError = String(e)`） | step3.rs:render_form ⑩（`deck_save_error`） | ✅ | 原版只在**展开高级 JSON**时才看得见这个错误（模板里 `v-if="jsonError"` 在 1206，属于高级段），原生常显；功能不丢，算改进 |
| KindPicker 头部：模式图标（`add-alt` / `arrows-horizontal`）+ 标题 + 关闭按钮 | Step3Editor.vue:1238-1244 | step3.rs:kind_picker_body(2644) | ✅ | 标题两种模式逐字一致 |
| KindPicker 副标题：add →「新幻灯片将插入到第 N 张之后」/ change →「选择新类型后将自动保留可迁移的公共字段」 | Step3Editor.vue:1245-1248 | step3.rs:kind_picker_body（`after = active + 1`） | ✅ | |
| KindPicker 网格：3 列、图标 + 中文名 + kind + 说明、change 模式高亮当前项、点一下即选中 | Step3Editor.vue:1249-1261 | step3.rs:kind_picker_body（`chunks(3)`、`is_current`） | ✅ | 3 列（`repeat(3, 1fr)`）与「当前项在 change 模式才高亮」逐条一致 |
| KindPicker 卡片悬停高亮（边框与底色变主色） | WorkflowView.vue:2648 | step3.rs:kind_picker_body（`.hover(...)`） | 🔧 | **本轮修**：原来没有悬停反馈 |
| KindPicker 卡片图标大小（`.kp-card-icon` = 1.5rem = 24px） | WorkflowView.vue:2651 | step3.rs:kind_picker_body（`with_size(Size::Large)`） | 🔧 | **本轮修**：原来跟随文字大小只有 ~15px（截图量得 12.7 vs 20 CSS），差 1.6 倍 |
| KindPicker 关闭：点遮罩 / 点右上 X | Step3Editor.vue:1236,1241 | step3.rs:open_kind_picker 的 `on_close` + 栏头 X + `window.close_dialog` | ✅ | **原版这两处是 bug**：`@click.self="$emit('open-kind-picker', kindPickerMode)"` 写成了再次打开，点遮罩和 X 都关不掉；原生按「关闭」实现（代码注释已写明） |
| 图表校验的失败出口「校验失败：…」 | Step3Editor.vue:313-317 | — | ➖ | 原版经 `invoke('chart_problems')`，命令可能失败；原生直接调 core 纯函数（step3.rs:1160），没有这条路径 |
| `aria-*` / `role="button"` / `role="status"` 等无障碍属性 | Step3Editor.vue:577-582, 606, 647, 826 | — | ➖ | GPUI 的可访问性树由框架生成，页面不写这些属性 |
| KindPicker 弹窗里的 700/600 字重（被 `Teleport to="body"` 挪出 `#app`，不受 `#app *` 拍平） | WorkflowView.vue:2608-2613, 2653-2657 | — | ➖ | README 规矩 12 已声明：原生一律常规体，靠颜色/字族/字号强调 |
| 视觉细节 | 见备注 | — | ✅ | 折进来的：阴影/圆角/过渡动画（`.edit-preview-stage` 的 `box-shadow` + `#fff` 底、按钮 transition、`.edit-rendering` 的淡入）；`transform: translateY(-1px)` 之类的悬停位移；**行尾垃圾桶悬停变红**（`.edit-row-del:hover` 的 error 前景/底色，kit 按钮的悬停色在组件内部，页面改不掉）；字号零头差异（`.edit-col-head` 0.68rem→原生 11.5px、`.edit-row-title` 0.72rem→12、`.edit-row-kind` 0.58rem→10、`.ej-qf-label` 0.58rem→10.5、`.ej-label` 0.68rem→11.5、输入框 0.75rem→kit 默认、`.kp-card-key/desc` 0.58/0.62rem→10.5）；`.edit-preview-clip` 的 `#b45309` 硬编码改成主题 `warning`；KindPicker 的头部图标/卡片图标在参考截图里是文字色而当前 CSS 写的是 `--studio-primary`（第十节，原生跟截图） |

### 二、`src/views/steps/Step3Editor.vue` — `<script>` 的副作用

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| `KIND_META` / `ALL_KINDS` / `KIND_FIELDS` / `SIMPLE_FIELDS` / `LIST_SECTIONS` / `OBJECT_SECTIONS` / `TONE_OPTIONS` / `DIRECTION_OPTIONS` 来自 Rust schema | Step3Editor.vue:12-23 | step3.rs:244（`editor_schema::editor_schema()`） | ✅ | 原生直接持有 schema，没有 store 缓存层 |
| `parsedJson`（jsonText 的解析）/ `currentDraft`（liveSlide 优先） | Step3Editor.vue:108-112 | step3.rs:`draft` / `chart_json_of` / `draft_chart` | ✅ | |
| `errorLine`：从报错文本里抠 `line N` / `position N` | Step3Editor.vue:120-131 | step3.rs:json_error_text(2535) | ✅ | 原生用 serde_json 的 `error.line()`，没行号时只给原因 |
| `syncSemanticErrors`：`direction` 非法、`tone/footer_tone/label_tone` 非法（递归）、`chart_focus` 缺 `chart` | Step3Editor.vue:133-166 | `huanshu_core::slide_edit::semantic_errors`（本文件 step3.rs:1278 调用） | ✅ | 规则在 core，有快照单测（`crates/huanshu-core/tests/slide_edit.rs`） |
| `advancedChartErrors`：`invoke('chart_problems', { chart })`，150ms 防抖、seq 作废、问题前缀 `chart：` | Step3Editor.vue:169-196 | step3.rs:schedule_json_chart_check(1237) | ✅ | 原生直接调 `chart_check::chart_problems_json`（少一次序列化） |
| 图表框的 watch（`[editorKind, chart]`，`deep` + `immediate`，有焦点跳过） | Step3Editor.vue:256-264 | step3.rs:sync_chart_box(1058) + `reset_chart`(1071) | ✅ | |
| `onChartJsonInput`：语法错**立即**报「JSON 语法错误：…」并作废等待中的校验；语法对则排防抖 | Step3Editor.vue:266-277 | step3.rs:on_chart_text_changed(1078) | ✅ | |
| `cancelChartCheck`：清计时、清 checking、seq +1 作废在途 | Step3Editor.vue:279-287 | step3.rs:cancel_chart_check(1115) | ✅ | |
| `scheduleChartCheck`：记下 `pendingChart` 与 `pendingSlide`，等待期间**不写回** | Step3Editor.vue:289-298 | step3.rs:schedule_chart_check(1091) | ✅ | |
| `flushChartCheck`：失焦时立即校验还在等待的输入 | Step3Editor.vue:300-305 | step3.rs:on_chart_blur(1125) | ✅ | 粘贴后马上点别处不丢修改 |
| `runChartCheck`：只认最新 seq、只写回发起时那一页、只有通过校验才 `updatePath('chart')`、失焦后规范化输入 | Step3Editor.vue:308-330 | step3.rs:run_chart_check(1151) | ✅ | |
| `onChartJsonBlur`：有错误保留输入让用户继续改 | Step3Editor.vue:332-341 | step3.rs:on_chart_blur | ✅ | |
| `replaceDraft` / `cloneDraft` / `updatePath`（走 `setAtPath`） | Step3Editor.vue:221-234 | step3.rs:set_path(427) + `slide_edit::set_at_path` | ✅ | |
| `addItem` / `removeItem` / `moveItem` / `addStringItem` / `updateStringItem` / `removeStringItem` | Step3Editor.vue:362-412 | step3.rs:456-494（`slide_edit::*`） | ✅ | 越界时原版不动，core 返回 `None` → 原生也不动 |
| `setPrimaryImage` / `appendGalleryImage`（**已有的不重复**） | Step3Editor.vue:418-429 | step3.rs:`set_path("image")` / append_gallery_image(499) | ✅ | |
| `buildIconQuery`：`iconQueryKeys` 各字段 trim 后非空用空格连接 | Step3Editor.vue:431-436 | step3.rs:icon_query(843) | ✅ | |
| `recommendIcon`：查询词空就不发；`invoke('recommend_icons_for_query', {query, limit: 6})`；候选按卡片路径存；**第一个候选直接写进 `{path}.icon`** | Step3Editor.vue:438-451 | step3.rs:recommend_icon(860) + finish_icon_recommendation(905) | ✅ | 原生加了三重身份判断（页 + 表单代次 + 项目），原版只靠 `iconLoadingKey` |
| `applySuggestedIcon`：点候选写进 `{path}.icon` | Step3Editor.vue:453-455 | step3.rs:apply_suggested_icon(946) | ✅ | |
| `setActiveSlide`：越界 / 同值直接 return | Step3Editor.vue:457-461 | step3.rs:select(409) | ✅ | |
| `useTypstThumbnails`：只发当前页、`serial` + `progressive`、`debounceMs: 120`、`resetKey: activeSlide`、宽度 = 舞台宽 × dpr、`overflow[0]` | Step3Editor.vue:470-509（命令是 `invoke('render_slide_thumbnails')`） | step3.rs:`SlideImages`（`new` 里 `max_cached: Some(4)`、`set_wanted(&[0])`、`schedule_preview` 的 120ms、`init_draft` 里的 `reset`）→ `app::render_pages`（`crate::tasks::run_bg`） | ✅ | `PreviewOptions::default()` 的 serial/progressive 都是 true（core `preview.rs:35-44`）；失败不自动重试（只给「重试」按钮） |
| `renderBusy`：`pending` 持续 150ms 才置真 | Step3Editor.vue:488-504 | step3.rs:sync_render_busy(751) | ✅ | |
| `handleEditorArrowKeydown`：window 级；KindPicker 开着不处理；带修饰键不处理；焦点在 INPUT/TEXTAREA/SELECT/contentEditable 里不处理 | Step3Editor.vue:511-535 | step3.rs:on_key(701) | ✅ | 原生按 README 规矩 6 用 `track_focus` + `on_key_down`，判据换成「根节点自己有焦点」——等价于原版「焦点不在表单里」 |
| `handleTextareaKeydown`：JSON 框里 Ctrl/⌘+S → 应用；其余转发 | Step3Editor.vue:537-544 | step3.rs:on_key（Ctrl/⌘+S 在**第三步任何位置**都生效） | ✅ | 有意改进（README 与 parity.md:143 已记） |
| JSON 框的 Tab → 插两个空格 | useSlideEditor.ts:147-155 | GPUI Kit `EditorState` 默认 `tab_size = 2`（`gpui-base/src/input/editor/indent.rs:23`，`tab` 绑到 `IndentInline`） | ✅ | |
| `onMounted` / `onBeforeUnmount`：注册、注销 window 监听 + 清 `renderBusyTimer` | Step3Editor.vue:546-556 | step3.rs:render 的 `.track_focus` + `release`(337) | ✅ | |
| 若干 `emit`（`update:activeSlide`、`replace-draft`、`apply-changes`、`delete-slide`、`move-slide`、`go-to-step-1` …） | Step3Editor.vue:47-64, 533-544 | step3.rs 直接调同名方法，经 `WorkflowPage::commit_deck` 落库 | ✅ | |

### 三、`src/composables/useSlideEditor.ts`

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| `SHARED_TEMPLATE_FIELDS`（新页/换版式要迁移的公共字段）与 `NON_CHAPTER_KINDS` | useSlideEditor.ts:11-17 | `huanshu_core::slide_edit::SHARED_TEMPLATE_FIELDS` / `NON_CHAPTER_KINDS`（slide_edit.rs:38,57） | ✅ | 规则在 core，有快照单测 |
| `createKindBlueprint`：schema 的 `kindDefaults[kind]` + 从 seed 里搬非空公共字段 + 强制 `kind` | useSlideEditor.ts:59-70 | `slide_edit::create_kind_blueprint`（slide_edit.rs:356） | ✅ | |
| `filterSlideJson`：按该 kind 的 `fields` 过滤，**额外保留 `aspect_ratio`** | useSlideEditor.ts:72-84 | `slide_edit::filter_slide_json`（slide_edit.rs:379） | ✅ | 两边一起改过（CLAUDE.md 有记） |
| `initEditorSlide`：草稿 = 深拷贝当前页、`jsonText` = 过滤后两空格缩进、清错误、`hasUnsavedChanges = false` | useSlideEditor.ts:86-92 | step3.rs:init_draft(383) + refresh_json_text(1181) | ✅ | |
| `replaceEditorDraft`：草稿与 `jsonText` 一起换、清错误、置未保存 | useSlideEditor.ts:94-100 | step3.rs:mark_unsaved(447) + refresh_json_text | ✅ | 表单每改一次都会重算 `jsonText`（150-452 注释） |
| `updateEditorScale` + `ResizeObserver`（`previewArea` 实测 → `previewFit` → 舞台宽高） | useSlideEditor.ts:102-120 | step3.rs:render（按视口减去三栏 + 栏头 + 16px 边距算） | ✅ | 结果等价；原版按 `ASPECT_DIMENSIONS[ratio]`，缺省 `ratio_16x9` |
| watch `activeSlide`（只在第 3 步）/ watch `currentStep`（进第三步 `nextTick` 后初始化） | useSlideEditor.ts:122-134 | step3.rs:select(409) → `init_draft` | ✅ | 原生没有「步骤」这一层，进第三步就是 `Step3::new` |
| `onEditorJsonInput`：能解析 → 草稿 = 解析结果（**不过滤**）、清错误、未保存；失败 → 记错误、草稿不动 | useSlideEditor.ts:136-145 | step3.rs:on_json_text_changed(1196) | ✅ | |
| `formatEditorJson`：能解析才 `replaceEditorDraft`（会过滤） | useSlideEditor.ts:157-164 | step3.rs:format_json(1220) | ✅ | |
| `applyEditorChanges`：`gen.blueprints[activeSlide] = JSON.parse(jsonText)` → `projectsStore.saveBlueprints`（`invoke('update_project_blueprints')`，紧凑 JSON）→ 成功清未保存 / 失败写 `jsonError` | useSlideEditor.ts:166-180 + src/stores/projects.ts:49-54 | step3.rs:apply(521) → commit(545) → `WorkflowPage::commit_deck`（mod.rs:666）→ `projects::update_project_blueprints` | ✅ | 唯一写入路径。前置条件对照：原版 `jsonError \|\| !liveSlide \|\| !projectId` → 原生 `jsonError \|\| !hasUnsaved \|\| saving \|\| locked`；少的那两条结果一样（草稿为空时 `jsonText` 也是空串、解析不过；没 id 时 `commit_deck` 本来就只改内存不落库），多的 `!hasUnsaved` 只是把「没有改动时按 Ctrl+S」从「重复写一次同样的牌组」变成无操作 |
| `openKindPicker` / `onKindPicked`：change → `createKindBlueprint(kind, liveSlide)`（**只改草稿**）；add → 插到当前页之后、章节号继承上一页（cover/overview/closing 不继承）、只改内存不落库 | useSlideEditor.ts:182-207 | step3.rs:open_kind_picker(600) + pick_kind(651) + `slide_edit::insert_new_slide`（slide_edit.rs:427） | ✅ | 章节号继承规则在 core，有单测 |
| `deleteEditorSlide`：只剩一页不动；删完当前页落到 `min(idx, len-1)`；**立即落库** | useSlideEditor.ts:209-214 | step3.rs:delete_slide(562) + `slide_edit::delete_slide`（slide_edit.rs:453） | ✅ | |
| `moveEditorSlide`：与相邻页交换、当前页跟过去、越界不动、**立即落库** | useSlideEditor.ts:216-223 | step3.rs:move_slide(576) + `slide_edit::move_slide`（slide_edit.rs:467） | ✅ | |
| `insertMediaUrl`：往 JSON 里插引用，没有 textarea 时退化成写剪贴板 | useSlideEditor.ts:225-237 | — | ➖ | 原版模板里**没有**任何触发入口（`grep -n insert-media-url src/views/WorkflowView.vue` 只在 543 行接了个空事件，模板里没有 `$emit('insert-media-url')`），parity.md:142 已记「未做」；原生也没有入口 |
| `editorKind` / `editorPreviewSlide` / `currentSlideDims` / `editorStageW/H` | useSlideEditor.ts:239-251 | step3.rs:`draft` + render 里的 `aspect`/`stage_w/h` | ✅ | |

### 四、`src/components/SlideFrame.vue`

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 有 `src` 时位图铺满整个框（`object-fit: fill`） | SlideFrame.vue:62-69,102-107 | slides.rs:slide_frame（`FrameState::Ready`，`stretch=false` → Contain） | ✅ | 原版 Stage 尺寸本身按页面比例算，所以 `fill` 与原生 `Contain` 在等比例框里结果相同；演示用 `stretch=true` |
| 没有图 + `pending`：延迟 300ms 才淡入「渲染中…」 | SlideFrame.vue:32-56,73 | slides.rs:`PENDING_DELAY` + `track_waiting`(183) + `FrameState::Loading.show_label` | ✅ | 原版 `pendingDelay: 300`；Step3 的栏头「渲染中…」是另一套 150ms 计时（上面已列） |
| 没有图 + `error`：「渲染失败」+「重试」按钮 | SlideFrame.vue:74-77 | slides.rs:`FrameState::Failed` + `on_retry` | ✅ | Step3 传了重试回调（`preview.refresh()`），不自动重试 |
| 有旧图 + `error`：角上「渲染失败 · 重试」（不遮画面） | SlideFrame.vue:81-89 | slides.rs:`FrameState::Ready { failed }` 的右上角浮层 | ✅ | |
| 视觉细节 | SlideFrame.vue:93-158 | — | ✅ | 折进来的：加载框底色（原版 `--studio-bg`，原生 `surface` + 细边）、淡入动画、毛玻璃（`backdrop-filter: blur(2px)`）、失败浮层的图标 |

---

### 五、统计

| 状态 | 条数 |
|---|---|
| ✅ 一致 | 90 |
| 🔧 本轮已修 | 5 |
| ⚠️ 有差距未修 | 0 |
| ➖ 有意不做 | 10 |
| 合计 | 105 |

（统计口径：第一至第四节四张表里的数据行；「视觉细节」行也计入 ✅。本轮没有遗留 ⚠️：1 条已 🔧（见第六节第 5 条），
另 4 条归 ➖「表现层，按 GPUI 原生做法」（见第八节）。）

### 六、本轮已修（🔧，全部在 `apps/huanshu-native/src/pages/workflow/step3.rs`）

1. **「导出 PDF」缺 `title` 提示**
   原版 `Step3Editor.vue:616-623`：`title="导出为 PDF 文档"`。
   改法：`widgets::ghost_button("export-pdf", "导出 PDF")` 链上 `.tooltip("导出为 PDF 文档")`。
   （文案脚本比对时，属性值里唯一缺的就是这一条。）
2. **KindPicker 卡片没有悬停高亮**
   原版 `WorkflowView.vue:2648`：`.kp-card:hover { border-color: var(--studio-primary); background: var(--studio-primary-bg) }`。
   改法：卡片 div 加 `.hover(|style| style.border_color(c.primary).bg(c.primary_bg))`（当前项本来就这个配色，悬停无副作用）。
3. **KindPicker 卡片图标只有 ~15px，原版是 24px**
   原版 `WorkflowView.vue:2651`：`.kp-card-icon { font-size: 1.5rem }` = 24px。
   证据：参考截图 `step3-kind-picker.png` 量到卡片图标的字形高约 30 图像 px（≈20 CSS），原生 `project-step3-kind-picker-0.png` 只有 19 图像 px（≈12.7 CSS），差 1.6 倍。
   改法：`carbon(&entry.icon).with_size(Size::Large)`（`Size::Large` → `size_6()` = 1.5rem；应用的 rem 没被改过，仍是 16px）。
4. **右栏栏头的删除按钮不是红色**
   原版 `WorkflowView.vue:2048`：`.edit-del-btn { color: var(--studio-error) }`（同栏头另外两个按钮是默认色，卡片里的删除按钮也不是红的）。
   改法：这一处不再用通用 `widgets::icon_button`，直接 `Button::new("slide-del").ghost().icon(carbon("trash-can").text_color(c.error)).tooltip("删除")`（新引入 `ButtonVariants as _`、`Size` 两个 import）。
5. **「应用更改」按钮不吸底**（第十五轮修）
   原版 `WorkflowView.vue:2061-2065` 用 `.ej-save-btn { position: sticky; bottom: 0 }` 把按钮吸在右栏底；GPUI 没有 sticky。
   改法：右栏拆成「滚动表单区 + 固定底操作区」（`render_actions`：应用更改三态按钮 + locked 提示 + 保存错误行），
   长表单下按钮始终可见；**有意偏离**：短表单时操作区落在栏底（原版按钮紧跟内容之后），与参考截图不同。
   （前 4 条是第十四轮；改动量 `git diff --stat` = 29 行新增 / 11 行删除（含注释），已 `rustfmt`。未跑任何 `cargo` 命令，按纪律交给集成方验证。）

### 七、有差距未修（⚠️）

本轮**没有遗留 ⚠️**。原 5 条的处理：

| # | 原差距 | 处理 |
|---|---|---|
| 1 | 「应用更改」按钮不吸底（原版 `.ej-save-btn { position: sticky; bottom: 0 }`，`WorkflowView.vue:2061-2065`） | 已 🔧：右栏拆成「滚动表单区 + 固定底操作区」（见第六节第 5 条） |
| 2 | 标签行里的「添加」按钮宽度（原版 `.edit-add-btn { width: 100% }` 占满标签右侧剩余宽度；`WorkflowView.vue:1956-1959`） | ➖：表现层，按 GPUI 原生做法（曾经的探针证据 `target/audit-flex/probe.html`） |
| 3 | 高级 JSON 出错行的行号标红（`WorkflowView.vue:2352-2356`） | ➖：表现层，按 GPUI 原生做法（错误文案里的「第 N 行：」仍在） |
| 4 | 图片素材小卡悬停底色（`WorkflowView.vue:2538-2540`） | ➖：表现层，按 GPUI 原生做法 |
| 5 | 左栏幻灯片行悬停底色（`WorkflowView.vue:1895`；`--studio-hover` 只在 notes 主题定义） | ➖：表现层，按 GPUI 原生做法 |

### 八、有意不做（➖，逐条附原版依据）

1. **左栏行的 Enter / Space 选中**（`Step3Editor.vue:577-582` 的 `tabindex="0"` + `@keydown.enter` / `@keydown.space`）：
   GPUI 里列表行没有焦点链（要一个一个 `track_focus` + Tab 序），换页已由 ↑/↓ 与点击承担；这是可达性增强，不是原版的功能点。
2. **版式表未就绪两态**（`Step3Editor.vue:670-677` 的「读取版式信息失败：」/「正在读取版式信息…」）：
   原生 schema 是同步取（`step3.rs:244`），不存在「正在读取」，也没有可失败的命令调用。
3. **`insertMediaUrl` 与它的剪贴板回退**（`useSlideEditor.ts:225-237`）：
   原版模板里没有任何触发入口（`WorkflowView.vue:543` 接了个空事件，模板里找不到 `insert-media-url` 的发出点），parity.md:142 已记「原版没有触发入口，未做」。
4. **标签行「添加」按钮宽度**（`WorkflowView.vue:1956-1959`）：表现层，按 GPUI 原生做法。
5. **高级 JSON 出错行的行号标红**（`WorkflowView.vue:2352-2356`）：表现层，按 GPUI 原生做法。
6. **图片素材小卡悬停底色**（`WorkflowView.vue:2538-2540`）：表现层，按 GPUI 原生做法。
7. **左栏行悬停底色**（`WorkflowView.vue:1895`，仅 notes 主题下原版有效）：表现层，按 GPUI 原生做法。
8. **图表校验的「校验失败：…」出口**（`Step3Editor.vue:313-317`）：原版经 `invoke('chart_problems')`，命令可能失败；原生直接调 core 的纯函数（step3.rs:1160），没有失败这条路径。
9. **`aria-*` / `role="button"` / `role="status"`**（模板多处，如 606、647、826）：GPUI 的可访问性树由框架生成，页面不写这些属性。
10. **`#app * { font-weight: normal !important }` 之外的字重**：原版 KindPicker 被 `Teleport to="body"` 挪出 `#app`，它的标题/卡片名仍是 700/600（`WorkflowView.vue:2608-2613, 2653-2657`）；原生按 README 规矩 12 一律常规体，这一条 README 已声明为有意偏离。

### 九、`parity.md` 第三步复查

逐行核对 `docs/native/parity.md` 第三步小节的 16 行（第十四轮为 124-141 行，第十五轮插入/改写了几行后行号右移），**都做完了**，没有发现「写了没做」的行。几点补充：

- parity.md 写「行尾垃圾桶只在**悬停该行时**显示」——✅ 代码属实（`group_hover`），但原版在只剩一页时是 `opacity: 0 !important`（`WorkflowView.vue:1948`），原生两条 `.when` 都写了，属实。
- parity.md 写「（`tools/drive.ps1` 逐页截图 + 联系表；联系表是未提交的临时产物）+ compare / swot / process 全分辨率逐项核对」——截图现在还在 `target/drive/`（129 个文件，`target/` 被 gitignore，随时可能被清掉），建议把关键几张（compare / swot / process 的逐项、issue_stack 的两种提示）收进 `docs/native/shell/`，否则这行以后没法复核。**这条不影响功能结论。**
- parity.md 写「`serial` / `progressive` 取 `PreviewOptions::default()`（都是真）」——✅ 属实（`crates/huanshu-core/src/preview.rs:35-44`）。
- parity.md 写「左 `layers` +「幻灯片」+ 数量徽章 + `reset` 图标（回第 1 步）」——✅ 代码属实；**参考截图里看不到这个 `reset` 按钮**（见第十节）。
- 第十五轮改动：parity.md 第三步「应用更改」那行的**已知偏差（原版 sticky 吸底）已删掉**，改为「固定底操作区」的做法说明（另有短表单时操作区落栏底的**有意偏离**）；其它 ⚠️（「添加」按钮宽度、行号标红、素材小卡悬停底色、左栏行悬停底色）在本文件第八节归 ➖「表现层，按 GPUI 原生做法」，parity.md 里本来就没有任何一行声称它们已完成，所以不算「写了没做」。

### 十、参考截图与当前原版 CSS 冲突的地方（供复核，不要据此改代码）

`docs/native/reference/project-step3.png` 与 `step3-kind-picker.png` 里有两处与现在的原版代码对不上，判断为**截图比代码旧或截图环境的问题**，本轮以代码为准：

1. **所有「只有图标的 ghost 按钮」在参考截图里都没渲染出来**：左栏栏头的 `reset`（`Step3Editor.vue:567-569`）、中栏的 `←` / `→`（607-615）、右栏栏头的 `↑`/`↓`/垃圾桶（657-667）在截图里都是空白（用 8 倍放大 + 逐像素扫描确认：这几处最亮的像素只有 (64,75,92)，而同一张截图里 `layers`/`view`/`edit` 这些静态图标正常显示）。同一张截图里同一行的「导出 PDF」图标（`document-pdf`）是正常的。
2. **KindPicker 的头部图标与卡片图标是文字色，而 CSS 里写的是 `--studio-primary`**（`WorkflowView.vue:2612`、`2651`）：截图标到的像素是 (212,232,248)（= 文字色），不是 `#38bdf8`。原生现在跟截图（文字色）；若以后确认应以 CSS 为准，`carbon(...).text_color(c.primary)` 两处即可。

### 十一、改动文件

第十四轮只改了一个文件，没有碰任何负责范围外的文件：

- `apps/huanshu-native/src/pages/workflow/step3.rs`（🔧 4 处 + 1 段已知差异注释；`rustfmt` 过）
- 新增证据文件（`target/` 下，不入库）：`target/audit-flex/probe.html`、`target/audit-flex/probe.png`、`target/audit-n-*.png`、`target/audit-r-*.png`、`target/kp-*.png`、`target/ref-head-icons*.png`

未做的事：没有 `git commit`、没有跑 `cargo build/check/test`、没有改原版 `src/`、没有改 `widgets.rs` / `shell.rs` / `present.rs` / `mod.rs` / `step1.rs` / `step2.rs` / `drive.ps1` / `shots.ps1`。
（`docs/native/parity.md` 在第十四轮未改；第十五轮按本轮口径更新了第三步「应用更改」那一行与向导/设置/首页/第二步的相关行。）

---

## 演示覆盖层逐页审计（原版 Vue → 原生 GPUI）

范围：`src/views/PresentationOverlay.vue`（template + script）与 `.pres-*` 样式（`src/views/WorkflowView.vue:2671-3087`）→ `apps/huanshu-native/src/present.rs`。

- 状态：✅ 一致　🔧 本轮已修　⚠️ 有差距未修　➖ 有意不做（附原版行号依据）
- 截图：`docs/native/reference/present.png`（窗口态）对照 `docs/native/shell/present-full-0.png` / `present-full-hover-0.png`（全屏态）
- 审计时点：本轮 A 节（全屏铺满、浮动控制条、触摸滑动、Esc 语义）已在 `present.rs` 落地后审计。

### 1. `PresentationOverlay.vue` template → `present.rs::render`

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 覆盖层盖在当前页上、打开即全屏（被拒退回窗口态） | PresentationOverlay.vue:138-143、337-344 | `Presentation::new` | ✅ | 原版 `requestFullscreen`；原生 `toggle_fullscreen` |
| 标题栏在演示期间仍可见 | TitleBar.vue:122-127（z-index 10000）> WorkflowView.vue:2682（overlay 2000）；`docs/native/reference/present.png` 可见标题栏 | shell.rs 渲染顺序（标题栏在覆盖层之下一层） | ✅ | 原版覆盖层铺满窗口但标题栏 z-index 更高盖在上面；原生覆盖层渲染在标题栏之下，视觉等价（标题栏不透明） |
| 快捷键提示浮层（3 秒） | :370-379 + CSS :2707-2753 | render（`opened_at` + `SHORTCUT_HINT`） | ➖ | 原版是 5 个 `kbd` 芯片分条；原生合并为一行文字提示（`:263`），无 `kbd` 样式。属视觉形态差异，功能（提示内容、3 秒）一致 |
| 导航箭头（两侧圆形、点击翻页） | :382-383 + CSS :2755-2780 | `nav_arrow` | ✅ | 48px、16px 边距（原版 52px/1rem，见「视觉细节」） |
| **点箭头会连带触发舞台点击（一次跳两页）** | 原版 `.pres-nav` 与 `.pres-stage` 平级（:382-386），箭头点击不会到舞台 | render（本轮给箭头加 `.occlude()`） | 🔧 | 原生把箭头放进舞台内部，舞台 `on_mouse_down` 与箭头 `on_click` 都收到：右箭头一次 +2 页。本轮修：`.occlude()` 挡舞台命中，实测 +1/−1（`target/drive/bug-*`、`a1/a2-*`） |
| 舞台点击下一页 | :386、261-265 | render（`on_mouse_down`） | ✅ | `didSwipe` 吞点击本轮已接入（见触摸行） |
| 拉伸/等比舞台几何（`stretchedViewport` padH 96 / padV 88，全屏 0） | :51-83 | `stage_size` | 🔧 | 改前不管全屏都减控制条+标题栏；改后全屏铺满、非全屏减 96/88（按覆盖层区域口径，即原版口径再减原生多出的 36px 标题栏，见 `present.rs` 注释） |
| 舞台背景 | CSS :2782-2790 | render（`.bg(rgb(0x000000))`） | ➖ | 原版覆盖层背景是 bg + 顶部径向渐变 + 面板线性渐变（CSS :2672-2687）；原生舞台区纯黑底。视觉细节，渐变省略 |
| 触摸滑动翻页（\|dx\|>\|dy\| 且 \|dx\|>50，左滑下一页；didSwipe 吞随后的点击） | :37-41、267-281 | `on_swipe` + `Swipe` 状态机 | ✅ | 鼠标滚轮不触发；5 个单测。真机触摸未验证（本机无触摸屏）→ 人工验收 |
| 全览（O）：标题/副标题、按页比例缩略图、点击进入 | :412-441 + CSS :2792-2888 | `overview_grid` | ✅ | 原版缩略框 256×144 按页比例适配（`overviewBoxSize` :196-200）；原生 CARD_W 240 定宽、高按比例。见「视觉细节」 |
| 胶片条（页码方块、点击跳页、`title` 提示） | :446-457 + CSS :2977-3006 | render（`pres-filmstrip`） | ➖ | 跳页功能一致；原版每格有 `:title="{idx+1}. {bp.title}"` 悬停提示、36×28、圆角 4px；原生无 tooltip、高 26px、圆角 5px。视觉细节 |
| 页码 + 进度条 | :460-470 + CSS :3015-3044 | render | ✅ | 原生进度条容器 110px（原版 80px，见「视觉细节」） |
| 主题圆点（切换并保存） | :472 | render（`theme_dots`） | ✅ | |
| 全览/拉伸/全屏/退出按钮（title 文案逐字） | :474-489 | render（`icon_button`×4） | ✅ | 原版全览图标 `app-switcher`、等比图标 `crop`；原生用 `grid`/`fit-to-width`，同义图标（原 parity 已记） |
| 全屏浮动控制条（半透明 72%、上边框 45%、默认透明下移 8px、悬停淡入 0.3s、不占位） | :444 + CSS :3046-3066 | render（全屏分支） | 🔧 | 本轮实现：`with_animation` 0.3s 两端态动画（悬停标志拼进元素 id 驱动动画重跑）；`backdrop-filter` 模糊按任务省略；悬停挂在控制条自己（`on_hover` + `InputModalityIndependent`） |
| 非全屏控制条照旧底部占位 | CSS :2965-2975 | render（非全屏分支） | ✅ | 面板底、不透明、常显 |

### 2. `PresentationOverlay.vue` script（副作用与按键）→ `present.rs::on_key` / `ShellHandle`

| 项 | 原版位置 | 原生位置 | 状态 | 备注 |
|---|---|---|---|---|
| 按键：O / →↓空格 / ←↑ / Home / End / F / S | :295-335 | `on_key` | ✅ | 全览开时 ←→ 不翻页，照原版 |
| Esc：先关全览；**全屏时只退全屏不退出演示**；非全屏才退出 | :311-316 | `on_key` | 🔧 | 改前原生全屏 Esc 直接退出演示；改后与原版一致（实测 `target/drive/e1-e4`） |
| 退出演示：退出全屏并还原最大化 | :283-293、125-136 | `exit` → `ShellHandle::close_presentation` | ✅ | 原生无「最大化还原」概念（窗口态即原状） |
| 舞台位图：当前页先出图，再预取 -1…+3，`max_cached` 10 | :202-254 | `wanted` | ✅ | |
| 换页动画（forward/backward/jump 三段过渡） | CSS :2907-2963 | — | ➖ | parity 已记：GPUI 下先不做 |
| 触摸手柄（touchstart/touchend 直接监听） | :267-281 | `on_swipe`（`ScrollWheelEvent.touch_phase`） | ✅ | GPUI 把触摸平移转成带 `touch_phase` 的滚动事件 |

### 3. 视觉细节（归一行）

原版有、原生按 GPUI 省略或形态不同的：覆盖层顶部径向渐变 + 面板线性渐变背景；`.pres-nav` 的 `backdrop-filter` 模糊与阴影（原生纯色面板）；浮动控制条的 `backdrop-filter: blur(20px) saturate(1.4)`（任务允许省略）；`kbd` 芯片样式；幻灯片圆角 20px + 双层阴影的过渡容器（原生 `slide_frame` 无圆角阴影）；全览卡片 hover 上浮与阴影；切页动画。已做等价的：箭头/控制条半透明底色、边框 hover 色、胶片条 active 高亮。
