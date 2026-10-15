# 任务：原生界面第四轮——第三步「编辑完善」（一）：编辑器主体（第十二步）

你在仓库 `E:\huanshu`（幻述）里工作。原生界面 `apps/huanshu-native` 已完成框架、设置、向导、第一步、工作流与第二步（上一轮已审过）。
第三步原版很大，分两轮做。**本轮（一）**把编辑器做成能用：三栏布局、按版式表生成的表单（简单字段、标签组、列表区、嵌套列表、
对象区）、换版式 / 新建 / 删除 / 移动幻灯片、应用更改（保存）、单页实时预览。**下一轮（二）**再做图标推荐、素材插图、图表 JSON 框、
高级 JSON 模式——本轮这几块留 `widgets::todo_block` 占位，不要做。

开始前**必须**先读完：

1. `apps/huanshu-native/README.md`——框架与「写页面的规矩」（逐条遵守）；
2. `docs/native/parity.md`——本轮要把「第三步」一节里属于本轮的行做成 ✅（见文末清单），属于下一轮的行保持 ⬜；
3. `CLAUDE.md` 开头三节，尤其「原生界面」里**生成状态是应用级 `GenerationStore`、第三步编辑必须写回 store** 那一条；
   以及「编辑器实时预览与 Step2 详情（第三步）」一节（只发当前页、`serial` + `progressive`、换页清图）；
4. 现有代码：`pages/workflow/step3.rs`（骨架）、`pages/workflow/mod.rs`（`ProjectModel`、`on_generation_changed`、`generation_state`、
   `show_step`）、`generation.rs`（`GenerationStore`）、`slides.rs`、`export.rs`、`widgets.rs`；
5. 原版源码：`src/views/steps/Step3Editor.vue`（`<script>` 全部、模板到「图表数据（JSON）」之前为止，以及末尾的 KindPicker）、
   `src/composables/useSlideEditor.ts`（全部）、`src/stores/editorSchema.ts`、`src/views/WorkflowView.vue`（`activeSlide`、`exportPdf`、
   Step3Editor 的事件接线、`.edit-*` / `.ej-*` / `.kp-*` 样式，约 1826–2460 行）；
6. core：`editor_schema.rs`（`EditorSchema` 的结构；快照 `tests/fixtures/editor-schema.json`）、`services/projects.rs::update_project_blueprints`、
   `services/export.rs::export_pdf`（`slides` 参数）；
7. 原版截图：`docs/native/reference/project-step3.png`、`step3-kind-picker.png`；原生现状 `docs/native/shell/project-step3-0.png`。

## 原则（同前几轮）

- **复刻**：布局、文案逐字、交互、规则以原版为准；可按 GPUI 特点调整控件形态，但**不能少功能**。拿不准时读原版代码。
- **不重写框架**：缺部件往 `widgets.rs` **加**；改框架接口要在汇报第 8 节说明理由。
- **数据只经 `huanshu_core::services`**；新规则放 core（带单测），界面里不写规则。

## 前几轮审查发现的问题（不要再犯）

1. **真机验证碰了真实数据**：只用 `tools/drive.ps1` / `tools/shots.ps1`（它们自己设置沙箱）。动手前备份沙箱
   （`target/sandbox-home` → `target/sandbox-home.bak5`），然后 `seed_demo --force` 重建（现在会自动跳过首次向导）。
2. **UI 线程里的延时用了 tokio**：界面里的延时一律 GPUI 的 `background_executor().timer(..)`。
3. **状态没有唯一来源**：上一轮把生成状态从页面挪进了应用级 store。本轮的牌组改动**只有一条写入路径**（见 B 节），不要在
   `Step3`、`ProjectModel`、store 三处各改各的。
4. **队列 / 标志没有清干净导致永久卡住**（上一轮 store 的排队重试）：凡是「进行中」标志（保存中、对话框打开），每条出口（成功、失败、
   版本作废、换页、离开页面）都要复位，汇报里逐条列出。
5. **汇报要诚实**：没真机验证的写「未验证」。

## 现状里要先纠正的偏差（骨架的问题）

