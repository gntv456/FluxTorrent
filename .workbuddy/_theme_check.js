// 第七轮主题优化验证：双主题截图 + 关键样式计算值。
// 纯 Node 内置 WebSocket CDP，无外部依赖。用法：node .workbuddy/_theme_check.js
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
    ws.onerror = (e) => reject(new Error("ws error"));
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
  const page = list.find(t => t.type === "page" && t.url === "about:blank");
  if (!page) throw new Error("no blank tab");

  const out = {};
  await cdp(page.webSocketDebuggerUrl, async (send, done) => {
    await send("Runtime.enable");
    await send("Page.enable");
    await send("Network.enable");
    // 会话 cookie（middleware 的 flux.session 存在性标记）+ localStorage token
    await send("Network.setCookie", { name: "flux.session", value: "1", domain: "127.0.0.1", path: "/" });
    await send("Network.setCookie", { name: "flux.token", value: TOK, domain: "127.0.0.1", path: "/" });
    await send("Page.navigate", { url: `${BASE}/login` });
    await new Promise(r => setTimeout(r, 2500));
    await send("Runtime.evaluate", { expression: `localStorage.setItem("flux.token", ${JSON.stringify(TOK)})` });

    for (const theme of ["baozi", "baozi-night"]) {
      await send("Page.navigate", { url: `${BASE}/torrents` });
      await new Promise(r => setTimeout(r, 3500));
      await send("Runtime.evaluate", { expression: `localStorage.setItem("flux-theme", ${JSON.stringify(theme)}); document.documentElement.dataset.theme = ${JSON.stringify(theme)};` });
      await new Promise(r => setTimeout(r, 700));
      const probe = await send("Runtime.evaluate", {
        expression: `(() => {
          // 种子列表是客户端组件，等渲染后再取样；取不到就报元素计数辅助定位
          const table = document.querySelector(".torrents-table tbody td") || document.querySelector(".nexus-table tbody td");
          const rowStyle = table ? getComputedStyle(table) : null;
          const rowhead = document.querySelector(".nexus-detail__label, .nexus-form td.rowhead");
          const menu = document.querySelector(".mainmenu-link");
          return JSON.stringify({
            theme: document.documentElement.dataset.theme,
            rowBg: rowStyle ? rowStyle.backgroundColor : null,
            rowColor: rowStyle ? rowStyle.color : null,
            menuBg: menu ? getComputedStyle(menu).backgroundColor : null,
            labelBg: rowhead ? getComputedStyle(rowhead).backgroundColor : null,
            scrollbarColor: getComputedStyle(document.body).scrollbarColor,
            dbg: { torrentsRows: document.querySelectorAll(".torrents-table tbody tr").length, menuCount: document.querySelectorAll(".mainmenu-link").length },
          });
        })()`,
        returnByValue: true,
      });
      out[theme] = JSON.parse(probe.result.value);
      const shot = await send("Page.captureScreenshot", { format: "png" });
      fs.writeFileSync(`D:/FluxTorrent/_theme_${theme === "baozi" ? "day" : "night"}.png`, Buffer.from(shot.data, "base64"));
    }

    // 焦点环：Tab
    await send("Page.navigate", { url: `${BASE}/login` });
    await new Promise(r => setTimeout(r, 2500));
    await send("Input.dispatchKeyEvent", { type: "keyDown", key: "Tab", windowsVirtualKeyCode: 9 });
    await new Promise(r => setTimeout(r, 400));
    const foc = await send("Runtime.evaluate", {
      expression: `(() => { const el = document.activeElement; const cs = getComputedStyle(el); return JSON.stringify({ tag: el.tagName, cls: (el.className||'').slice(0,40), outline: cs.outlineStyle + " " + cs.outlineWidth + " " + cs.outlineColor }); })()`,
      returnByValue: true,
    });
    out.focus = JSON.parse(foc.result.value);
    done();
  });

  console.log(JSON.stringify(out, null, 2));
  // 判定
  const ok = [];
  ok.push(["日间行底为浅色", (out.baozi.rowBg || "").includes("251")]);
  ok.push(["夜间行底为深色", /rgba\((2[0-9]|1[0-9]|0)/.test(out["baozi-night"].rowBg || "")]);
  ok.push(["夜间菜单钮深底", /rgba\((1|2)\d/.test(out["baozi-night"].menuBg || "")]);
  ok.push(["细滚动条生效", (out.baozi.scrollbarColor || "none none") !== "auto"]);
  ok.push(["focus-visible 环生效", (out.focus.outline || "").startsWith("solid")]);
  let pass = 0;
  for (const [n, c] of ok) { console.log((c ? "PASS " : "FAIL ") + n); if (c) pass++; }
  console.log(`===== 主题验证：${pass}/${ok.length} PASS =====`);
}

main().catch(e => { console.error("ERR", e.message); process.exit(1); });
