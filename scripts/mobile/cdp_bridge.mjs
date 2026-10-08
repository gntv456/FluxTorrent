// 模拟器 WebView 真机联调桥(纯 CDP, 无 playwright)
// 用法: node _mcdp.mjs <script.js>
// script.js 导出 default async ({ page, evalPage, navigate, shot }) => {}
//  - evalPage(fnOrString, ...args): 在页面里执行 JS 并返回结果
//  - navigate(url): location.href 跳转 + 等稳定
//  - shot(path): Page.captureScreenshot 存 png
import { pathToFileURL } from 'url';

const CDP_HTTP = 'http://127.0.0.1:9222';

async function getPageWs() {
  // WebView 常残留多个 page target(历史标签), 死标签 CDP 无响应——
  // 逐个探活(1+1 eval), 取第一个应答的; 全死则报错
  const list = await (await fetch(`${CDP_HTTP}/json/list`)).json();
  const pages = list.filter(t => t.type === 'page' && t.webSocketDebuggerUrl);
  if (!pages.length) throw new Error('no page target; open a URL in the emulator browser first');
  // 优先试 target 里 attached 的(最近活跃), 其余按序
  pages.sort((a, b) => (JSON.parse(b.description || '{}').attached ? 1 : 0) - (JSON.parse(a.description || '{}').attached ? 1 : 0));
  for (const t of pages) {
    const alive = await new Promise((resolve) => {
      let done = false;
      let ws;
      try { ws = new WebSocket(t.webSocketDebuggerUrl); } catch { return resolve(false); }
      const killer = setTimeout(() => { if (!done) { done = true; try { ws.close(); } catch {} resolve(false); } }, 6000);
      ws.addEventListener('open', () => {
        ws.send(JSON.stringify({ id: 1, method: 'Runtime.evaluate', params: { expression: '1+1', returnByValue: true } }));
      });
      ws.addEventListener('error', () => { if (!done) { done = true; clearTimeout(killer); resolve(false); } });
      ws.addEventListener('message', ev => {
        const m = JSON.parse(ev.data);
        if (m.id === 1 && !done) { done = true; clearTimeout(killer); try { ws.close(); } catch {} resolve(true); }
      });
    });
    if (alive) return t.webSocketDebuggerUrl;
  }
  throw new Error('all page targets dead; restart the emulator browser app');
}

class CDPSession {
  constructor(ws) { this.ws = ws; this.id = 0; this.pending = new Map(); this.handlers = new Map();
    ws.addEventListener('message', ev => {
      const msg = JSON.parse(ev.data);
      if (msg.id && this.pending.has(msg.id)) {
        const { resolve, reject } = this.pending.get(msg.id); this.pending.delete(msg.id);
        msg.error ? reject(new Error(JSON.stringify(msg.error))) : resolve(msg.result);
      } else if (msg.method && this.handlers.has(msg.method)) {
        this.handlers.get(msg.method)(msg.params);
      }
    });
  }
  send(method, params = {}) {
    const id = ++this.id;
    this.ws.send(JSON.stringify({ id, method, params }));
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`CDP send timeout: ${method}`));
      }, 60000);
      this.pending.set(id, { resolve: (v) => { clearTimeout(timer); resolve(v); }, reject: (e) => { clearTimeout(timer); reject(e); } });
    });
  }
  on(method, fn) { this.handlers.set(method, fn); }
  close() { this.ws.close(); }
}

// 页面稳定探测: 等网络安静 + DOM 不再变化
async function waitStable(sess, { quiet = 1500, deadline = 60000 } = {}) {
  const t0 = Date.now();
  let lastSig = '', lastChange = Date.now();
  while (Date.now() - t0 < deadline) {
    const sig = await sess.send('Runtime.evaluate', { expression:
      `JSON.stringify([document.readyState, document.documentElement.outerHTML.length, performance.getEntriesByType('resource').length])`,
      returnByValue: true }).then(r => r.result.value).catch(() => '');
    if (sig !== lastSig) { lastSig = sig; lastChange = Date.now(); }
    else if (Date.now() - lastChange > quiet && sig.includes('complete')) return;
    await new Promise(r => setTimeout(r, 400));
  }
}

