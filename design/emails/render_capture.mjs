// 用真实 Chrome（CDP）渲染捕获到的 HTML part，量溢出 / 取样对比度 / 截屏。
// 只读本地 file://，无网络请求。输出 design/_shots/mail-render-report.json。
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

const CHROME = 'C:\\Users\\52sy\\AppData\\Local\\Google\\Chrome\\Application\\chrome.exe';
const ROOT = 'D:\\FluxTorrent';
const CAP = path.join(ROOT, 'design', 'emails', '_smtp_capture', 'html');
const SHOTS = path.join(ROOT, 'design', '_shots');
const PORT = 9455;

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

function launch() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'fx-cdp-'));
  return spawn(CHROME, [
    '--headless=new', `--remote-debugging-port=${PORT}`, `--user-data-dir=${dir}`,
    '--no-first-run', '--no-default-browser-check', '--hide-scrollbars',
    '--allow-file-access-from-files', '--disable-gpu',
    '--force-device-scale-factor=1', 'about:blank',
  ], { stdio: 'ignore' });
}

async function targetWs() {
  for (let i = 0; i < 80; i++) {
    try {
      const r = await fetch(`http://127.0.0.1:${PORT}/json/new?about:blank`, { method: 'PUT' });
      if (r.ok) return (await r.json()).webSocketDebuggerUrl;
    } catch {}
    await sleep(150);
  }
  throw new Error('CDP 未就绪');
}

function connect(url) {
  const ws = new WebSocket(url);
  let id = 0;
  const pending = new Map();
  const events = [];
  ws.onmessage = (ev) => {
    const m = JSON.parse(ev.data);
    if (m.id && pending.has(m.id)) {
      const { res, rej } = pending.get(m.id);
      pending.delete(m.id);
      m.error ? rej(new Error(m.error.message)) : res(m.result);
    } else if (m.method) {
      events.push(m.method);
    }
  };
  const ready = new Promise((res, rej) => { ws.onopen = res; ws.onerror = rej; });
  return {
    ready,
    send(method, params = {}) {
      const mid = ++id;
      ws.send(JSON.stringify({ id: mid, method, params }));
      return new Promise((res, rej) => {
        pending.set(mid, { res, rej });
        setTimeout(() => {
          if (pending.has(mid)) {
            pending.delete(mid);
            rej(new Error(`${method} 超时`));
          }
        }, 30000);
      });
    },
    events,
    close: () => ws.close(),
  };
}

const SAMPLE = `(() => {
  const parse = (s) => {
    const m = /rgba?\\(([^)]+)\\)/.exec(s || '');
    if (!m) return null;
    const p = m[1].split(',').map(Number);
    return { r: p[0], g: p[1], b: p[2], a: p.length > 3 ? p[3] : 1 };
  };
  const bgOf = (el) => {
    let e = el;
    while (e) {
      const c = parse(getComputedStyle(e).backgroundColor);
      if (c && c.a > 0.9) return c;
      e = e.parentElement;
    }
    return { r: 255, g: 255, b: 255, a: 1 };
  };
  const lum = (c) => {
    const f = (v) => { v /= 255; return v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4); };
    return 0.2126 * f(c.r) + 0.7152 * f(c.g) + 0.0722 * f(c.b);
  };
  const ratio = (a, b) => {
    const l1 = lum(a), l2 = lum(b);
    const hi = Math.max(l1, l2), lo = Math.min(l1, l2);
    return (hi + 0.05) / (lo + 0.05);
  };
  const inv = (c) => ({ r: 255 - c.r, g: 255 - c.g, b: 255 - c.b, a: 1 });
  const hidden = (e) => {
    let n = e;
    while (n) {
      if (n.getAttribute && n.getAttribute('aria-hidden') === 'true') return true;
      n = n.parentElement;
    }
    const cs = getComputedStyle(e);
    return cs.display === 'none' || cs.visibility === 'hidden'
      || parseFloat(cs.opacity) === 0 || parseFloat(cs.fontSize) < 4;
  };
  const nodes = [...document.querySelectorAll('h1,p,a,td,span,strong,small,div')]
    .filter((e) => e.innerText && e.innerText.trim().length > 0
      && e.children.length === 0 && !hidden(e));
  const out = [];
  for (const e of nodes.slice(0, 60)) {
    const fg = parse(getComputedStyle(e).color) || { r: 0, g: 0, b: 0, a: 1 };
    const bg = bgOf(e);
    out.push({
      text: e.innerText.trim().slice(0, 24),
      tag: e.tagName.toLowerCase(),
      fg: [fg.r, fg.g, fg.b], bg: [bg.r, bg.g, bg.b],
      ratio: +ratio(fg, bg).toFixed(2),
      invertRatio: +ratio(inv(fg), inv(bg)).toFixed(2),
    });
  }
  return out;
})()`;

