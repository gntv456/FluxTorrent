export default async (h) => {
  // top 页 11px 表格字在哪个组件
  await h.navigate('http://localhost:3000/top');
  const r = await h.evalPage(`(() => {
    const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
    let n, hit = null;
    while ((n = walker.nextNode())) {
      const t = n.textContent.trim();
      if (t === '排名' || t === '魔力') {
        const el = n.parentElement;
        hit = { txt: t, tag: el.tagName, cls: String(el.className).slice(0, 40), fs: getComputedStyle(el).fontSize,
          table: el.closest('table') ? String(el.closest('table').className).slice(0, 50) : null,
          wrap: el.closest('[class*=wide-table],[class*=scroll]') ? String(el.closest('[class*=wide-table],[class*=scroll]').className).slice(0, 30) : null };
        break;
      }
    }
    return hit;
  })()`);
  console.log(JSON.stringify(r));
};
