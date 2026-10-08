// 把 img/ 下的截图拼成一张联系表，便于一次性审阅
import { spawn } from "node:child_process";
import { mkdtempSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const EDGE = "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe";
const PORT = 9334;
const dir = decodeURI(new URL(".", import.meta.url).pathname).replace(/^\/(\w:)/, "$1");
const out = process.argv[2] || "review.png";
const files = process.argv.slice(3);

const udd = mkdtempSync(join(process.env.TEMP || "/tmp", "fluxreview-"));
const browser = spawn(EDGE, ["--headless=new", `--remote-debugging-port=${PORT}`,
  `--user-data-dir=${udd}`, "--no-first-run", "--hide-scrollbars", "about:blank"], { stdio: "ignore" });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const html = `<!doctype html><meta charset="utf-8"><body style="margin:0;background:#222;font:14px system-ui">
<div style="display:grid;grid-template-columns:repeat(3,1fr);gap:6px;padding:6px">
${files.map((f) => `<figure style="margin:0"><img src="img/${f}.png" style="width:100%;display:block"><figcaption style="color:#fff;background:#000;padding:3px 6px">${f}</figcaption></figure>`).join("")}
</div></body>`;
const page1 = html.replace("<body>", `<base href="file:///${dir.replace(/\\/g, "/")}"><body>`);
writeFileSync(join(dir, "_review.html"), page1);

let target = null;
for (let i = 0; i < 40 && !target; i++) {
  await sleep(500);
  try {
    const l = await (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json();
    target = l.find((t) => t.type === "page");
  } catch {}
}
if (!target) throw new Error("no CDP target on " + PORT);
let id = 0; const pending = new Map(); const events = [];
const ws = new WebSocket(target.webSocketDebuggerUrl);
await new Promise((r) => (ws.onopen = r));
ws.onmessage = (m) => {
  const msg = JSON.parse(m.data);
  if (msg.method) { events.push({ method: msg.method, ts: Date.now() }); return; }
  const p = pending.get(msg.id); if (p) { pending.delete(msg.id); p.resolve(msg.result); }
};
const send = (method, params = {}) => new Promise((resolve) => { const mid = ++id; pending.set(mid, { resolve }); ws.send(JSON.stringify({ id: mid, method, params })); });

await send("Page.enable");
await send("Emulation.setDeviceMetricsOverride", { width: 1900, height: 1200, deviceScaleFactor: 1, mobile: false });
await send("Page.navigate", { url: "file:///" + join(dir, "_review.html").replace(/\\/g, "/") });
for (let i = 0; i < 40 && !events.some((e) => e.method === "Page.loadEventFired"); i++) await sleep(250);
await sleep(2500);
const shot = await send("Page.captureScreenshot", { format: "png", captureBeyondViewport: true });
writeFileSync(join(dir, out), Buffer.from(shot.data, "base64"));
console.log("saved", join(dir, out));
ws.close(); browser.kill();
