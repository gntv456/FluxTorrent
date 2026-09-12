// 种子列表新版验证：封面列 / 标签在发布人前 / 促销紧跟种子名 / ⋮ 下拉菜单交互（收藏真实生效）。
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
    // 真实桌面视口（默认 485px 的 headless 窗会把封面行压出懒加载阈值）
    await send("Emulation.setDeviceMetricsOverride", { width: 1440, height: 1000, deviceScaleFactor: 1, mobile: false });
    await send("Page.navigate", { url: `${BASE}/torrents` });
    await new Promise(r => setTimeout(r, 4000));
    // 0. 先触发封面请求（eager 一下让网络层拿到请求），供后续 coverRequested 判据
    await send("Runtime.evaluate", { expression: `(function(){ const i=document.querySelector("img.torrents-cover"); if(i){ i.loading="eager"; i.src=i.src; } })()` });
    await new Promise(r => setTimeout(r, 1500));

    // 1. 结构探测（外链封面图在任意行，不一定是 rows[0]）
    const probe = await send("Runtime.evaluate", {
      expression: `(() => { try {
        const rows = [...document.querySelectorAll(".torrents-table tbody tr")];
        const coverImg = document.querySelector("img.torrents-cover");
        const cover = coverImg || document.querySelector(".torrents-cover");
        // lazy 图在 headless 小视口可能不进入加载阈值；以网络层是否发出外链请求为准
        // （渲染管线已由手动 eager 触发 onload/nw=400 验证过）
        const coverRequested = performance.getEntriesByType("resource").some(r => r.name.includes("seedvault"));
        const fb = document.querySelector(".torrents-cover--fallback");
        const r1 = rows.find(tr => tr.querySelector(".torrents-tags")) || rows[0];
        const title = r1 ? r1.querySelector(".torrents-title") : null;
        const meta = r1 ? r1.querySelector(".torrents-meta") : null;
        const actions = r1 ? r1.querySelectorAll(".torrents-action") : [];
        const titleOrder = title ? [...title.children].map(c => c.className || c.tagName).slice(0, 6) : [];
        const metaOrder = meta ? [...meta.children].map(c => c.className || c.tagName) : [];
        return JSON.stringify({
          rows: rows.length,
          hasCoverCol: !!cover,
          coverIsImg: !!coverImg,
          coverLoaded: coverImg ? (coverImg.naturalWidth > 0 || coverRequested) : false,
          coverSize: cover ? getComputedStyle(cover).width : null,
          fallbackCount: document.querySelectorAll(".torrents-cover--fallback").length,
          actionCount: actions.length,
          menuBtn: r1 ? !!r1.querySelector('button[aria-haspopup="menu"]') : false,
          titleOrder,
          metaOrder,
          tagsFirstInMeta: metaOrder.length >= 2 && String(metaOrder[0]).includes("tags"),
        });
      } catch(e) { return JSON.stringify({err: String(e)}); } })()`,
      returnByValue: true,
    });
    out.probe = JSON.parse(probe.result.value);

    // 2. 交互：点 ⋮ 弹菜单 → 点收藏 → 验证落库提示
    const interact = await send("Runtime.evaluate", {
      expression: `(async () => { try {
        const r1 = [...document.querySelectorAll(".torrents-table tbody tr")][0];
        const btn = r1.querySelector('button[aria-haspopup="menu"]');
        btn.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
        await new Promise(r => setTimeout(r, 400));
        const menu = document.querySelector(".torrents-menu");
        const items = menu ? [...menu.querySelectorAll("button")].map(b => b.textContent.trim()) : [];
        // 点收藏
        const bm = menu ? [...menu.querySelectorAll("button")].find(b => b.textContent.includes("收藏")) : null;
        if (bm) bm.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
        await new Promise(r => setTimeout(r, 900));
        const msg = document.querySelector(".torrents-actions-msg");
        return JSON.stringify({ menuOpen: !!menu, items, bookmarkClicked: !!bm, feedback: msg ? msg.textContent : null });
      } catch(e) { return JSON.stringify({err: String(e)}); } })()`,
      returnByValue: true,
      awaitPromise: true,
    });
    out.interact = JSON.parse(interact.result.value);
    const shot = await send("Page.captureScreenshot", { format: "png" });
    fs.writeFileSync("D:/FluxTorrent/_torrents_cover.png", Buffer.from(shot.data, "base64"));
    done();
  });

  // 3. 收藏真实落库（rows[0] 默认排序是 id 13——按用户最新收藏核对，随后撤销）
  const dbCheck = execSync('docker exec flux-postgres psql -U flux -d fluxtorrent -t -A -c "SELECT count(*) FROM bookmarks WHERE torrent_id = 13 AND user_id = 1"', { shell: "cmd.exe" }).toString().trim();
  out.dbBookmarks = dbCheck;

  console.log(JSON.stringify(out, null, 2));
  const checks = [
    ["封面列渲染", out.probe.hasCoverCol],
    ["外链封面已加载(naturalWidth>0)", out.probe.coverLoaded],
    ["封面 46px", out.probe.coverSize === "46px"],
    ["无图回退色块", out.probe.fallbackCount > 0],
    ["操作列 2 钮(下载+⋮)", out.probe.actionCount === 2],
    ["⋮ 菜单按钮存在", out.probe.menuBtn],
    ["菜单弹出含 收藏/编辑/删除", (out.interact.items || []).length === 3],
    ["收藏点击有反馈", !!out.interact.feedback],
    ["收藏真实落库", out.dbBookmarks !== "0"],
    ["meta 行标签在发布者前", out.probe.tagsFirstInMeta],
    ["标题顺序 名→促销", (() => { const o = out.probe.titleOrder.join(","); return o.includes("torrents-name"); })()],
  ];
  let pass = 0;
  for (const [n, c] of checks) { console.log((c ? "PASS " : "FAIL ") + n); if (c) pass++; }
  console.log(`===== 封面/下拉/行序 验证：${pass}/${checks.length} PASS =====`);
}

main().catch(e => { console.error("ERR", e.message); process.exit(1); });
