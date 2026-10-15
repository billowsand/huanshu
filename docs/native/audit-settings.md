# 设置页逐页审计（`SettingsView.vue` + `settings.css` + `SystemFontPicker.vue`）

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

## 一、`src/views/SettingsView.vue`

### 模板：侧栏（233–243）

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 「返回工作台」按钮（arrow-left 图标 + 文案，hover 变色） | SettingsView.vue:234；settings.css:46-47 | mod.rs::render（nav 的 `ghost_button`） | ✅ | 文案、图标逐字一致 |
| 「返回工作台」的返回目标 | SettingsView.vue:210-213 | mod.rs::leave + `ShellHandle::leave_settings`（`shell.rs` 的 `Shell.settings_origin`） | 🔧 | 本轮修：`Shell.settings_origin` 记下进设置前的路由，设置页三个出口都走 `leave_settings()` 回来源——从工作流侧栏进来就回**那个项目的那个步骤**（`project:<id>:<step>`，恢复项目与步骤；`show_step` 把步骤同步回路由），没有来源时回 `Home` |
| 「设置」标题 | SettingsView.vue:235；settings.css:48 | mod.rs::render | ✅ | 20px 与 CSS 一致 |
| 五个分区项（图标 + 文案 + 选中态 + hover） | 236-241；settings.css:50-54 | mod.rs::render（遍历 `SettingsTab::ALL`） | ✅ | 顺序、图标（screen / cube / settings-adjust / data-base / help）、文案、选中左侧竖条一致 |
| 版本号 `v{version}` | 242；settings.css:55 | mod.rs::render（`env!("CARGO_PKG_VERSION")`） | ✅ | 两侧都是 v0.4.0 |

### 模板：主栏与分区头（245–257）

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 分区标题（h1）+ 分区说明 | 248-251；settings.css:61-63 | mod.rs::render + `tab_description` | ✅ | 五段说明逐字；h1 字号本轮改 26→24（见 CSS 一节） |
| 未就绪提示：「正在加载设置…」/「设置加载失败，请重试。」+「重新加载」按钮 | 252-255 | mod.rs::read_settings / reload_settings（失败态）+ core `AppState::{settings_load_error, settings_file_error, reload_settings}` | 🔧 | 本轮修：core 把 `settings.json` 读/解析失败做成真实信号（`state.rs` 的 `settings_load_error` + 磁盘探针 `settings_file_error` + `reload_settings`）；失败态显示「设置加载失败，请重试。」+ 原因 + 「重新加载」按钮，分区不渲染，保存按钮禁用并在 `save()` 里再挡一次；2 个新单测。**loading 窗口仍无**（原生同步读 core，见下一行的 ➖） |
| 未就绪 / 保存中整块禁用（`<fieldset :disabled="!ready \|\| saving">`） | 257 | mod.rs::save / render | ➖ | 原生同步读完即 ready；保存是同一帧内完成的同步 core 调用，没有可观察的 saving 窗口（等价于原版的 transient 禁用） |

### 模板：外观与字体（259–341）

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

### 模板：模型服务（344–450）

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

### 模板：生成设置（452–475）

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 「生成参数」+「自动修复轮数」（说明、「建议 2–4 轮」、数字输入 0–10） | 456-463 | generation.rs::generation | ✅ | 三行文案逐字；输入框宽度 96 vs 92px（视觉） |
| 「生成并发数」（说明、「本地模型建议 1，最多 8」、数字输入 1–8） | 464-471 | generation.rs::generation | ✅ | 同上 |
| 负载提示块（information 图标 + 文案） | 473；settings.css:228-229 | generation.rs::generation | 🔧 | 本轮：移到面板**外**、主色浅底、图标主色（原来在面板内且无底色） |
| 保存前校验：修复轮数 0–10 整数、并发数 1–8 整数，失败提示并跳到本分区 | 157-162 | mod.rs::save + generation.rs::`generation_valid` | ✅ | 文案逐字；输入框里的非数字文本按失败处理（原版 `Number.isInteger` 对空串/NaN 也是失败）；单测 `generation_ranges_match_the_original_rules` |