- **布局**：原版是三栏网格 `220px | 1fr | 390px`，**每栏有自己的栏头**（同一高度、连成一条）：
  - 左栏头：`layers` 图标 +「幻灯片」+ 数量徽章 + 右侧「重新生成」图标按钮（`reset`，回第 1 步）；
  - 中栏头：`view` 图标 +「实时预览」+「● 未保存」（有未应用修改时）+「渲染中…」（预览请求等待超过 150ms 时）+ 右侧
    `←  N / M  →` 翻页按钮（两端禁用）+「导出 PDF」+「演示」；
  - 右栏头：`edit` 图标 +「编辑 第 N 张」+ 右侧上移 / 下移 / 删除三个图标按钮（两端与只剩一页时禁用）。
  骨架把这些挤成了一条工具栏，宽度也是 200 / 360，要改掉。
- **左栏行**：序号、标题（单行截断）、版式中文名（`editor_schema` 的 label）；行尾垃圾桶按钮**只在悬停该行时显示**，只剩一页时不显示；
  底部「新建幻灯片」按钮。
- **当前页放在模型上**：原版 `activeSlide` 在 `WorkflowView`，切到别的步骤再回来、打开演示都用它。骨架放在 `Step3` 里，
  切步骤就丢。改到 `ProjectModel`（例如 `active_slide: usize`），演示的起始页也读它。

## A. core：编辑规则搬进 Rust（`crates/huanshu-core/src/slide_edit.rs`）

纯函数，输入输出都是 `serde_json::Value`（牌组按**原始 JSON** 处理，与 `ProjectModel.deck` 一致；强类型往返会挡掉能渲染的牌组）。
**逐条照抄原版的分支与边界行为**：

- 路径：`parse_path`（`cards[2].title`、`compare_data.left.items[0].text`）、`get_at_path`、`set_at_path`（原版 `setAtPath`：
  中间缺的层按下一段是数字建 `[]`、否则建 `{}`；数组下标越界时 JS 会留空洞，序列化成 `null`——照做）；
- 表单取值：`string_value`（非字符串一律 `""`）、`bool_value`（JS `Boolean(v)` 的真值规则）、`array_value`（非数组一律 `[]`）；
- 列表操作（返回新的草稿）：`add_item(path, new_item)`、`remove_item(path, index)`、`move_item(path, index, dir)`（越界不动）、
  `add_string_item` / `update_string_item` / `remove_string_item`；
- 版式：`create_kind_blueprint(schema, kind, seed)`（`kind` 的 `defaults` + `SHARED_TEMPLATE_FIELDS` 里 `hasMeaningfulValue` 为真的字段，
  最后写 `kind`）、`filter_slide_json(schema, slide)`（只保留该 kind 的 `fields`，见下面的「有意修改」）；
- 牌组操作：`insert_new_slide(deck, active, kind)`（插在 `active + 1`，章节号继承上一页；上一页是 cover / overview / closing 时不写 `section`，
  原版 `NON_CHAPTER_KINDS`）、`delete_slide(deck, index)`（只剩一页时不删；返回新的当前页 `min(index, len - 1)`）、`move_slide(deck, index, dir)`
  （越界不动；返回新的当前页）。

**对照测试用原版代码生成期望值**（与前两轮同样做法）：一次性 `bun` 脚本放 `target/` 下、不提交，把 `useSlideEditor.ts` 与 `Step3Editor.vue`
里的上述函数**原样**拷进去，`schemaStore` 换成读 `crates/huanshu-core/tests/fixtures/editor-schema.json`，对一组输入跑一遍写成
`crates/huanshu-core/tests/fixtures/slide-edit.json`；Rust 单测逐条比对（**按值比较**，不比较键的顺序）。输入至少覆盖：
全部 18 个 kind 互相切换（`create_kind_blueprint` 带 / 不带 seed，seed 里有空字符串、空数组、空对象）、每个 kind 的 `filter_slide_json`、
多层路径与越界下标的 `set_at_path`、每种列表操作的两端、新建页在 cover / overview / closing / 普通页之后、删除与移动的两端。

**有意修改（两边一起改）**：原版 `filterSlideJson` 只保留该 kind 的 `fields`，而**没有一个 kind 的 `fields` 含 `aspect_ratio`**——
应用一次更改就把这一页的 `aspect_ratio` 丢了。第 2 步取 `blueprints[0].aspect_ratio` 定看板比例，`get_retry_layouts` 也会读它，
32:9 / 48:9 的牌组改过封面后第 2 步就按 16:9 显示。改成「`fields` 之外，原来有 `aspect_ratio` 的照样保留」，
**TypeScript 的 `filterSlideJson` 同步改这一行**，再跑脚本生成快照（汇报里写明这是有意修改）。

