#!/usr/bin/env python3
"""Render four representative pages in every theme and build a comparison sheet.

    python3 keynote/tools/theme_sheet.py keynote/blueprints.json keynote/preview

The deck itself ships with one theme (报刊·钴蓝 / light-editorial); this sheet shows
what the very same blueprints look like under the other five theme dictionaries.
"""

import json
import subprocess
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parents[2]
RENDER = ROOT / "target/release/examples/render_fixture"
PAPER = "#f6f5f0"
INK = "#141414"
MUTED = "#73736f"
COBALT = "#2549d8"
EDGE = "#d8d6cf"
CJK = "/System/Library/Fonts/Hiragino Sans GB.ttc"

THEMES = [
    ("light-editorial", "报刊·钴蓝"),
    ("light-notes", "便签·稿纸"),
    ("light-terminal", "终端·纸"),
    ("dark-terminal", "终端·墨"),
    ("dark-yellow", "暗·琥珀"),
    ("dark-blue", "暗·星图"),
]

# cover · feature grid · chart focus · SWOT
PICKS = [0, 3, 18, 25]


def main():
    deck_path = Path(sys.argv[1] if len(sys.argv) > 1 else "keynote/blueprints.json")
    out = Path(sys.argv[2] if len(sys.argv) > 2 else "keynote/preview")
    deck = json.loads(deck_path.read_text(encoding="utf-8"))
    subset = [deck[i] for i in PICKS]
    tmp = ROOT / "target/theme-sheet-input.json"
    tmp.parent.mkdir(parents=True, exist_ok=True)
    tmp.write_text(json.dumps(subset, ensure_ascii=False, indent=2), encoding="utf-8")

    tw, th = 470, 264
    rows = len(THEMES)
    label_w, pad, gap = 190, 40, 16
    W = pad * 2 + label_w + len(PICKS) * tw + (len(PICKS) - 1) * gap
    H = 150 + rows * (th + gap) + 70
    sheet = Image.new("RGB", (W, H), PAPER)
    d = ImageDraw.Draw(sheet)
    f_title = ImageFont.truetype(CJK, 40, index=1)
    f_label = ImageFont.truetype(CJK, 22, index=1)
    f_small = ImageFont.truetype(CJK, 16)

    d.text((pad, 40), "同一份蓝图 · 六套主题", font=f_title, fill=INK)
    d.text((pad, 92), "上排是演示所用的主题；其余五套只需改一个主题 id", font=f_small, fill=MUTED)
    d.rectangle([pad, 126, W - pad, 129], fill=COBALT)

    for col, index in enumerate(PICKS):
        x = pad + label_w + col * (tw + gap)
        d.text((x, 138 + 4), f"{index + 1:02d}  {deck[index]['title'][:26]}", font=f_small, fill=MUTED)

    for row, (theme_id, label) in enumerate(THEMES):
        y = 150 + row * (th + gap)
        d.text((pad, y + 8), label, font=f_label, fill=INK)
        d.text((pad, y + 38), theme_id, font=f_small, fill=MUTED)
        out_dir = ROOT / f"target/theme-sheet/{theme_id}"
        if out_dir.exists():
            for stale in out_dir.glob("*.png"):
                stale.unlink()
        subprocess.run(
            [
                str(RENDER),
                str(tmp),
                str(out_dir),
                "--png",
                "--theme",
                theme_id,
                "--assets",
                str(ROOT / "keynote/assets.json"),
            ],
            check=True,
            stdout=subprocess.DEVNULL,
        )
        for col in range(len(PICKS)):
            page = out_dir / f"{col + 1:02d}-{subset[col]['kind']}.png"
            img = Image.open(page).resize((tw, th), Image.LANCZOS)
            x = pad + label_w + col * (tw + gap)
            sheet.paste(img, (x, y))
            d.rectangle([x, y, x + tw, y + th], outline=EDGE, width=1)

    target = out / "主题对照.png"
    out.mkdir(parents=True, exist_ok=True)
    sheet.save(target)
    print(f"{target}  {sheet.size[0]}x{sheet.size[1]}")


if __name__ == "__main__":
    main()
