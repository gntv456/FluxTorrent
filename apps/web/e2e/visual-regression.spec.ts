import { test, expect, type Page } from "@playwright/test";

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

async function login(page: Page) {
  await page.goto("/login");
  await page
    .locator('input[autocomplete="username"], [placeholder="用户名"]')
    .first()
    .fill("root");
  await page
    .locator('input[type="password"], [placeholder="密码"]')
    .first()
    .fill("password123");
  await page.getByRole("button", { name: /^登录$/ }).first().click();
  await page.waitForURL((u) => !u.pathname.includes("login"), {
    timeout: 15_000,
  });
}

// 主题经 localStorage 预置（ThemeNoFlash 首屏前读），双主题各拍一张。
// 语言同理钉住 flux.locale=zh-CN：E2E_CHANNEL=msedge 的系统语言是
// en-US，缺省回退英文界面后 `/^登录$/` 定位不到按钮（2026-10-14
// 首跑 20/20 超时的根因），显式预置与 smoke.spec.ts 同语的中文。
for (const theme of ["brand", "brand-night"] as const) {
  test.describe(`视觉基线 · ${theme}`, () => {
    test.use({
      viewport: { width: 1280, height: 800 },
      reducedMotion: "reduce",
    });

    // 语言必须在**请求发出前**就已在 cookie 里：addInitScript 里的
    // document.cookie 要等文档开载后才落，首个 /login 请求不带它，
    // SSR 照样按英文渲染（2026-10-14 三轮 20/20 失败的真根因——
    // 首轮是缺 cookie，后两轮是 cookie 时机不对）。
    test.beforeEach(async ({ context }) => {
      await context.addCookies([
        { name: "flux.locale", value: "zh-CN", domain: "localhost", path: "/" },
      ]);
    });

    for (const p of PAGES) {
      test(`${p.name}`, async ({ page }) => {
        await page.addInitScript((t) => {
          localStorage.setItem("flux-theme", t);
        }, theme);
        await login(page);
        await page.goto(p.path);
        await page.waitForLoadState("networkidle");
        // 等一拍让 RSC 流式内容落定，避免拍到骨架中间态
        await page.waitForTimeout(300);
        await expect(page.locator("main#main-content")).toBeVisible();
        await expect(page.locator("main#main-content")).toHaveScreenshot(
          `${p.name}.png`,
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
