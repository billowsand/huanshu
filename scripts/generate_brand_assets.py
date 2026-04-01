#!/usr/bin/env python3
from __future__ import annotations

import math
import struct
import zlib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
TAURI_ICONS = ROOT / "src-tauri" / "icons"
BRAND_ASSETS = ROOT / "src" / "assets" / "brand"
PUBLIC_DIR = ROOT / "public"


Color = tuple[int, int, int, int]


def clamp(value: float, low: float = 0.0, high: float = 1.0) -> float:
    return max(low, min(high, value))


def mix(a: float, b: float, t: float) -> float:
    return a + (b - a) * t


def mix_color(a: Color, b: Color, t: float) -> Color:
    return tuple(int(round(mix(x, y, t))) for x, y in zip(a, b))  # type: ignore[return-value]


def with_alpha(color: Color, alpha: float) -> Color:
    return color[0], color[1], color[2], int(round(color[3] * clamp(alpha)))


def rgba(r: int, g: int, b: int, a: int = 255) -> Color:
    return r, g, b, a


def blend(dst: Color, src: Color) -> Color:
    sa = src[3] / 255.0
    da = dst[3] / 255.0
    out_a = sa + da * (1.0 - sa)
    if out_a <= 0.0:
      return 0, 0, 0, 0

    def channel(d: int, s: int) -> int:
        return int(round((s * sa + d * da * (1.0 - sa)) / out_a))

    return channel(dst[0], src[0]), channel(dst[1], src[1]), channel(dst[2], src[2]), int(round(out_a * 255))


def coverage(sd: float, aa: float = 0.9) -> float:
    return clamp(0.5 - sd / (2.0 * aa))


def length(x: float, y: float) -> float:
    return math.hypot(x, y)


def rotate_inverse(x: float, y: float, cx: float, cy: float, angle_deg: float) -> tuple[float, float]:
    angle = math.radians(-angle_deg)
    s = math.sin(angle)
    c = math.cos(angle)
    dx = x - cx
    dy = y - cy
    return dx * c - dy * s, dx * s + dy * c


def sd_round_rect(x: float, y: float, cx: float, cy: float, w: float, h: float, r: float, angle_deg: float = 0.0) -> float:
    px, py = rotate_inverse(x, y, cx, cy, angle_deg)
    qx = abs(px) - w / 2.0 + r
    qy = abs(py) - h / 2.0 + r
    ox = max(qx, 0.0)
    oy = max(qy, 0.0)
    return length(ox, oy) + min(max(qx, qy), 0.0) - r


def sd_circle(x: float, y: float, cx: float, cy: float, radius: float) -> float:
    return length(x - cx, y - cy) - radius


def sd_segment(x: float, y: float, ax: float, ay: float, bx: float, by: float) -> float:
    pax = x - ax
    pay = y - ay
    bax = bx - ax
    bay = by - ay
    denom = bax * bax + bay * bay
    h = 0.0 if denom == 0.0 else clamp((pax * bax + pay * bay) / denom)
    dx = pax - bax * h
    dy = pay - bay * h
    return length(dx, dy)


def png_chunk(tag: bytes, data: bytes) -> bytes:
    return (
        struct.pack(">I", len(data))
        + tag
        + data
        + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
    )


def encode_png(width: int, height: int, rows: list[bytearray]) -> bytes:
    raw = bytearray()
    for row in rows:
        raw.append(0)
        raw.extend(row)
    header = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    return b"".join(
        [
            b"\x89PNG\r\n\x1a\n",
            png_chunk(b"IHDR", header),
            png_chunk(b"IDAT", zlib.compress(bytes(raw), 9)),
            png_chunk(b"IEND", b""),
        ]
    )


