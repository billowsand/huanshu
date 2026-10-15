# 第三步「编辑完善」审计（对照 Tauri + Vue 原版）

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

**条目统计（105 行）**：✅ 90 / 🔧 4 / ⚠️ 5 / ➖ 6。
分节：`Step3Editor.vue` 模板 ✅ 45 / 🔧 4 / ⚠️ 5 / ➖ 5；`Step3Editor.vue` script ✅ 26；
`useSlideEditor.ts` ✅ 14 / ➖ 1；`SlideFrame.vue` ✅ 5。
🔧 与 ⚠️ 的明细在第六、七节。

---

## 一、`src/views/steps/Step3Editor.vue` — 模板（用户能看到 / 操作的东西）

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 左栏栏头：`layers` 图标 + 「幻灯片」 + 数量徽章 | Step3Editor.vue:564-566 | step3.rs:render（`col_head` + `widgets::chip(Muted)`） | ✅ | 参考截图里这栏头只有「幻灯片 18」，没有右边的重置按钮（见第十节） |
| 栏头右端「重新生成」按钮（`title="重新生成"`，回第 1 步） | Step3Editor.vue:567-569 | step3.rs:render + `go_to_step_1`(590) | ✅ | 原版点击是 `currentStep = 1`（WorkflowView.vue:545） |
| 左栏列表行：序号 / 标题 / 版式中文名（缺省回落 kind）/ 选中态 | Step3Editor.vue:571-588 | step3.rs:slide_row(2328) | ✅ | 版式名回落逻辑同（587） |
| 行点击选中 | Step3Editor.vue:580 | step3.rs:select(409) | ✅ | 换页丢掉未应用的修改（原版 `initEditorSlide` 如此，无确认框） |
| 行键盘 Enter / Space 选中（`tabindex="0"` + `role="button"`） | Step3Editor.vue:577-582 | — | ➖ | GPUI 列表行没有 Tab 焦点链，行不可聚焦；换页由 ↑/↓（`on_key`）与点击承担。属可达性，不是功能缺失 |
| 行尾垃圾桶：`title="删除"`、只在悬停该行时显示、只剩一页时 disabled + 透明 | Step3Editor.vue:589 + WorkflowView.vue:1931-1948 | step3.rs:slide_row（`group`/`group_hover`、`count<=1` 时 opacity 0） | ✅ | 悬停变红（WorkflowView.vue:1947）是 kit 按钮悬停色改不掉，见「视觉细节」；行本身的悬停底色见 ⚠️ 5 |
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
| 栏头上移 / 下移 / 删除（禁用条件） | Step3Editor.vue:657-667 | step3.rs:render（`slide-up/down/del`） | ✅ | 删除另加 `locked`（原版没有生成中只读，见 parity.md:141 的有意偏离） |
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
| 卡片里的字符串列表（每个 stringList 一行标签 + 「添加」+ 每项输入框 + 删除） | Step3Editor.vue:914-937 | step3.rs:string_list（有 label → 标签行） | ✅ | 标签行「添加」的宽度差异见 ⚠️ |
| 嵌套列表（`nestedLists`）：卡片标题 + 删除（**没有**上移/下移）+ 字段 | Step3Editor.vue:939-981 | step3.rs:nested_block(1737) | ✅ | 原版嵌套字段只有 text / textarea（`process.steps`），原生用通用 field_card，多支持的类型用不到 |
| 对象区（`objectSections`）：区标题（无添加按钮）+ 字段网格 + 子列表区 | Step3Editor.vue:985-1149 | step3.rs:object_block(1817) | ✅ | 对象区各 def 的 `stringLists` 在快照里都是空（原生多渲染了一轮，无副作用）；boolean 不显示上方小标签也一致（992-1016） |
| 子列表区：区标题 + 添加 + emptyText + 卡片（上移/下移/删除 + 字段 + 图标助手 + 字符串列表） | Step3Editor.vue:1030-1148 | step3.rs:list_block + list_card | ✅ | |
| 图表数据（JSON）框：标签 + hint 两态（「正在校验…」/「type / data / encoding / series，校验通过才会应用到预览」） | Step3Editor.vue:1151-1158 | step3.rs:render_form ⑧ | ✅ | |
| 图表 JSON 框本体：`rows="12"`、`spellcheck="false"`、占位 JSON、错误时整框变红 + 「JSON 语法错误：…」/「问题；问题」 | Step3Editor.vue:1159-1174 | step3.rs:render_form ⑧ + `TextareaState::rows(12)` + `error_row` | ✅ | 等宽字体（`.ej-chart-json` 的 `--font-code`）与 12.8px 字号都对上了（WorkflowView.vue:2181-2187） |
| 高级 Blueprint JSON：标签 + 「保留给高级用户和特殊字段」+ 「格式化」 | Step3Editor.vue:1177-1184 | step3.rs:render_form ⑨ + `format_json(1220)` | ✅ | |
| 高级 JSON 编辑框：行号栏 + 「第 N 行：{原因}」+ 语义错误块（`warning` 图标 + 每条一行） | Step3Editor.vue:1185-1217 | step3.rs:render_form ⑨ + `json_error_text(2535)` + `semantic_errors_block(2496)` | ✅ | 原版行号栏会把出错的那一行标红，见 ⚠️ |
| 「应用更改」三态（保存中… / 应用更改 (Ctrl+S) / 已保存）+ 禁用条件 | Step3Editor.vue:1220-1229 | step3.rs:render_form ⑩ | ✅ | 禁用语一致：`jsonError \|\| !hasUnsavedChanges \|\| editorSaving`（原生另加 `locked`） |
| 「应用更改」按钮不吸底（长表单要滚到底才能点） | WorkflowView.vue:2061-2065 | step3.rs:render_form ⑩（列尾普通元素） | ⚠️ | `.ej-save-btn { position: sticky; bottom: 0 }`；GPUI 没有 sticky，见第七节 ⚠️ 1 |
| 标签行里的「添加」按钮宽度 | WorkflowView.vue:1956-1959 | step3.rs:string_list / list_block / nested_block（紧凑靠右） | ⚠️ | 原版 `.edit-add-btn { width: 100% }` 会占满标签右侧剩余宽度；见 ⚠️ 2 |
| 高级 JSON 出错行的行号标红 | WorkflowView.vue:2352-2356 | — | ⚠️ | GPUI Kit 的 Editor 行号栏不可着色；见 ⚠️ 3 |
| 图片素材小卡悬停底色 | WorkflowView.vue:2538-2540 | step3.rs:render_form ⑤（`widgets::media_tile`） | ⚠️ | 需要 `widgets.rs` 支持；见 ⚠️ 4 |
| 左栏行悬停底色（仅 notes 主题下原版有） | WorkflowView.vue:1895 + src/themes/notes.css:83 | step3.rs:slide_row | ⚠️ | 需要 `StudioColors` 加 hover token；见 ⚠️ 5 |
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

