// Seedlight UI 换装验证：Aurora token 落地 + 导航收敛 + 双主题 + 移动底部 Tab。
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
  const page = list.find(t => t.type === "page" && t.url === "about:blank");
  const out = {};

  let page2 = null;
  await cdp(page.webSocketDebuggerUrl, async (send, done) => {
    await send("Runtime.enable");
    await send("Page.enable");
    await send("Network.enable");
    await send("Emulation.setDeviceMetricsOverride", { width: 1440, height: 900, deviceScaleFactor: 1, mobile: false });
    await send("Network.setCookie", { name: "flux.session", value: "1", domain: "127.0.0.1", path: "/" });
    await send("Network.setCookie", { name: "flux.token", value: TOK, domain: "127.0.0.1", path: "/" });
    await send("Page.navigate", { url: `${BASE}/login` });
    await new Promise(r => setTimeout(r, 2500));
    await send("Runtime.evaluate", { expression: `localStorage.setItem("flux.token", ${JSON.stringify(TOK)})` });

    for (const theme of ["baozi", "baozi-night"]) {
      await send("Page.navigate", { url: `${BASE}/torrents` });
      await new Promise(r => setTimeout(r, 3500));
      await send("Runtime.evaluate", { expression: `localStorage.setItem("flux-theme", ${JSON.stringify(theme)}); document.documentElement.dataset.theme = ${JSON.stringify(theme)};` });
      await new Promise(r => setTimeout(r, 800));
      const probe = await send("Runtime.evaluate", {
        expression: `(() => {
          const menu = document.querySelector(".mainmenu-link");
          const btn = document.querySelector(".btn-aurora");
          const fontLink = document.querySelector('link[href*="ZCOOL"]');
          // 展开「更多」下拉（React 合成事件需真实点击语义）
          const moreBtn = [...document.querySelectorAll(".mainmenu button")].pop();
          if (moreBtn) {
            moreBtn.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
          }
          return JSON.stringify({
            theme: document.documentElement.dataset.theme,
            menuCount: document.querySelectorAll(".mainmenu > li").length,
            moreBtnText: moreBtn ? moreBtn.textContent.trim() : null,
            auroraBtn: !!btn,
            zcoolLinked: !!fontLink,
            h1Font: (document.querySelector("h1") ? getComputedStyle(document.querySelector("h1")).fontFamily : "").slice(0, 40),
          });
        })()`,
        returnByValue: true,
      });
      out[theme] = JSON.parse(probe.result.value);
      // 下拉展开是 React 状态更新，点击后单独再取
      await new Promise(r => setTimeout(r, 500));
      const drop = await send("Runtime.evaluate", {
        expression: `(() => { try { return JSON.stringify({ groups: document.querySelectorAll(".mainmenu-drop__group").length, links: document.querySelectorAll(".mainmenu-drop__link").length, sky: getComputedStyle(document.documentElement).getPropertyValue("--sky").trim(), bodyBg: getComputedStyle(document.body).backgroundColor }); } catch(e) { return JSON.stringify({err: String(e)}); } })()`,
        returnByValue: true,
      });
      if (!drop || !drop.result || !drop.result.value) {
        out[theme].dropErr = JSON.stringify(drop).slice(0, 200);
      } else {
        Object.assign(out[theme], JSON.parse(drop.result.value));
      }
      const shot = await send("Page.captureScreenshot", { format: "png" });
      fs.writeFileSync(`D:/FluxTorrent/_seedlight_${theme === "baozi" ? "day" : "night"}.png`, Buffer.from(shot.data, "base64"));
    }

    // 移动端：底部 5 Tab + 凸起发布 —— 新开标签做干净会话（旧标签 evaluate 上下文偶尔滞留桌面态）
    execSync('curl -s -X PUT "http://127.0.0.1:9222/json/new?about:blank" > NUL', { shell: "cmd.exe" });
    await new Promise(r => setTimeout(r, 1000));
    const list2 = JSON.parse(execSync('curl -s http://127.0.0.1:9222/json', { shell: "cmd.exe" }).toString());
    page2 = list2.find(t => t.type === "page" && t.url === "about:blank" && t.webSocketDebuggerUrl !== page.webSocketDebuggerUrl);
    done();
  });

  if (!page2) throw new Error("no mobile tab");
  // 移动端独立标签页会话
  await cdp(page2.webSocketDebuggerUrl, async (send2, done2) => {
      await send2("Runtime.enable");
      await send2("Page.enable");
      await send2("Network.enable");
      await send2("Emulation.setDeviceMetricsOverride", { width: 390, height: 844, deviceScaleFactor: 2, mobile: true });
      await send2("Network.setCookie", { name: "flux.session", value: "1", domain: "127.0.0.1", path: "/" });
      await send2("Network.setCookie", { name: "flux.token", value: TOK, domain: "127.0.0.1", path: "/" });
      await send2("Page.navigate", { url: `${BASE}/torrents` });
      await new Promise(r => setTimeout(r, 4500));
    const mob = await send2("Runtime.evaluate", {
      expression: `(() => { try {
        // 兜底：任何 fixed 且贴底的 nav
        const navs = [...document.querySelectorAll("nav")].filter(n => { const cs = getComputedStyle(n); return cs.position === "fixed" && cs.bottom === "0px" && cs.display !== "none"; });
        const tabs = navs.flatMap(n => [...n.querySelectorAll("a")]);
        const center = tabs.find(a => /发布|Publish|發佈/.test(a.textContent));
        const cs = center ? getComputedStyle(center.querySelector("span")) : null;
        return JSON.stringify({ navCount: navs.length, tabCount: tabs.length, centerBtn: !!center, centerBg: cs ? (cs.backgroundImage !== "none" ? cs.backgroundImage : cs.backgroundColor) : null,
          centerLift: center ? getComputedStyle(center).marginTop : null, innerW: window.innerWidth });
      } catch(e) { return JSON.stringify({err: String(e)}); } })()`,
      returnByValue: true,
    });
    out.mobile = mob && mob.result && mob.result.value ? JSON.parse(mob.result.value) : { err: "no result" };
    const shot = await send2("Page.captureScreenshot", { format: "png" });
    fs.writeFileSync("D:/FluxTorrent/_seedlight_mobile.png", Buffer.from(shot.data, "base64"));
    done2();
  });

  console.log(JSON.stringify(out, null, 2));
  const checks = [
    ["日间 sky=2FA8FF", (out.baozi.sky || "").toUpperCase().includes("2FA8FF")],
    ["夜间 sky 提亮(47B3FF)", (out["baozi-night"].sky || "").toUpperCase().includes("47B3FF")],
    ["桌面一级导航收敛 ≤6", out.baozi.menuCount <= 6],
    ["更多下拉分组 ≥4", out.baozi.groups >= 4],
    ["下拉链接 ≥20", out.baozi.links >= 20],
    ["极光发布按钮存在", out.baozi.auroraBtn],
    ["ZCOOL 字体已挂载", out.baozi.zcoolLinked],
    ["移动 5 Tab", out.mobile.tabCount === 5],
    ["中间发布凸起", !!out.mobile.centerBtn && out.mobile.centerLift !== "0px"],
    ["凸起钮极光渐变", (out.mobile.centerBg || "").includes("gradient")],
  ];
  let pass = 0;
  for (const [n, c] of checks) { console.log((c ? "PASS " : "FAIL ") + n); if (c) pass++; }
  console.log(`===== Seedlight 验证：${pass}/${checks.length} PASS =====`);
}

main().catch(e => { console.error("ERR", e.message); process.exit(1); });