class Canvas:
    def __init__(self, size: int):
        self.size = size
        self.rows = [bytearray(size * 4) for _ in range(size)]

    def blend_pixel(self, x: int, y: int, color: Color) -> None:
        if x < 0 or y < 0 or x >= self.size or y >= self.size or color[3] <= 0:
            return
        row = self.rows[y]
        idx = x * 4
        dst = row[idx], row[idx + 1], row[idx + 2], row[idx + 3]
        out = blend(dst, color)
        row[idx : idx + 4] = bytes(out)

    def draw_bounds(self, left: float, top: float, right: float, bottom: float, painter) -> None:
        min_x = max(0, int(math.floor(left)))
        min_y = max(0, int(math.floor(top)))
        max_x = min(self.size - 1, int(math.ceil(right)))
        max_y = min(self.size - 1, int(math.ceil(bottom)))
        for y in range(min_y, max_y + 1):
            py = y + 0.5
            for x in range(min_x, max_x + 1):
                px = x + 0.5
                color = painter(px, py)
                if color is not None:
                    self.blend_pixel(x, y, color)

    def to_png(self) -> bytes:
        return encode_png(self.size, self.size, self.rows)


def radial_glow(x: float, y: float, cx: float, cy: float, radius: float) -> float:
    d = length(x - cx, y - cy) / max(radius, 1e-6)
    return clamp(1.0 - d * d)


def brand_palette() -> dict[str, Color]:
    return {
        "bg_top": rgba(13, 22, 35),
        "bg_bottom": rgba(27, 45, 67),
        "bg_warm": rgba(246, 184, 91, 255),
        "bg_cool": rgba(63, 178, 219, 255),
        "cool_card_a": rgba(112, 225, 247, 255),
        "cool_card_b": rgba(32, 144, 201, 255),
        "warm_card_a": rgba(255, 217, 146, 255),
        "warm_card_b": rgba(240, 166, 63, 255),
        "sheet_a": rgba(255, 247, 233, 255),
        "sheet_b": rgba(247, 228, 196, 255),
        "ink": rgba(22, 60, 99, 255),
        "orb": rgba(244, 178, 75, 255),
        "spark": rgba(89, 209, 242, 255),
    }


def draw_background(canvas: Canvas, scale: float, round_mask: bool) -> None:
    p = brand_palette()
    size = canvas.size
    center = size / 2.0
    radius = 29.0 * scale * size / 64.0

    def painter(px: float, py: float) -> Color | None:
        if round_mask:
            sd = sd_circle(px, py, center, center, radius)
        else:
            sd = sd_round_rect(px, py, center, center, radius * 2.0, radius * 2.0, 18.0 * scale * size / 64.0)
        cov = coverage(sd, 1.2)
        if cov <= 0.0:
            return None
        t = clamp(py / max(size - 1, 1))
        base = mix_color(p["bg_top"], p["bg_bottom"], t)
        warm = radial_glow(px, py, size * 0.28, size * 0.2, size * 0.62)
        cool = radial_glow(px, py, size * 0.78, size * 0.82, size * 0.8)
        r = clamp((base[0] + p["bg_warm"][0] * warm * 0.18 + p["bg_cool"][0] * cool * 0.11) / 255.0) * 255
        g = clamp((base[1] + p["bg_warm"][1] * warm * 0.13 + p["bg_cool"][1] * cool * 0.12) / 255.0) * 255
        b = clamp((base[2] + p["bg_warm"][2] * warm * 0.06 + p["bg_cool"][2] * cool * 0.18) / 255.0) * 255
        return int(r), int(g), int(b), int(round(255 * cov))

    canvas.draw_bounds(0, 0, size - 1, size - 1, painter)

    stroke_color = rgba(255, 255, 255, 22)

    def stroke_painter(px: float, py: float) -> Color | None:
        if round_mask:
            sd = abs(sd_circle(px, py, center, center, radius - 0.8)) - 0.8
        else:
            sd = abs(sd_round_rect(px, py, center, center, radius * 2.0, radius * 2.0, 18.0 * scale * size / 64.0)) - 0.9
        cov = coverage(sd, 1.0)
        if cov <= 0.0:
            return None
        return with_alpha(stroke_color, cov)

    canvas.draw_bounds(0, 0, size - 1, size - 1, stroke_painter)


