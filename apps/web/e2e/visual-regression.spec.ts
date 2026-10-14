import { test, expect } from "@playwright/test";

/**
 * 视觉回归（DS-PLAN-01 W-09）：P0 十页 × 明暗双主题截图基线对比。
 *
 * - 基线首次生成：pnpm test:visual:update（有意为之才更新，勿随手刷）；
 * - 日常运行：pnpm test:visual（PR 改 UI 必须过对比；先人工审 diff，
 *   跑顺后升 CI 必过，见 DS §5.3）；
 * - reducedMotion: "reduce" 让本站动效全局短路（base.css 末尾
 *   !important 块），截图无动画中间态；
 * - 动态内容（time 元素/数据属性标记）用 mask 遮罩，避免假阳性。
 *
 * 前置：本地栈在跑（web 3000），账号 root/password123（同 smoke.spec.ts）。
 */

const PAGES = [
  { name: "torrents", path: "/torrents" },
  { name: "torrent-detail", path: "/torrent/40" },
  { name: "upload", path: "/upload" },
  { name: "messages", path: "/messages" },
  { name: "forums", path: "/forums" },
  { name: "forum-topic", path: "/forums/topic/13" },
  { name: "search", path: "/torrents?search=test" },
  { name: "me", path: "/users/1" },
  { name: "top", path: "/top" },
  { name: "notifications", path: "/notifications" },
] as const;

/** 登录 API 取 JWT：绕过 UI 登录的水合竞态（workers=1 连跑 20 次，
 *  个别用例在 React 挂载前点到 SSR 按钮 → 点击无效不跳转；API 日志
 *  证实 login 全部 200，是客户端侧竞态）。双 cookie 口径同
 *  `e2e/lhci-login-token.cjs`：`flux_token`=JWT（API 鉴权）+
 *  `flux.session`=1（middleware 页面准入，middleware.ts:41）。
 *  凭证同 smoke.spec.ts：root / password123。
 */
async function sessionToken(base: string): Promise<string> {
  if (cachedToken) return cachedToken;
  const res = await fetch(`${base}/api/v1/auth/login`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ username: "root", password: "password123" }),
  });
  const j = (await res.json().catch(() => null)) as {
    code?: number;
    data?: { token?: string };
    message?: string;
  } | null;
  if (!j || j.code !== 0 || !j.data?.token) {
    throw new Error(`login failed: ${j?.message ?? res.status}`);
  }
  cachedToken = j.data.token;
  return cachedToken;
}

// 主题经 localStorage 预置（ThemeNoFlash 首屏前读），双主题各拍一张。
// 语言同理钉住 flux.locale=zh-CN：E2E_CHANNEL=msedge 的系统语言是
// en-US，缺省回退英文界面后 `/^登录$/` 定位不到按钮（2026-10-14
// 首跑 20/20 超时的根因），显式预置与 smoke.spec.ts 同语的中文。
//
// 限流坑（2026-10-14 基线生成实录）：login 5 次/分/用户名（auth_http
// 限流），20 用例 beforeEach 各登一次 ⇒ 必撞 429「操作太频繁」。
// 修法：模块级取一次 JWT，全部用例共用（会话 TTL 24h 远超跑批时长）。
let cachedToken: string | null = null;
for (const theme of ["brand", "brand-night"] as const) {
  test.describe(`视觉基线 · ${theme}`, () => {
    test.use({
      viewport: { width: 1280, height: 800 },
      reducedMotion: "reduce",
    });

    // 凭证与语言都必须在**请求发出前**就已在 cookie 里：
    //  1) addInitScript 里的 document.cookie 要等文档开载后才落，首个请求
    //     不带它 ⇒ SSR 照样按英文渲染（三轮 20/20 失败的根因之一）；
    //  2) UI 登录有水合竞态（点击先于 React 挂载 → 不跳转）。
    // 故直接登录 API 换 JWT（**整个跑批只登一次**，避开 5/分限流），
    // 再经 addCookies 给每个 context 戴好三件套。
    test.beforeEach(async ({ context, baseURL }) => {
      const base = baseURL ?? "http://localhost:3000";
      const host = new URL(base).hostname;
      cachedToken ??= await sessionToken(base);
      await context.addCookies([
        { name: "flux_token", value: cachedToken, domain: host, path: "/" },
        { name: "flux.session", value: "1", domain: host, path: "/" },
        { name: "flux.locale", value: "zh-CN", domain: host, path: "/" },
      ]);
    });

    for (const p of PAGES) {
      test(`${p.name}`, async ({ page }) => {
        await page.addInitScript((t) => {
          localStorage.setItem("flux-theme", t);
        }, theme);
        await page.goto(p.path);
        await page.waitForLoadState("networkidle");
        // 等一拍让 RSC 流式内容落定，避免拍到骨架中间态
        await page.waitForTimeout(300);
        await expect(page.locator("main#main-content")).toBeVisible({
          // 容器刚重建时首访是冷缓存（RSC 流式 + 无页面缓存），5s 会误判
          timeout: 15_000,
        });
        // 快照名必须带主题：明暗两轮同写 `${p.name}.png` 会互相覆盖——
        // 首轮入库后明色用例全拿暗色基线比（实测 97-98% 像素 diff）。
        await expect(page.locator("main#main-content")).toHaveScreenshot(
          `${theme}-${p.name}.png`,
          {
            // 相对时间/动态计数类元素遮罩（无 testid 时按语义标签兜底）
            mask: [
              page.locator("main time"),
              page.locator("main [data-volatile]"),
            ],
            maxDiffPixelRatio: 0.01,
          },
        );
      });
    }
  });
}
