import asyncio, json, pathlib
from playwright.async_api import async_playwright

ROOT = pathlib.Path(__file__).parent
URL = (ROOT / "抽卡页-高保真样图.html").as_uri()
OUT = ROOT / "_shots"
OUT.mkdir(exist_ok=True)

CASES = [
    ("rules", "#f-desk [data-modal='rules']", None),
    ("wallet", "#f-desk [data-modal='wallet']", None),
    ("progress", "#f-mob .icon-btn.only-narrow", None),
    ("rateSpec", "#f-desk [data-modal='rateSpec']", "rates"),
    ("export", "#f-desk [data-modal='export']", "records"),
    ("confirm", "#f-desk .v-album .acard .mini-btn.gold", "album"),
    ("card", "#f-desk .v-album .acard .card", "album"),
]


async def main():
    async with async_playwright() as p:
        b = await p.chromium.launch(channel="msedge")
        pg = await b.new_page(viewport={"width": 1600, "height": 1000})
        errs = []
        pg.on("console", lambda m: errs.append(f"{m.type}: {m.text}") if m.type == "error" else None)
        pg.on("pageerror", lambda e: errs.append(f"pageerror: {e}"))
        await pg.goto(URL)
        await pg.wait_for_timeout(700)
        await pg.add_style_tag(content=".review-bar{display:none!important}")
        rep = {"console": [], "cases": {}}
        for kind, sel, view in CASES:
            if view:
                await pg.evaluate(
                    """(v)=>{const app=document.querySelector('#f-desk .app'); app.setAttribute('data-v',v);}""",
                    view,
                )
                if view == "rates":
                    pass
                await pg.wait_for_timeout(200)
            try:
                el = await pg.query_selector(sel)
                if not el:
                    rep["cases"][kind] = "selector missing"
                    continue
                await el.click(force=True)
                await pg.wait_for_timeout(220)
                frame = sel.split(" ")[0].lstrip("#")
                shot = await pg.query_selector(f"#{frame} .modal")
                opened = await pg.evaluate(
                    """(k)=>{const app=document.querySelector(`#${k} .app`);
                       const m=app.querySelector('.modal');
                       return {open:m.classList.contains('on'),
                               title:m.querySelector('.m-title')?.textContent.trim(),
                               h:m.querySelector('.modal-card')?.offsetHeight};}""",
                    frame,
                )
                rep["cases"][kind] = opened
                scr = await pg.query_selector(f"#{frame} .screen")
                await scr.screenshot(path=str(OUT / f"modal-{kind}.png"))
                await pg.evaluate("""()=>{document.querySelectorAll('.app .modal').forEach(m=>{
                    m.classList.remove('on');});}""")
                await pg.wait_for_timeout(120)
            except Exception as e:
                rep["cases"][kind] = "ERR " + str(e)[:90]
        rep["console"] = errs
        print(json.dumps(rep, ensure_ascii=False, indent=1))
        await b.close()

asyncio.run(main())
