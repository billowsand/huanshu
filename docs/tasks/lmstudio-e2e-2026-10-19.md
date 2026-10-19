# 本地 LM Studio 全流程回归（2026-10-19）

结论：本次样例的生成契约、编辑回写、存储及 Typst 渲染通过；自由文本图表解读仍存在事实错误，内容质量不能判定为全部通过。

## 环境与隔离

- 服务：http://127.0.0.1:12345；文本模型 `qwen/qwen3.5-9b`，嵌入模型 `text-embedding-bge-m3`。
- 全部数据库、模型请求记录、图片、PDF 在 `target/lmstudio-e2e/` 的隔离目录内；没有打开或修改用户正常数据目录。
- 直接调用生产生成、修复、重试、项目保存、预览及 PDF 导出服务；编辑回写调用与原生表单共用的规则，未模拟鼠标操作界面。

## 实测覆盖

- `fixed/`：20 个内容样例，覆盖全部内容版式、spotlight 的 text/flow/matrix/steps 及五种图表；生产组装封面、目录、结束页后共 23 页。
- `pipeline-fixed/`：从 Markdown 经嵌入缓存、页面规划、版式分配、内容生成、校验修复、组装及保存生成 21 页（18 个内容页）。
- `pipeline-fixed/retry-summary.json`：真实生成记录上的图表页重试通过；图表源数据和其他 20 页完全不变。
- `repair/`：真实反馈修复通过，process 每阶段 subtitle、全页 summary 可生成并回写；图表源数据与章节不变，修复遵循项目选定的 32:9 比例。
- 每套牌组均测试 6 主题 × 3 比例（16:9、32:9、48:9）的 PDF 与预览位图，页数一致，无溢出或缺图报告；抽查实际 PNG 的排版及新增字段。
- `chart-final/`：加入确定性计算事实后重测五种图表，共 8 页，再次通过 18 种渲染组合。此处通过仅指结构与渲染，不代表下面的解读文字问题已解决。
- 核心自动测试：224 通过、1 忽略；Typst 自动测试：66 通过、1 忽略。忽略项为已有手动探针，另有上述真实服务测试补充。测试日志位于 `target/lmstudio-e2e-final-tests.log` 和 `target/lmstudio-e2e-typst-tests.log`。

每套测试同时验证严格字段契约、typed JSON 往返、编辑字段回写不丢其他字段、项目保存后读取一致。`fixed/` 和完整流水线日志没有出现确定性兜底页面记录。

## 发现并修复

1. 模型在对象数组中夹入字符串时，原实现尚未进入 typed 修复循环就失败。现在先进行契约检查，携带具体错误最多请求三次完整更正，保留每次原始回答；网络错误直接报错。
2. 图标规范化给 chart_focus.points 填充 schema/模板不支持的 icon，引起重复修复。现在按编辑契约判断该列表是否支持图标；focus_example 的图标仍正常使用。
3. 自动内容修复使用另一套宽松解析路径，模型会混淆页面与规划元数据。现在与首次生成及反馈修复共用严格入口，并保留原始图表及章节。
4. 反馈修复可能带回旧页面比例。现在使用项目当前选定的比例。
5. 代码计算图表极值、并列项、相邻增量、增速和占比，提供给模型；零或负基数不生成误导性百分比。加入对应自动测试。

## 尚未解决的内容质量问题

即使给出准确计算结果，当前本地模型也可能违反事实。例如 `chart-final/chart-donut/final.json` 同时生成“三季度份额最高（44.44%）”和“二季度占比最高（33.33%）”；分组柱图还出现“上年增速快于本年”的过度概括。更早样例曾把 50%→33.33% 说成增速加快。

因此，JSON/编辑器/渲染一致性已经有实际测试证据，但不能用 schema 校验或提示词证明自然语言结论准确。后续应把数值结论限制为程序计算的事实选择和受控表述，或加入独立内容验收；仅继续加提示词不能消除这类风险。本次保留失败样例供复现，没有把语义问题记为通过，也没有擅自覆盖用户手写的图表解读。

另一个显示口径需注意：圆环图标签已有“最大余数法、两位小数合计 100%”规则，12/18/24 样例显示为 22.22%/33.33%/44.45%；按单项独立四舍五入的正文则为 44.44%。这不是数据丢失，但若要求正文和标签完全相同，还需要统一百分比展示口径。

## 可执行文件

已重新编译 release 并复制到 `dist-package/huanshu-native-lmstudio-tested_windows_x64.exe`（89,168,384 字节），与 `target/release/huanshu-native.exe` 的 SHA-256 完全一致：`57CD2D9E98589460FA25FDFF5BBE9816F415C29FF85BE0C8ECA23D330EEF3191`。没有启动用户正常数据目录中的应用。

## 复现

从仓库根目录执行，输出目录必须位于本仓库 `target/` 内：

```powershell
cargo run -p huanshu-core --example contract_e2e -- matrix target/lmstudio-e2e/new-matrix
cargo run -p huanshu-core --example contract_e2e -- pipeline target/lmstudio-e2e/new-pipeline
cargo run -p huanshu-core --example contract_e2e -- retry target/lmstudio-e2e/new-pipeline
```

`repair` 模式使用本次 `fixed/` 中的 process 和 chart-line 作为真实模型底稿。可用 `LMSTUDIO_BASE_URL`、`LMSTUDIO_MODEL` 覆盖服务与模型；`E2E_ONLY` 可选择逗号分隔的 matrix 样例名。