async function main() {
  fs.mkdirSync(SHOTS, { recursive: true });
  const proc = launch();
  const ws = connect(await targetWs());
  await ws.ready;
  const page = ws.send;
  await page('Page.enable');
  await page('Runtime.enable');

  const metrics = async (w, h) => {
    await page('Emulation.setDeviceMetricsOverride', {
      width: w, height: h, deviceScaleFactor: 1, mobile: w < 500,
    });
  };
  const navigate = async (url) => {
    const before = ws.events.length;
    await page('Page.navigate', { url });
    for (let i = 0; i < 200; i++) {
      if (ws.events.slice(before).includes('Page.loadEventFired')) break;
      await sleep(50);
    }
    await sleep(120);
  };
  const evaluate = async (expr) => {
    const r = await page('Runtime.evaluate', {
      expression: expr, returnByValue: true, awaitPromise: true,
    });
    if (r.exceptionDetails) throw new Error(r.exceptionDetails.text);
    return r.result.value;
  };
  const shot = async (name, full = true) => {
    const r = await page('Page.captureScreenshot', {
      format: 'png', captureBeyondViewport: full, fromSurface: true,
    });
    fs.writeFileSync(path.join(SHOTS, name), Buffer.from(r.data, 'base64'));
  };
  const measure = () => evaluate(`(() => {
    const de = document.documentElement;
    const imgs = [...document.images].filter((i) => i.offsetParent !== null).length;
    const text = (document.body.innerText || '').replace(/\\s+/g, '');
    return { scrollWidth: de.scrollWidth, clientWidth: de.clientWidth,
      scrollHeight: de.scrollHeight, visibleImgs: imgs, textLen: text.length };
  })()`);

  const rows = [];
  const mailNames = ['invite', 'password-reset', 'email-verify', 'review-approved',
    'review-rejected', 'application-approved', 'application-rejected',
    'appeal-accepted', 'appeal-rejected', 'account-status', 'hr-warning'];

  const states = [
    { key: 'normal', dark: false, css: '' },
    { key: 'noimg', dark: false, css: 'img{display:none!important}' },
    { key: 'invert', dark: false, css: 'html{filter:invert(1) hue-rotate(180deg)}' },
    { key: 'dark', dark: true, css: '' },
  ];
  const strip = `(() => {
    for (const e of document.querySelectorAll('[style]'))
      e.setAttribute('style', e.getAttribute('style').replace(/background[^;]*;?/gi, ''));
    for (const e of document.querySelectorAll('[bgcolor]')) e.removeAttribute('bgcolor');
    return true;
  })()`;

  for (const name of mailNames) {
    const file = path.join(CAP, `zh-cn-clean-${name}.html`);
    if (!fs.existsSync(file)) continue;
    for (const st of states) {
      for (const w of st.key === 'normal' ? [600, 375] : [600]) {
        await metrics(w, 1200);
        await page('Emulation.setEmulatedMedia', {
          features: st.dark ? [{ name: 'prefers-color-scheme', value: 'dark' }] : [],
        });
        await navigate(`file:///${file.replace(/\\/g, '/')}`);
        if (st.css) await evaluate(`(() => { const s=document.createElement('style'); s.textContent=${JSON.stringify(st.css)}; document.head.appendChild(s); return true; })()`);
        await sleep(80);
        const m = await measure();
        const lum = await evaluate(SAMPLE);
        rows.push({
          mail: name, state: st.key, width: w, ...m,
          overflowX: m.scrollWidth - m.clientWidth,
          minRatio: Math.min(...lum.map((x) => x.ratio)),
          minInvertRatio: Math.min(...lum.map((x) => x.invertRatio)),
          lowSamples: lum.filter((x) => x.ratio < 4.5).slice(0, 3),
        });
        await shot(`mail-${name}-${st.key}-${w}.png`);
      }
    }
    // 剥背景 + 二进制安全兜底
    await metrics(600, 1200);
    await page('Emulation.setEmulatedMedia', { features: [] });
    await navigate(`file:///${file.replace(/\\/g, '/')}`);
    await evaluate(strip);
    const st = await evaluate(SAMPLE);
    const m2 = await measure();
    rows.push({
      mail: name, state: 'stripbg', width: 600, ...m2,
      overflowX: m2.scrollWidth - m2.clientWidth,
      minRatio: Math.min(...st.map((x) => x.ratio)),
      minInvertRatio: Math.min(...st.map((x) => x.invertRatio)),
      lowSamples: st.filter((x) => x.ratio < 4.5).slice(0, 3),
    });
    await shot(`mail-${name}-stripbg-600.png`);
  }

  // 恶意输入样本：确认转义后视觉不被破坏
  for (const tag of ['zh-cn', 'zh-tw', 'en']) {
    const file = path.join(CAP, `${tag}-evil-review-rejected.html`);
    if (!fs.existsSync(file)) continue;
    await page('Emulation.setEmulatedMedia', { features: [] });
    await metrics(600, 1200);
    await navigate(`file:///${file.replace(/\\/g, '/')}`);
    const m = await measure();
    rows.push({ mail: `${tag}-evil-review-rejected`, state: 'normal', width: 600, ...m,
      overflowX: m.scrollWidth - m.clientWidth });
    await shot(`mail-evil-review-rejected-${tag}-600.png`);
  }

  // 索引页两档
  const idx = path.join(ROOT, 'design', 'emails', 'index.html');
  for (const w of [1440, 375]) {
    try {
      await page('Emulation.setEmulatedMedia', { features: [] });
      await metrics(w, 1000);
      // 索引页有 33 个 iframe，等到 load 事件会拖很久；固定等待后直接量。
      ws.events.length = 0;
      await page('Page.navigate', { url: `file:///${idx.replace(/\\/g, '/')}` });
      await sleep(1200);
      const m = await evaluate(`(() => {
        const de = document.documentElement;
        const tables = [...document.querySelectorAll('.matrix')].map((d) => ({
          scrollWidth: d.scrollWidth, clientWidth: d.clientWidth,
          tabindex: d.getAttribute('tabindex'),
          overflowX: getComputedStyle(d).overflowX,
        }));
        return { scrollWidth: de.scrollWidth, clientWidth: de.clientWidth,
          scrollHeight: de.scrollHeight, tables };
      })()`);
      rows.push({ mail: 'index', state: 'normal', width: w,
        overflowX: m.scrollWidth - m.clientWidth, ...m });
      await evaluate('document.getElementById("degrade").scrollIntoView({block:"start"})');
      await sleep(400);
      await shot(`index-degrade-${w}.png`, false);
    } catch (e) {
      rows.push({ mail: 'index', state: 'normal', width: w,
        overflowX: null, error: String(e).slice(0, 120) });
    }
  }

  const bad = rows.filter((r) => (r.overflowX || 0) > 0);
  const report = {
    chrome: '149.0.7827.197',
    viewport: 'CDP Emulation.setDeviceMetricsOverride, dsf=1',
    renders: rows.length,
    overflow_pages: bad.map((r) => `${r.mail}/${r.state}/${r.width}`),
    min_contrast: Math.min(...rows.filter((r) => r.minRatio).map((r) => r.minRatio)),
    min_contrast_invert_model: Math.min(...rows.filter((r) => r.minInvertRatio).map((r) => r.minInvertRatio)),
    rows,
  };
  fs.writeFileSync(path.join(SHOTS, 'mail-render-report.json'), JSON.stringify(report, null, 2));
  console.log(JSON.stringify({ renders: rows.length, overflow: report.overflow_pages,
    min_contrast: report.min_contrast, min_invert: report.min_contrast_invert_model }, null, 2));
  ws.close();
  proc.kill();
}

main().catch((e) => { console.error(e); process.exit(1); });