## 二、`src/views/steps/Step3Editor.vue` — `<script>` 的副作用

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
| `handleTextareaKeydown`：JSON 框里 Ctrl/⌘+S → 应用；其余转发 | Step3Editor.vue:537-544 | step3.rs:on_key（Ctrl/⌘+S 在**第三步任何位置**都生效） | ✅ | 有意改进（README 与 parity.md:140 已记） |
| JSON 框的 Tab → 插两个空格 | useSlideEditor.ts:147-155 | GPUI Kit `EditorState` 默认 `tab_size = 2`（`gpui-base/src/input/editor/indent.rs:23`，`tab` 绑到 `IndentInline`） | ✅ | |
| `onMounted` / `onBeforeUnmount`：注册、注销 window 监听 + 清 `renderBusyTimer` | Step3Editor.vue:546-556 | step3.rs:render 的 `.track_focus` + `release`(337) | ✅ | |
| 若干 `emit`（`update:activeSlide`、`replace-draft`、`apply-changes`、`delete-slide`、`move-slide`、`go-to-step-1` …） | Step3Editor.vue:47-64, 533-544 | step3.rs 直接调同名方法，经 `WorkflowPage::commit_deck` 落库 | ✅ | |

## 三、`src/composables/useSlideEditor.ts`

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
| `insertMediaUrl`：往 JSON 里插引用，没有 textarea 时退化成写剪贴板 | useSlideEditor.ts:225-237 | — | ➖ | 原版模板里**没有**任何触发入口（`grep -n insert-media-url src/views/WorkflowView.vue` 只在 543 行接了个空事件，模板里没有 `$emit('insert-media-url')`），parity.md:139 已记「未做」；原生也没有入口 |
| `editorKind` / `editorPreviewSlide` / `currentSlideDims` / `editorStageW/H` | useSlideEditor.ts:239-251 | step3.rs:`draft` + render 里的 `aspect`/`stage_w/h` | ✅ | |

