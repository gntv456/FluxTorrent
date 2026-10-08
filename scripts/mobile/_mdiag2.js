export default async (h) => {
  // 逐元素审查 torrents 页: 5 个 tablink 是什么/为何 nav 525px 宽, 以及 overflow-x 谁引入
  const info = await h.evalPage(`(() => {
    const nav = document.querySelector('nav.fixed');
    const tabs = nav ? [...nav.querySelectorAll('a,button')].map(a => {
      const r = a.getBoundingClientRect();
      return { t: (a.textContent || '').trim().slice(0, 10), w: Math.round(r.width), right: Math.round(r.right), cls: String(a.className).slice(0, 40) };
    }) : [];
    const navInfo = nav ? { w: nav.getBoundingClientRect().width, sw: nav.scrollWidth, style: nav.getAttribute('class') } : null;
    // 溢出链精确化: 从 documentElement 往下找 scrollWidth > clientWidth 的容器
    const chain = [];
    const walk = (el, depth) => {
      if (depth > 8) return;
      if (el.scrollWidth > el.clientWidth + 2) {
        chain.push({ tag: el.tagName.toLowerCase(), cls: String(el.className).slice(0, 50), sw: el.scrollWidth, cw: el.clientWidth });
      }
      for (const c of el.children) walk(c, depth + 1);
    };
    walk(document.body, 0);
    return { navInfo, tabs: tabs.slice(0, 8), chain: chain.slice(0, 12), bodyScroll: { sw: document.body.scrollWidth, cw: document.body.clientWidth } };
  })()`);
  console.log(JSON.stringify(info, null, 1));
};
