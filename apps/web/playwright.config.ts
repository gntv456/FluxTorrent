import { defineConfig } from "@playwright/test";

/**
 * Playwright E2E（E14 骨架）：
 * - 前置：本地栈在跑（docker compose up，web 3000 / api 8080），账号 root
 * - 定位：浏览器层冒烟——登录态走通、关键页 200、控制台无未捕获错误。
 *   深链路（发种/支付/H&R）仍由 scripts/*_e2e.py 的 API 级 e2e 承担，
 *   Playwright 只补「真实浏览器里页面活着」这一层，不重复造用例。
 * - 运行：cd apps/web && npx playwright test（CI 可选 job，本地手动）
 */
export default defineConfig({
  testDir: "./e2e",
  timeout: 30_000,
  retries: 0,
  workers: 1, // 单 worker：登录态共享 storageState，避免并发互踩
  use: {
    // 用系统浏览器（本机 chromium 修订号与包不齐时免下载；Windows 默认 Edge）
    channel: process.env.E2E_CHANNEL ?? "msedge",
    baseURL: process.env.E2E_BASE_URL ?? "http://localhost:3000",
    screenshot: "only-on-failure",
    trace: "retain-on-failure",
  },
  reporter: [["list"]],
});
