# -*- coding: utf-8 -*-
"""Export v3.1 props icons: multi-size PNG, ICO, SVG wrapper."""
import base64, io
from pathlib import Path
from PIL import Image

ROOT = Path(r"D:\FluxTorrent\design\icons\v3-shanhaijing\v3.1-props")
SRC, PNG, SVG, ICO = ROOT/"src", ROOT/"png", ROOT/"svg", ROOT/"ico"
PNG_SIZES = [512, 256, 128, 64]
ICO_SIZES = [16, 32, 48, 64, 128]

ICONS = [
    ("cat-default","默认","昆仑古镜"),
    ("cat-ebook","电子书","竹简玉简"),
    ("cat-comic","漫画","横卷画轴"),
    ("cat-movie","电影","场记板"),
    ("cat-documentary","纪录片","三山五岳"),
    ("cat-anime","动漫","四格漫画"),
    ("cat-edu","教育","毛笔"),
    ("cat-audiobook","有声书","青铜铃铛"),
    ("cat-software","软件","青铜齿轮"),
    ("cat-sports","体育","古代蹴鞠"),
    ("cat-game","游戏","六博棋盘"),
    ("cat-music","音乐","青铜编钟"),
    ("cat-tv","剧集","屏幕播放"),
    ("cat-shortdrama","短剧","快进箭头"),
    ("cat-variety","综艺","悲喜面具"),
    ("cat-other","其他","三金圆点"),
]

for key, zh, sym in ICONS:
    img = Image.open(SRC/f"{key}.png").convert("RGBA")
    w,h = img.size; s = min(w,h)
    img = img.crop(((w-s)//2,(h-s)//2,(w+s)//2,(h+s)//2))
    for sz in PNG_SIZES:
        img.resize((sz,sz), Image.LANCZOS).save(PNG/f"{key}-{sz}.png","PNG",optimize=True)
    img.resize((256,256), Image.LANCZOS).save(PNG/f"{key}.png","PNG",optimize=True)
    img.save(ICO/f"{key}.ico", format="ICO", sizes=[(sz,sz) for sz in ICO_SIZES])
    buf = io.BytesIO()
    img.resize((256,256), Image.LANCZOS).save(buf,"PNG",optimize=True)
    b64 = base64.b64encode(buf.getvalue()).decode()
    (SVG/f"{key}.svg").write_text(
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" width="256" height="256" role="img" aria-label="{zh}">\n  <image href="data:image/png;base64,{b64}" x="0" y="0" width="256" height="256"/>\n</svg>\n',
        encoding="utf-8")
    print(f"OK {key}")