def draw_rotated_card(canvas: Canvas, cx: float, cy: float, w: float, h: float, r: float, angle: float, color_a: Color, color_b: Color, alpha: float, stroke: Color | None = None, stroke_width: float = 1.1) -> None:
    def fill_painter(px: float, py: float) -> Color | None:
        sd = sd_round_rect(px, py, cx, cy, w, h, r, angle)
        cov = coverage(sd, 1.0)
        if cov <= 0.0:
            return None
        _, local_y = rotate_inverse(px, py, cx, cy, angle)
        t = clamp((local_y + h / 2.0) / max(h, 1e-6))
        color = mix_color(color_a, color_b, t)
        return with_alpha(color, alpha * cov)

    pad = max(r, stroke_width) + 3.0
    canvas.draw_bounds(cx - w / 2.0 - pad, cy - h / 2.0 - pad, cx + w / 2.0 + pad, cy + h / 2.0 + pad, fill_painter)

    if stroke is not None:
        def stroke_painter(px: float, py: float) -> Color | None:
            sd = abs(sd_round_rect(px, py, cx, cy, w, h, r, angle)) - stroke_width / 2.0
            cov = coverage(sd, 1.0)
            if cov <= 0.0:
                return None
            return with_alpha(stroke, cov)

        canvas.draw_bounds(cx - w / 2.0 - pad, cy - h / 2.0 - pad, cx + w / 2.0 + pad, cy + h / 2.0 + pad, stroke_painter)


def draw_circle(canvas: Canvas, cx: float, cy: float, radius: float, color: Color, alpha: float = 1.0) -> None:
    def painter(px: float, py: float) -> Color | None:
        cov = coverage(sd_circle(px, py, cx, cy, radius), 0.9)
        if cov <= 0.0:
            return None
        return with_alpha(color, alpha * cov)

    canvas.draw_bounds(cx - radius - 2.0, cy - radius - 2.0, cx + radius + 2.0, cy + radius + 2.0, painter)


def draw_segment(canvas: Canvas, ax: float, ay: float, bx: float, by: float, width: float, color: Color, alpha: float = 1.0) -> None:
    def painter(px: float, py: float) -> Color | None:
        cov = coverage(sd_segment(px, py, ax, ay, bx, by) - width / 2.0, 0.95)
        if cov <= 0.0:
            return None
        return with_alpha(color, alpha * cov)

    pad = width + 2.0
    canvas.draw_bounds(min(ax, bx) - pad, min(ay, by) - pad, max(ax, bx) + pad, max(ay, by) + pad, painter)


def transform(value: float, size: int, scale: float, center: float | None = None) -> float:
    c = size / 2.0 if center is None else center
    return c + (value - 32.0) * scale * size / 64.0


def measure(value: float, size: int, scale: float) -> float:
    return value * scale * size / 64.0


