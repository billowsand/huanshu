# Auto-Slidev 模板扩展说明

## 新增一个 Slide 模板的标准流程

当你要给 Auto-Slidev 增加一个新模板时，按下面顺序改：

1. `src/components/`
   新建 `KeynoteXxxSlide.vue`，Props 设计需清楚。
2. `src-tauri/src/types.rs`
   新增 `SlideKind` 枚举项，以及该模板对应的数据结构。
3. `src-tauri/src/generator/render.rs`
   把 blueprint 渲染成 `<KeynoteXxxSlide ... />`。
4. `src-tauri/src/generator/planning.rs`
   在 layout plan 提示词里写清楚：
   - 什么时候应该选这个模板
   - 什么时候不能选这个模板
   - 它和相邻模板的边界是什么
5. `src-tauri/src/generator/utils.rs`
   在 `blueprint_schema_hint()` 里加入严格 JSON 示例。
6. `src-tauri/src/generator/slides.rs`
   在内容生成提示词里加入该模板的字段要求。
7. `src-tauri/src/generator/normalize.rs`
   增加默认值修复、长度收敛、tone/icon 修复。
8. `src-tauri/src/validate.rs`
   增加结构校验，避免生成结果进入非法状态。
9. 文档
   更新 `CLAUDE.md`，把组件用途、Props 和新增流程记录下来。


## 做新模板时最容易漏掉的点

- 只写了 Vue 组件，没有把它接到 `SlideKind`
- 只改了 layout 选择，没有补内容生成提示词
- schema hint 不完整，导致 LLM 输出字段漂移
- 没有 normalize/validate，偶发坏 JSON 会直接进渲染阶段
- 没有写"不要使用此模板"的规则，导致模板被误选