**允许的重复（五）**：TypeScript 那几个函数继续保留，文件头注释「改一边必须改另一边」，由快照测试守住。不要自己改 `CLAUDE.md` 的重复清单，
汇报里提出来由审查者加。

## B. 牌组的唯一写入路径（`pages/workflow/mod.rs` + `generation.rs`）

原版第三步直接改 `gen.blueprints`（pinia store）再整副存库。原生版里 store 正持有本项目时它的牌组是 `ProjectModel.deck` 的来源
（`on_generation_changed` 会把 store 的牌组镜像进模型），所以**只改模型会被 store 下一次通知盖掉**；store 正为别的项目跑时它不描述本项目，
又不能写进去。做法：

- `GenerationStore` 加一个写入口，例如 `set_deck(project_id, deck: Vec<Value>)`：只有 `live_for(project_id)` 成立且**不在生成中**时才写
  （生成中的牌组有空位，下标对不上），写完 `cx.notify()`；
- `WorkflowPage` 加**唯一**的提交函数，例如 `commit_deck(deck, active, save: bool)`：改 `ProjectModel.deck` 与 `active_slide` → 调 store 的写入口 →
  `save` 时 `run_bg` 调 `services::projects::update_project_blueprints`（整副 JSON）。保存失败要给出错误（见 D 节「应用更改」）。
  `on_generation_changed` 的镜像比较后相等就不动，不会回环；
- **生成进行中（本项目 `state.running`）时第三步只读**：应用、新建、删除、移动、换版式都禁用，右栏保存按钮旁显示「生成进行中，完成后可编辑」。
  原版没挡（它的第三步遇到空位会出错），这是有意偏离，写进 parity 与汇报第 8 节；
- 哪些操作存库照原版：**应用更改、删除、移动**立即整副存库；**新建**只插进牌组、不存库（原版如此，新页随下一次应用 / 删除 / 移动一起存）。

## C. 编辑草稿（`pages/workflow/step3.rs`）

原版 `useSlideEditor` 的状态搬进 `Step3`：

- `draft: Option<Value>`（原版 `liveSlide`，**未过滤**的整页）、`has_unsaved`、`saving`、`save_error`；
- 进入第 3 步、换页（列表点击、↑/↓、翻页按钮、删除 / 移动之后）时 `init_draft`：草稿 = 牌组当前页的克隆，`has_unsaved = false`。
  **换页会丢掉未应用的修改**——原版如此，照做（不要加确认框）；
- 每次表单改动：`slide_edit` 的函数算出新草稿 → `has_unsaved = true` → 预览；
- **应用更改**：草稿经 `filter_slide_json` 后写回牌组当前页 → `commit_deck(.., save = true)` → 成功后草稿 = 过滤后的页、`has_unsaved = false`；
  `saving` 期间按钮显示「保存中…」并禁用；按钮三态文案逐字：`保存中…` / `应用更改 (Ctrl+S)` / `已保存`（图标 `renew` / `save` / `checkmark`）；
- **Ctrl+S**：原版只在高级 JSON 文本框里生效；这里在第三步的任何位置（含输入框里）都生效，算有意的小改进，写进汇报第 8 节。

## D. 第三步界面（`pages/workflow/step3.rs`）

### 预览（中栏）

- **只发当前这一页**：`SlideImages` 的牌组是 `[draft]`（下标恒为 0），宽度 = 舞台显示宽度；草稿变化后 **120ms** 防抖再 `set_deck`
  （原版 `debounceMs: 120`），`serial` + `progressive` 用默认值；**换页时清掉旧图**（原版 `resetKey`）：给 `SlideImages` 加 `reset(cx)`
  （内部调 `PreviewCache::reset`，返回的图逐张 `drop_image`），不要靠重建 `SlideImages`；
- 舞台按项目比例（`ProjectModel::aspect_ratio()`）在中栏里取最大的框，四周留 16px；
- 「渲染中…」：给 `SlideImages` 加 `pending()`（`PreviewCache::is_pending`），等待持续超过 150ms 才显示（GPUI 计时器）；
- 裁切提示文案不变（「内容过多，已缩到最小字号仍放不下，超出部分被裁切」，前面 `warning-alt` 图标）；
- 离开第三步 `release` 照旧。

### 右栏表单（按 `editor_schema` 生成，**一行规则都不要手写**）

顺序与文案逐字照原版模板：

