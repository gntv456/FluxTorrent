// 模拟器全站巡检: 逐页导航 + DOM 审计 + adb 截图(由外层 bash 做)
// 输出 JSON 行: 每页 { url, title, vw, sw, overflowX, widest, tapTargets, tinyText, hScrollEls }
export default async (h) => {
  const routes = process.env.MROUTES
    ? JSON.parse(process.env.MROUTES)
    : [
        ['home', '/'],
        ['torrents', '/torrents'],
        ['torrent-detail', '/torrent/1'],
        ['forums', '/forums'],
        ['forum-board', '/forums'],
        ['shop', '/shop'],
        ['games', '/games'],
        ['top', '/top'],
        ['users', '/users'],
        ['messages', '/messages'],
        ['notifications', '/notifications'],
        ['usercp', '/usercp'],
        ['upload', '/upload'],
        ['offers', '/offers'],
        ['faq', '/faq'],
        ['rules', '/rules'],
        ['login-page', '/login'],
      ];

  const loginFirst = process.env.MLOGIN !== '0';
  if (loginFirst) {
    const r = await h.evalPage(`(async () => {
      const r = await fetch('/api/v1/auth/login', {
        method: 'POST', headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ username: 'root', password: 'password123' }), redirect: 'manual',
      });
      const j = await r.json().catch(() => null);
      document.cookie = 'flux.session=1; path=/; max-age=86400';
      document.cookie = 'flux.locale=zh-CN; path=/; max-age=31536000';
      return r.status;
    })()`);
    console.error('login:', r);
  }

  for (const [name, path] of routes) {
    try {
      await h.navigate('http://localhost:3000' + path);
      const audit = await h.evalPage(`(() => {
        const de = document.documentElement;
        const vw = de.clientWidth;
        const out = {
          url: location.pathname + location.search,
          title: document.title.slice(0, 40),
          vw, sw: de.scrollWidth, sh: de.scrollHeight,
          overflowX: de.scrollWidth - vw,
          widest: null, hScrollEls: [], tinyText: [], smallTap: [], bodyHead: '',
        };
        let wMax = 0;
        for (const el of document.querySelectorAll('body *')) {
          const r = el.getBoundingClientRect();
          const cs = getComputedStyle(el);
          if (cs.display === 'none' || cs.visibility === 'hidden') continue;
          if (r.width > wMax) { wMax = r.width; out.widest = el.tagName.toLowerCase() + '.' + String(el.className).split(' ')[0].slice(0, 40) + '@' + Math.round(r.width); }
          // 自身可横滚且内容超出的元素(合理的表格滚动容器除外)
          if (el.scrollWidth - el.clientWidth > 4 && el.clientWidth > 0) {
            const cls = String(el.className);
            if (!/wide-table|scroll|hscroll|feed|marquee|swipe|carousel|ticker/i.test(cls) && el.tagName !== 'HTML' && out.hScrollEls.length < 6) {
              out.hScrollEls.push(el.tagName.toLowerCase() + '.' + cls.split(' ')[0].slice(0, 36) + '+' + (el.scrollWidth - el.clientWidth));
            }
          }
        }
        // 小于 12px 的文本节点(移动端可读性)
        const tw = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
        let n, seen = 0;
        while ((n = tw.nextNode()) && seen < 4000) {
          seen++;
          const t = n.textContent.trim();
          if (t.length < 2) continue;
          const el = n.parentElement;
          if (!el) continue;
          const fs = parseFloat(getComputedStyle(el).fontSize);
          if (fs < 12 && out.tinyText.length < 8) out.tinyText.push(t.slice(0, 14) + '@' + fs + 'px');
        }
        // 可点元素命中区 < 32px(盲区检查, 只抽 a/button/[role=button])
        for (const el of document.querySelectorAll('a,button,[role=button]')) {
          const r = el.getBoundingClientRect();
          if (r.width > 0 && r.height > 0 && (r.height < 26 || r.width < 26) && out.smallTap.length < 8) {
            out.smallTap.push((el.textContent || el.getAttribute('aria-label') || '').trim().slice(0, 10) + '@' + Math.round(r.width) + 'x' + Math.round(r.height));
          }
        }
        out.bodyHead = document.body.innerText.slice(0, 120).replace(/[\\n]/g, '|');
        return out;
      })()`);
      console.log(JSON.stringify({ name, ...audit }));
    } catch (e) {
      console.log(JSON.stringify({ name, path, error: String(e).slice(0, 200) }));
    }
  }
};
