// 种子列表新版（好学站口径）渲染验证：三行结构/促销徽标+剩余/行为列/行高亮。
const { execSync } = require("child_process");
const fs = require("fs");

const BASE = "http://127.0.0.1:3000";
const TOK = (fs.readFileSync("D:/FluxTorrent/_p2tok.txt", "utf8") || "").trim();

function cdp(wsUrl, steps) {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(wsUrl);
    let id = 0;
    const pending = new Map();
    const send = (method, params) => new Promise((res) => {
      const mid = ++id;
      pending.set(mid, res);
      ws.send(JSON.stringify({ id: mid, method, params: params || {} }));
    });
    ws.onmessage = (ev) => {
      const msg = JSON.parse(ev.data);
      if (msg.id && pending.has(msg.id)) { pending.get(msg.id)(msg.result); pending.delete(msg.id); }
    };
    ws.onerror = () => reject(new Error("ws error"));
    ws.onopen = async () => {
      try { await steps(send, () => ws.close()); resolve(); }
      catch (e) { reject(e); }
    };
  });
}

async function main() {
  execSync('curl -s -X PUT "http://127.0.0.1:9222/json/new?about:blank" > NUL', { shell: "cmd.exe" });
  await new Promise(r => setTimeout(r, 800));
  const list = JSON.parse(execSync('curl -s http://127.0.0.1:9222/json', { shell: "cmd.exe" }).toString());
  const page = list.reverse().find(t => t.type === "page" && t.url === "about:blank");
  const out = {};

  await cdp(page.webSocketDebuggerUrl, async (send, done) => {
    await send("Runtime.enable");
    await send("Page.enable");
    await send("Network.enable");
    await send("Network.setCookie", { name: "flux.session", value: "1", domain: "127.0.0.1", path: "/" });
    await send("Network.setCookie", { name: "flux.token", value: TOK, domain: "127.0.0.1", path: "/" });
    await send("Page.navigate", { url: `${BASE}/login` });
    await new Promise(r => setTimeout(r, 2500));
    await send("Runtime.evaluate", { expression: `localStorage.setItem("flux.token", ${JSON.stringify(TOK)})` });
    await send("Page.navigate", { url: `${BASE}/torrents` });
    await new Promise(r => setTimeout(r, 4000));
    const p = await send("Runtime.evaluate", {
      expression: `(() => { try {
        const rows = [...document.querySelectorAll(".torrents-table tbody tr")];
        const r0 = rows[0];
        const title = r0 ? r0.querySelector(".torrents-title") : null;
        const sub = r0 ? r0.querySelector(".torrents-subtitle") : null;
        const meta = r0 ? r0.querySelector(".torrents-meta") : null;
        const cat = r0 ? r0.querySelector(".torrents-cat-block") : null;
        const actions = r0 ? r0.querySelectorAll(".torrents-action") : [];
        const promo = r0 ? r0.querySelector(".torrents-promo") : null;
        const left = r0 ? r0.querySelector(".torrents-left") : null;
        const headers = [...document.querySelectorAll(".torrents-table th")].map(th => th.textContent.trim());
        const anyPromoRow = [...document.querySelectorAll(".torrents-table tbody tr")].some(tr => tr.style.background);
        return JSON.stringify({
          rowCount: rows.length,
          hasCatBlock: !!cat,
          hasTitleLine: !!title,
          hasSubtitle: !!sub,
          hasMetaLine: !!meta,
          actionCount: actions.length,
          actionHrefs: [...actions].map(a => a.getAttribute("href")),
          promoBadge: promo ? promo.textContent : null,
          promoLeft: left ? left.textContent : null,
          headers,
          anyPromoRowBg: anyPromoRow,
          titleSample: title ? title.textContent.trim().slice(0, 40) : null,
          metaSample: meta ? meta.textContent.trim().slice(0, 40) : null,
        });
      } catch(e) { return JSON.stringify({err: String(e)}); } })()`,
      returnByValue: true,
    });
    out.main = JSON.parse(p.result.value);
    const shot = await send("Page.captureScreenshot", { format: "png" });
    fs.writeFileSync("D:/FluxTorrent/_torrents_new.png", Buffer.from(shot.data, "base64"));
    done();
  });

  console.log(JSON.stringify(out.main, null, 2));
  const m = out.main;
  const checks = [
    ["渲染行数 ≥ 10", m.rowCount >= 10],
    ["类型色块列", m.hasCatBlock],
    ["标题三行结构（主标题）", m.hasTitleLine],
    ["副题链行", m.hasSubtitle !== false],
    ["上传者/日期行", m.hasMetaLine],
    ["表头含操作列", m.headers.some(h => /操作|Actions|操作/.test(h))],
    ["行为列 2 钮", m.actionCount === 2],
    ["下载钮指向种子", (m.actionHrefs || []).some(h => h && h.includes("torrent"))],
  ];
  let pass = 0;
  for (const [n, c] of checks) { console.log((c ? "PASS " : "FAIL ") + n); if (c) pass++; }
  console.log(`===== 种子列表新版：${pass}/${checks.length} PASS =====`);
}

main().catch(e => { console.error("ERR", e.message); process.exit(1); });
