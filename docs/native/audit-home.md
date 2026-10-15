# 首页与全局组件逐页审计（原版 Vue → 原生 GPUI）

范围：首页与其用到的全局组件。原版 `src/views/HomeView.vue`、`src/components/{ExportDialog,PasswordDialog,PdfExportDialog,SkeletonCard,BrandMark,ThemeToggle,TitleBar}.vue`
→ 原生 `apps/huanshu-native/src/{pages/home.rs,password_dialog.rs,export.rs,widgets.rs}`。

- 状态：✅ 一致　🔧 本轮已修　⚠️ 有差距未修（附估计工作量）　➖ 有意不做（附原版行号依据）
- 截图：`docs/native/reference/home*.png` 对照 `docs/native/shell/home-0.png`（原版 2160×1350、原生 2182×1361，都按 1.5 倍缩放换算成逻辑像素测量）
- 主题相关的纯视觉细节（阴影、渐变、毛玻璃、动画、字号亚像素）不逐条列，每个文件末尾归一行「视觉细节」。

## 1. `HomeView.vue` → `pages/home.rs`

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

## 2. `ExportDialog.vue` → `home.rs::open_export`（+ `password_dialog.rs`）

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 标题「导出演示文稿」 | ExportDialog.vue:119 | home.rs::open_export | ✅ | |
| 描述「准备导出「X」」 | ExportDialog.vue:120-123 | home.rs::open_export | ✅ | 原版的 else 分支「选择导出方式并确认」（:122）在首页不可达：`handleExport` 总会带上项目名（HomeView.vue:57-59） |
| 右上关闭 ✕ | ExportDialog.vue:111-113 | GPUI Kit Dialog 默认关闭按钮 | ✅ | 原版 12/12，Kit 在 8/8 |
| 点遮罩关闭 | ExportDialog.vue:109 | GPUI Kit Dialog 默认 `overlay_closable` | ✅ | |
| 三个选项：普通导出 / 加密导出 / PDF 文档（标题 + 说明 + 图标） | ExportDialog.vue:126-152 | home.rs::open_export | ✅ | 文案与图标逐字（`document-export` / `locked` / `document-pdf`） |
| 「选中一个选项 + 按主按钮确认」两段式交互、选中态高亮 | ExportDialog.vue:128、137、145 + 210-215 | home.rs::open_export | ⚠️ | 原生是「点选项即执行」的单段式（三个选项分别是立即导出 / 开密码框 / 导 PDF），没有选中态、也没有「开始导出/加密导出/导出 PDF」按钮与 `disabled=!canSubmit`。功能都到得了，形态不同。估计 60–80 行（要引入选中态 + 主按钮 + 主按钮标签随格式变） |
| 加密面板：`导出密码` + 输入框 + 眼睛 | ExportDialog.vue:157-168 | password_dialog.rs::PasswordDialog（Encrypt） | ⚠️ | 原生把加密面板做成第二个对话框（与 parity.md 的说明一致）；密码、眼睛、强度条、门槛都在，但**面板内嵌形态**不同。估计 40 行（要挪进同一个对话框） |
| 强度条 + 等级文案（太弱…强） | ExportDialog.vue:170-180 | password_dialog.rs::{strength_score,strength_level} | ✅ | 评分规则与 `levels[min(score,4)]` 一致（单测 `strength_scores_follow_the_original_rules`） |
| `确认密码` + 不一致红边 + 「两次输入的密码不一致」 | ExportDialog.vue:183-201 | password_dialog.rs::render | ✅ | 逐字 |
| 提示「密码至少 8 位，需包含大写字母、小写字母和数字。」 | ExportDialog.vue:203-205 | password_dialog.rs::render | 🔧 | 原生两处都没有这句话（`PasswordDialog.vue` 本身没有，只有这里），本轮补在 encrypt 模式的表单尾部 |
| 提交时的强度门槛与错误行 | ExportDialog.vue:93-96、208 | password_dialog.rs::submit | ✅ | `密码必须至少8字符，包含大小写字母和数字`，正则等价（单测 `encrypt_gate_agrees_with_core_password_validation`） |
| 输入框回车提交（`@keyup.enter`） | ExportDialog.vue:164、192 | password_dialog.rs::new/render | 🔧 | 原生原本没有回车提交，本轮补（见 PasswordDialog.vue 一节） |
| 打开时重置表单 | ExportDialog.vue:53-64 | home.rs::open_encrypt_dialog（每次新建对话框实体） | ✅ | 结构上等价 |
| 关掉加密时清空密码 | ExportDialog.vue:66-73 | — | ✅ | 原生没有「同框切格式」这条路径（加密出口固定是新对话框） |
| 「取消」按钮 | ExportDialog.vue:211 | home.rs::open_export（DialogClose） | ✅ | |
| 视觉细节 | — | — | ➖ | 做了：选项行用 `c.panel/surface` + `hover` 边框色、主色图标。按 GPUI 省略：卡片 hover 上浮 1px、`box-shadow: inset` 选中描边、`max-width: 92vw` 与 640px 断点（:475-485）、弹出缩放的进出动画（:454-473） |

