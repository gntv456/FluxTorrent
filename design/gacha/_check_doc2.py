import pathlib
from playwright.sync_api import sync_playwright

f = pathlib.Path(r"D:\FluxTorrent\design\gacha\抽卡页-设计方案.html").as_uri()
out = pathlib.Path(r"D:\FluxTorrent\design\gacha\_shots")
with sync_playwright() as p:
    b = p.chromium.launch(channel="msedge")
    pg = b.new_page(viewport={"width": 1440, "height": 1000})
    pg.goto(f)
    pg.wait_for_timeout(1200)
    for sid in ["s4", "s5", "s7", "s11"]:
        pg.evaluate("document.querySelector('#%s').scrollIntoView()" % sid)
        pg.wait_for_timeout(500)
        pg.screenshot(path=str(out / ("doc-" + sid + ".png")))
    b.close()
