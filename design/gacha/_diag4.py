import json, pathlib
from playwright.sync_api import sync_playwright

f = pathlib.Path(r"D:\FluxTorrent\design\gacha\抽卡页-高保真样图.html").as_uri()
with sync_playwright() as p:
    b = p.chromium.launch(channel="msedge")
    pg = b.new_page(viewport={"width": 1600, "height": 1000})
    pg.goto(f)
    pg.wait_for_timeout(1200)
    pg.evaluate("""() => {
      const app = document.querySelector('#f-desk .screen .app');
      window.revSwitch(document.querySelector('.vbtn[data-rv="reveal"]'), new Event('click'));
      return 1;
    }""")
    pg.wait_for_timeout(1500)
    info = pg.evaluate("""() => {
      const app = document.querySelector('#f-desk .screen .app');
      const q = s => app.querySelector(s);
      const box = el => el ? (r => ({w: Math.round(r.width), h: Math.round(r.height), x: Math.round(r.x), y: Math.round(r.y)}))(el.getBoundingClientRect()) : null;
      return {
        view: app.dataset.v,
        wrap: box(q('.pull-wrap')),
        grid: box(q('.js-pullgrid')),
        cells: app.querySelectorAll('.pull-cell').length,
        inner: box(q('.pull-inner')),
        face: box(q('.face')),
        html: (q('.js-pullgrid')||{}).innerHTML?.length,
        cellHTML: (q('.pull-cell')||{}).outerHTML?.slice(0,120),
        wrapStyle: q('.pull-wrap') ? getComputedStyle(q('.pull-wrap')).display + '|' + getComputedStyle(q('.pull-wrap')).flexDirection : null,
      };
    }""")
    print(json.dumps(info, ensure_ascii=False, indent=1))
    b.close()
