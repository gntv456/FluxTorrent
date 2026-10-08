export default async (h) => {
  // 在页面上下文里用 fetch 登录: Set-Cookie(HttpOnly) 由浏览器自动落盘, 顺序敏感已排除
  const res = await h.evalPage(`(async () => {
    const r = await fetch('/api/v1/auth/login', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ username: 'root', password: 'password123' }),
      redirect: 'manual',
    });
    const j = await r.json().catch(() => null);
    return { status: r.status, code: j && j.code, hasToken: !!(j && j.data && j.data.token) };
  })()`);
  console.log('login in page ctx:', JSON.stringify(res));

  // 会话标记 cookie(前端约定) + 语言
  await h.evalPage(`(() => {
    document.cookie = 'flux.session=1; path=/; max-age=86400';
    document.cookie = 'flux.locale=zh-CN; path=/; max-age=31536000';
  })()`);

  await h.navigate('http://localhost:3000/torrents');
  const probe = await h.evalPage(`(() => ({
    url: location.href,
    usermenu: !!document.querySelector('.usermenu,[class*=usermenu]'),
    bodyHead: document.body.innerText.slice(0, 160),
  }))()`);
  console.log(JSON.stringify(probe, null, 1));
  await h.shot(new URL('./shots/00-torrents.png', import.meta.url).pathname
      .replace(/^\/([A-Za-z]:)/, '$1'));
};
