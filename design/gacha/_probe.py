import json
from playwright.sync_api import sync_playwright

URL = "file:///D:/FluxTorrent/design/gacha/%E6%8A%BD%E5%8D%A1%E9%A1%B5-%E9%AB%98%E4%BF%9D%E7%9C%9F%E6%A0%B7%E5%9B%BE.html"

with sync_playwright() as p:
    b = p.chromium.launch(channel="msedge")
    pg = b.new_page(viewport={"width": 1440, "height": 1000})
    pg.goto(URL)
    pg.wait_for_timeout(500)
    data = pg.evaluate("""() => {
      const out = {};
      ['1440','1024','680','390'].forEach(w => {
        const sc = document.querySelector('.screen[data-w="'+w+'"]');
        const d = sc.querySelector('.dotmatrix');
        const r = d.getBoundingClientRect();
        out[w] = {w: Math.round(r.width), cols: getComputedStyle(d).gridTemplateColumns.split(' ').length, screen: Math.round(sc.getBoundingClientRect().width)};
      });
      return out;
    }""")
    print(json.dumps(data, ensure_ascii=False))
    b.close()
