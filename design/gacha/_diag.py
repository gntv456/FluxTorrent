import asyncio
from playwright.async_api import async_playwright
import pathlib
URL=(pathlib.Path("抽卡页-高保真样图.html")).resolve().as_uri()
async def m():
    async with async_playwright() as p:
        b=await p.chromium.launch(channel="msedge")
        pg=await b.new_page()
        errs=[]
        pg.on("pageerror", lambda e: errs.append(str(e)+"\nSTACK:\n"+ (e.stack or "")))
        await pg.goto(URL); await pg.wait_for_timeout(600)
        print("\n".join(errs))
        await b.close()
asyncio.run(m())
