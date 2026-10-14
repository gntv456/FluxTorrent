// Lighthouse CI 登录态：登录 API 取 JWT，打印到 stdout（W-08）。
//
// 演进记录（2026-10-13，两个此路不通的方案）：
//   ① puppeteerScript 登录：lhci 把 Lighthouse 作为独立子进程跑，
//      puppeteer 的 cookie 罐与 Lighthouse 自起 Chrome 不共享；
//   ② 预置 Chrome profile + --user-data-dir：Chrome 取**第一个**
//      --user-data-dir，chrome-launcher 的临时目录先入 flags，
//      种子 profile 被忽略（实测 cookie 不落）。
// 可行方案：Lighthouse 的 extraHeaders 设置经 CDP
// Network.setExtraHTTPHeaders 给**所有请求**（含文档导航）戴 Cookie。
// 本站双 cookie 口径：flux_token=JWT（API 鉴权）+ flux.session=1
// （Next middleware 页面准入，见 middleware.ts）。
//
// 凭证走环境变量（勿提交）：LHC_USER（默认 root）/ LHC_PASS。
// 登录 API 形态：POST /api/v1/auth/login，JSON {username, password}，
// 成功信封 data.token 即 JWT（HS256，24h）。

const BASE = process.env.LHC_BASE_URL ?? "http://localhost:3000";

async function main() {
  const user = process.env.LHC_USER ?? "root";
  const pass = process.env.LHC_PASS ?? "";
  if (!pass) {
    console.error("[lhci-login-token] 未设 LHC_PASS");
    process.exit(2);
  }
  const res = await fetch(`${BASE}/api/v1/auth/login`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ username: user, password: pass }),
  });
  const j = await res.json().catch(() => null);
  if (!j || j.code !== 0 || !j.data?.token) {
    console.error(`[lhci-login-token] 登录失败：${j?.message ?? res.status}`);
    process.exit(1);
  }
  // 只打印 token 本体（配置层拼 Cookie 头）
  process.stdout.write(j.data.token);
}

main();