async function main() {
  const [,, scriptPath] = process.argv;
  if (!scriptPath) { console.error('usage: node _mcdp.mjs <script.js>'); process.exit(1); }
  const ws = new WebSocket(await getPageWs());
  await new Promise((res, rej) => { ws.addEventListener('open', res); ws.addEventListener('error', rej); });
  const sess = new CDPSession(ws);
  await sess.send('Page.enable').catch(() => {});
  await sess.send('Runtime.enable').catch(() => {});
  // WebView Browser Tester 不吃 viewport meta(布局视口 524 而非 412)——
  // 用 CDP 设备度量覆盖模拟真实手机视口, 否则所有像素级审计结论失真
  const EMU = process.env.MEMU !== '0';
  if (EMU) {
    await sess.send('Emulation.setDeviceMetricsOverride', {
      width: 412, height: 915, deviceScaleFactor: 2.625, mobile: true,
    }).then(() => console.error('[emu] viewport 412x915 mobile=true ok'))
      .catch(e => console.error('[emu] override FAILED:', String(e).slice(0, 120)));
  }

  const evalPage = async (fn, ...args) => {
    let expr;
    if (typeof fn === 'string') {
      expr = fn.startsWith('(') || fn.startsWith('async') ? fn : `(() => (${fn}))()`;
    } else {
      const argJson = JSON.stringify(args).replace(/\\/g, '\\\\');
      // 函数体先去掉注释再压缩成单行: 块注释里若残留 */ 之外的换行无碍,
      // 但字面换行在部分 WebView 上会炸, 统一替换为空格
      let body = fn.toString().replace(/\/\*[\s\S]*?\*\//g, '').replace(/\/\/[^\n]*/g, '');
      body = body.replace(/\n\s*/g, ' ');
      expr = `(${body})(...${argJson})`;
    }
    const r = await sess.send('Runtime.evaluate', { expression: expr, returnByValue: true, awaitPromise: true });
    if (r.exceptionDetails) {
      const d = r.exceptionDetails;
      throw new Error('page eval failed: ' + (d.exception?.description || d.text || '').slice(0, 400));
    }
    return r.result.value;
  };
  const navigate = async (url) => {
    await evalPage(`location.href = ${JSON.stringify(url)}`).catch(() => {});
    await new Promise(r => setTimeout(r, 1500));
    await waitStable(sess).catch(() => {});
    // WebView Browser Tester 忽略 viewport meta: 布局视口由 initial-scale=1 换算成
    // 524px 而非 412. 每次导航后重申设备度量覆盖, 恢复真实手机布局视口
    if (EMU) await sess.send('Emulation.setDeviceMetricsOverride', {
      width: 412, height: 915, deviceScaleFactor: 2.625, mobile: true,
    }).catch(() => {});
  };
  const shot = async (path) => {
    // WebView 对无参 captureScreenshot 可能卡死; 限定视口矩形并先试小格式
    const layout = await sess.send('Page.getLayoutMetrics').catch(() => null);
    let clip;
    if (layout && layout.cssVisualViewport) {
      const v = layout.cssVisualViewport;
      clip = { x: 0, y: 0, width: Math.ceil(v.clientWidth || 412), height: Math.ceil(v.clientHeight || 915), scale: 1 };
    }
    const r = await sess.send('Page.captureScreenshot', { format: 'jpeg', quality: 70, ...(clip ? { clip } : {}) });
    const { writeFile } = await import('fs/promises');
    await writeFile(path, Buffer.from(r.data, 'base64'));
    return path;
  };

  const helpers = { ws, sess, evalPage, navigate, shot,
    reviewport: async () => {
      if (EMU) await sess.send('Emulation.setDeviceMetricsOverride', {
        width: 412, height: 915, deviceScaleFactor: 2.625, mobile: true,
      }).catch(() => {});
    },
    get url() { return evalPage('location.href'); } };

  const mod = await import(pathToFileURL(scriptPath).href);
  await mod.default(helpers);
  sess.close();
}

main().catch(e => { console.error(e); process.exit(1); });