def draw_brand_mark(canvas: Canvas, with_background: bool = True, round_mask: bool = False, scale: float = 1.0) -> None:
    p = brand_palette()

    if with_background:
        draw_background(canvas, scale, round_mask)

    cx_back = transform(25.8, canvas.size, scale)
    cy_back = transform(29.5, canvas.size, scale)
    cx_mid = transform(35.5, canvas.size, scale)
    cy_mid = transform(23.5, canvas.size, scale)
    cx_front = transform(32.0, canvas.size, scale)
    cy_front = transform(29.5, canvas.size, scale)

    draw_rotated_card(
        canvas,
        cx_back,
        cy_back,
        measure(34, canvas.size, scale),
        measure(23, canvas.size, scale),
        measure(7, canvas.size, scale),
        -11,
        p["cool_card_a"],
        p["cool_card_b"],
        0.28,
    )
    draw_rotated_card(
        canvas,
        cx_mid,
        cy_mid,
        measure(33, canvas.size, scale),
        measure(23, canvas.size, scale),
        measure(7, canvas.size, scale),
        8,
        p["warm_card_a"],
        p["warm_card_b"],
        0.36,
    )
    draw_rotated_card(
        canvas,
        transform(33.2, canvas.size, scale),
        transform(31.7, canvas.size, scale),
        measure(36, canvas.size, scale),
        measure(25, canvas.size, scale),
        measure(8, canvas.size, scale),
        -3,
        rgba(0, 0, 0, 255),
        rgba(0, 0, 0, 255),
        0.08,
    )
    draw_rotated_card(
        canvas,
        cx_front,
        cy_front,
        measure(36, canvas.size, scale),
        measure(25, canvas.size, scale),
        measure(8, canvas.size, scale),
        -3,
        p["sheet_a"],
        p["sheet_b"],
        1.0,
        stroke=rgba(21, 50, 78, 34),
        stroke_width=measure(1.2, canvas.size, scale),
    )

    diamond_cx = transform(31.5, canvas.size, scale)
    diamond_cy = transform(27.0, canvas.size, scale)
    diamond_w = measure(15, canvas.size, scale)
    diamond_h = measure(15, canvas.size, scale)
    diamond_r = measure(4, canvas.size, scale)
    diamond_stroke = measure(2.6, canvas.size, scale)

    def diamond_painter(px: float, py: float) -> Color | None:
        sd = abs(sd_round_rect(px, py, diamond_cx, diamond_cy, diamond_w, diamond_h, diamond_r, 45)) - diamond_stroke / 2.0
        cov = coverage(sd, 0.9)
        if cov <= 0.0:
            return None
        return with_alpha(p["ink"], cov)

    pad = diamond_w / 2.0 + 4.0
    canvas.draw_bounds(diamond_cx - pad, diamond_cy - pad, diamond_cx + pad, diamond_cy + pad, diamond_painter)

    draw_circle(canvas, diamond_cx, diamond_cy, measure(6.2, canvas.size, scale), p["orb"], 0.12)
    draw_circle(canvas, diamond_cx, diamond_cy, measure(3.2, canvas.size, scale), p["orb"], 1.0)

    sx = transform(45.0, canvas.size, scale)
    sy = transform(18.0, canvas.size, scale)
    short = measure(4.5, canvas.size, scale)
    long = measure(6.5, canvas.size, scale)
    stroke = measure(2.0, canvas.size, scale)
    for ax, ay, bx, by in (
        (sx, sy - long, sx, sy + long),
        (sx - long, sy, sx + long, sy),
        (sx - short, sy - short, sx + short, sy + short),
        (sx - short, sy + short, sx + short, sy - short),
    ):
        draw_segment(canvas, ax, ay, bx, by, stroke, p["spark"], 0.95)
    draw_circle(canvas, sx, sy, measure(1.5, canvas.size, scale), p["spark"], 1.0)

    if round_mask:
        mask_radius = canvas.size * 0.455
        for y in range(canvas.size):
            py = y + 0.5
            row = canvas.rows[y]
            for x in range(canvas.size):
                px = x + 0.5
                cov = coverage(sd_circle(px, py, canvas.size / 2.0, canvas.size / 2.0, mask_radius), 1.1)
                idx = x * 4
                alpha = row[idx + 3]
                row[idx + 3] = int(round(alpha * cov))


def write_png(path: Path, size: int, with_background: bool = True, round_mask: bool = False, scale: float = 1.0) -> bytes:
    path.parent.mkdir(parents=True, exist_ok=True)
    canvas = Canvas(size)
    draw_brand_mark(canvas, with_background=with_background, round_mask=round_mask, scale=scale)
    png = canvas.to_png()
    path.write_bytes(png)
    return png


def build_ico(path: Path) -> None:
    sizes = [16, 24, 32, 48, 64, 256]
    images = [render_icon_bytes(size) for size in sizes]
    header = struct.pack("<HHH", 0, 1, len(images))
    directory = bytearray()
    offset = 6 + 16 * len(images)
    for size, data in zip(sizes, images):
        width = 0 if size == 256 else size
        height = 0 if size == 256 else size
        directory.extend(struct.pack("<BBBBHHII", width, height, 0, 0, 1, 32, len(data), offset))
        offset += len(data)
    path.write_bytes(header + bytes(directory) + b"".join(images))


def build_icns(path: Path) -> None:
    type_map = {
        16: b"icp4",
        32: b"icp5",
        64: b"icp6",
        128: b"ic07",
        256: b"ic08",
        512: b"ic09",
        1024: b"ic10",
    }
    entries = []
    for size, code in type_map.items():
        data = render_icon_bytes(size)
        entries.append(code + struct.pack(">I", len(data) + 8) + data)
    payload = b"".join(entries)
    path.write_bytes(b"icns" + struct.pack(">I", len(payload) + 8) + payload)


def render_icon_bytes(size: int) -> bytes:
    canvas = Canvas(size)
    draw_brand_mark(canvas, with_background=True, round_mask=False, scale=1.0)
    return canvas.to_png()


