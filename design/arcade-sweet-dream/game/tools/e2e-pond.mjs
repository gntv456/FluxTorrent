import { chromium } from "file:///D:/FluxTorrent/node_modules/.pnpm/playwright@1.63.0/node_modules/playwright/index.mjs";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const shots = path.resolve(here, "../_shots");
// 需真实登录态：FT_TOKEN=<flux_token> 或作为第 1 个参数传入 token 文件路径
const token = (process.env.FT_TOKEN || (process.argv[2] ? fs.readFileSync(process.argv[2], "utf8") : "")).trim();
if (!token) throw new Error("缺少 FT_TOKEN（真实登录 cookie flux_token）");
const EDGE = "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe";

const browser = await chromium.launch({ executablePath: EDGE, headless: true });
const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 1 });
await ctx.addCookies([
  { name: "flux_token", value: token, url: "http://localhost:3001" },
  { name: "flux.session", value: "1", url: "http://localhost:3001" },
]);
const page = await ctx.newPage();
page.on("response", (r) => {
  const u = r.url();
  if (u.includes("/api/v1/games/fishing/pond")) console.log("API", r.status(), r.request().method(), u.replace("http://localhost:3001", ""));
});
page.on("pageerror", (e) => console.log("pageerror:", e.message));

const ev = (body) => page.evaluate(body);
const inShadow = (body) => page.evaluate(`(() => {
  const r = document.querySelector('div[aria-label="甜梦钓场完整渔场"]').shadowRoot;
  ${body}
})()`);

await page.goto("http://localhost:3001/games/fishing/pond", { waitUntil: "domcontentloaded" });
await page.waitForFunction(() => {
  const h = document.querySelector('div[aria-label="甜梦钓场完整渔场"]');
  return h && h.shadowRoot && h.shadowRoot.querySelector("#pondBubble");
}, { timeout: 30000 });
await page.waitForTimeout(2500);

const hud = await inShadow(`
  return {
    magic: r.querySelector("#hudMagic b")?.textContent || "",
    bubble: r.querySelector("#pondBubble")?.textContent.trim() || "",
    cast: !!r.querySelector('[data-act="cast"]'),
    tabbar: r.querySelector(".tabbar")?.children.length || 0
  };`);
console.log("HUD:", JSON.stringify(hud));
await page.screenshot({ path: path.join(shots, "pond-server-1440.png") });

// 抛竿 → 咬钩 → 起竿
await inShadow(`r.querySelector('[data-act="cast"]').click(); return true;`);
await page.waitForFunction(() => {
  const r = document.querySelector('div[aria-label="甜梦钓场完整渔场"]').shadowRoot;
  return (r.querySelector('#pondBubble')?.textContent || "").includes("上钩");
}, { timeout: 9000 });
await inShadow(`const b = r.querySelector('.cta[data-act="hook"]') || r.querySelector('.bite-btn[data-act="hook"]'); b.click(); return true;`);
await page.waitForTimeout(700);
const reel = await inShadow(`return !!r.querySelector("#reelRoot")`);
console.log("reel shown:", reel);
if (reel) await page.screenshot({ path: path.join(shots, "pond-server-reel.png") });

// 按住 / 松开循环收线
const btn = page.locator("#holdBtn");
for (let i = 0; i < 80; i++) {
  if ((await btn.count()) === 0) break;
  const box = await btn.boundingBox();
  if (!box) break;
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.down();
  await page.waitForTimeout(240);
  await page.mouse.up();
  await page.waitForTimeout(150);
}
await page.waitForTimeout(1200);
const after = await inShadow(`
  const sheet = r.querySelector(".sheet.on");
  return {
    bubble: r.querySelector("#pondBubble")?.textContent.trim() || "",
    reel: !!r.querySelector("#reelRoot"),
    sheet: sheet ? sheet.textContent.replace(/\\s+/g, " ").trim().slice(0, 180) : null,
    magic: r.querySelector("#hudMagic b")?.textContent || ""
  };`);
console.log("AFTER:", JSON.stringify(after));
await page.screenshot({ path: path.join(shots, "pond-server-settle.png") });

// 真实赛季榜：新开一页（避免结算弹层遮挡），右侧快捷入口 data-view=rank
const p2 = await ctx.newPage();
await p2.goto("http://localhost:3001/games/fishing/pond", { waitUntil: "domcontentloaded" });
await p2.waitForFunction(() => {
  const h = document.querySelector('div[aria-label="甜梦钓场完整渔场"]');
  return h && h.shadowRoot && h.shadowRoot.querySelector("#pondBubble");
}, { timeout: 30000 });
await p2.waitForTimeout(1800);
await p2.evaluate(() => {
  const r = document.querySelector('div[aria-label="甜梦钓场完整渔场"]').shadowRoot;
  const q = r.querySelector('[data-view="rank"]'); if (q) q.click();
});
await p2.waitForTimeout(1500);
const rank = await p2.evaluate(() => {
  const r = document.querySelector('div[aria-label="甜梦钓场完整渔场"]').shadowRoot;
  return {
    rows: [...r.querySelectorAll(".rank-row")].map(n => n.textContent.replace(/\\s+/g," ").trim()),
    summary: (r.querySelector(".rank-summary")?.textContent || "").replace(/\\s+/g," ").trim()
  };
});
console.log("RANK:", JSON.stringify(rank.rows), "|", rank.summary);
await p2.screenshot({ path: path.join(shots, "pond-server-rank.png") });

// 商店：确认兑换入口已移除，全部商品直接花站点魔力
await p2.evaluate(() => {
  const r = document.querySelector('div[aria-label="甜梦钓场完整渔场"]').shadowRoot;
  const tab = r.querySelector('[data-tab="shop"]'); if (tab) tab.click();
});
await p2.waitForTimeout(600);
const shop = await p2.evaluate(() => {
  const r = document.querySelector('div[aria-label="甜梦钓场完整渔场"]').shadowRoot;
  return {
    tabs: [...r.querySelectorAll('.stab')].map(n => n.textContent.trim().replace(/\s+/g, '')),
    hasExchange: !!r.querySelector('[data-cat="exchange"]'),
    hint: (r.querySelector('.ph-s')?.textContent || '').trim(),
  };
});
console.log("SHOP:", JSON.stringify(shop));
await p2.screenshot({ path: path.join(shots, "pond-server-shop.png") });

// 手机断点：同一服务端存档，验证单列布局
await p2.setViewportSize({ width: 390, height: 844 });
await p2.reload({ waitUntil: "domcontentloaded" });
await p2.waitForTimeout(300);
await p2.waitForFunction(() => {
  const h = document.querySelector('div[aria-label="甜梦钓场完整渔场"]');
  return h && h.shadowRoot && h.shadowRoot.querySelector("#pondBubble");
}, { timeout: 30000 });
await p2.waitForTimeout(2000);
await p2.screenshot({ path: path.join(shots, "pond-server-390.png") });
console.log("mobile shot ok");

await browser.close();
