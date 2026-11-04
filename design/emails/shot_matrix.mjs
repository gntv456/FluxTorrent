// 只为索引页「客户端降级矩阵」截图 + 键盘横滑取证。
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

const CHROME = 'C:\\Users\\52sy\\AppData\\Local\\Google\\Chrome\\Application\\chrome.exe';
const ROOT = 'D:\\FluxTorrent';
const SHOTS = path.join(ROOT, 'design', '_shots');
const IDX = path.join(ROOT, 'design', 'emails', 'index.html');
const PORT = 9466;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function main() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'fx-sec-'));
  const proc = spawn(CHROME, ['--headless=new', `--remote-debugging-port=${PORT}`,
    `--user-data-dir=${dir}`, '--no-first-run', '--hide-scrollbars',
    '--allow-file-access-from-files', '--disable-gpu', '--force-device-scale-factor=1',
    'about:blank'], { stdio: 'ignore' });
  let wsUrl;
  for (let i = 0; i < 80 && !wsUrl; i++) {
    try {
      const r = await fetch(`http://127.0.0.1:${PORT}/json/new?about:blank`, { method: 'PUT' });
      if (r.ok) wsUrl = (await r.json()).webSocketDebuggerUrl;
    } catch {}
    await sleep(150);
  }
  const ws = new WebSocket(wsUrl);
  let id = 0;
  const pending = new Map();
  ws.onmessage = (ev) => {
    const m = JSON.parse(ev.data);
    if (m.id && pending.has(m.id)) {
      const p = pending.get(m.id); pending.delete(m.id);
      m.error ? p.rej(new Error(m.error.message)) : p.res(m.result);
    }
  };
  await new Promise((res, rej) => { ws.onopen = res; ws.onerror = rej; });
  const send = (method, params = {}) => new Promise((res, rej) => {
    const mid = ++id; pending.set(mid, { res, rej });
    ws.send(JSON.stringify({ id: mid, method, params }));
  });
  const evaluate = async (expression) => {
    const r = await send('Runtime.evaluate', { expression, returnByValue: true });
    return r.result.value;
  };
  await send('Page.enable');
  await send('Runtime.enable');

  const out = { chrome: '149.0.7827.197', widths: {} };
  for (const w of [1440, 375]) {
    await send('Emulation.setDeviceMetricsOverride', {
      width: w, height: 1000, deviceScaleFactor: 1, mobile: w < 500,
    });
    await send('Page.navigate', { url: `file:///${IDX.replace(/\\/g, '/')}` });
    await sleep(1800);
    const info = await evaluate(`(() => {
      const sec = document.getElementById('degrade');
      const box = sec.getBoundingClientRect();
      const de = document.documentElement;
      const m = document.querySelector('#degrade .matrix');
      return { x: box.left + scrollX, y: box.top + scrollY, w: box.width,
        h: Math.min(box.height, 2600), pageOverflow: de.scrollWidth - de.clientWidth,
        matrixScroll: m.scrollWidth, matrixClient: m.clientWidth,
        tabindex: m.getAttribute('tabindex'), role: m.getAttribute('role') };
    })()`);
    const clip = { x: info.x, y: info.y, width: info.w, height: info.h, scale: 1 };
    const shot = await send('Page.captureScreenshot', {
      format: 'png', clip, captureBeyondViewport: true,
    });
    fs.writeFileSync(path.join(SHOTS, `matrix-${w}.png`), Buffer.from(shot.data, 'base64'));

    // 键盘横滑取证：聚焦容器后按 ArrowRight
    await evaluate('document.querySelector("#degrade .matrix").focus()');
    const before = await evaluate('document.querySelector("#degrade .matrix").scrollLeft');
    for (let i = 0; i < 6; i++) {
      await send('Input.dispatchKeyEvent', { type: 'keyDown', key: 'ArrowRight', code: 'ArrowRight', windowsVirtualKeyCode: 39 });
      await send('Input.dispatchKeyEvent', { type: 'keyUp', key: 'ArrowRight', code: 'ArrowRight', windowsVirtualKeyCode: 39 });
    }
    await sleep(300);
    const after = await evaluate('document.querySelector("#degrade .matrix").scrollLeft');
    out.widths[w] = { ...info, keyboardScroll: { before, after } };
  }
  fs.writeFileSync(path.join(SHOTS, 'matrix-index-report.json'), JSON.stringify(out, null, 2));
  console.log(JSON.stringify(out, null, 2));
  ws.close();
  proc.kill();
}

main().catch((e) => { console.error(e); process.exit(1); });