## 3. `PasswordDialog.vue` → `password_dialog.rs`

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 标题「设置密码」/「输入密码」 | PasswordDialog.vue:91-93 | password_dialog.rs::render | ✅ | 逐字 |
| 描述「为「X」设置导出密码」/「请输入密码以解密文件」 | PasswordDialog.vue:95-100 | password_dialog.rs::render（`desc`） | ✅ | encrypt 无项目名时不显示描述，与 `v-if` 同 |
| 右上关闭 ✕ | PasswordDialog.vue:82-84 | password_dialog.rs::open + render | 🔧 | 原生原来会**同时出现两个 ✕**（GPUI Kit 自带的 + 内容里自绘的，`step3.rs:628` 是正确写法），本轮 `close_button(false)` 关掉 Kit 那个，保留走 `on_cancel` 的自绘 ✕ |
| 点遮罩关闭（Esc 同理） | PasswordDialog.vue:80 | GPUI Kit Dialog 默认 `overlay_closable` | ⚠️ | 原生遮罩/Esc 关闭走 Kit 自己的路径，**不走** `on_cancel`：原版点遮罩 = `handleClose` → 首页清掉待导入字节、清密码框状态。原生关掉后 `password_dialog` 还指着旧实体、decrypt 的 `pending_import` 字节留在内存（下次打开即被覆盖，**无可见后果**，所以本轮没改）。做法照 `workflow/step3.rs:625-637`：`Dialog::on_close` 里调用同一个 cancel 回调（约 10 行） |
| 圆形图标（locked；encrypt 绿 / decrypt 蓝） | PasswordDialog.vue:86-89、218-226 | password_dialog.rs::render | ✅ | 颜色字面值与原版一致（0x22c55e / 0x3b82f6 系） |
| `密码` 标签 + 输入框（占位「设置密码/输入密码」）+ 眼睛切换 | PasswordDialog.vue:103-115 | password_dialog.rs::render | ✅ | `masked(true)` + `set_masked` 对应 `type=text/password` |
| 强度条（encrypt 恒定显示，空密码时是空轨道 + 空标签） | PasswordDialog.vue:117-127 | password_dialog.rs::render | ⚠️ | 原生空密码时整条不渲染（轨道也不显示），且文案位置在输入框下方；原版是无条件渲染。影响很小（没有信息丢失）。估计 10 行：空密码时保留空轨道、标签为空 |
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

