# 第二步「AI 生成」逐页审计（`Step2Generate.vue` + `RetryLayoutDialog.vue` + `stores/generation.ts`）

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

## 一、`src/views/steps/Step2Generate.vue`

### 模板：头部

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 状态圆点（`gen-orb--{stage}` 配色 + 生成中脉冲） | Step2Generate.vue:410；配色 CSS WorkflowView.vue:1321-1347 | step2.rs::Render::render（`orb_color` / `orb_pulse`） | ✅ | 阶段→颜色映射一致（生成中的八个阶段主色、done 绿、error 红、其余灰）；原版 done/error 另有 3px 光晕、脉冲是 `scale(1.3)`+opacity 1→0.65，原生只做透明度脉冲（视觉细节） |
| 标题四态（`{stageLabel}...` / `生成完成 · N 张幻灯片` / `生成遇到问题` / `准备开始`） | 413-416 | step2.rs::Render::render（`title`） | ✅ | 逐字；`stageLabel` 走 core `stage_label`（`STAGE_LABELS[stage] ?? stage`） |
| 生成中的 `...` 闪烁 | 413；CSS WorkflowView.vue:1359 | step2.rs::Render::render | ✅ | 原生是静态 `...`（视觉细节） |
| 第二行 `message`（`v-if="props.message"`、max-width 420px、省略号） | 418；CSS WorkflowView.vue:1362-1370 | step2.rs::Render::render（`.max_w(420).truncate()`） | ✅ | |
| 统计 chip「幻灯片」 | 421-425 | step2.rs::stat_chip(`items.len`) | ✅ | 数字用文字色（原版 `.gen-stat-n` 非 accent） |
| 统计 chip「关键事件」 | 426-429 | step2.rs::stat_chip(`snap.important`) | ✅ | core `important_log_count`（`log.important`） |
| 统计 chip「生成记录」（数字用主色） | 430-434 | step2.rs::stat_chip(`snap.prompt_count`, accent) | ✅ | core `prompt_log_count`（`input`/`prompt_io`/`slide`/`repair`） |

### 模板：整体进度

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

### 模板：看板

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

### 模板：详情

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

### 模板：底栏与对话框挂载

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 底栏整条（`v-if="!props.running"`） | 667 | step2.rs::Render::render（`bottom`） | ✅ | 生成中不显示；其余阶段显示空条（原版同样只显示内边距） |
| 「前往编辑」（done 态，有页在重试时禁用） | 668-670 | step2.rs::Render::render | ✅ | 禁用条件 = `retryingPages.size > 0` |
| 「返回修改」（error 态） | 671-673 | step2.rs::Render::render | ✅ | 回第 1 步 |
| 「重新生成全部」（error 态，有页在重试时禁用） | 674-676 | step2.rs::Render::render | ✅ | 走 `WorkflowPage::start_generation`，与第一步「开始 AI 生成」同一条路（原版 `@retry="onStep1StartGenerate"`，WorkflowView.vue:508） |
| `RetryLayoutDialog` 挂载与 `retry-page` 事件 | 679 | step2.rs::Step2::show_retry_dialog → `GenerationStore::retry_page` | ✅ | 原版 `@retry-page="gen.retryPage"`（WorkflowView.vue:509） |
| 窄窗口（`max-width: 1200px`）两栏改一栏 | 1129-1137 | step2.rs::Render::render（`narrow`，NARROW_MAX_W） | 🔧 | 改前恒定两栏（详情 340px 起），窄窗口下看板被挤成一条；现在窗口宽 ≤1200px 时详情整宽排在看板下方、两者平分高度 |

### `<script setup>` 的副作用与快捷键

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

## 二、`src/components/RetryLayoutDialog.vue`

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

## 三、`src/stores/generation.ts`（行为契约 → `generation.rs`）

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

## 四、本轮的修改（🔧）与遗留（⚠️）

### 🔧 已修

