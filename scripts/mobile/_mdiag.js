export default async (h) => {
  // 登录态修复 + 深挖 /torrents 横向溢出根因
  const st = await h.evalPage(`(async () => {
    const r = await fetch('/api/v1/auth/login', {
      method: 'POST', headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ username: 'root', password: 'password123' }), redirect: 'manual',
    });
    document.cookie = 'flux.session=1; path=/; max-age=86400';
    return r.status;
  })()`);
  console.error('relogin:', st);

  await h.navigate('http://localhost:3000/torrents');
  const info = await h.evalPage(`(() => {
    const vw = document.documentElement.clientWidth;
    const bad = [];
    // 找出所有右缘超出视口的元素及其父链
    for (const el of document.querySelectorAll('body *')) {
      const r = el.getBoundingClientRect();
      if (r.right > vw + 2 && r.width > 30) {
        const chain = [];
        let p = el;
        for (let i = 0; i < 4 && p && p !== document.body; i++) {
          chain.push(p.tagName.toLowerCase() + (p.className && typeof p.className === 'string' ? '.' + p.className.split(' ').slice(0,2).join('.') : ''));
          p = p.parentElement;
        }
        bad.push({ tag: el.tagName.toLowerCase(), cls: String(el.className).slice(0, 60), w: Math.round(r.width), right: Math.round(r.right), chain: chain.join(' < ') });
      }
    }
    bad.sort((a, b) => b.w - a.w);
    return { vw, count: bad.length, top: bad.slice(0, 10) };
  })()`);
  console.log(JSON.stringify(info, null, 1));
};
