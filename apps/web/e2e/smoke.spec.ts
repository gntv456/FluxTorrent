import { test, expect } from "@playwright/test";

/**
 * 登录 + 三页冒烟（E14 首套）。
 * 登录态断言标准：跳转后 body 不再含登录表单（登录成功的可观察信号）。
 */
async function login(page: import("@playwright/test").Page) {
  await page.goto("/login");
  await page
    .locator('input[autocomplete="username"], [placeholder="用户名"]')
    .first()
    .fill("root");
  await page
    .locator('input[type="password"], [placeholder="密码"]')
    .first()
    .fill("password123");
  await page
    .getByRole("button", { name: /^登录$/ })
    .first()
    .click();
  await page.waitForURL((u) => !u.pathname.includes("login"), {
    timeout: 15_000,
  });
}

test.describe("冒烟：登录与关键页", () => {
  // 语言必须在**请求发出前**就已在 cookie 里：addInitScript 里的
  // document.cookie 要等文档开载后才落，首个 /login 请求不带它，
  // SSR 照样按英文渲染，`/^登录$/` 就永远等不到（2026-10-14 实测，
  // 同 visual-regression.spec.ts 的修法）。
  test.beforeEach(async ({ context }) => {
    await context.addCookies([
      { name: "flux.locale", value: "zh-CN", domain: "localhost", path: "/" },
    ]);
  });

  test("登录成功跳首页", async ({ page }) => {
    await login(page);
    expect(page.url()).not.toContain("/login");
    const body = await page.locator("body").innerText();
    expect(body.length).toBeGreaterThan(80);
  });

  test("资源库页可打开（登录态）", async ({ page }) => {
    await login(page);
    await page.goto("/torrents");
    await page.waitForLoadState("networkidle");
    const body = await page.locator("body").innerText();
    expect(body.length).toBeGreaterThan(20);
  });

  test("等级页在当前镜像可用（旧镜像跳过）", async ({ page }) => {
    await login(page);
    const resp = await page.goto("/classes");
    // 旧镜像无该路由（Next 404）：skip 而非 fail——镜像更新后自动生效
    test.skip(resp?.status() === 404, "容器镜像早于 /classes 路由");
    await expect(page.locator("body")).toContainText(/等级|Level/, {
      timeout: 10_000,
    });
  });
});

/** 资源库行点击的区间口径（M4 双栏）。
 *
 *  `.tsplit` 的可见性由 CSS 判（pages.css：≥lg `display:none`），
 *  拦截点击改 URL `?selected=` 由 JS 判（torrent-split.tsx）。两边一旦
 *  不一致，宽屏上就是「点了没反应」——所以两条都在真浏览器里钉，
 *  而且要连 CSS 一起验（单测 mock 不到那条样式）。
 *
 *  两段共用一次登录：login 限流 5 次/分/用户名，本文件已有三次 UI 登录，
 *  再加一次就会把「操作太频繁啦」演成 waitForURL 超时（假红，2026-10-16）。
 */
test.describe("资源库行点击：双栏区间", () => {
  test.beforeEach(async ({ context }) => {
    await context.addCookies([
      { name: "flux.locale", value: "zh-CN", domain: "localhost", path: "/" },
    ]);
  });

  test("≥lg 直接进详情；md–lg 走内联预览", async ({ page }) => {
    await login(page);

    // ① 桌面（≥lg）：右栏不渲染，点击必须换页
    await page.setViewportSize({ width: 1280, height: 800 });
    await page.goto("/torrents");
    const deskLink = page.locator("a.torrents-name").first();
    await expect(deskLink).toBeVisible({ timeout: 20_000 });
    const name = (await deskLink.innerText()).trim();
    await deskLink.click();
    await expect(page).toHaveURL(/\/torrent\/\d+/);
    // 换页要看到内容，不只是 URL 变了
    await expect(page.locator("body")).toContainText(name, { timeout: 15_000 });

    // ①b 详情页右栏三卡要随页面滚动，不能吸附后把「相关种子」压在背后
    const side0 = await page.locator(".td-side").boundingBox();
    // behavior:instant —— 本站 `scroll-behavior: smooth`，默认滚动是动画，
    // 立刻读 scrollY 会拿到 0（假红，2026-10-16 实录）
    const scrolled = await page.evaluate(() => {
      window.scrollTo({ top: 300, behavior: "instant" });
      return window.scrollY;
    });
    await page.waitForTimeout(300);
    const side1 = await page.locator(".td-side").boundingBox();
    const rel1 = await page.locator(".td-side-rel").boundingBox();
    // 前置条件本身也钉住：这页至少要能滚 200px，否则「位移=滚动量」是假绿
    expect(scrolled).toBeGreaterThanOrEqual(200);
    expect(Math.round(side0!.y - side1!.y)).toBe(Math.round(scrolled));
    // 同栏上下两段不许重叠（sticky 时 rel 会从卡组底下穿上来）
    expect(rel1!.y).toBeGreaterThanOrEqual(side1!.y + side1!.height);

    // ② md–lg（768–1023）：双栏预览照旧，点「打开详情」才换页
    await page.setViewportSize({ width: 900, height: 800 });
    await page.goto("/torrents");
    const mdLink = page.locator("a.torrents-name").first();
    await expect(mdLink).toBeVisible({ timeout: 20_000 });
    await mdLink.click();
    await expect(page).toHaveURL(/[?&]selected=\d+/);
    await expect(page.locator(".tsplit__card")).toBeVisible({
      timeout: 15_000,
    });
    await page.locator(".tsplit__open").click();
    await expect(page).toHaveURL(/\/torrent\/\d+/);
  });
});
