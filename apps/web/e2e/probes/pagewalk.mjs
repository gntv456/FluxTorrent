// 登录态核心页面走廊探针（渲染层 + API 失败面）
// 用法：node apps/web/e2e/probes/pagewalk.mjs
// （原 apps/web/_tmp_pagewalk.mjs：一次性探针落在应用目录里会被行宽门禁拦，
//   挪到 e2e/probes/ 后 import 仍走 apps/web/node_modules 的 playwright）
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
let badApis = [];
let consoleErrs = [];
// 头像探测文件 example.invalid 是站点现有配置，忽略其 DNS 失败噪音
const noise = (t) => /ERR_NAME_NOT_RESOLVED|example\.invalid/.test(t);
page.on("response", (r) => {
  const u = r.url();
  if (u.includes("/api/v1/") && r.status() >= 400) {
    badApis.push(`${r.status()} ${u.replace(base, "")}`);
  }
});
page.on("console", (m) => {
  if (m.type() === "error" && !noise(m.text())) {
    consoleErrs.push(m.text().slice(0, 160));
  }
});
page.on("pageerror", (e) => {
  if (!noise(e.message)) {
    consoleErrs.push("pageerror: " + e.message.slice(0, 160));
  }
});

const routes = [
  "/",
  "/torrents",
  "/torrent/1",
  "/top",
  "/forums",
  "/messages",
  "/notifications",
  "/me",
  "/my",
  "/myhr",
  "/shop",
  "/bank",
  "/gacha",
  "/farm",
  "/games",
  "/tasks",
  "/requests",
  "/subtitles",
  "/teams",
  "/medal-wall",
  "/users",
  "/users/1",
  "/upload",
  "/staffbox",
];
for (const path of routes) {
  badApis = [];
  consoleErrs = [];
  let http = 0;
  try {
    const resp = await page.goto(base + path, {
      waitUntil: "load",
      timeout: 25000,
    });
    http = resp?.status() ?? 0;
  } catch (e) {
    console.log(JSON.stringify({ path, error: e.message.slice(0, 100) }));
    continue;
  }
  await page.waitForTimeout(1800);
  const info = await page.evaluate(() => {
    const txt = document.body.innerText.replace(/\s+/g, " ");
    return {
      len: txt.length,
      skel: document.querySelectorAll(
        "[class*='skeleton'], .skeleton, [class*='animate-pulse']",
      ).length,
      errPage: /页面出错了|Something went wrong/.test(txt),
      head: txt.slice(0, 90),
    };
  });
  console.log(
    JSON.stringify({
      path,
      http,
      ...info,
      badApis: [...new Set(badApis)],
      errs: [...new Set(consoleErrs)],
    }),
  );
}
await browser.close();