def write_svg_sources() -> None:
    BRAND_ASSETS.mkdir(parents=True, exist_ok=True)
    PUBLIC_DIR.mkdir(parents=True, exist_ok=True)

    app_icon_svg = """<svg width="512" height="512" viewBox="0 0 64 64" fill="none" xmlns="http://www.w3.org/2000/svg">
  <defs>
    <linearGradient id="bg" x1="10" y1="8" x2="54" y2="58" gradientUnits="userSpaceOnUse">
      <stop stop-color="#101826" />
      <stop offset="1" stop-color="#1E334A" />
    </linearGradient>
    <radialGradient id="glowWarm" cx="0" cy="0" r="1" gradientUnits="userSpaceOnUse" gradientTransform="translate(19 13) rotate(40) scale(28 24)">
      <stop stop-color="#F5C56B" stop-opacity="0.28" />
      <stop offset="1" stop-color="#F5C56B" stop-opacity="0" />
    </radialGradient>
    <radialGradient id="glowCool" cx="0" cy="0" r="1" gradientUnits="userSpaceOnUse" gradientTransform="translate(50 49) rotate(135) scale(34 28)">
      <stop stop-color="#48B0DB" stop-opacity="0.22" />
      <stop offset="1" stop-color="#48B0DB" stop-opacity="0" />
    </radialGradient>
    <linearGradient id="cool" x1="14" y1="20" x2="44" y2="42" gradientUnits="userSpaceOnUse">
      <stop stop-color="#70E1F7" />
      <stop offset="1" stop-color="#2090C9" />
    </linearGradient>
    <linearGradient id="warm" x1="18" y1="15" x2="51" y2="36" gradientUnits="userSpaceOnUse">
      <stop stop-color="#FFD992" />
      <stop offset="1" stop-color="#F0A63F" />
    </linearGradient>
    <linearGradient id="sheet" x1="16" y1="16" x2="48" y2="42" gradientUnits="userSpaceOnUse">
      <stop stop-color="#FFF7E9" />
      <stop offset="1" stop-color="#F7E4C4" />
    </linearGradient>
  </defs>
  <rect x="3" y="3" width="58" height="58" rx="18" fill="url(#bg)" />
  <rect x="3" y="3" width="58" height="58" rx="18" fill="url(#glowWarm)" />
  <rect x="3" y="3" width="58" height="58" rx="18" fill="url(#glowCool)" />
  <rect x="3.5" y="3.5" width="57" height="57" rx="17.5" stroke="white" stroke-opacity="0.08" />
  <rect x="9" y="18" width="34" height="23" rx="7" transform="rotate(-11 9 18)" fill="url(#cool)" opacity="0.3" />
  <rect x="19" y="12" width="33" height="23" rx="7" transform="rotate(8 19 12)" fill="url(#warm)" opacity="0.4" />
  <rect x="15.2" y="19.2" width="36" height="25" rx="8" transform="rotate(-3 15.2 19.2)" fill="black" fill-opacity="0.08" />
  <rect x="14" y="17" width="36" height="25" rx="8" transform="rotate(-3 14 17)" fill="url(#sheet)" />
  <rect x="14" y="17" width="36" height="25" rx="8" transform="rotate(-3 14 17)" stroke="#15324E" stroke-opacity="0.14" />
  <rect x="24" y="19.5" width="15" height="15" rx="4" transform="rotate(45 24 19.5)" stroke="#163C63" stroke-width="2.6" />
  <circle cx="31.5" cy="27" r="6.2" fill="#F4B24B" fill-opacity="0.12" />
  <circle cx="31.5" cy="27" r="3.2" fill="#F4B24B" />
  <path d="M45 11.5V24.5" stroke="#59D1F2" stroke-width="2" stroke-linecap="round" />
  <path d="M38.5 18H51.5" stroke="#59D1F2" stroke-width="2" stroke-linecap="round" />
  <path d="M40.5 13.5L49.5 22.5" stroke="#59D1F2" stroke-width="2" stroke-linecap="round" />
  <path d="M40.5 22.5L49.5 13.5" stroke="#59D1F2" stroke-width="2" stroke-linecap="round" />
  <circle cx="45" cy="18" r="1.5" fill="#59D1F2" />
</svg>
"""

    mark_svg = """<svg width="256" height="256" viewBox="0 0 64 64" fill="none" xmlns="http://www.w3.org/2000/svg">
  <defs>
    <linearGradient id="cool" x1="14" y1="20" x2="44" y2="42" gradientUnits="userSpaceOnUse">
      <stop stop-color="#70E1F7" />
      <stop offset="1" stop-color="#2090C9" />
    </linearGradient>
    <linearGradient id="warm" x1="18" y1="15" x2="51" y2="36" gradientUnits="userSpaceOnUse">
      <stop stop-color="#FFD992" />
      <stop offset="1" stop-color="#F0A63F" />
    </linearGradient>
    <linearGradient id="sheet" x1="16" y1="16" x2="48" y2="42" gradientUnits="userSpaceOnUse">
      <stop stop-color="#FFF7E9" />
      <stop offset="1" stop-color="#F7E4C4" />
    </linearGradient>
  </defs>
  <rect x="9" y="18" width="34" height="23" rx="7" transform="rotate(-11 9 18)" fill="url(#cool)" opacity="0.3" />
  <rect x="19" y="12" width="33" height="23" rx="7" transform="rotate(8 19 12)" fill="url(#warm)" opacity="0.4" />
  <rect x="14" y="17" width="36" height="25" rx="8" transform="rotate(-3 14 17)" fill="url(#sheet)" />
  <rect x="14" y="17" width="36" height="25" rx="8" transform="rotate(-3 14 17)" stroke="#15324E" stroke-opacity="0.12" />
  <rect x="24" y="19.5" width="15" height="15" rx="4" transform="rotate(45 24 19.5)" stroke="#163C63" stroke-width="2.6" />
  <circle cx="31.5" cy="27" r="3.2" fill="#F4B24B" />
  <path d="M45 11.5V24.5" stroke="#59D1F2" stroke-width="2" stroke-linecap="round" />
  <path d="M38.5 18H51.5" stroke="#59D1F2" stroke-width="2" stroke-linecap="round" />
  <path d="M40.5 13.5L49.5 22.5" stroke="#59D1F2" stroke-width="2" stroke-linecap="round" />
  <path d="M40.5 22.5L49.5 13.5" stroke="#59D1F2" stroke-width="2" stroke-linecap="round" />
  <circle cx="45" cy="18" r="1.5" fill="#59D1F2" />
</svg>
"""

    (BRAND_ASSETS / "huanshu-app-icon.svg").write_text(app_icon_svg, encoding="utf-8")
    (BRAND_ASSETS / "huanshu-mark.svg").write_text(mark_svg, encoding="utf-8")
    (PUBLIC_DIR / "favicon.svg").write_text(app_icon_svg, encoding="utf-8")