| 原版位置 | 改了什么 |
|---|---|
| Step2Generate.vue:555 | 详情标题的兜底文案：抽出 `plan_title`，`card_title` 兜底「第 N 张」、新增 `detail_title` 兜底「待生成幻灯片」（改前详情也显示「第 N 张」）（`step2.rs`） |
| Step2Generate.vue:557 + CSS 852-872 | 详情头状态 chip 改回中性色：原版 `.sdp-chip` 没有色调类，改前按 `slide_status` 的 tone 上了绿/蓝/红（与版式 chip 的 `--accent` 抢重）（`step2.rs`） |
| Step2Generate.vue:1129-1137 | 窄窗口单列：新增 `NARROW_MAX_W = 1200.0`，窗口宽 ≤1200px 时工作区改用 `flex_col`（详情整宽排在看板下方、两者 `flex_1` 平分高度），宽窗口仍是 1.5fr / 0.9fr 两栏（`step2.rs`） |
| RetryLayoutDialog.vue:70 | 下拉 `autofocus`：候选装载完成后聚焦下拉——`let focus = select.read(cx).focus_handle(cx); window.focus(&focus, cx);`（`SelectState` 实现了 `Focusable`，见 gpui-component 的 `impl Focusable for SelectState`）（`step2.rs`） |
| WorkflowView.vue:500、505 | 选中下标跨步骤保留：下标移到 `ProjectModel::selected_slide`（`Option<usize>`），`Step2::select` 写、渲染归位（越界/未选回落 `preferred_slide_index`）、`WorkflowPage::start_generation` 整次重新生成前清空；4 个新单测（`resolve_selected`）（`mod.rs` / `step2.rs`） |
| stores/generation.ts:401-435 | store 不持有本项目时的静态状态：`GenerationStore` 新增静态缓存（`StaticCache` 单槽，打开项目时经 `run_bg` 读最近运行 + 日志一次，运行变了才失效，`snapshot_for` 每帧只读缓存）；静态分支用 `progress::statuses_from_logs` 恢复失败页、关键事件 / 生成记录统计、最近 10 条日志、看板条目；失败段与「重试这一页」出现但按全局规则禁用（`!live` 即禁用，因为静态分支下 store 的重试会排到别的项目的运行上；「重新生成全部」仅 `busy_elsewhere` 禁用）；6 个新单测（`generation.rs::tests`）（`generation.rs` / `step2.rs`） |

### ⚠️ 未修

本轮**没有遗留 ⚠️**：原来的 3 条 ⚠️ 中，「选中页跨步骤保留」与「store 不持有本项目时的静态状态（含连带的重试按钮）」已 🔧；「卡片 Tab 键盘可达」按新原则归入 ➖（表现层，按 GPUI 原生做法）。

### 与 parity.md 第二步小节的核对

12 行逐条核对（顶部状态与统计、生成流水线与事件、整体进度/五阶段/查看详情、看板卡片、只渲染可见卡片、
键盘 ↑/↓、详情大图/标题/状态/版式、详情三段 + 调试 + 日志、单页与换版式重试、重新生成全部、前往编辑）：
**没有发现「标了 ✅ 但其实没做完」的行**。两点措辞瑕疵（不构成不实）：
- 「顶部状态与统计」行写「数字用主色」，实际只有「生成记录」一个数字用主色（另两个是文字色，Step2Generate.vue:422-433 + CSS WorkflowView.vue:1386-1394）。
- 「详情：大图、标题、状态、版式」行在本轮修复前，兜底标题与状态 chip 配色与原版有差距（已 🔧），该行当时未记这两个差距。

第十五轮同步（`parity.md` 第二步小节）：①「生成状态是应用级 store」那行的**已知偏差**（其他项目的第二步只有静态快照、缺日志 / 逐页状态 / 失败段）已修，该行改为 ✅；② 新增一行「选中页下标跨步骤保留」（`ProjectModel::selected_slide`）。

### 附带发现（不在我负责范围，供参考）

- `docs/native/reference/project-step2.png` 与 `project-step1.png` **字节完全相同**（都是第 1 步「准备素材」的画面）；第二步的原版对照图实际是 `step2-board.png` / `step2-detail.png`。原生侧 `docs/native/shell/project-step2-0.png`（正常态）与 `project-step2-error-0.png`（失败态）是对的。

### 本轮的验证情况

- 第十四轮只改了 `apps/huanshu-native/src/pages/workflow/step2.rs`；改完 `rustfmt --edition 2021`（解析通过、无格式差异），并补上了 `Focusable as _` 的导入（原文件没有这个 trait，`focus_handle` 需要它）。
- 第十五轮的三处 🔧 跨 `pages/workflow/mod.rs`（`selected_slide` / `start_generation` 清空 / `show_step`）、`pages/workflow/step2.rs`（`resolve_selected`、静态快照的失败段禁用）与 `generation.rs`（`StaticCache`）三个文件，共 10 个新单测（`resolve_selected` 4 个、`generation.rs::tests` 6 个）；这些改动由本轮负责人验证（`cargo test` 见其汇报），审计侧只同步文档。
- 两轮都**没有跑** `cargo check / build / test`（共享 target 目录，按约定由集成方统一验证），也**没有**跑 `tools/shots.ps1` / `drive.ps1`（会触发编译）；审计侧不改代码。第十四轮改动建议集成后用 `tools/drive.ps1 -Route project:1:2` 走一遍：详情标题（选一张还没规划结果的页应显示「待生成幻灯片」）、详情头状态 chip 是中性色、窗口拉窄到 1200px 及以下时两栏变一栏、「重试这一页」对话框打开后焦点在下拉上。
- 纯逻辑部分（看板条目、逐页状态、整体进度、四张文案表）不在本文件里，由 core 的 `tests/generation_progress.rs` 快照守护。

