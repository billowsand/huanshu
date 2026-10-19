# 幻灯片字段契约

测试阶段按表达任务收敛字段，不保留旧文稿迁移和字段别名。

- `editor_schema.rs` 定义每种版式的字段、表单类型、枚举、默认内容。
- `slide_contract.rs` 从表单定义构建 JSON Schema，补充结构必填项、图表结构、SWOT 唯一性与面板模式约束。
- 生成提示直接使用该 Schema，不再另写一套 JSON 样例字段表。章节号、比例、原文图表仍由流水线附加。
- 生成结果在反序列化前检查字段，内部通用蓝图序列化时只输出当前版式的字段。
- 手工 JSON 不经过静默过滤。格式化与表单编辑保留输入，非法字段显示路径，并阻止应用、保存和导出。
- 预览与导出共用 `prepare_export_values` 的字段检查。未知版式仍可显示诊断页；图表数据与图片资源问题保留明确占位提示。

## 本次字段决策

| 范围 | 契约 |
|---|---|
| section_intro / feature_grid | 移除卡片 tag。章节卡的要点数组可在表单编辑。 |
| outcome_grid | title / body / tone / icon / 可选 tag；移除 conclusion / footer_tag / footer_tone。 |
| 所有卡片 | 移除 top_bar_class；样式由 tone 与主题控制。 |
| focus_example | points 不含 step；ranking_title 是可选内容，不再写死“正确的关注顺序”。 |
| chart_focus | points 不含 step，由模板依顺序编号；移除 emphasis.trend。 |
| SWOT | key 仅接受四种全名，必须各出现一次，输入顺序不决定版面位置。 |
| spotlight | kind 为 text / flow / matrix / steps，统一使用 items；text 不接受非空 items。 |
| 图片 | image / images 仅适用于 spotlight。 |
| 色调 | amber / blue / green / red，表单、默认值和生成使用同一词表。 |
| 其他有效内容 | 表单与类型补入 layers_title、process.summary、phases[].subtitle。 |
| feature_grid 高级样式 | 移除 number、titleTone、camelCase footer 别名及对象式 items，条目统一为纯文本。 |

## 验证

`cargo test -p huanshu-core` 验证默认页与强类型往返、非法字段保留及报错、图表导入、枚举、保存和导出边界。

`cargo test -p slide-typst` 验证所有主题中的实际文本、SWOT 排列与摘要位置、页面数量、缩放与溢出报告。

`cargo check -p huanshu-native` 验证原生编辑器集成。