def generate_all_assets() -> None:
    write_svg_sources()

    png_specs = {
        TAURI_ICONS / "32x32.png": (32, True, False, 1.0),
        TAURI_ICONS / "64x64.png": (64, True, False, 1.0),
        TAURI_ICONS / "128x128.png": (128, True, False, 1.0),
        TAURI_ICONS / "128x128@2x.png": (256, True, False, 1.0),
        TAURI_ICONS / "icon.png": (512, True, False, 1.0),
        TAURI_ICONS / "StoreLogo.png": (50, True, False, 1.0),
        TAURI_ICONS / "Square30x30Logo.png": (30, True, False, 1.0),
        TAURI_ICONS / "Square44x44Logo.png": (44, True, False, 1.0),
        TAURI_ICONS / "Square71x71Logo.png": (71, True, False, 1.0),
        TAURI_ICONS / "Square89x89Logo.png": (89, True, False, 1.0),
        TAURI_ICONS / "Square107x107Logo.png": (107, True, False, 1.0),
        TAURI_ICONS / "Square142x142Logo.png": (142, True, False, 1.0),
        TAURI_ICONS / "Square150x150Logo.png": (150, True, False, 1.0),
        TAURI_ICONS / "Square284x284Logo.png": (284, True, False, 1.0),
        TAURI_ICONS / "Square310x310Logo.png": (310, True, False, 1.0),
        TAURI_ICONS / "ios" / "AppIcon-20x20@1x.png": (20, True, False, 1.0),
        TAURI_ICONS / "ios" / "AppIcon-20x20@2x.png": (40, True, False, 1.0),
        TAURI_ICONS / "ios" / "AppIcon-20x20@2x-1.png": (40, True, False, 1.0),
        TAURI_ICONS / "ios" / "AppIcon-20x20@3x.png": (60, True, False, 1.0),
        TAURI_ICONS / "ios" / "AppIcon-29x29@1x.png": (29, True, False, 1.0),
        TAURI_ICONS / "ios" / "AppIcon-29x29@2x.png": (58, True, False, 1.0),
        TAURI_ICONS / "ios" / "AppIcon-29x29@2x-1.png": (58, True, False, 1.0),
        TAURI_ICONS / "ios" / "AppIcon-29x29@3x.png": (87, True, False, 1.0),
        TAURI_ICONS / "ios" / "AppIcon-40x40@1x.png": (40, True, False, 1.0),
        TAURI_ICONS / "ios" / "AppIcon-40x40@2x.png": (80, True, False, 1.0),
        TAURI_ICONS / "ios" / "AppIcon-40x40@2x-1.png": (80, True, False, 1.0),
        TAURI_ICONS / "ios" / "AppIcon-40x40@3x.png": (120, True, False, 1.0),
        TAURI_ICONS / "ios" / "AppIcon-60x60@2x.png": (120, True, False, 1.0),
        TAURI_ICONS / "ios" / "AppIcon-60x60@3x.png": (180, True, False, 1.0),
        TAURI_ICONS / "ios" / "AppIcon-76x76@1x.png": (76, True, False, 1.0),
        TAURI_ICONS / "ios" / "AppIcon-76x76@2x.png": (152, True, False, 1.0),
        TAURI_ICONS / "ios" / "AppIcon-83.5x83.5@2x.png": (167, True, False, 1.0),
        TAURI_ICONS / "ios" / "AppIcon-512@2x.png": (1024, True, False, 1.0),
        TAURI_ICONS / "android" / "mipmap-mdpi" / "ic_launcher.png": (48, True, False, 1.0),
        TAURI_ICONS / "android" / "mipmap-hdpi" / "ic_launcher.png": (72, True, False, 1.0),
        TAURI_ICONS / "android" / "mipmap-xhdpi" / "ic_launcher.png": (96, True, False, 1.0),
        TAURI_ICONS / "android" / "mipmap-xxhdpi" / "ic_launcher.png": (144, True, False, 1.0),
        TAURI_ICONS / "android" / "mipmap-xxxhdpi" / "ic_launcher.png": (192, True, False, 1.0),
        TAURI_ICONS / "android" / "mipmap-mdpi" / "ic_launcher_round.png": (48, True, True, 1.0),
        TAURI_ICONS / "android" / "mipmap-hdpi" / "ic_launcher_round.png": (72, True, True, 1.0),
        TAURI_ICONS / "android" / "mipmap-xhdpi" / "ic_launcher_round.png": (96, True, True, 1.0),
        TAURI_ICONS / "android" / "mipmap-xxhdpi" / "ic_launcher_round.png": (144, True, True, 1.0),
        TAURI_ICONS / "android" / "mipmap-xxxhdpi" / "ic_launcher_round.png": (192, True, True, 1.0),
        TAURI_ICONS / "android" / "mipmap-mdpi" / "ic_launcher_foreground.png": (108, False, False, 0.92),
        TAURI_ICONS / "android" / "mipmap-hdpi" / "ic_launcher_foreground.png": (162, False, False, 0.92),
        TAURI_ICONS / "android" / "mipmap-xhdpi" / "ic_launcher_foreground.png": (216, False, False, 0.92),
        TAURI_ICONS / "android" / "mipmap-xxhdpi" / "ic_launcher_foreground.png": (324, False, False, 0.92),
        TAURI_ICONS / "android" / "mipmap-xxxhdpi" / "ic_launcher_foreground.png": (432, False, False, 0.92),
    }

    for path, (size, with_background, round_mask, scale) in png_specs.items():
        write_png(path, size, with_background=with_background, round_mask=round_mask, scale=scale)

    (TAURI_ICONS / "android" / "values" / "ic_launcher_background.xml").write_text(
        """<?xml version="1.0" encoding="utf-8"?>\n<resources>\n  <color name="ic_launcher_background">#101826</color>\n</resources>\n""",
        encoding="utf-8",
    )

    build_ico(TAURI_ICONS / "icon.ico")
    build_icns(TAURI_ICONS / "icon.icns")


if __name__ == "__main__":
    generate_all_assets()
