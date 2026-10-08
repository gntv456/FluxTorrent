export default async (h) => {
  await h.navigate('http://localhost:3000/');
  const r = await h.evalPage(`(() => {
    // 找出 scrollWidth-clientWidth=6 的 div.flex 是谁
    const hits = [];
    for (const el of document.querySelectorAll('body *')) {
      if (el.scrollWidth - el.clientWidth === 6 && el.clientWidth > 0) {
        const chain = [];
        let p = el;
        for (let i = 0; i < 4 && p && p !== document.body; i++) {
          chain.push(p.tagName.toLowerCase() + (typeof p.className === 'string' && p.className ? '.' + p.className.split(' ')[0] : ''));
          p = p.parentElement;
        }
        hits.push({ cls: String(el.className).slice(0, 60), html: el.outerHTML.slice(0, 120), chain: chain.join(' < ') });
        if (hits.length >= 3) break;
      }
    }
    return hits;
  })()`);
  console.log(JSON.stringify(r, null, 1));

  // shop 的 10.5px 魔力标签
  await h.navigate('http://localhost:3000/shop');
  const r2 = await h.evalPage(`(() => {
    const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
    let n; const hits = [];
    while ((n = walker.nextNode()) && hits.length < 3) {
      const t = n.textContent.trim();
      if (t === '魔力') {
        const el = n.parentElement;
        const fs = getComputedStyle(el).fontSize;
        if (parseFloat(fs) < 11.5) hits.push({ cls: String(el.className).slice(0, 50), fs, pcls: String(el.parentElement.className).slice(0, 40) });
      }
    }
    return hits;
  })()`);
  console.log(JSON.stringify(r2));
};
