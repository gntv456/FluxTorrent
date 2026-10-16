import { test, expect, type Page } from "@playwright/test";

/**
 * 响应式门禁（S3 收敛与治理 · DS-PLAN-01「新页面必须过 4 断点」）
 *
 * 与 visual-regression.spec.ts 的分工：那边做**明暗双主题截图基线对比**
 * （单视口 1280×800），本 spec 做**四断点结构断言**——不引入截图基线
 * 维护成本，只机械抓最常见、也最能自动判定的破版：
 *
 *   1. 横向溢出：scrollWidth > clientWidth 即失败（移动端破版头号来源）
 *   2. h1 存在：每页至少一个 h1（loading 态与正常态都算）
 *
 * 断言层的收益/成本远优于截图比对：零基线文件、无假阳性，
 *  CI 可无脑跑。截图比对留给 visual-regression 的既有机制。
 *
 * 运行：cd apps/web && npx playwright test e2e/responsive.spec.ts
 * 前置：本地栈在跑（web 3000），账号 root/password123（同 smoke.spec.ts）。
 */

/** DS 规定的四档断点（1440 / 1024 / 768 / 390） */
const BREAKPOINTS = [
  { name: "xl", width: 1440, height: 900 },
  { name: "lg", width: 1024, height: 800 },
  { name: "md", width: 768, height: 900 },
  { name: "sm", width: 390, height: 844 },
] as const;

/** 覆盖首页/列表/详情/后台四类页面；登录态统一注入，匿名页同样可过 */
const PAGES = [
  { name: "home", path: "/" },
  { name: "torrents", path: "/torrents" },
  { name: "torrent-detail", path: "/torrent/40" },
  { name: "forums", path: "/forums" },
  { name: "medals", path: "/medals" },
  { name: "top", path: "/top" },
  { name: "bank", path: "/bank" },
  { name: "admin-torrents", path: "/admin?tool=torrents" },
] as const;

const BASE = process.env.E2E_BASE_URL ?? "http://localhost:3000";
let cachedToken: string | null = null;

/** 登录 API 取 JWT（同 visual-regression：绕过 UI 登录的水合竞态） */
async function login(): Promise<string> {
  if (cachedToken) return cachedToken;
  const res = await fetch(`${BASE}/api/v1/auth/login`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ username: "root", password: "password123" }),
  });
  const j = (await res.json()) as { code: number; data?: { token?: string } };
  if (j.code !== 0 || !j.data?.token) throw new Error("login failed");
  cachedToken = j.data.token;
  return cachedToken;
}

/** 双 cookie 在文档开载前落定（同 visual-regression 的 addInitScript 口径） */
async function auth(page: Page): Promise<void> {
  const token = await login();
  const host = new URL(BASE).hostname;
  await page.addInitScript(
    ([tk, h]) => {
      document.cookie = `flux_token=${tk}; path=/; domain=${h}`;
      document.cookie = "flux.session=1; path=/";
      document.cookie = "flux.locale=zh-CN; path=/";
    },
    [token, host] as const,
  );
}

for (const bp of BREAKPOINTS) {
  test.describe(`${bp.name} · ${bp.width}px`, () => {
    test.use({ viewport: { width: bp.width, height: bp.height } });

    for (const p of PAGES) {
      test(`${p.name} 无横向溢出且 h1 存在`, async ({ page }) => {
        const errors: string[] = [];
        page.on("pageerror", (e) => errors.push(e.message));
        await auth(page);
        await page.goto(p.path, { waitUntil: "networkidle" });
        // 客户端 fetch 的页面（my/orders 等）需等一拍，否则量到的是
        // loading 态的 scrollWidth
        await page.waitForTimeout(400);

        const m = await page.evaluate(() => ({
          scrollW: document.documentElement.scrollWidth,
          clientW: document.documentElement.clientWidth,
          h1: document.querySelectorAll("h1").length,
        }));

        expect(
          m.scrollW,
          `${p.path} 横向溢出：${m.scrollW} > ${m.clientW}`,
        ).toBeLessThanOrEqual(m.clientW + 1);
        expect(m.h1, `${p.path} 缺少 h1（连 loading 态也没有）`).toBeGreaterThan(0);
        expect(errors, `${p.path} 有未捕获错误`).toEqual([]);
      });
    }
  });
}
