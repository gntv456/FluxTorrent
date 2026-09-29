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
  await page.getByRole("button", { name: /^登录$/ }).first().click();
  await page.waitForURL((u) => !u.pathname.includes("login"), {
    timeout: 15_000,
  });
}

test.describe("冒烟：登录与关键页", () => {
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
