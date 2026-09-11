const fs = require('fs');
const lhPath = 'C:/Users/52sy/.workbuddy/binaries/node/workspace/node_modules/';
const lhMod = require(lhPath + 'lighthouse');
const lighthouse = lhMod.default || lhMod;
const chromeLauncher = require(lhPath + 'chrome-launcher');

const token = fs.readFileSync('D:/FluxTorrent/_p2tok.txt', 'utf8').trim();
const origin = 'http://127.0.0.1:3000';
const url = origin + '/admin/settings';
const form = process.argv[2] === 'mobile' ? 'mobile' : 'desktop';

/** 用内置 WebSocket 做最小 CDP 调用：先在同源页面写入 localStorage 令牌，
 *  再跑 Lighthouse（同一 Chrome 实例的新标签页共享 localStorage），
 *  以消除"仅有 cookie、无 localStorage"造成的 401 噪声。 */
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

(async () => {
  const chrome = await chromeLauncher.launch({
    chromePath: 'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe',
    chromeFlags: ['--headless=new', '--no-sandbox', '--disable-gpu', '--disable-dev-shm-usage'],
  });
  const port = chrome.port;

  // ---- seed localStorage on the origin ----
  const tabRes = await fetch(`http://127.0.0.1:${port}/json/new?${encodeURIComponent(origin + '/')}`, {
    method: 'PUT',
  });
  const tab = await tabRes.json();
  await cdp(tab.webSocketDebuggerUrl, async (send) => {
    await send('Runtime.enable');
    await send('Network.enable');
    for (const [name, value] of [
      ['flux.session', '1'],
      ['flux.token', token],
    ]) {
      await send('Network.setCookie', { name, value, domain: '127.0.0.1', path: '/', url: origin });
    }
    await new Promise((r) => setTimeout(r, 800));
    await send('Runtime.evaluate', {
      expression: `localStorage.setItem('flux.token', ${JSON.stringify(token)}); 'seeded'`,
      returnByValue: true,
    });
  });
  await fetch(`http://127.0.0.1:${port}/json/close/${tab.id}`);

  const options = {
    logLevel: 'error',
    output: ['json', 'html'],
    port,
    onlyCategories: ['performance', 'accessibility', 'best-practices'],
    extraHeaders: { Cookie: `flux.session=1; flux.token=${token}` },
    formFactor: form,
    ...(process.env.LH_THROTTLE === 'devtools' ? { throttlingMethod: 'devtools' } : {}),
    screenEmulation:
      form === 'mobile'
        ? { mobile: true, width: 390, height: 844, deviceScaleFactor: 3, disabled: false }
        : { mobile: false, width: 1440, height: 900, deviceScaleFactor: 1, disabled: false },
  };
  const rr = await lighthouse(url, options);
  const lhr = rr.lhr;
  const reports = Array.isArray(rr.report) ? rr.report : [rr.report];
  // output 顺序为 ['json','html']
  fs.writeFileSync(`D:/FluxTorrent/.workbuddy/_lh_${form}.json`, reports[0]);
  fs.writeFileSync(`D:/FluxTorrent/.workbuddy/lighthouse-${form}.html`, reports[1] || reports[0]);
  const lines = [`form=${form}`, `finalUrl=${lhr.finalDisplayedUrl || lhr.finalUrl}`];
  lines.push(
    'scores: ' +
      Object.entries(lhr.categories)
        .map(([k, v]) => `${k}=${Math.round((v.score || 0) * 100)}`)
        .join(' '),
  );
  for (const k of [
    'first-contentful-paint',
    'largest-contentful-paint',
    'speed-index',
    'total-blocking-time',
    'cumulative-layout-shift',
    'interactive',
    'server-response-time',
  ]) {
    const a = lhr.audits[k];
    if (a) lines.push(`  ${k} = ${a.displayValue}`);
  }
  // list every failing audit id per category
  for (const [ck, cat] of Object.entries(lhr.categories)) {
    const fails = cat.auditRefs
      .map((r) => lhr.audits[r.id])
      .filter((a) => a && a.score !== null && a.score < 1 && a.scoreDisplayMode !== 'informative' && a.scoreDisplayMode !== 'notApplicable');
    if (fails.length) lines.push(`-- ${ck} fail: ` + fails.map((a) => `${a.id}(${a.score})`).join(', '));
  }
  fs.writeFileSync(`D:/FluxTorrent/.workbuddy/_lh_${form}.txt`, lines.join('\n'), 'utf8');
  await chrome.kill();
  console.log(`OK ${form}`);
})().catch((e) => {
  fs.writeFileSync(
    `D:/FluxTorrent/.workbuddy/_lh_${form}.txt`,
    'ERROR: ' + (e && e.stack ? e.stack : String(e)),
    'utf8',
  );
  process.exit(1);
});