### 模板：存储与缓存（477–507）

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 「文件位置」+ 数据目录 / 媒体目录（只读展示 + 帮助） | 480-489 | storage.rs::storage | ✅ | 原版是 `readonly` `<input>`，原生是同样样式的文本块（只读语义一致） |
| 「图标向量缓存」+ 状态徽章（已初始化 / 模型已变更 / 未初始化） | 492-495 | storage.rs::`embedding_status` + `status_badge` | 🔧 | 本轮：未初始化改用默认弱化底（原版 494 行给的是 `is-untested`，而 settings.css 没有这条配色） |
| 「为图标建立语义索引，提高内容与图标的匹配效果。」 | 496 | storage.rs::storage | ✅ | 逐字 |
| 摘要两行：当前向量模型（未配置）/ 缓存对应模型（—） | 497-500 | storage.rs::storage | ✅ | 「缓存对应模型」的虚线分隔本轮补上（🔧，见模型服务那行） |
| 「初始化缓存」按钮：禁用 = 初始化中 / 已就绪 / 有未保存的模型配置；三态文案 | 501-504 | storage.rs::storage + init_embeddings | ✅ | 禁用条件与文案逐字；`configDirty` 时右侧提示「请先保存模型配置，再初始化缓存。」 |
| 初始化过程中的消息（正在初始化… / 完成 / 错误） | 505 | storage.rs::init_embeddings | ✅ | 三条文案逐字 |

### 模板：关于与帮助（509–525）

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 品牌图 44px +「幻述 Studio」+「版本 {version}」 | 512-515 | mod.rs::about | ✅ | 逐字 |
| 「将 Markdown 文档自动转化为精美的演示文稿。」 | 516 | mod.rs::about | ✅ | 逐字 |
| 说明与「开始使用」之间的分隔线 | 517；settings.css:240 | mod.rs::about | 🔧 | 本轮补 1px 分隔线 |
| 「开始使用」+ 三步说明 | 518-523 | mod.rs::about | ✅ | 三段逐字；原版是 `<ol>` 数字序号，原生是圆形序号（视觉） |

### 模板：底部保存条（530–541）

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 状态点 + 四态文案（正在保存… / 更改已保存 / 有未保存的更改 / 所有更改已保存） | 531-533 | mod.rs::render | ✅ | 文案与优先级逐字 |
| 脏且未保存时的「Ctrl + S」提示 | 534；settings.css:254 | mod.rs::render | 🔧 | 本轮补（原来只有文案、没有这个提示块） |
| 状态点颜色：脏时主色、平时成功色 | settings.css:251-252 | mod.rs::render | 🔧 | 本轮 `warning` → `primary` |
| 保存失败行 | 536；settings.css:256 | mod.rs::render | ✅ | 文案 `保存未完成：{error}。未保存的修改仍保留，可重试。` 逐字 |
| 「撤销更改」（禁用 = 不脏或保存中） | 538 | mod.rs::render + revert | ✅ | 禁用条件一致 |
| 「保存更改」（禁用 = 不脏 / 未就绪 / 保存中；保存中显示「保存中…」） | 539 | mod.rs::render + save | ✅ | 原生没有 `config.saving` 这一路（同步保存），其余条件一致 |

### 模板：离开确认（544–554）

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 遮罩点击 = 继续编辑（`@click.self="answerLeave('stay')"`） | 544 | mod.rs::leave（GPUI Kit `Dialog` 遮罩） | ✅ | 点遮罩关掉对话框即「继续编辑」 |
| 标题「有未保存的更改」+ 说明「离开前要保存吗？放弃后，主题与字体会恢复为上次保存的样式。」 | 546-547 | mod.rs::leave | ✅ | 逐字 |
| 「放弃更改」/「继续编辑」/「保存并离开」（放弃在左、保存并离开为主按钮） | 549-551；settings.css:263-264 | mod.rs::leave | ✅ | 放弃 = `revert` 后离开；保存并离开 = 保存成功（不再脏）才离开，失败留在本页看错误——与原版 `return !dirty.value` 同义 |
| 打开确认框时焦点落在「保存并离开」 | 191-201（`leaveSaveButton.focus()`） | mod.rs::leave | ➖ | 表现层，按 GPUI 原生做法 |

### 脚本副作用

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

## 二、`src/views/settings.css`

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

## 三、`src/components/SystemFontPicker.vue`

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

## 四、🔧 与 ⚠️ 明细

### 🔧 已修（19 处）

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

### ⚠️ 未修

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

## 五、与 `parity.md` 设置小节的核对

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

## 六、本轮的验证情况

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