1. 「幻灯片类型」：图标、中文名、kind、「更换」按钮（打开 KindPicker 的 change 模式）；
2. 「编辑模式」+「打开高级 JSON / 收起高级 JSON」按钮与说明文字：本轮按钮可点，展开后是一个 `todo_block`（下一轮做）；
3. 「基础信息」（`simple_fields` ∩ 当前 kind 的 `fields`）：`text` 单行、`textarea` 多行（`rows`，缺省 3）、`select` 下拉（首项「请选择」= 空串）；
   `textarea` 占整行，其余两列网格（原版 `.ej-structured-grid`）；
4. 「标签组」（kind 的 `fields` 含 `badges` 时）：每项一个输入框 + 删除，「添加标签」；
5. 「图片素材」：本轮只做主图 / 图集的**文本输入**（主图输入框、图集每项输入 + 删除 +「添加图集图片」），素材条（缩略图、设为主图、加入图集）
   留 `todo_block`；
6. 列表区（`list_sections(kind)`）：每区标题 +「添加」（用 `new_item`）；空时显示 `empty_text`；每项一张卡片「{label} {n}」+ 上移 / 下移 / 删除；
   卡片里是字段（多了 `boolean` → 勾选框，勾选框自带标签、不再显示上方小标签）、`string_lists`（每项输入 + 删除 +「添加」）、
   `nested_lists`（嵌套卡片 + 删除 +「添加」，嵌套字段只有 text / textarea）；`issue_stack` 的 `cards` 区上方那句提示按张数二选一（逐字）；
   有 `icon_query_keys` 的卡片里的「图标助手」本轮留 `todo_block`；
7. 对象区（`object_sections(kind)`）：字段网格 + 子列表区（同 6，但路径前缀是 `section.key`）；
8. `chart_focus` 的「图表数据（JSON）」本轮留 `todo_block`；
9. 保存按钮（C 节三态）+ 保存失败时按钮下方显示错误（原版写进 `jsonError`，下一轮才有 JSON 区，本轮先放这里）。

**GPUI 的做法（这是本轮最容易出错的地方）**：

- 输入框是有状态的实体（`InputState`），**不能每帧新建**（会丢焦点、丢光标、漏内存）。按**完整路径**（如 `cards[2].title`）缓存：
  `HashMap<String, Entity<InputState>>` + 各自的订阅；render 里按需创建（创建要 `&mut Window`，render 里有），初值取草稿；
- 输入事件 → `set_at_path` 改草稿；**不要**在 render 里回写正在编辑的输入框；
- **结构变化时整组重建缓存**：换页、应用更改、换版式、列表的添加 / 删除 / 移动（下标变了，路径就指向别的项）。用一个「表单代次」计数，
  变了就清空缓存与订阅、下一帧按新草稿重建。字段里打字**不**重建；
- 下拉用 `SelectState`、勾选用 GPUI Kit 的 `Checkbox`，同样按路径缓存 / 重建；
- 右栏整列 `overflow_y_scroll`。版式最多的页（`compare`、`swot`、`process`）表单很长，量一下打字时的帧耗时，卡就在汇报里写出来。

### KindPicker（GPUI Kit `Dialog`）

- 标题按模式：`新建幻灯片 — 选择类型`（图标 `add-alt`）/ `更换幻灯片类型`（图标 `arrows-horizontal`）；副标题逐字：
  `新幻灯片将插入到第 {N} 张之后` / `选择新类型后将自动保留可迁移的公共字段`；
- 网格列出 `schema.kinds` 全部 18 个：图标、中文名、kind、说明；change 模式下当前 kind 高亮；
- 选中：change → 草稿 = `create_kind_blueprint(kind, 草稿)`、`has_unsaved = true`、表单重建；add → `insert_new_slide` + `commit_deck(.., save = false)`、
  当前页 = 新页；
- 关闭：右上角关闭按钮、点遮罩、Esc 都能关。**原版点遮罩或关闭按钮关不掉**（事件写成了再次打开），这是原版的 bug，修掉并写进汇报第 8 节。
- 外框只让 `Dialog` 画（上一轮重试对话框画了两层，审查时改掉的），内容 `w_full`。

### 其余

