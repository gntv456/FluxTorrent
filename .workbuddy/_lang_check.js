// 验证 usercp 站点语言切换生效：保存后整站语言跟随（cookie 同步 + RSC 重取）。
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

    // 1) 进控制面板（网站设定 tab）—— select 是客户端 fetch 后渲染，多等两拍
    await send("Page.navigate", { url: `${BASE}/my?tab=tracker` });
    await new Promise(r => setTimeout(r, 4500));
    let selFound = false, beforeProbe = null;
    for (let i = 0; i < 6 && !selFound; i++) {
      const p = await send("Runtime.evaluate", {
        expression: `(() => { const sel = [...document.querySelectorAll("select")].find(s => ["en","chs","cht"].includes(s.value)); return sel ? sel.value : null; })()`,
        returnByValue: true,
      });
      if (p.result.value) { selFound = true; beforeProbe = p.result.value; }
      else await new Promise(r => setTimeout(r, 1000));
    }
    out.before = { found: selFound, lang: beforeProbe, cookie: "" };

    // 2) 切到 English 并点保存
    const switched = await send("Runtime.evaluate", {
      expression: `(() => { try {
        const sel = [...document.querySelectorAll("select")].find(s => ["en","chs","cht"].includes(s.value));
        if (!sel) return JSON.stringify({err: "no lang select"});
        const setter = Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, "value").set;
        setter.call(sel, "en");
        sel.dispatchEvent(new Event("change", { bubbles: true }));
        return JSON.stringify({ ok: true, now: sel.value });
      } catch(e) { return JSON.stringify({err: String(e)}); } })()`,
      returnByValue: true,
    });
    out.switch = JSON.parse(switched.result.value);
    await new Promise(r => setTimeout(r, 400));
    const clicked = await send("Runtime.evaluate", {
      expression: `(() => { try {
        const btn = [...document.querySelectorAll('input[type="submit"], button')].find(b => /保存|儲存|Save/.test(b.value || b.textContent));
        if (!btn) return JSON.stringify({err: "no save btn"});
        btn.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
        return JSON.stringify({ ok: true });
      } catch(e) { return JSON.stringify({err: String(e)}); } })()`,
      returnByValue: true,
    });
    out.click = JSON.parse(clicked.result.value);
    await new Promise(r => setTimeout(r, 2500));

    // 3) cookie 同步 + 整站语言 —— refresh 是 RSC 重取，多等几拍再读导航文案
    let after = null;
    for (let i = 0; i < 8; i++) {
      await new Promise(r => setTimeout(r, 900));
      const p = await send("Runtime.evaluate", {
        expression: `(() => { try {
          const m = document.cookie.match(/flux.locale=([^;]+)/);
          const home = [...document.querySelectorAll(".mainmenu-link")].map(a => a.textContent.trim()).slice(0, 4);
          return JSON.stringify({ localeCookie: m ? decodeURIComponent(m[1]) : null, nav: home, htmlLang: document.documentElement.lang });
        } catch(e) { return JSON.stringify({err: String(e)}); } })()`,
        returnByValue: true,
      });
      after = JSON.parse(p.result.value);
      if (after.localeCookie === "en" && /Home|Library|Forum/.test((after.nav || []).join(","))) break;
    }
    out.after = after;
    done();
  });

  // 4) 后端库值核对
  const db = execSync('docker exec flux-postgres psql -U flux -d fluxtorrent -t -A -c "SELECT site_language FROM users WHERE id=1"', { shell: "cmd.exe" }).toString().trim();
  out.dbLang = db;

  console.log(JSON.stringify(out, null, 2));
  const checks = [
    ["初始语言下拉存在", out.before.found],
    ["切到 en 成功", out.switch.ok === true],
    ["保存按钮已点", out.click.ok === true],
    ["locale cookie 已同步为 en", out.after.localeCookie === "en"],
    ["整站导航已英文", (out.after.nav || []).some(x => /Home|Library|Forum/.test(x))],
    ["html lang=en", out.after.htmlLang === "en"],
    ["库 site_language=en", out.dbLang === "en"],
  ];
  let pass = 0;
  for (const [n, c] of checks) { console.log((c ? "PASS " : "FAIL ") + n); if (c) pass++; }
  console.log(`===== 语言切换验证：${pass}/${checks.length} PASS =====`);

  // 5) 复原：库和 cookie 改回 chs
  execSync('docker exec flux-postgres psql -U flux -d fluxtorrent -t -A -c "UPDATE users SET site_language=\'chs\' WHERE id=1" > NUL', { shell: "cmd.exe" });
  console.log("(已复原库值为 chs)");
}

main().catch(e => { console.error("ERR", e.message); process.exit(1); });