## 四、`src/components/SlideFrame.vue`

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 有 `src` 时位图铺满整个框（`object-fit: fill`） | SlideFrame.vue:62-69,102-107 | slides.rs:slide_frame（`FrameState::Ready`，`stretch=false` → Contain） | ✅ | 原版 Stage 尺寸本身按页面比例算，所以 `fill` 与原生 `Contain` 在等比例框里结果相同；演示用 `stretch=true` |
| 没有图 + `pending`：延迟 300ms 才淡入「渲染中…」 | SlideFrame.vue:32-56,73 | slides.rs:`PENDING_DELAY` + `track_waiting`(183) + `FrameState::Loading.show_label` | ✅ | 原版 `pendingDelay: 300`；Step3 的栏头「渲染中…」是另一套 150ms 计时（上面已列） |
| 没有图 + `error`：「渲染失败」+「重试」按钮 | SlideFrame.vue:74-77 | slides.rs:`FrameState::Failed` + `on_retry` | ✅ | Step3 传了重试回调（`preview.refresh()`），不自动重试 |
| 有旧图 + `error`：角上「渲染失败 · 重试」（不遮画面） | SlideFrame.vue:81-89 | slides.rs:`FrameState::Ready { failed }` 的右上角浮层 | ✅ | |
| 视觉细节 | SlideFrame.vue:93-158 | — | ✅ | 折进来的：加载框底色（原版 `--studio-bg`，原生 `surface` + 细边）、淡入动画、毛玻璃（`backdrop-filter: blur(2px)`）、失败浮层的图标 |

---

## 五、统计

| 状态 | 条数 |
|---|---|
| ✅ 一致 | 90 |
| 🔧 本轮已修 | 4 |
| ⚠️ 有差距未修 | 5 |
| ➖ 有意不做 | 6 |
| 合计 | 105 |

（统计口径：第一至第四节四张表里的数据行；「视觉细节」行也计入 ✅。⚠️ 的 5 条在表里各占一行，明细见第七节。）

## 六、本轮已修（🔧，全部在 `apps/huanshu-native/src/pages/workflow/step3.rs`）

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

改动量：`git diff --stat` = 29 行新增 / 11 行删除（含注释），已 `rustfmt`。未跑任何 `cargo` 命令（按纪律交给集成方）。

## 七、有差距未修（⚠️）

