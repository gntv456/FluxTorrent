import asyncio, json, pathlib
from playwright.async_api import async_playwright
URL=(pathlib.Path("抽卡页-高保真样图.html")).resolve().as_uri()
async def m():
    async with async_playwright() as p:
        b=await p.chromium.launch(channel="msedge")
        pg=await b.new_page(viewport={"width":1600,"height":1000})
        await pg.goto(URL); await pg.wait_for_timeout(700)
        info=await pg.evaluate("""()=>{
          const app=document.querySelector('#f-desk .app'); app.setAttribute('data-v','album');
          const scr=document.querySelector('#f-desk .screen');
          const view=app.querySelector('.v-album'); const grid=app.querySelector('.alb-grid');
          return {screenH:scr.getBoundingClientRect().height, appH:app.getBoundingClientRect().height,
                  viewH:view.getBoundingClientRect().height, gridH:grid.getBoundingClientRect().height,
                  screenOverflow:getComputedStyle(scr).overflowY, appOverflow:getComputedStyle(app).overflowY,
                  viewOverflow:getComputedStyle(view).overflowY, gridRows:grid.children.length,
                  docH:document.querySelector('.browser').getBoundingClientRect().height};
        }""")
        print(json.dumps(info,indent=1))
        await b.close()
asyncio.run(m())
