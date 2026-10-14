# 任务：原生界面第一轮补全——设置、首次向导、加密导入导出（第九步）

你在仓库 `E:\huanshu`（幻述）里工作。原生界面 `apps/huanshu-native`（GPUI Kit）的**框架已经由审阅方搭好**，
本轮在这个框架里把三块功能补全到与原版（Tauri + Vue）一致。开始前**必须**先读完：

1. `apps/huanshu-native/README.md`——框架结构与「写页面的规矩」（八条，逐条遵守）；
2. `docs/native/parity.md`——功能对照表，本轮要把其中「设置」「首次向导」两节和「首页」的加密两行全部做成 ✅；
3. `apps/huanshu-native/src/pages/home.rs`——**样板页**，新代码照它的写法；
4. `CLAUDE.md` 开头的「代码结构」「隔离数据目录」「原生界面」三节；
5. 原版源码：`src/views/SettingsView.vue`、`src/components/SystemFontPicker.vue`、`src/views/FirstRunWizard.vue`、
   `src/components/PasswordDialog.vue`、`src/components/ExportDialog.vue`、`src/views/HomeView.vue`（`handleExport*` / `handleImport` / `handlePasswordSubmit`）；
6. 原版截图：`docs/native/reference/settings-*.png`、`wizard-*.png`、`home-export-*.png`、`home-delete-confirm.png`。

## 原则

- **复刻**：布局、文案（逐字）、交互、校验规则、错误提示以原版为准；可以按 GPUI 的特点调整控件形态
  （例如原版的 `<select>` 用 GPUI Kit 的 `Select`），但**不能少功能**。拿不准时读原版代码，不要凭感觉。
- **不重写框架**：不改 `shell.rs` / `router.rs` / `theme/` / `slides.rs` / `tasks.rs` 的接口；缺部件就往 `widgets.rs` 里**加**，
  不在页面里写第二份样式。确实需要改框架接口时，先在汇报第 8 节说明理由。
- **数据只经 `huanshu_core::services`**；需要新的 core 函数时加在 core（带单测），不要在界面里拼 SQL 或读写配置文件。

## 前几轮审查发现的问题（不要再犯）

1. **真机验证改坏真实数据**：只在沙箱里跑（`HUANSHU_HOME=E:\huanshu\target\sandbox-home`）。本轮的设置页会**写**设置：
   每次启动前确认环境变量已设置；动手前先备份沙箱（复制 `target/sandbox-home` 到 `target/sandbox-home.bak`）。
2. **测量量错了对象 / 报告与事实不符**：没验证的写「未验证」；截图逐张亲眼看过再写「一致」。
3. **异步结果落错地方**：测试连接、初始化向量缓存是异步的，结果回来时用户可能已换了服务卡片或离开页面——带上发起时的身份比对。
4. **全应用的按键观察者**：快捷键（`Ctrl+S`）用 `track_focus` + `on_key_down`，不要用 `observe_keystrokes`。
5. **悄悄改测试期望值**：有意修改的快照 / fixture 在汇报里单独列出。

## A. 设置页（`pages/settings.rs`）

把 `todo_block` 全部替换成实现，parity 表「设置」一节全部 ✅：

1. **字体**（「外观与字体」下半部分）：七个角色（界面 / 标题 / 副标题 / 正文 / 注释与标签 / 数字与序号 / 代码与编辑器，
   名称与说明照原版 `fontRoles`），每个角色一个**可搜索**的系统字体选择（`huanshu_core::fonts::enumerate_system_fonts`，在后台枚举；
   显示「已安装 N 种字体」；未选时显示「默认」），「统一所有字体」开关（开启后改任一角色同步到全部）、「恢复默认」；
   右侧「演示预览 / 界面预览」两个标签页（文案照抄原版），预览随草稿实时变化。
2. **模型服务**：三类服务卡片（大语言模型 / 向量模型 / 多模态模型）及状态（`serviceState`）；选中服务的表单：服务地址、API Key、
   模型（从 `list_models` 拉取的下拉，可切「手动输入模型 ID」）、SOCKS5 代理开关与地址 / 用户名 / 密码；「连接状态」卡片
   （服务类型、模型、响应耗时、测试连接按钮与错误）；「这个服务用于」列表。**把服务表单做成一个独立组件**
   （放 `pages/settings/` 子模块或 `widgets` 之外的单独文件），B 节的向导要复用它。
3. **生成设置**：自动修复轮数、生成并发数（范围、默认值、提示文案照原版）。
4. **存储与缓存**：图标向量缓存（状态、当前向量模型、缓存对应模型、「初始化缓存」；模型配置未保存时禁用并提示）。
5. **保存条**：一个统一的草稿（主题 + 字体 + 模型 + 生成参数），状态文案四种（正在保存… / 更改已保存 / 有未保存的更改 /
   所有更改已保存）、撤销更改、保存更改；保存失败显示错误。界面字体保存后立即生效（`theme::apply`）。`Ctrl+S` 保存。