| # | 差距 | 原版依据 | 估计工作量 |
|---|---|---|---|
| 1 | 「应用更改」按钮不吸底：长表单时要滚到底才能点，原版随时可见 | `WorkflowView.vue:2061-2065` `.ej-save-btn { position: sticky; bottom: 0 }`（模板 `Step3Editor.vue:1220`，右栏滚动容器 `.edit-json-inner` 内） | GPUI 没有 sticky 定位（`grep "fn sticky" gpui-base` 0 命中）。二选一：① 等 GPUI 支持 sticky；② 把右栏拆成「滚动区 + 固定底栏」，但**短表单时按钮会贴到栏底**，与参考截图（按钮紧跟内容）不符，需要先拿一张长表单的原版截图确认取舍。约 0.5 天含目检 |
| 2 | 标签行里的「添加」按钮宽度：原版占满「标签右侧的剩余宽度、文字居中」，原生是紧凑按钮靠右（列表区 / 嵌套列表 / 卡片字符串列表 / 图标助手行，共 4 处） | `WorkflowView.vue:1956-1959` `.edit-add-btn { width: 100%; justify-content: center }` + 模板里的 `.ej-label-row`（如 `Step3Editor.vue:820-825`、`889-899`、`914-919`、`939-945`） | 已用 Edge headless 复现原版渲染（`target/audit-flex/probe.html` + `probe.png`，CSS 与 DOM 逐条抄自原版）：按钮确实被压成剩余宽度、文字居中，但**同时把同行的标签挤成竖排两行**（`.ej-label` 没有 `nowrap`，CJK 的 min-content 是一字宽）。原生要做出宽按钮得包一层 `flex_1` + `w_full()`，仍然复现不了那个折行——两条路都要求先拿原版截图定标准。约 2 小时 + 目检 |
| 3 | 高级 JSON 出错时，原版行号栏会把出错那一行的行号标红 | `WorkflowView.vue:2352-2356` `.ej-line-num--error { color: error; background: error-bg }`，模板 `Step3Editor.vue:1188-1193` | GPUI Kit 的 `Editor` 行号栏没有「某一行着色」的接口（组件内部），要么等 kit 支持，要么放弃这个提示（错误文案里的「第 N 行：」仍在）。≈1 天（含读 kit 源码） |
| 4 | 图片素材小卡没有悬停底色 | `WorkflowView.vue:2538-2540` `.ej-media-item:hover { background: var(--studio-primary-bg) }` | 卡片现在画在 `widgets.rs::media_tile`（**别人的范围**）：需要给它加一个 hover 参数或内部加 `.hover(...)`。≈5 行 + 目检 |
| 5 | 左栏幻灯片行缺悬停底色（notes 主题下原版有） | `WorkflowView.vue:1895` `.edit-slide-row:hover { background: var(--studio-hover) }`；而 `--studio-hover` **只在** `src/themes/notes.css:83` 定义，其余五个主题下这条声明无效（悬停本来就没有底色） | 要给 `theme/StudioColors` 加一个 hover token（**别人的文件**）才做得出来；不改 theme 就只能在 notes 主题下少一个悬停底色。≈0.5 小时 + 六主题目检 |

## 八、有意不做（➖，逐条附原版依据）

1. **左栏行的 Enter / Space 选中**（`Step3Editor.vue:577-582` 的 `tabindex="0"` + `@keydown.enter` / `@keydown.space`）：
   GPUI 里列表行没有焦点链（要一个一个 `track_focus` + Tab 序），换页已由 ↑/↓ 与点击承担；这是可达性增强，不是原版的功能点。
2. **版式表未就绪两态**（`Step3Editor.vue:670-677` 的「读取版式信息失败：」/「正在读取版式信息…」）：
   原生 schema 是同步取（`step3.rs:244`），不存在「正在读取」，也没有可失败的命令调用。
3. **`insertMediaUrl` 与它的剪贴板回退**（`useSlideEditor.ts:225-237`）：
   原版模板里没有任何触发入口（`WorkflowView.vue:543` 接了个空事件，模板里找不到 `insert-media-url` 的发出点），parity.md:139 已记「原版没有触发入口，未做」。
4. **图表校验的「校验失败：…」出口**（`Step3Editor.vue:313-317`）：原版经 `invoke('chart_problems')`，命令可能失败；原生直接调 core 的纯函数（step3.rs:1160），没有失败这条路径。
5. **`aria-*` / `role="button"` / `role="status"`**（模板多处，如 606、647、826）：GPUI 的可访问性树由框架生成，页面不写这些属性。
6. **`#app * { font-weight: normal !important }` 之外的字重**：原版 KindPicker 被 `Teleport to="body"` 挪出 `#app`，它的标题/卡片名仍是 700/600（`WorkflowView.vue:2608-2613, 2653-2657`）；原生按 README 规矩 12 一律常规体，这一条 README 已声明为有意偏离。

