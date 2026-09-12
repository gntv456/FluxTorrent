// 管理面板 UI 抽检：/admin 12 tab + /admin/settings + staff-tools 30 工具 tab 逐个加载验证。
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

// staff-tools 30 个工具 tab 的 query 参数（从 ToolTab 类型枚举）
const TOOLS = ["faq","rules","cats","bans","mail","promo","staffmess","adduser","bonus","warned",
  "ipcheck","maxlogin","upload","resetpass","deldisabled","emailbans","testip","stats",
  "cleanup","ads","notconnect","uploaders","agents","polls","dbstats","syslog","locations",
  "hrpardon","plugins","agentrules"];
// admin 12 tab 的 query 参数
const TABS = ["panel","overview","reviews","reports","appeals","users","torrents","p2tools","audit","cheaters","content","tools"];

async function main() {
  execSync('curl -s -X PUT "http://127.0.0.1:9222/json/new?about:blank" > NUL', { shell: "cmd.exe" });
  await new Promise(r => setTimeout(r, 800));
  const list = JSON.parse(execSync('curl -s http://127.0.0.1:9222/json', { shell: "cmd.exe" }).toString());
  const page = list.reverse().find(t => t.type === "page" && t.url === "about:blank");
  const out = { tabs: {}, tools: {}, settings: null };
  let pass = 0, total = 0;

  await cdp(page.webSocketDebuggerUrl, async (send, done) => {
    await send("Runtime.enable");
    await send("Page.enable");
    await send("Network.enable");
    await send("Network.setCookie", { name: "flux.session", value: "1", domain: "127.0.0.1", path: "/" });
    await send("Network.setCookie", { name: "flux.token", value: TOK, domain: "127.0.0.1", path: "/" });
    await send("Page.navigate", { url: `${BASE}/login` });
    await new Promise(r => setTimeout(r, 2500));
    await send("Runtime.evaluate", { expression: `localStorage.setItem("flux.token", ${JSON.stringify(TOK)})` });

    const probeTab = async (url, key, store) => {
      await send("Page.navigate", { url });
      await new Promise(r => setTimeout(r, 2600));
      const p = await send("Runtime.evaluate", {
        expression: `(() => { try {
          const main = document.querySelector("main");
          const text = main ? main.textContent : "";
          const hasData = !!(text && text.trim().length > 40);
          const hasError = /加载失败|network error|服务异常/.test(text);
          return JSON.stringify({ ok: hasData && !hasError, len: text.trim().length, hasError });
        } catch(e) { return JSON.stringify({ok:false, err:String(e)}); } })()`,
        returnByValue: true,
      });
      store[key] = JSON.parse(p.result.value);
    };

    // admin 12 tab
    for (const t of TABS) {
      await probeTab(`${BASE}/admin?tab=${t}`, t, out.tabs);
    }
    // settings 页
    await probeTab(`${BASE}/admin/settings`, "settings", out);
    // staff-tools 30 工具（admin?tool=xxx 路由进 tools tab 的子工具）
    for (const t of TOOLS) {
      await probeTab(`${BASE}/admin?tool=${t}`, t, out.tools);
    }
    done();
  });

  const checks = [];
  for (const t of TABS) checks.push([`tab ${t}`, out.tabs[t] && out.tabs[t].ok]);
  checks.push(["settings 页", out.settings && out.settings.ok]);
  for (const t of TOOLS) checks.push([`tool ${t}`, out.tools[t] && out.tools[t].ok]);
  for (const [n, c] of checks) { console.log((c ? "PASS " : "FAIL ") + n + (!c ? ` ${JSON.stringify((out.tabs[n]||out.tools[n]||{}))}` : "")); if (c) pass++; }
  console.log(`===== 管理面板 UI：${pass}/${checks.length} PASS =====`);
}

main().catch(e => { console.error("ERR", e.message); process.exit(1); });
