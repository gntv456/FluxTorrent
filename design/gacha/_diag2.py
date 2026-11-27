import asyncio, json, pathlib
from playwright.async_api import async_playwright
URL=(pathlib.Path("抽卡页-高保真样图.html")).resolve().as_uri()
async def m():
    async with async_playwright() as p:
        b=await p.chromium.launch(channel="msedge")
        pg=await b.new_page(viewport={"width":1600,"height":1000})
        errs=[]
        pg.on("pageerror", lambda e: errs.append(str(e)))
        await pg.goto(URL); await pg.wait_for_timeout(700)
        await pg.evaluate("document.querySelector('#f-desk .btn-gold').click()")
        await pg.wait_for_timeout(1600)
        info=await pg.evaluate("""()=>{
          const app=document.querySelector('#f-desk .app');
          const g=app.querySelector('.js-pullgrid');
          const cell=g.querySelector('.pull-cell');
          const cs=cell?getComputedStyle(cell):null;
          const inner=cell?cell.querySelector('.pull-inner'):null;
          return {v:app.getAttribute('data-v'), cells:g.children.length, gH:g.getBoundingClientRect().height,
                  cellH:cell?cell.getBoundingClientRect().height:0, cellW:cell?cell.getBoundingClientRect().width:0,
                  innerH:inner?inner.getBoundingClientRect().height:0,
                  revealH:app.querySelector('.v-reveal').getBoundingClientRect().height,
                  appH:app.getBoundingClientRect().height,
                  html:g.innerHTML.slice(0,120)};
        }""")
        print(json.dumps(info,ensure_ascii=False,indent=1)); print(errs)
        await b.close()
asyncio.run(m())