## 4. `PdfExportDialog.vue` → `export.rs::show_report`

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 「正在生成 PDF…」进行中对话框 + 「首次导出需要扫描系统字体，会稍慢一些。」 | PdfExportDialog.vue:49-55 | — | ⚠️ | 原生没有进行中对话框：导出期间只有卡片按钮转圈。估计 30 行（要在导出前开一个对话框、结束后替换成结果——首页与第三步两处调用方都要配合） |
| 失败态：对话框里「导出失败」+ 错误正文 + 「知道了」 | PdfExportDialog.vue:57-69 | export.rs::show_report | ⚠️ | 原生只推一条「导出失败：…」轻提示，没有对话框与「知道了」。估计 25 行 |
| 「在文件夹中显示」失败时把错误显示在框里 | PdfExportDialog.vue:68-75 | export.rs::show_report | ⚠️ | 原生推轻提示（`打开文件夹失败：…`）。估计 5 行 |
| 结果标题「PDF 已导出」/「PDF 已导出，有几处需要留意」 | PdfExportDialog.vue:78-80 | export.rs::show_report（`dialog.title("PDF 导出完成")`） | ⚠️ | 文案不同，且原生没有「有留意项」这一态 |
| 结果摘要「{文件名} · {N} 页 · {x.x} 秒」 | PdfExportDialog.vue:81-83 | export.rs::show_report | ⚠️ | 原生是「文件」整条路径 +「页数 N 页 · M ms」两行（秒→毫秒、全路径 vs 文件名） |
| 说明列表：调用方 note（第三步未应用修改） | PdfExportDialog.vue:86-89 | export.rs::show_report（`note` 参数） | ✅ | 原生保留了这条（`information` 图标 + warning 色）；本行登记它对应原版的位置 |
| 说明列表：占位页按版式分组（「{版式}（第 3、5 页）」） | PdfExportDialog.vue:19-25、90-98 | export.rs::show_report | ⚠️ | 原生是「占位页 第 3 页（kind）、第 5 页（kind）」，用 kind **原值**而非 schema 的中文版式名（原版 `kindLabel` 走 `editorSchema`）。估计 15 行 |
| 说明列表：裁切页「第 N 页 内容过多，已缩到最小字号仍放不下，超出部分被裁掉。」 | PdfExportDialog.vue:99-102 | export.rs::show_report | ⚠️ | 原生只列「裁切页 第 N 页」，缺后半句解释 |
| 说明列表：缺图「第 N 页图片 `引用` 未导出：原因」 | PdfExportDialog.vue:103-106 | export.rs::show_report | ⚠️ | 原生只给「缺失图片 N 处」，**丢掉了页码、引用与原因**（这一条信息丢失最明显） |
| 说明列表：主题回落「主题「X」还没有 PDF 版式，已回落到「便签·稿纸」。」 | PdfExportDialog.vue:107-110 + 34-39 | export.rs::show_report | ⚠️ | 原生「「id」不可用，已回落到便签」（用 id 不用主题名，文案不同） |
| 无回落时的常驻提示「PDF 版式跟随当前主题。」 | PdfExportDialog.vue:112 | — | ⚠️ | 原生没有 |
| 关闭 ✕ / 点遮罩关闭（导出中不可关） | PdfExportDialog.vue:47、58-60、72-74 | GPUI Kit Dialog 默认 | ✅ | 原生没有「导出中」这一态，结果框总能关 |
| 「完成」按钮 | PdfExportDialog.vue:118 | export.rs::show_report（`关闭`） | ⚠️ | 原版叫「完成」；原生还有个「在文件夹中显示」✅（`reveal` → core `reveal_in_file_manager`） |
| `dismiss()` 清状态 | PdfExportDialog.vue:77-81 + usePdfExport.ts:77-81 | GPUI Kit 关对话框即回收 | ✅ | 原生没有跨页面的全局 PDF 状态 |
| 视觉细节 | — | — | ➖ | 做了：警示/成功/失败三色图标（`information`/`checkmark`）、等宽显示引用、可滚动列表。按 GPUI 省略：`max-height: 220px` 滚动区、`prefers-reduced-motion` 分支（:292-294）、弹窗淡入淡出、`code` 背景高亮 |
| 整节汇总 | PdfExportDialog.vue 全文件 | export.rs::show_report | ⚠️ | 上面 11 条 ⚠️ 是同一个决定：原生把「结果摘要对话框」重写成「字段行 + 关闭」，信息量与文案都低于原版。**一次重做全部条目约 120–150 行**（含 `editorSchema` 版式名、缺图明细、错误对话框与进行中对话框），建议单开一轮；这个组件在 `WorkflowView.vue:553` 也挂了一份，修好两页同时受益（第三步的调用在 `workflow/step3.rs`，本轮范围外） |

## 5. `SkeletonCard.vue` → `home.rs` 内联的骨架

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 骨架行数与形状（首页 `:rows="2"`，末行 60% 宽、行高 13px、行距 10px） | SkeletonCard.vue:8-12、17-26 + HomeView.vue:248 | home.rs::render（`projects == None`） | ⚠️ | 原生每条骨架只有一条固定 220×12 的矩形。估计 6 行：改成两行（第二行 60%） |
| 微光动画（`skeleton-shimmer`） | SkeletonCard.vue:45-57 | — | ➖ | 动画细节，GPUI 下不做（与 parity.md 的「卡片入场动画 ➖」同类） |
| 视觉细节 | — | — | ➖ | 骨架底色用 `c.surface`（同原版 `--studio-surface`）、圆角 4px 一致；省略微光 |

