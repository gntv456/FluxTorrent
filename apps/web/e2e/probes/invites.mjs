// 登录态邀请页真渲染探针（配额/筛选/发信弹窗/撤销确认）
// 用法：node apps/web/e2e/probes/invites.mjs
// （原 apps/web/_tmp_invites_probe.mjs：一次性探针落在应用目录里会被行宽门禁拦）
import { chromium } from "@playwright/test";

const base = process.env.BASE ?? "http://localhost:3000";
const login = await fetch(`${base}/api/v1/auth/login`, {
  method: "POST",
  headers: { "Content-Type": "application/json" },
  body: JSON.stringify({ username: "root", password: "password123" }),
});
const j = await login.json();
if (!j?.data?.token) {
  console.error("login failed", login.status, j?.message);
  process.exit(2);
}
const browser = await chromium.launch({
  headless: true,
  channel: "msedge",
});
const ctx = await browser.newContext({
  viewport: { width: 1440, height: 1000 },
});
await ctx.addCookies([
  { url: base, name: "flux_token", value: j.data.token },
  { url: base, name: "flux.session", value: "1" },
]);
const page = await ctx.newPage();
const errs = [];
page.on("console", (m) => {
  if (m.type() === "error") errs.push(m.text().slice(0, 200));
});
page.on("pageerror", (e) => errs.push("pageerror: " + e.message.slice(0, 200)));
const body = () =>
  page.evaluate(() => document.body.innerText.replace(/\s+/g, " "));

const resp = await page.goto(base + "/invites", {
  waitUntil: "load",
  timeout: 30000,
});
await page.waitForFunction(
  () => /我的邀请配额/.test(document.body.innerText),
  null,
  { timeout: 15000 },
);
await page.waitForTimeout(1200);
const txt = await body();
const header = txt.slice(0, 120);
console.log(JSON.stringify({ step: "load", http: resp.status(), header }));
const quota = txt.match(/我的邀请配额[^]{0,160}/)?.[0] ?? "NO-QUOTA";
console.log(JSON.stringify({ step: "quota", quota }));

// 筛选：已过期 → 再点取消
const expiredBtn = page
  .getByRole("button", { name: "已过期", exact: true })
  .first();
if (await expiredBtn.count()) {
  await expiredBtn.click();
  await page.waitForTimeout(1200);
  const t2 = await body();
  console.log(
    JSON.stringify({
      step: "filter",
      banner: t2.match(/筛选中：[^ ]+/)?.[0] ?? t2.slice(0, 80),
    }),
  );
  await expiredBtn.click();
  await page.waitForTimeout(800);
  console.log(
    JSON.stringify({
      step: "unfilter",
      banner: /筛选中/.test(await body()) ? "STILL-FILTERED" : "cleared",
    }),
  );
}

// 发信弹窗（若列表有未用邀请）
const sendBtn = page
  .getByRole("button", { name: "发送邀请", exact: true })
  .first();
if (await sendBtn.count()) {
  await sendBtn.click({ timeout: 5000 }).catch(() => {});
  await page.waitForTimeout(500);
  const t3 = await body();
  const note = t3.match(/邀请链接将发送到对方邮箱[^]*?）/) ?.[0] ?? "NO-DIALOG";
  console.log(
    JSON.stringify({
      step: "email-dialog",
      note: note.slice(0, 160),
      has72: /72 小时/.test(t3),
    }),
  );
  await page
    .getByRole("button", { name: "取消", exact: true })
    .first()
    .click()
    .catch(() => {});
  await page.waitForTimeout(300);
} else {
  console.log(
    JSON.stringify({ step: "email-dialog", skipped: "no unused invite" }),
  );
}

// 撤销确认弹窗（只看不确认）
const revokeBtn = page
  .getByRole("button", { name: "撤销", exact: true })
  .first();
if (await revokeBtn.count()) {
  await revokeBtn.click({ timeout: 5000 }).catch(() => {});
  await page.waitForTimeout(400);
  const t4 = await body();
  console.log(
    JSON.stringify({ step: "revoke-confirm", shown: /不返还配额/.test(t4) }),
  );
  await page
    .getByRole("button", { name: "取消", exact: true })
    .first()
    .click()
    .catch(() => {});
  await page.waitForTimeout(300);
}
console.log(JSON.stringify({ step: "console", errs: [...new Set(errs)] }));
await browser.close();
