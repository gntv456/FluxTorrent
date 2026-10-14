import { test, expect } from "@playwright/test";
import AxeBuilder from "@axe-core/playwright";

/**
 * a11y 扫描（DS-PLAN-01 §9-6）：P0 十页 axe 零 critical。
 *
 * - 判据 = §9-6 原文「axe 零 critical」：critical 阻断读屏/键盘的必须为零；
 *   serious/moderate 只出数据（进 metrics-log），不拦 PR——它们多属
 *   明色对比度债（muted/faint/brand 的 AA-L 组，浅色主题重塑收编）
 *   与第三方组件形态，清偿属渐进。
 * - 登录态与语言钉法同 visual-regression.spec.ts：凭证/语言都必须在
 *   **请求发出前**就在 cookie 里（addInitScript 的 document.cookie 要等
 *   文档开载后才落，SSR 照样按英文渲染）；UI 登录有水合竞态，故直接
 *   登录 API 换 JWT 再 addCookies。
 * - 运行：cd apps/web && npx playwright test e2e/axe.spec.ts
 *   前置：本地栈在跑（web 3000），账号 root/password123。
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

const TAGS = ["wcag2a", "wcag21a", "wcag2aa", "wcag21aa"];

/** token 模块级缓存：workers=1 全程只登一次。登录端点有限流
 *  （「操作太频繁啦」），逐用例登录会在第 6-8 个用例被掐。 */
let cachedToken: string | null = null;

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
    throw new Error(`登录失败：${j?.message ?? res.status}`);
  }
  cachedToken = j.data.token;
  return cachedToken;
}

test.beforeEach(async ({ context, baseURL }) => {
  const base = baseURL ?? "http://localhost:3000";
  const host = new URL(base).hostname;
  const token = await sessionToken(base);
  await context.addCookies([
    { name: "flux_token", value: token, domain: host, path: "/" },
    { name: "flux.session", value: "1", domain: host, path: "/" },
    { name: "flux.locale", value: "zh-CN", domain: host, path: "/" },
  ]);
});

for (const p of PAGES) {
  test(`axe · ${p.name}`, async ({ page }) => {
    await page.goto(p.path);
    await expect(page.locator("main#main-content")).toBeVisible({
      timeout: 15_000,
    });
    const results = await new AxeBuilder({ page }).withTags(TAGS).analyze();
    const byImpact = (impact: string) =>
      results.violations.filter((v) => v.impact === impact);
    const critical = byImpact("critical");
    const serious = byImpact("serious");
    // §9-6 判据：critical 必须为零；其余档位出数据不拦
    expect(
      critical,
      `critical 违规 ${critical.length} 条：` +
        critical
          .map((v) => `${v.id}(${v.nodes.length} 节点): ${v.help}`)
          .join("; "),
    ).toHaveLength(0);
    if (serious.length || results.violations.length > critical.length) {
      const line = results.violations
        .map(
          (v) => `  [${v.impact ?? "?"}] ${v.id} ×${v.nodes.length}: ${v.help}`,
        )
        .join("\n");
      console.log(`\n[axe] ${p.name} 违规清单：\n${line}\n`);
    }
  });
}