6. **离开确认**：有未保存更改时，返回工作台弹「有未保存的更改」（保存并离开 / 放弃更改 / 继续编辑，文案照原版）；
   放弃时主题与字体恢复为上次保存的样式。
7. 为纯逻辑写单测（放在对应模块的 `#[cfg(test)]`）：草稿是否 dirty 的判断、统一字体的同步、服务状态推断。

## B. 首次向导（`pages/wizard.rs`）

parity 表「首次向导」一节全部 ✅：

1. 第一步：数据目录 / 媒体目录可编辑，「浏览...」用 `rfd` 选目录；默认值**必须来自 core 当前的 `app_settings`**（尊重沙箱，见 CLAUDE.md
   关于原版向导的警告）。
2. 第二步：复用 A.2 的服务表单（三类服务），「下一步」前校验（`verifyModels` 的规则），错误显示在表单下方；「跳过」照原版。
3. 第三步：初始化向量库（状态文案、开始初始化、进行中禁用）。
4. 完成页：摘要（目录、三类模型、向量库），「进入 Studio」先保存设置（目录改过时 `save_app_settings`，并按原版提示「需要重启应用」），
   再 `complete_first_run`，然后进首页。

## C. 首页加密导入导出（`pages/home.rs`）

1. 密码对话框组件（对应 `PasswordDialog.vue`，encrypt / decrypt 两种模式）：密码与确认密码、显示/隐藏、强度条与标签
   （评分规则照原版第 23–41 行：≥8 +1、≥12 +1、大小写都有 +1、有数字 +1……），提交条件与错误文案照原版；
   提交前的强度要求与 core 的 `crypto::validate_password_strength` 一致（写单测对照）。
2. 导出对话框里的「加密导出」：选中后输入密码 → `export_project(id, Some(true), Some(password))` → 保存文件。
3. 导入：`is_encrypted_file` 为真时弹 decrypt 模式；密码错误时在对话框里显示错误并允许重试（原版 `externalError`），不关闭对话框。

## 分支与提交

从 `feat/native-shell` 新建分支 `feat/native-settings`，按顺序提交（每步提交前跑完验证）：

1. `feat(native): settings — fonts, model services, generation, storage, save bar`（A）
2. `feat(native): first-run wizard`（B）
3. `feat(native): encrypted .keynn export and import`（C）

每个提交都要同步更新 `docs/native/parity.md` 对应行（状态 + 依据）。

## 验证

```bash
cargo fmt --all
cargo test --workspace
cargo build -p huanshu-native --release
```

真机验证（**全部在沙箱**）：

- 截图：`powershell -NoProfile -ExecutionPolicy Bypass -File apps/huanshu-native/tools/shots.ps1`，必要时在 `shots.ps1` 里加路由；
  与 `docs/native/reference/` 同名截图**逐张对比**，在汇报里列出每张的差异（没有差异写「一致」）。
- 设置：改主题 + 一个字体 + 一个模型地址 → 保存 → 退出重开，确认都保留；改了不保存 → 返回 → 确认框三个按钮各试一次。
- 测试连接：LM Studio 没在跑时应显示连接失败的错误（照原版文案）；在跑时显示模型列表与耗时。没条件的写「未验证」。
- 向导：用一个**新的**沙箱目录（例如 `target/sandbox-wizard`，空目录）启动，走完四步，确认 `bootstrap.json` 与数据库都写在这个目录里、
  真实目录没有任何变化（汇报里贴两边目录的修改时间）。
- 加密：在沙箱里加密导出一个项目 → 删除 → 用错误密码导入（看到错误、可重试）→ 用正确密码导入，页数一致。
- 自动化点击可以照 `apps/huanshu-native/tools/` 的做法（SetCursorPos + SendKeys）；做不到的交互写进人工验收清单。

## 最终汇报（按此格式，不要省略）

1. 三个提交的 hash 与一句话说明
2. 每条验证命令的结果（贴输出末尾几行）
3. parity 表本轮改动的行（改动前 → 改动后，附依据）
4. 截图对比清单：每张原生截图对应哪张原版截图、差异是什么
5. 真机验证逐项结果（没做的写「未做」）；给用户的人工验收清单
6. 新增 / 修改的 core 函数与单测
7. 有意修改的测试期望值及原因；没有就写「无」
8. 与本说明或原版不一致的实现及原因；改动了框架接口的地方；没做完或不确定的点
