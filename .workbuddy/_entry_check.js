// 验证 /admin 面板页出现「站点设定」入口卡，且 /admin/settings 可达
const fs = require('fs');
const lhPath = 'C:/Users/52sy/.workbuddy/binaries/node/workspace/node_modules/';
const chromeLauncher = require(lhPath + 'chrome-launcher');

const token = fs.readFileSync('D:/FluxTorrent/_p2tok.txt', 'utf8').trim();
const origin = 'http://127.0.0.1:3000';

function cdp(wsUrl, steps) {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(wsUrl);
    let id = 0;
    const pending = new Map();
    const send = (method, params) =>
      new Promise((res) => {
        const mid = ++id;
        pending.set(mid, res);
        ws.send(JSON.stringify({ id: mid, method, params: params || {} }));
      });
    ws.onmessage = (ev) => {
      const msg = JSON.parse(ev.data);
      if (msg.id && pending.has(msg.id)) {
        pending.get(msg.id)(msg.result);
        pending.delete(msg.id);
      }
    };
    ws.onerror = (e) => reject(new Error('ws error ' + e.message));
    ws.onopen = async () => {
      try {
        const r = await steps(send);
        ws.close();
        resolve(r);
      } catch (e) {
        try { ws.close(); } catch {}
        reject(e);
      }
    };
  });
}

async function check(send, width, height) {
  await send('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: 1, mobile: width < 800 });
  await send('Page.enable');
  await send('Page.navigate', { url: origin + '/admin' });
  await new Promise((r) => setTimeout(r, 3500));
  const adminEval = await send('Runtime.evaluate', {
    expression: `(() => {
      const card = document.querySelector('a[href="/admin/settings"]');
      const tabs = [...document.querySelectorAll('[role="tab"]')].map(b => b.textContent.trim());
      return JSON.stringify({
        title: document.title,
        tabs,
        hasEntry: !!card,
        entryText: card ? card.textContent.trim().slice(0, 80) : null,
      });
    })()`,
    returnByValue: true,
  });
  const admin = JSON.parse(adminEval.result.value);

  // 点击入口卡进入新页面
  await send('Runtime.evaluate', { expression: `document.querySelector('a[href="/admin/settings"]').click()` });
  await new Promise((r) => setTimeout(r, 4000));
  const settingsEval = await send('Runtime.evaluate', {
    expression: `(() => JSON.stringify({
      url: location.pathname,
      h1: (document.querySelector('h1')||{}).textContent || '',
      navBtns: [...document.querySelectorAll('button')].length,
    }))()`,
    returnByValue: true,
  });
  const settings = JSON.parse(settingsEval.result.value);
  return { admin, settings };
}

(async () => {
  const chrome = await chromeLauncher.launch({
    chromePath: 'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe',
    chromeFlags: ['--headless=new', '--no-sandbox', '--disable-gpu', '--disable-dev-shm-usage'],
  });
  const port = chrome.port;
  const out = {};
  for (const [form, w, h] of [['desktop', 1440, 900], ['mobile', 390, 844]]) {
    const tabRes = await fetch(`http://127.0.0.1:${port}/json/new?${encodeURIComponent(origin + '/')}`, { method: 'PUT' });
    const tab = await tabRes.json();
    const res = await cdp(tab.webSocketDebuggerUrl, async (send) => {
      await send('Runtime.enable');
      await send('Network.enable');
      await send('Page.enable');
      // 等首页加载完成，确保 evaluate 跑在目标 origin 上（否则 localStorage 落到 about:blank）
      await new Promise((r) => setTimeout(r, 1200));
      for (const [name, value] of [['flux.session', '1'], ['flux.token', token]]) {
        await send('Network.setCookie', { name, value, domain: '127.0.0.1', path: '/', url: origin });
      }
      const seed = await send('Runtime.evaluate', {
        expression: `(() => { localStorage.setItem('flux.token', ${JSON.stringify(token)}); return localStorage.getItem('flux.token') ? 'seeded-ok' : 'seed-fail'; })()`,
        returnByValue: true,
      });
      if (seed.result.value !== 'seeded-ok') throw new Error('localStorage seed failed: ' + JSON.stringify(seed));
      return check(send, w, h);
    });
    await fetch(`http://127.0.0.1:${port}/json/close/${tab.id}`);
    out[form] = res;
  }
  await chrome.kill();
  fs.writeFileSync('D:/FluxTorrent/.workbuddy/_entry_check.txt', JSON.stringify(out, null, 2), 'utf8');
  console.log('OK');
})().catch((e) => {
  fs.writeFileSync('D:/FluxTorrent/.workbuddy/_entry_check.txt', 'ERROR: ' + (e && e.stack ? e.stack : String(e)), 'utf8');
  process.exit(1);
});
