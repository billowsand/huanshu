"""Generates spikes/gpui-kit/assets/sample.md: a ~10k-character mixed CJK/Latin
Markdown document for the editor page of the GPUI Kit spike (C1), with headings,
lists, tables, code blocks and long paragraphs."""

import io
import os

SENTENCES = [
    "幻述把每一页幻灯片都交给 Typst 排版，屏幕上的缩略图、演示与导出共用同一套模板，因此所见与所得不会漂移。",
    "原来的 Vue 模板已经全部删除，WebView 只剩外壳；这也是我们评估原生界面的起点。",
    "如果渲染时间超过一帧，用户就会觉得界面卡住，所以任何一次 Typst 编译都必须离开 UI 线程。",
    "Typst 的 comemo 缓存只增不减，必须在每次编译之后主动淘汰，否则长时间编辑会把内存吃光。",
    "候选项框、下划线的未上屏文本、标点全半角，这三件事决定了中文用户能不能舒服地写文档。",
    "GPUI 在 Windows 上通过 ImmSetCompositionWindow 通知输入法把候选框放到光标的位置。",
    "Release build matters: 只有优化过的构建才能给出可信的延迟数字，debug 下的耗时只能说明 debug。",
    "缩略图的宽度按 64 像素取整，宽度档位变化才重新光栅化，否则每一次窗口拖动都会触发整副牌重渲。",
    "一次请求里只包含变化的页面：整批丢弃会让上一批的页面永远等不到图。",
    "标题改一个字就要重排整页，因为自动缩放会重新测量所有文本块。",
]

LATIN = [
    "The renderer keeps one font store per process, and scanning the system fonts costs about half a second.",
    "A single compile lays out the whole deck; rasterization is incremental and measured in milliseconds.",
    "`cargo test --workspace` must keep passing: the spike lives in its own Cargo workspace.",
    "Images are validated before they reach Typst, because one corrupt PNG used to fail the whole export.",
]


def paragraph(index: int) -> str:
    parts = [SENTENCES[(index * 3 + i) % len(SENTENCES)] for i in range(4)]
    if index % 3 == 0:
        parts.insert(2, LATIN[index % len(LATIN)])
    return "".join(parts)


def main() -> None:
    out = io.StringIO()
    out.write("# 幻述原生界面验证文档\n\n")
    out.write(
        "这份文档由 `spikes/gpui-kit/assets/sample.md` 的生成脚本产出，用来在 GPUI Kit 的多行编辑器里"
        "验证滚动、软换行、选择、输入法与撤销重做。全文约一万字符，中英混排。\n\n"
    )
    for chapter in range(1, 7):
        out.write(f"## 第 {chapter} 章 · 渲染与界面\n\n")
        for section in range(1, 5):
            out.write(f"### {chapter}.{section} 小节标题\n\n")
            for p in range(3):
                out.write(paragraph(chapter * 10 + section + p) + "\n\n")
            out.write("- 第一条要点：输入延迟用十次中位数衡量。\n")
            out.write("- 第二条要点：内存曲线要连续观察两分钟。\n")
            out.write("- 第三条要点：窗口缩放时不应出现撕裂。\n\n")
            if section == 2:
                out.write("| 项目 | 取值 | 说明 |\n| --- | --- | --- |\n")
                out.write("| 缩略图宽度 | 560 px | 显示宽度 × 缩放比例 |\n")
                out.write("| 宽度档位 | 64 px | 取整后再重渲 |\n")
                out.write("| 缓存上限 | 10 页 | 演示模式预取窗口 |\n\n")
            if section == 3:
                out.write("```rust\n")
                out.write("// 后台线程里编译，结果回到 UI 线程再刷新。\n")
                out.write("let pages = renderer.render_pixels(&slides, &options, width, only)?;\n")
                out.write("cx.notify();\n")
                out.write("```\n\n")
            if section == 4:
                out.write("> 引用：一个坏公式会让整份导出失败，所以数学公式要先试编译。\n\n")
    out.write("## 附录 · 术语表\n\n")
    for term, desc in [
        ("Blueprint", "一页幻灯片的结构化 JSON，由 LLM 生成、由 Typst 渲染。"),
        ("Thumbnail", "光栅化后的 PNG 或 RGBA 像素，用于看板与演示。"),
        ("Stage", "演示模式里铺满屏幕的那一块画面。"),
        ("Serial", "同一时刻只允许一个渲染在途的调度策略。"),
    ]:
        out.write(f"- **{term}**：{desc}\n")
    out.write("\n")
    text = out.getvalue()
    path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "assets", "sample.md")
    path = os.path.normpath(path)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8", newline="\n") as handle:
        handle.write(text)
    cjk = sum(1 for ch in text if "\u4e00" <= ch <= "\u9fff")
    print(f"{path}: {len(text)} chars, {cjk} CJK, {text.count(chr(10)) + 1} lines")


if __name__ == "__main__":
    main()
