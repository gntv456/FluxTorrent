# -*- coding: utf-8 -*-
"""Export v4 aurora icons: multi-size PNG, ICO, SVG wrapper."""
import os, base64, io
from pathlib import Path
from PIL import Image

ROOT = Path(r"D:\FluxTorrent\design\icons\v4-aurora")
SRC = ROOT / "src"
PNG = ROOT / "png"
SVG = ROOT / "svg"
ICO = ROOT / "ico"

PNG_SIZES = [512, 256, 128, 64]
ICO_SIZES = [16, 32, 48, 64, 128]

ICONS = [
    ("cat-default",     "默认",   "流光种子"),
    ("cat-ebook",       "电子书", "打开的书"),
    ("cat-comic",       "漫画",   "对话气泡"),
    ("cat-movie",       "电影",   "场记板"),
    ("cat-documentary", "纪录片", "地球定位"),
    ("cat-anime",       "动漫",   "闪光星星"),
    ("cat-edu",         "教育",   "学士帽"),
    ("cat-audiobook",   "有声书", "头戴耳机"),
    ("cat-software",    "软件",   "代码括号"),
    ("cat-sports",      "体育",   "足球"),
    ("cat-game",        "游戏",   "游戏手柄"),
    ("cat-music",       "音乐",   "八分音符"),
    ("cat-tv",          "剧集",   "电视播放"),
    ("cat-shortdrama",  "短剧",   "快进胶片"),
    ("cat-variety",     "综艺",   "派对面具"),
    ("cat-other",       "其他",   "三点"),
]

def main():
    for key, zh, sym in ICONS:
        img = Image.open(SRC / f"{key}.png").convert("RGBA")
        w, h = img.size
        s = min(w, h)
        img = img.crop(((w-s)//2, (h-s)//2, (w+s)//2, (h+s)//2))

        for sz in PNG_SIZES:
            img.resize((sz, sz), Image.LANCZOS).save(PNG / f"{key}-{sz}.png", "PNG", optimize=True)
        img.resize((256, 256), Image.LANCZOS).save(PNG / f"{key}.png", "PNG", optimize=True)

        img.save(ICO / f"{key}.ico", format="ICO", sizes=[(sz, sz) for sz in ICO_SIZES])

        buf = io.BytesIO()
        img.resize((256, 256), Image.LANCZOS).save(buf, "PNG", optimize=True)
        b64 = base64.b64encode(buf.getvalue()).decode("ascii")
        svg = (
            f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" '
            f'width="256" height="256" role="img" aria-label="{zh}">\n'
            f'  <image href="data:image/png;base64,{b64}" x="0" y="0" width="256" height="256"/>\n'
            f'</svg>\n'
        )
        (SVG / f"{key}.svg").write_text(svg, encoding="utf-8")
        print(f"OK {key}")

if __name__ == "__main__":
    main()
