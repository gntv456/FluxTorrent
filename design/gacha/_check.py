import asyncio, json, os, pathlib
from playwright.async_api import async_playwright

ROOT = pathlib.Path(__file__).parent
URL = (ROOT / "抽卡页-高保真样图.html").as_uri()
OUT = ROOT / "_shots"
OUT.mkdir(exist_ok=True)


async def main():
    async with async_playwright() as p:
        b = await p.chromium.launch(channel="msedge")
        pg = await b.new_page(viewport={"width": 1600, "height": 1000}, device_scale_factor=1)
        errs = []
        pg.on("console", lambda m: errs.append(f"{m.type}: {m.text}") if m.type in ("error", "warning") else None)
        pg.on("pageerror", lambda e: errs.append(f"pageerror: {e}"))
        await pg.goto(URL)
        await pg.wait_for_timeout(900)
        await pg.add_style_tag(content=".review-bar{display:none!important}")

        report = {"console": errs, "frames": {}}

        # 元素溢出检查
        for fid in ["f-desk", "f-tab", "f-fold", "f-mob"]:
            info = await pg.evaluate(
                """(id)=>{
                  const f=document.getElementById(id); if(!f) return {missing:true};
                  const scr=f.querySelector('.screen'); const app=f.querySelector('.app');
                  return {frameW:f.clientWidth, screenW:scr?scr.scrollWidth:0,
                          appW:app?app.clientWidth:0, appScroll:app?app.scrollWidth:0,
                          view:app?app.getAttribute('data-v'):null,
                          screenH:scr?scr.clientHeight:0,
                          appH:app?app.clientHeight:0};
                }""",
                fid,
            )
            report["frames"][fid] = info
            el = await pg.query_selector("#" + fid)
            if el:
                try:
                    await el.screenshot(path=str(OUT / f"{fid}.png"))
                except Exception as e:
                    report["frames"][fid]["shotError"] = str(e)[:120]

        # 逐视图（桌面帧）
        views = ["pool", "reveal", "album", "rates", "records"]
        for v in views:
            await pg.evaluate(
                """(v)=>{
                  const f=document.getElementById('f-desk');
                  const app=f.querySelector('.app'); app.setAttribute('data-v',v);
                  if(v==='reveal' && !app.querySelector('.js-pullgrid').children.length){
                     // 触发演示抽卡：切到同视图后模拟
                  }
                }""",
                v,
            )
            if v == "reveal":
                await pg.evaluate(
                    """window.revSwitch(document.querySelector(".vbtn[data-rv='reveal']"), {})"""
                )
                await pg.wait_for_timeout(80)
                await pg.evaluate("document.querySelectorAll('#f-desk .pull-cell').forEach(c=>c.classList.add('flipped'))")
            await pg.wait_for_timeout(250)
            scr = await pg.query_selector("#f-desk .screen")
            try:
                await scr.screenshot(path=str(OUT / f"view-{v}.png"))
            except Exception as e:
                report.setdefault("shotErr", []).append(f"{v}: {str(e)[:100]}")
            info = await pg.evaluate(
                """()=>{const app=document.querySelector('#f-desk .app');
                  return {appScroll:app.scrollWidth, appW:app.clientWidth, appH:app.clientHeight,
                          cards:app.querySelectorAll('.card').length,
                          album:app.querySelectorAll('.acard').length};}"""
            )
            report.setdefault("views", {})[v] = info

        # 全局横向溢出
        report["bodyOverflow"] = await pg.evaluate(
            "document.documentElement.scrollWidth - document.documentElement.clientWidth"
        )

        print(json.dumps(report, ensure_ascii=False, indent=1))
        await b.close()


asyncio.run(main())