## 九、`parity.md` 第三步复查（不改 parity.md，只报告）

逐行核对 `docs/native/parity.md:124-141` 的 16 行，**都做完了**，没有发现「写了没做」的行。几点补充：

- parity.md:127 写「行尾垃圾桶只在**悬停该行时**显示」——✅ 代码属实（`group_hover`），但原版在只剩一页时是 `opacity: 0 !important`（`WorkflowView.vue:1948`），原生两条 `.when` 都写了，属实。
- parity.md:135 写「（`tools/drive.ps1` 逐页截图 + 联系表；联系表是未提交的临时产物）+ compare / swot / process 全分辨率逐项核对」——截图现在还在 `target/drive/`（129 个文件，`target/` 被 gitignore，随时可能被清掉），建议把关键几张（compare / swot / process 的逐项、issue_stack 的两种提示）收进 `docs/native/shell/`，否则这行以后没法复核。**这条不影响功能结论。**
- parity.md:130 写「`serial` / `progressive` 取 `PreviewOptions::default()`（都是真）」——✅ 属实（`crates/huanshu-core/src/preview.rs:35-44`）。
- parity.md:126 写「左 `layers` +「幻灯片」+ 数量徽章 + `reset` 图标（回第 1 步）」——✅ 代码属实；**参考截图里看不到这个 `reset` 按钮**（见第十节）。
- 本轮另有两处「parity.md 没提、但确实与原版有差距」的地方，已记在 ⚠️ 1（保存按钮 sticky 吸底）与 ⚠️ 2（标签行「添加」按钮宽度）；parity.md 里没有任何一行声称这两处已完成，所以不算「写了没做」。

## 十、参考截图与当前原版 CSS 冲突的地方（供复核，不要据此改代码）

`docs/native/reference/project-step3.png` 与 `step3-kind-picker.png` 里有两处与现在的原版代码对不上，判断为**截图比代码旧或截图环境的问题**，本轮以代码为准：

1. **所有「只有图标的 ghost 按钮」在参考截图里都没渲染出来**：左栏栏头的 `reset`（`Step3Editor.vue:567-569`）、中栏的 `←` / `→`（607-615）、右栏栏头的 `↑`/`↓`/垃圾桶（657-667）在截图里都是空白（用 8 倍放大 + 逐像素扫描确认：这几处最亮的像素只有 (64,75,92)，而同一张截图里 `layers`/`view`/`edit` 这些静态图标正常显示）。同一张截图里同一行的「导出 PDF」图标（`document-pdf`）是正常的。
2. **KindPicker 的头部图标与卡片图标是文字色，而 CSS 里写的是 `--studio-primary`**（`WorkflowView.vue:2612`、`2651`）：截图标到的像素是 (212,232,248)（= 文字色），不是 `#38bdf8`。原生现在跟截图（文字色）；若以后确认应以 CSS 为准，`carbon(...).text_color(c.primary)` 两处即可。

## 十一、改动文件

只改了一个文件，没有碰任何负责范围外的文件：

- `apps/huanshu-native/src/pages/workflow/step3.rs`（🔧 4 处 + 1 段已知差异注释；`rustfmt` 过）
- 新增证据文件（`target/` 下，不入库）：`target/audit-flex/probe.html`、`target/audit-flex/probe.png`、`target/audit-n-*.png`、`target/audit-r-*.png`、`target/kp-*.png`、`target/ref-head-icons*.png`

未做的事：没有 `git commit`、没有跑 `cargo build/check/test`、没有改原版 `src/`、没有改 `widgets.rs` / `shell.rs` / `present.rs` / `mod.rs` / `step1.rs` / `step2.rs` / `drive.ps1` / `shots.ps1`、没有改 `docs/native/parity.md`。
