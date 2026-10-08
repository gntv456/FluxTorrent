// 一次性截图驱动：无头 Edge + CDP，登录本地实例后逐页截图
// 用法: node _shots.mjs <profile:desktop|mobile> <name>=<path> [<name>=<path> ...]
import { spawn } from "node:child_process";
import { mkdtempSync, writeFileSync, mkdirSync } from "node:fs";
import { join } from "node:path";

const EDGE = "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe";
const PORT = 9333;
const BASE = "http://localhost:3000";
const OUT = decodeURI(new URL("./img", import.meta.url).pathname).replace(/^\/(\w:)/, "$1");

const profile = process.argv[2] === "mobile" ? "mobile" : "desktop";
const targets = process.argv.slice(profile === "mobile" || profile === "desktop" ? 3 : 2);
const specs = (profile === "mobile" ? targets : targets).map((t) => {
  const i = t.indexOf("=");
  // 路径不带前导斜杠传入（Git Bash 会把 "/games" 改写成 Windows 路径）
  const raw = t.slice(i + 1).replace(/^\/+/, "");
  const [p, scroll] = raw.split("@@");
  return { name: t.slice(0, i), path: p === "root" ? "/" : "/" + p, scroll: Number(scroll || 0) };
});

const VIEWPORT = profile === "mobile"
  ? { width: 390, height: 844, deviceScaleFactor: 3, mobile: true }
  : { width: 1440, height: 900, deviceScaleFactor: 2, mobile: false };

const udd = mkdtempSync(join(process.env.TEMP || "/tmp", "fluxshots-"));
const browser = spawn(EDGE, [
  "--headless=new",
  `--remote-debugging-port=${PORT}`,
  `--user-data-dir=${udd}`,
  "--no-first-run",
  "--no-default-browser-check",
  "--disable-background-networking",
  "--hide-scrollbars",
  "--force-device-scale-factor=1",
  "--lang=zh-CN",
  "about:blank",
], { stdio: "ignore" });

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function target() {
  for (let i = 0; i < 60; i++) {
    try {
      const list = await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json();
      const page = list.find((t) => t.type === "page" && t.webSocketDebuggerUrl);
      if (page) return page;
    } catch {}
    await sleep(500);
  }
  throw new Error("no CDP target");
}

let id = 0;
const pending = new Map();
let ws;
function send(method, params = {}) {
  return new Promise((resolve, reject) => {
    const mid = ++id;
    pending.set(mid, { resolve, reject });
    ws.send(JSON.stringify({ id: mid, method, params }));
  });
}
const events = [];
function waitEvent(name, ms) {
  return new Promise((resolve) => {
    const t0 = Date.now();
    const tick = () => {
      const hit = events.find((e) => e.method === name && e.ts >= t0);
      if (hit) { events.length = 0; return resolve(hit); }
      if (Date.now() - t0 > ms) return resolve(null);
      setTimeout(tick, 100);
    };
    tick();
  });
}

async function evalJs(expression, awaitPromise = false) {
  const r = await send("Runtime.evaluate", { expression, awaitPromise, returnByValue: true });
  if (r.exceptionDetails) throw new Error(JSON.stringify(r.exceptionDetails).slice(0, 300));
  return r.result?.value;
}

async function goto(path) {
  await send("Page.navigate", { url: BASE + path });
  await waitEvent("Page.loadEventFired", 15000);
  await sleep(profile === "mobile" ? 1600 : 1400);
}

async function shot(name, path, { fullPage = false, scroll = 0 } = {}) {
  await goto(path);
  await evalJs(`(() => {
    document.querySelectorAll('[class*=fixed]').forEach(e => e.remove());
    [...document.querySelectorAll('div,section')].filter(e =>
      /把本站安装到桌面|像原生应用一样使用/.test(e.textContent || '') && e.offsetHeight < 140
    ).forEach(e => e.remove());
    return "cleaned";
  })()`);
  if (scroll) {
    await evalJs(`window.scrollTo(0, ${scroll}); "ok"`);
    await sleep(900);
  }
  const res = await send("Page.captureScreenshot", {
    format: "png",
    captureBeyondViewport: fullPage,
    clip: fullPage ? undefined : { x: 0, y: 0, width: VIEWPORT.width, height: VIEWPORT.height, scale: 1 },
  });
  const file = join(OUT, `${name}.png`);
  writeFileSync(file, Buffer.from(res.data, "base64"));
  console.log("saved", file);
}

async function main() {
  mkdirSync(OUT, { recursive: true });
  const page = await target();
  ws = new WebSocket(page.webSocketDebuggerUrl);
  await new Promise((r) => (ws.onopen = r));
  ws.onmessage = (m) => {
    const msg = JSON.parse(m.data);
    if (msg.method) { events.push({ method: msg.method, ts: Date.now() }); return; }
    const p = pending.get(msg.id);
    if (p) { pending.delete(msg.id); msg.error ? p.reject(new Error(JSON.stringify(msg.error))) : p.resolve(msg.result); }
  };
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Emulation.setDeviceMetricsOverride", VIEWPORT);
  if (profile === "mobile") {
    await send("Emulation.setUserAgentOverride", {
      userAgent: "Mozilla/5.0 (Linux; Android 13; Pixel 7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Mobile Safari/537.36",
    });
  }

  await goto("/login");
  const status = await evalJs(
    `fetch('/api/v1/auth/login', {method:'POST',headers:{'Content-Type':'application/json'},
      credentials:'include', body: JSON.stringify({username:'root', password:'password123'})})
      .then(r=>r.status)`,
    true
  );
  console.log("login http status:", status);
  // middleware 以 flux.session 标记 cookie 判登录态（token 本身 HttpOnly）
  await evalJs(`document.cookie = 'flux.session=1; path=/; max-age=43200; samesite=lax'; "ok"`);
  await sleep(600);

  for (const s of specs) await shot(s.name, s.path, { scroll: s.scroll });
  ws.close();
  browser.kill();
}

main().catch((e) => { console.error(e); browser.kill(); process.exit(1); });