## 6. `BrandMark.vue` → `widgets::brand_mark` / `assets/brand-mark.svg`

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 品牌 SVG（3 个渐变 + 7 个图形） | BrandMark.vue:27-120 | icons.rs（`assets/brand-mark.svg`，`img("brand/mark.svg")`） | ✅ | 逐路径核对一致（`#70E1F7…#2090C9`、`#FFD992…#F0A63F`、`#FFF7E9…#F7E4C4`、旋转矩形、45° 方框、`#F4B24B` 圆点、`#59D1F2` 星） |
| `size` 属性（默认 24；各处 16/24/28/30/48） | BrandMark.vue:5-17 | widgets::brand_mark(size) + 各调用点 | ✅ | 标题栏 16、卡片 24、标题区 28、空状态 48（原版 30，见「视觉细节」） |
| `title="幻述"` 悬停提示 | BrandMark.vue:24 | — | ➖ | GPUI 的 `img` 元素没有 tooltip 通道（`widgets::TooltipText` 只给 `Stateful<Div>`）；品牌名在标题栏有文字，信息不丢 |
| `aria-hidden="true"` | BrandMark.vue:25 | — | ➖ | GPUI 无对应的无障碍属性标记，纯装饰图形不参与命中 |
| 视觉细节 | — | — | ➖ | `inline-flex` 居中与 `flex-shrink: 0` 用 `.flex_none()` 表达；`line-height: 0` 在 GPUI 无意义。空状态的品牌标 48（原版 `.empty-icon-circle` 里的 30，HomeView.vue:262） |

## 7. `ThemeToggle.vue` → `widgets::theme_dots`

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

## 8. `TitleBar.vue` → `widgets::title_bar`（GPUI Kit `TitleBar`）

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 品牌小底 +「幻述」 | TitleBar.vue:39-42 | widgets::{title_bar,brand_chip} | ✅ | `BrandMark :size="16"` |
| 最小化按钮（title「最小化」） | TitleBar.vue:45-53 | GPUI Kit `TitleBar` | ✅ | 由 Kit 画，走系统命令 |
| 最大化/还原（图标与 title 随状态） | TitleBar.vue:54-95 | GPUI Kit `TitleBar` | ✅ | Kit 用自己的图标与系统 title；原版 `isMaximized` 同步（:9-11、27-30）由 Kit/系统负责 |
| 关闭按钮（hover 红底 `#e81123`） | TitleBar.vue:96-115、200-203 | GPUI Kit `TitleBar` | ✅ | 红底 hover 是 Kit/系统样式 |
| 拖动区、双击最大化、贴靠 | TitleBar.vue:37-38 | GPUI Kit `TitleBar`（Windows 命中测试） | ✅ | parity.md 已注明「贴靠与多屏需人工验收」 |
| 标题栏高度 36px | style.css:2 + TitleBar.vue:126 | GPUI Kit `TitleBar` 默认高度 | ⚠️ | 实测截图：原版 36 逻辑像素、原生 34（`home.png` y=36、`home-0.png` y=34）。Kit 的高度写死在组件里，改要自绘标题栏；影响 2px |
| 视觉细节 | — | — | ➖ | 品牌底原版是深色渐变 + 内阴影（:147-163），原生 `brand_chip` 用固定深色 `0x111a28` + 细边（同「有意保留的固定深色」思路）；`user-select: none` 在 GPUI 无意义 |

## 统计

| 状态 | 条数 |
|---|---|
| ✅ 一致 | 70 |
| 🔧 本轮已修 | 10 |
| ⚠️ 有差距未修 | 18 |
| ➖ 有意不做 | 16 |
| （合计条目） | 114 |

说明：「回车提交」在 `ExportDialog.vue` 与 `PasswordDialog.vue` 两节各占一行（同一个密码框，实际是一处改动）；「导出 PDF」一行里含两处改动（文件名净化 + 全局互斥），所以 🔧 行 10 条 = 下方改动表 10 项。

⚠️ 按工作量排序：PDF 结果对话框整节（第 4 节 12 条，含 1 条汇总，120–150 行）＞ 导出对话框两段式交互（第 2 节 2 条，60–100 行）＞ 骨架两行（第 5 节 1 条，6 行）＞ 遮罩关闭不走 `on_cancel` / 强度条空态（第 3 节 2 条）/ 标题栏高度（第 8 节 1 条，各 10 行内）。

## 本轮改动（🔧）

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
