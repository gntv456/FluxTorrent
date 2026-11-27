import pathlib
from playwright.sync_api import sync_playwright

f = pathlib.Path(r"D:\FluxTorrent\design\gacha\抽卡页-设计方案.html").as_uri()
out = pathlib.Path(r"D:\FluxTorrent\design\gacha\_shots")
with sync_playwright() as p:
    b = p.chromium.launch(channel="msedge")
    pg = b.new_page(viewport={"width": 1440, "height": 1000})
    errs = []
    pg.on("console", lambda m: errs.append(m.text) if m.type == "error" else None)
    pg.on("pageerror", lambda e: errs.append(str(e)))
    pg.goto(f)
    pg.wait_for_timeout(1500)
    info = pg.evaluate("""() => ({
      scrollW: document.documentElement.scrollWidth,
      innerW: window.innerWidth,
      h: document.documentElement.scrollHeight,
      secs: [...document.querySelectorAll('section')].map(s => s.id + ':' + Math.round(s.getBoundingClientRect().height))
    })""")
    print(info, errs)
    pg.screenshot(path=str(out / "doc-full.png"), full_page=True)
    pg.screenshot(path=str(out / "doc-top.png"))
    pg.evaluate("document.querySelector('#s7').scrollIntoView()")
    pg.wait_for_timeout(600)
    pg.screenshot(path=str(out / "doc-s7.png"))
    b.close()
