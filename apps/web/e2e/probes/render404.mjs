// headless Chromium 真渲染 404 边界探针（rAF 正常的环境）
// 用法：node apps/web/e2e/probes/render404.mjs
// （原 apps/web/_tmp_render_probe.mjs：一次性探针落在应用目录里会被行宽门禁拦）
import { chromium } from "@playwright/test";

const base = process.env.BASE ?? "http://localhost:3000";
const pass = process.env.PASS ?? "password123";
const login = await fetch(`${base}/api/v1/auth/login`, {
  method: "POST",
  headers: { "Content-Type": "application/json" },
  body: JSON.stringify({ username: "root", password: pass }),
});
const j = await login.json();
if (!j?.data?.token) {
  console.error("login failed:", login.status, j?.message);
  process.exit(2);
}
const browser = await chromium.launch({
  headless: true,
  channel: "msedge",
});
const ctx = await browser.newContext({
  viewport: { width: 1440, height: 900 },
});
await ctx.addCookies([
  { name: "flux_token", value: j.data.token, domain: "localhost", path: "/" },
  { name: "flux.session", value: "1", domain: "localhost", path: "/" },
]);
const page = await ctx.newPage();
const errs = [];
page.on("console", (m) => {
  if (m.type() === "error") errs.push(m.text().slice(0, 160));
});
page.on("pageerror", (e) => errs.push("pageerror: " + e.message.slice(0, 160)));

const paths = [
  "/invites",
  "/users/6858",
  "/torrent/999999",
  "/boards/999999",
];
for (const path of paths) {
  errs.length = 0;
  const resp = await page
    .goto(base + path, { waitUntil: "load", timeout: 30000 })
    .catch((e) => {
      console.log(path, "GOTO-ERR", e.message);
      return null;
    });
  if (!resp) continue;
  let verdict = "timeout";
  if (path === "/invites") {
    verdict = await page
      .waitForFunction(
        () => /邀请码|配额/.test(document.body.innerText),
        null,
        { timeout: 15000 },
      )
      .then(() => "rendered")
      .catch(() => "timeout");
  } else {
    verdict = await page
      .waitForFunction(
        () =>
          document.body.innerText.includes("404 · 页面不存在") ||
          document.body.innerText.includes("页面出错了"),
        null,
        { timeout: 15000 },
      )
      .then(() => "rendered")
      .catch(() => "timeout");
  }
  const text = (await page.evaluate(() => document.body.innerText))
    .replace(/\s+/g, " ")
    .slice(0, 220);
  const skel = await page.evaluate(
    () =>
      document.querySelectorAll(
        ".skeleton, [class*='skeleton'], [class*='animate-pulse']",
      ).length,
  );
  console.log(
    JSON.stringify({
      path,
      http: resp.status(),
      verdict,
      skeletonNodes: skel,
      text,
      errs: [...errs],
    }),
  );
}
await browser.close();
