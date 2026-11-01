// 认证页移动端收口验收（M5.1）：390px 实测横滚/主按钮高度/JS 异常。
//
// 用法（仓库根）：
//   node apps/web/_chk_auth_mobile.mjs
// 覆盖：/login /register /forgot /reset /apply /email-verify /self-unban
// （游客态即可，无需登录 cookie）。截图落 _tmp_auth_mobile/。
import { chromium } from "@playwright/test";

const BASE = process.env.FLUX_BASE ?? "http://localhost:3000";
const SHOTS = process.env.FLUX_SHOTS ?? "_tmp_auth_mobile";
const TOKEN = process.env.FLUX_TOKEN ?? "";
const PAGES = (process.env.FLUX_PAGES ??
  "/login,/register,/forgot,/reset,/apply,/email-verify,/self-unban")
  .split(",")
  .map((s) => s.trim())
  .filter(Boolean);
const VIEW = { width: 390, height: 844 };

const browser = await chromium.launch({ headless: true, channel: "msedge" });
const results = [];

for (const path of PAGES) {
  const ctx = await browser.newContext({ viewport: VIEW });
  // 需要登录态的页面（FLUX_TOKEN 存在时注入三件套；url 字段才生效）
  if (TOKEN) {
    await ctx.addCookies([
      { name: "flux_token", value: TOKEN, url: BASE },
      { name: "flux.session", value: "1", url: BASE },
      { name: "flux.locale", value: "zh-CN", url: BASE },
    ]);
  }
  const page = await ctx.newPage();
  const errors = [];
  page.on("pageerror", (e) => errors.push(String(e).slice(0, 120)));
  page.on("console", (m) => {
    if (m.type() === "error") errors.push(m.text().slice(0, 120));
  });

  let http = 0;
  try {
    const resp = await page.goto(BASE + path, { waitUntil: "load",
      timeout: 20000 });
    http = resp ? resp.status() : 0;
  } catch (e) {
    errors.push("GOTO " + String(e).slice(0, 80));
  }
  await page.waitForTimeout(600);

  const m = await page.evaluate(() => {
    const vw = window.innerWidth;
    const doc = document.documentElement;
    const wide = [];
    for (const el of document.querySelectorAll("body *")) {
      const r = el.getBoundingClientRect();
      if (r.width > 0 && r.right > vw + 1 && wide.length < 4) {
        const cls = String(el.className).slice(0, 60);
        const w = Math.round(r.width);
        const right = Math.round(r.right);
        wide.push(`${el.tagName.toLowerCase()}.${cls} w=${w} r=${right}`);
      }
    }
    const sel = 'button[type="submit"], button:not([type])';
    const btns = [...document.querySelectorAll(sel)]
      .filter((b) =>
        /登录|登入|注册|发送|重置|提交|login|register/i.test(
          b.textContent || "",
        ))
      .map((b) => {
        const r = b.getBoundingClientRect();
        return { text: (b.textContent || "").trim().slice(0, 12),
          h: Math.round(r.height), w: Math.round(r.width) };
      });
    return { vw, vh: window.innerHeight, sw: doc.scrollWidth,
      docH: doc.scrollHeight, bw: document.body.scrollWidth, wide, btns };
  });

  const name = path.replace(/[^a-zA-Z0-9]+/g, "_");
  await page.screenshot({ path: `${SHOTS}/v390${name}.png`,
    fullPage: true });
  results.push({ path, http, ...m, errors });
  await ctx.close();
}
await browser.close();

let bad = 0;
for (const r of results) {
  const hs = r.sw > r.vw + 1;
  // 404/410 = 动态路由无内容（非布局问题），跳过；横滚与 JS 异常才是目标
  const missing = r.http === 404 || r.http === 410;
  const btn = r.btns.find((b) => b.h >= 44);
  // 手机版登录页重设计（2026-11-01）的硬判据：**整页不产生纵向滚动**。
  // 旧版在 390×844 下整页 1189px（溢出 345px：Passkey / 开放注册横幅 /
  // 四行辅助链接 / 安全告知全在折叠线外），新壳是「品牌带 + 贴底抽屉」，
  // 一屏完成登录是设计目标而非观感偏好。
  // 判据选「整页高度」而不是「主按钮在不在首屏」——后者拿旧版一比就不
  // 承重（旧版登录钮底边 644 < 844，本来就看得见），钉它等于钉了个恒真。
  // 只让带主按钮的页参与判定：/reset 待邮件、/apply 未开放那类纯提示页
  // 没有可提交的表单，高度天然短，判它们没意义。
  const folded = !!btn && r.docH > r.vh + 1;
  const ok = missing || (!hs && !folded && r.errors.length === 0 &&
    r.http >= 200 && r.http < 400);
  if (!ok) bad += 1;
  console.log(`${ok ? (missing ? "−" : "✓") : "✗"} ${r.path}  http=${r.http} ` +
    `scrollW=${r.sw}/${r.vw}${hs ? " ⚠️横滚" : ""} ` +
    `btn44=${btn ? `${btn.h}px「${btn.text}」` : "无≥44px主按钮"} ` +
    `scrollH=${r.docH}/${r.vh}${folded ? " ⚠️超折叠线" : ""} ` +
    `err=${r.errors.length}`);
  for (const w of r.wide) console.log(`    超宽: ${w}`);
  for (const b of r.btns.slice(0, 3)) {
    if (!btn || b.h < 44) console.log(`    按钮: ${b.h}px×${b.w}px「${b.text}」`);
  }
  for (const e of r.errors.slice(0, 3)) console.log(`    err: ${e}`);
}
console.log(bad === 0 ? "\n✓ 全部通过" : `\n✗ ${bad}/${results.length} 页有问题`);
process.exit(bad === 0 ? 0 : 1);
