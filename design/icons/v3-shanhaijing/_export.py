# -*- coding: utf-8 -*-
"""Export v3 shanhaijing icons: multi-size PNG, multi-resolution ICO, SVG wrapper."""
import os, base64, io
from pathlib import Path
from PIL import Image

ROOT = Path(r"D:\FluxTorrent\design\icons\v3-shanhaijing")
SRC = ROOT / "src"
PNG = ROOT / "png"
SVG = ROOT / "svg"
ICO = ROOT / "ico"

PNG_SIZES = [512, 256, 128, 64]
ICO_SIZES = [16, 32, 48, 64, 128]

ICONS = [
    ("cat-default",     "默认",   "昆仑玄珠"),
    ("cat-ebook",       "电子书", "上古玉简"),
    ("cat-comic",       "漫画",   "重明鸟"),
    ("cat-movie",       "电影",   "烛龙睁眼"),
    ("cat-documentary", "纪录片", "白泽知万物"),
    ("cat-anime",       "动漫",   "九尾狐幻化"),
    ("cat-edu",         "教育",   "文昌笔"),
    ("cat-audiobook",   "有声书", "夔牛雷音"),
    ("cat-software",    "软件",   "鲁班木鸢"),
    ("cat-sports",      "体育",   "夸父追日"),
    ("cat-game",        "游戏",   "六博棋"),
    ("cat-music",       "音乐",   "凤凰钟磬"),
    ("cat-tv",          "剧集",   "螣蛇乘雾"),
    ("cat-shortdrama",  "短剧",   "精卫衔石"),
    ("cat-variety",     "综艺",   "天吴八首"),
    ("cat-other",       "其他",   "帝江混沌"),
]

def main():
    for key, zh, sym in ICONS:
        src_path = SRC / f"{key}.png"
        img = Image.open(src_path).convert("RGBA")
        # square-crop just in case
        w, h = img.size
        s = min(w, h)
        img = img.crop(((w-s)//2, (h-s)//2, (w+s)//2, (h+s)//2))

        # PNG multi-size
        for sz in PNG_SIZES:
            out = PNG / f"{key}-{sz}.png"
            img.resize((sz, sz), Image.LANCZOS).save(out, "PNG", optimize=True)
        # also a single @256 canonical
        img.resize((256, 256), Image.LANCZOS).save(PNG / f"{key}.png", "PNG", optimize=True)

        # ICO multi-resolution: let Pillow generate all sizes from source
        img.save(
            ICO / f"{key}.ico",
            format="ICO",
            sizes=[(sz, sz) for sz in ICO_SIZES],
        )

        # SVG wrapper: embed 256px PNG as base64, with viewBox
        buf = io.BytesIO()
        img.resize((256, 256), Image.LANCZOS).save(buf, "PNG", optimize=True)
        b64 = base64.b64encode(buf.getvalue()).decode("ascii")
        svg = (
            f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" '
            f'width="256" height="256" role="img" aria-label="{zh}">{sym}">\n'
            f'  <image href="data:image/png;base64,{b64}" x="0" y="0" width="256" height="256"/>\n'
            f'</svg>\n'
        )
        (SVG / f"{key}.svg").write_text(svg, encoding="utf-8")

        print(f"OK {key} ({zh} / {sym})")

if __name__ == "__main__":
    main()