- 键盘：**只有 ↑/↓** 换页（原版 `handleEditorArrowKeydown`）；带修饰键、KindPicker 打开时、**焦点在输入框里时**都不处理——在 GPUI 里判断
  「第三步的根节点自己有焦点」（`focus_handle.is_focused(window)`，不是 `contains_focused`）；点左栏行、点预览区时把焦点拿回根节点，
  否则点过输入框之后方向键就不灵了；
- 「导出 PDF」导出**内存里的牌组**（`services::export::export_pdf` 传 `slides`，含未存库的新建页），有未应用修改时结果对话框里加一行原版的提示
  「编辑器里还有未应用的修改，PDF 按应用前的内容导出。」（给 `export::show_report` 加可选的 `note`）；
- 「演示」从当前页开始，用内存里的牌组；
- 左栏标题行的「重新生成」→ 回第 1 步（原版 `go-to-step-1`）。

## 分支与提交

从 `feat/native-step2` 开 `feat/native-step3a`，四个提交：

1. `feat(core): slide editing rules shared with step 3`（A 节规则、快照、TS 的 `aspect_ratio` 一行）
2. `feat(native): one write path for deck edits`（B 节，`active_slide` 挪到模型）
3. `feat(native): step 3 editor — layout, forms, kind picker, apply`（C、D 节）
4. `docs(native): parity and screenshots for step 3 editing`（parity、截图、README 里新增的规矩）

## 验证

```bash
cargo fmt --all
cargo test --workspace
cargo build -p huanshu-native --release
```

真机验证（**全部在沙箱**，先备份再 `seed_demo --force`）：

- 「全部版式 16:9」逐页走一遍全部 18 个版式：每页截一张右栏表单（`tools/drive.ps1`），确认每个 `list_sections` / `object_sections`
  都渲染出来、字段标签与原版一致；挑 `compare`、`swot`、`process` 三页与原版代码逐项核对；
- 编辑闭环：改标题 → 预览更新、出现「● 未保存」→ Ctrl+S → 「已保存」→ 回首页再进项目 → 修改还在（用 Python `sqlite3` 只读打开
  `target/sandbox-home/data/projects.db`，贴出该页 `title` 前后的值）；
- 列表：添加 / 上移 / 下移 / 删除一项，确认输入框里的值跟着正确的项走（这一条专门验证「结构变化重建缓存」）；
- 换版式：feature_grid → section_list，确认 `title` / `subtitle` / `section` 保留、默认内容来自 schema；
- 新建：在第 5 页（普通内容页）之后新建，新页 `section` 与第 5 页相同；在封面之后新建，新页没有 `section`；
- 删除 / 移动：立即存库（同样用 sqlite 证明）；只剩一页时删除按钮不可用；
- 键盘：焦点在输入框里按 ↑/↓ **不换页**；点左栏行后 ↑/↓ 换页；KindPicker 打开时 ↑/↓ 无效；Esc / 点遮罩 / 关闭按钮都能关掉 KindPicker；
- 32:9 项目（「全部版式 32:9」）：舞台比例正确；改封面并应用后，`aspect_ratio` 仍在（sqlite 证明），第 2 步看板仍是 32:9；
- 截图与 `reference/project-step3.png`、`step3-kind-picker.png` 逐张对比列差异；`tools/shots.ps1` 的 `project-step3` 重拍，并加一张 KindPicker。

## 最终汇报（按此格式，不要省略）

1. 四个提交的 hash 与一句话说明
2. 每条验证命令的结果（贴输出末尾几行）
3. parity 表本轮改动的行（改动前 → 改动后，附依据）
4. A 节快照：生成脚本怎么写的、覆盖了哪些输入、Rust 与 TS 有没有不一致（有的话怎么处理的）
5. 截图对比清单与差异
6. 真机验证逐项结果（没做的写「未做」）；给用户的人工验收清单
7. 新增 / 修改的 core 函数与单测；有意修改的测试期望值及原因（没有就写「无」）
8. 与本说明或原版不一致的实现及原因；改动了框架接口的地方（`SlideImages::reset` / `pending`、`show_report` 的 `note`、store 的写入口）；
   每个「进行中」标志的复位出口；没做完或不确定的点

本轮要做成 ✅ 的 parity 行：工具栏（拆成三个栏头）、左侧列表、中间预览、裁切提示、只发当前页编译、版式卡片、更换版式、新建 / 删除 / 上下移动、
字段表单、列表区 / 嵌套列表 / 对象区、应用更改与未保存标记。保持 ⬜ 的：图标推荐、素材插图、图表 JSON 框、高级 JSON 模式。
