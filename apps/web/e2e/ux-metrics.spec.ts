import { test, expect } from "@playwright/test";
import { computeUxReport } from "../lib/ux-metrics-report";
import type { UxMetric, UxProfile } from "../lib/ux-metrics";

/** 数据闭环 E2E（不依赖后端）：把一段合成的新手旅程写进 localStorage，
 *  验证 /account-health 的使用足迹卡片能读出来、能导出、能清除，
 *  并用与 UI 相同的 computeUxReport 复算口径——避免看板与报表两套算法。 */

const NOW = Date.UTC(2026, 10, 19, 12, 0, 0);
const FIRST = NOW - 8 * 86400_000;
const base = new Date(FIRST).toISOString().slice(0, 10);
const day = (delta: number) =>
  new Date(Date.parse(base + "T00:00:00Z") + delta * 86400_000)
    .toISOString()
    .slice(0, 10);

const events: UxMetric[] = [
  { name: "signup_complete", at: FIRST + 500, value: 500 },
  { name: "app_ready", at: FIRST + 1000, value: 1000 },
  { name: "guide_open", at: FIRST + 1500, value: 1500, label: "survival" },
  { name: "search_submit", at: FIRST + 2000, value: 2000 },
  { name: "first_torrent_open", at: FIRST + 3000, value: 3000 },
  { name: "checkin", at: FIRST + 4000, value: 4000 },
  { name: "empty_state", at: FIRST + 4500, value: 4500, label: "filter" },
];
const profile: UxProfile = {
  firstSeen: FIRST,
  activeDays: [base, day(1), day(7)],
  events,
};

test.use({ storageState: undefined });

test.beforeEach(async ({ context, baseURL }) => {
  await context.addCookies([
    { name: "flux_token", value: "e2e-ux-footprint", url: baseURL! },
  ]);
  await context.addInitScript((p) => {
    localStorage.setItem("flux-ux-metrics", JSON.stringify(p.events));
    localStorage.setItem("flux-ux-first-seen", String(p.firstSeen));
    localStorage.setItem("flux-ux-active-days", JSON.stringify(p.activeDays));
  }, profile);
});

test("footprint: render, export, clear and aggregation parity", async ({
  page,
}) => {
  const expected = computeUxReport(profile, NOW);

  await page.goto("/account-health");
  const card = page.locator(".ux-foot");
  await expect(card).toBeVisible();
  // 活跃天数：聚合器与页面展示同一份数据
  await expect(card).toContainText(String(expected.activeDays));
  // 七步漏斗中「打开种子」计数为 1
  const openStep = expected.steps.find((s) => s.name === "first_torrent_open")!;
  expect(openStep.count).toBe(1);
  await expect(card).toContainText("1");

  const [download] = await Promise.all([
    page.waitForEvent("download"),
    card.getByRole("button", { name: /导出|Export/ }).click(),
  ]);
  expect(download.suggestedFilename()).toBe("flux-ux-footprint.json");
  const stream = await download.createReadStream();
  let raw = "";
  for await (const chunk of stream ?? []) raw += String(chunk);
  const payload = JSON.parse(raw);
  expect(payload.schema).toBe("flux-ux-footprint/v1");
  expect(payload.report.activeDays).toBe(expected.activeDays);
  expect(payload.report.d1Return).toBe(true);
  expect(payload.report.d7Return).toBe(true);
  expect(payload.report.ttfvMs).toBe(2000);

  const clear = card.getByRole("button", { name: /清除|Clear/ });
  await clear.click(); // 第一段：确认
  await clear.click(); // 第二段：执行
  const stored = await page.evaluate(() => ({
    events: localStorage.getItem("flux-ux-metrics"),
    first: localStorage.getItem("flux-ux-first-seen"),
    days: localStorage.getItem("flux-ux-active-days"),
  }));
  expect(stored.events).toBeNull();
  expect(stored.first).toBeNull();
  expect(stored.days).toBeNull();
});
