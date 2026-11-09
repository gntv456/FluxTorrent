/** 主菜单/子菜单真机渲染验证（可复跑）
 *
 * 前置：本地栈 + 前端 dev 在 http://localhost:3000，root/password123。
 * 运行：cd apps/web && node e2e/nav-menu-verify.mjs
 * 产物：design/_shots/nav-2026-11-09/*.png + nav-verify.json
 *
 * 覆盖：1440/1180/1024/768/375 × brand/brand-night/tide/shanhaijing；
 * 平铺一级、三层级联右飞、更多反手、Esc/点外收起、抽屉逐级展开、
 * 山海经侧栏展开/轨道/隐藏三态与 main 左边界实测。
 */
import { chromium } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";

const BASE = process.env.E2E_BASE_URL ?? "http://localhost:3000";
const OUT = path.resolve("../../design/_shots/nav-2026-11-09");
fs.mkdirSync(OUT, { recursive: true });

// 选项以 lib/themes.ts 注册表为准：brand / brand-night / shanhaijing /
// shanhaijing-light。旧版常说的「tide」是 brand 族的顶栏 shell（.tide-header），
// 不是独立可选主题——写错 id 会被 parseChoice 归到 auto→brand，量测会失真。
const THEMES = ["brand", "brand-night", "shanhaijing", "shanhaijing-light"];
const SIZES = [
  { w: 1440, h: 900 },
  { w: 1180, h: 900 },
  { w: 1024, h: 800 },
  { w: 768, h: 900 },
  { w: 375, h: 844 },
];

const login = await fetch(`${BASE}/api/v1/auth/login`, {
  method: "POST",
  headers: { "content-type": "application/json" },
  body: JSON.stringify({ username: "root", password: "password123" }),
}).then((r) => r.json());
const token = login?.data?.token;
if (!token) throw new Error("root 登录失败");

const browser = await chromium.launch({
  channel: process.env.E2E_CHANNEL ?? "msedge",
});
const report = { base: BASE, generatedAt: new Date().toISOString(), runs: [] };
const failures = [];

function shot(page, name) {
  return page.screenshot({ path: path.join(OUT, `${name}.png`) });
}

function visibleLists(page) {
  return page.evaluate(() =>
    Array.from(document.querySelectorAll(".nd-list"))
      .filter((el) => getComputedStyle(el).visibility === "visible")
      .map((el) => {
        const r = el.getBoundingClientRect();
        return {
          left: Math.round(r.left),
          right: Math.round(r.right),
          width: Math.round(r.width),
          rows: el.querySelectorAll(":scope > li").length,
        };
      }),
  );
}

for (const theme of THEMES) {
  for (const { w, h } of SIZES) {
    console.log(`▶ ${theme}@${w}`);
    const context = await browser.newContext({ viewport: { width: w, height: h } });
    // 认证用 Context 级 cookie：addInitScript 里的 document.cookie 赶不上
    // 首个导航请求，SSR 会把 `/` 当游客重定向到 /login（2026-11-09 实测）。
    await context.addCookies([
      { name: "flux_token", value: token, url: BASE },
      { name: "flux.session", value: "1", url: BASE },
      { name: "flux.locale", value: "zh-CN", url: BASE },
    ]);
    await context.addInitScript(
      ({ th }) => {
        localStorage.setItem("flux-theme", th);
        localStorage.removeItem("flux-shj-rail");
      },
      { th: theme },
    );
    const page = await context.newPage();
    try {
    const errors = [];
    page.on("pageerror", (e) => errors.push(e.message));
    await page.goto(`${BASE}/`, { waitUntil: "domcontentloaded", timeout: 60000 });
    // 等待「该断点下真正可见」的导航根节点：站点壳要等会话拉取完成后才挂载，
    // attached 会命中桌面隐藏层的 .mainmenu-link（768 下因此误判就绪）。
    // 山海经 768–1179 按设计不渲染汉堡（轨道承载导航），改等侧栏。
    const shjWide = theme.startsWith("shanhaijing") && w >= 768;
    const needDrawer = w < 1024 && !shjWide;
    await page.waitForSelector(
      w >= 1024 ? ".mainmenu-link" : needDrawer ? ".navdrawer-trigger" : ".shj-atlas",
      { state: "visible", timeout: 45000 },
    );
    await page.waitForTimeout(500);

    const run = { theme, width: w, height: h, errors };
    run.themeApplied = await page.evaluate(
      () => document.documentElement.dataset.theme,
    );
    run.overflow = await page.evaluate(
      () =>
        document.documentElement.scrollWidth -
        document.documentElement.clientWidth,
    );
    if (run.overflow > 1) {
      failures.push(`${theme}@${w} 页面横向溢出 ${run.overflow}px`);
    }
    run.mainLeft = await page.evaluate(() => {
      const m = document.querySelector("main");
      return m ? Math.round(m.getBoundingClientRect().left) : null;
    });
    run.mainRight = await page.evaluate(() => {
      const m = document.querySelector("main");
      return m ? Math.round(m.getBoundingClientRect().right) : null;
    });
    // 内容真实让位：main 右边界应贴着视口（不为侧栏额外让右）
    if (run.mainRight !== null && run.mainRight < w - 24) {
      failures.push(`${theme}@${w} main 右边界异常 ${run.mainRight}（视口 ${w}）`);
    }

    // 山海经侧栏三态 + 内容让位（量 main 而不是侧栏自身）
    if (theme === "shanhaijing") {
      const railBox = await page.evaluate(() => {
        const el = document.querySelector(".shj-atlas");
        if (!el) return null;
        const r = el.getBoundingClientRect();
        return { width: Math.round(r.width), display: getComputedStyle(el).display };
      });
      run.sidebar = railBox;
      if (w >= 1180) {
        if (!railBox || railBox.width !== 236) {
          failures.push(`${theme}@${w} 侧栏未展开到 236（实测 ${railBox?.width}）`);
        }
        if (run.mainLeft !== 236) {
          failures.push(`${theme}@${w} main 左边界应为 236（实测 ${run.mainLeft}）`);
        }
        run.brandVisible = await page.evaluate(() => {
          const b = document.querySelector(".shj-atlas__brand");
          return b ? getComputedStyle(b).display !== "none" : false;
        });
        await shot(page, `${theme}-${w}-sidebar-expanded`);
        await page.locator(".shj-atlas__fold").click({ force: true });
        await page.waitForTimeout(350);
        run.railMainLeft = await page.evaluate(() => {
          const m = document.querySelector("main");
          const a = document.querySelector(".shj-atlas");
          return {
            main: m ? Math.round(m.getBoundingClientRect().left) : null,
            width: a ? Math.round(a.getBoundingClientRect().width) : null,
            brand: (() => {
              const b = document.querySelector(".shj-atlas__brand");
              return b ? getComputedStyle(b).display !== "none" : false;
            })(),
          };
        });
        if (run.railMainLeft.width !== 64 || run.railMainLeft.main !== 64) {
          failures.push(`${theme}@${w} 收轨后应为 64（实测 ${JSON.stringify(run.railMainLeft)}）`);
        }
        await shot(page, `${theme}-${w}-sidebar-rail`);
        await page.locator(".shj-atlas__fold").click({ force: true });
        await page.waitForTimeout(350);
      } else if (w >= 768) {
        if (!railBox || railBox.width !== 64 || run.mainLeft !== 64) {
          failures.push(
            `${theme}@${w} 应为轨道 64 + main 64（实测 ${JSON.stringify(railBox)} / ${run.mainLeft}）`,
          );
        }
        await shot(page, `${theme}-${w}-sidebar-rail`);
      } else if (railBox && railBox.display !== "none") {
        failures.push(`${theme}@${w} 侧栏应隐藏（实测 ${JSON.stringify(railBox)}）`);
      }
    }

    if (w >= 1024) {
      // 一级悬停 → 第一层
      const lib = page.locator(".mainmenu-link", { hasText: "资源库" }).first();
      await lib.hover();
      await page.waitForTimeout(240);
      run.cascade1 = await visibleLists(page);
      if (!run.cascade1.length) failures.push(`${theme}@${w} 资源库一级未展开`);
      // 二级悬停 → 第三层
      const sub = page.locator(".nd-list:visible > .nd-has > .nd-row").first();
      if (await sub.count()) {
        await sub.hover();
        await page.waitForTimeout(240);
        run.cascade2 = await visibleLists(page);
      }
      for (const r of run.cascade2 ?? []) {
        if (r.right > w + 0.5) failures.push(`${theme}@${w} 级联右缘出屏 ${r.right}`);
      }
      await shot(page, `${theme}-${w}-cascade`);

      // Esc：焦点回到触发条且面板关闭
      await lib.focus();
      await page.waitForTimeout(120);
      await page.keyboard.press("Escape");
      await page.waitForTimeout(160);
      run.esc = await page.evaluate(() => {
        const a = document.activeElement;
        return {
          tag: a?.tagName ?? "",
          cls: a?.className ?? "",
          isTrigger: !!a?.hasAttribute("data-nav-trigger"),
          text: a?.textContent?.trim() ?? "",
          expanded: a?.getAttribute("aria-expanded"),
          visible: Array.from(document.querySelectorAll(".nd-list")).some(
            (el) => getComputedStyle(el).visibility === "visible",
          ),
        };
      });
      if (run.esc.visible) failures.push(`${theme}@${w} Esc 后级联未收起`);
      if (!run.esc.isTrigger || !run.esc.text.includes("资源库")) {
        failures.push(
          `${theme}@${w} Esc 后焦点未回到触发条（${JSON.stringify(run.esc)}）`,
        );
      }

      // 「更多」最后一项反手 + 深层不出屏
      const more = page.locator(".mainmenu-link", { hasText: "更多" }).first();
      await more.hover();
      await page.waitForTimeout(240);
      const grp = page.locator(".nd-list:visible > .nd-has > .nd-row").first();
      if (await grp.count()) {
        await grp.hover();
        await page.waitForTimeout(240);
        run.moreDeep = await visibleLists(page);
        for (const r of run.moreDeep) {
          if (r.right > w + 0.5 || r.left < -0.5) {
            failures.push(`${theme}@${w} 更多深层出屏 ${JSON.stringify(r)}`);
          }
        }
      }
      await shot(page, `${theme}-${w}-more`);
      // 点外部收起
      await page.mouse.click(w - 8, h - 8);
      await page.waitForTimeout(150);
      run.outside = await page.evaluate(() =>
        Array.from(document.querySelectorAll(".nd-list")).some(
          (el) => getComputedStyle(el).visibility === "visible",
        ),
      );
      if (run.outside) failures.push(`${theme}@${w} 点外部未收起`);
    } else if (theme.startsWith("shanhaijing") && w >= 768) {
      // 山海经 768–1179：侧栏恒为 64px 轨态并真实让位，汉堡按设计收起
      // （theme-shanhaijing.css：≥768 隐藏 .lg:hidden 与底部 TabBar）——
      // 导航改由「轨道 + 分组右飞层」承载，这里验证右飞层可达且不出屏。
      run.railMode = true;
      const gtitle = page.locator(".shj-atlas__gtitle").first();
      if (await gtitle.count()) {
        await gtitle.hover();
        await page.waitForTimeout(300);
        run.railFlyout = await page.evaluate(() => {
          const f = document.querySelector(".shj-atlas__flyout");
          if (!f) return null;
          const r = f.getBoundingClientRect();
          return {
            left: Math.round(r.left),
            right: Math.round(r.right),
            items: f.querySelectorAll(".shj-atlas__item").length,
          };
        });
        if (!run.railFlyout) {
          failures.push(`${theme}@${w} 轨道态分组右飞层未展开`);
        } else {
          if (run.railFlyout.right > w + 0.5) {
            failures.push(`${theme}@${w} 轨道右飞层出屏 ${run.railFlyout.right}`);
          }
          if (!run.railFlyout.items) {
            failures.push(`${theme}@${w} 轨道右飞层无入口`);
          }
        }
        await shot(page, `${theme}-${w}-rail-flyout`);
        await page.mouse.move(w - 6, h - 6);
        await page.waitForTimeout(150);
      }
    } else {
      // <lg 抽屉：逐级展开到叶子，再折回验证「返回」
      await page.evaluate(() => document.querySelector(".navdrawer-trigger")?.click());
      await page.waitForSelector(".navdrawer", { state: "attached", timeout: 15000 });
      await page.waitForTimeout(300);
      run.drawerFocus = await page.evaluate(() => ({
        role: document.activeElement?.getAttribute("role"),
        cls: document.activeElement?.className,
        inDialog: !!document.activeElement?.closest(".navdrawer"),
      }));
      await shot(page, `${theme}-${w}-drawer`);
      const libToggle = page.locator('.navdrawer__expand[aria-label="资源库"]').first();
      if (await libToggle.count()) {
        await page.evaluate(() =>
          document
            .querySelector('.navdrawer__expand[aria-label="资源库"]')
            ?.click(),
        );
        await page.waitForTimeout(220);
        // 资源库下的子级是「空 href 分组」→ 整行 button；逐级向下钻，
        // 直到出现可见叶子链接（每轮只点未展开项，避免把同一项又关回去）。
        const countDeep = () =>
          page.evaluate(() =>
            Array.from(document.querySelectorAll(".navdrawer a.navdrawer__link")).filter(
              (a) => a.getBoundingClientRect().height > 0,
            ).length,
          );
        for (let d = 0; d < 4 && (await countDeep()) === 0; d++) {
          const clicked = await page.evaluate(() => {
            const el = Array.from(
              document.querySelectorAll(
                ".navdrawer__sub:not([hidden]) .navdrawer__expand, " +
                  ".navdrawer__sub:not([hidden]) button.navdrawer__link--grow",
              ),
            ).find((b) => b.getAttribute("aria-expanded") === "false");
            if (!el) return false;
            el.click();
            return true;
          });
          if (!clicked) break;
          await page.waitForTimeout(220);
        }
      }
      run.drawer = await page.evaluate(() => {
        const panel = document.querySelector(".navdrawer");
        const visLinks = Array.from(
          panel?.querySelectorAll("a.navdrawer__link") ?? [],
        ).filter((a) => a.getBoundingClientRect().height > 0);
        return {
          expandedToggles: panel?.querySelectorAll(
            '.navdrawer__link[aria-expanded="true"], .navdrawer__expand[aria-expanded="true"]',
          ).length,
          visibleDeepLinks: visLinks.length,
          arias: Array.from(
            panel?.querySelectorAll("[aria-expanded]") ?? [],
          ).every((el) => /true|false/.test(el.getAttribute("aria-expanded") ?? "")),
          overflow:
            document.documentElement.scrollWidth -
            document.documentElement.clientWidth,
        };
      });
      if (!run.drawer.visibleDeepLinks) {
        failures.push(`${theme}@${w} 抽屉未展开出深层入口`);
      }
      if (!run.drawer.arias) {
        failures.push(`${theme}@${w} 抽屉 aria-expanded 未同步`);
      }
      if (run.drawer.overflow > 1) {
        failures.push(`${theme}@${w} 抽屉态横向溢出`);
      }
      await shot(page, `${theme}-${w}-drawer-expanded`);
      // 返回：再点一次资源库，整棵子树收起且 aria-expanded=false
      // （用 DOM click：dev 指示器是 fixed 浮层，force 点击仍会被它接走）
      const clickedBack = await page.evaluate(() => {
        const btn = document.querySelector('.navdrawer__expand[aria-label="资源库"]');
        if (!btn) return false;
        btn.click();
        return true;
      });
      if (clickedBack) {
        await page.waitForTimeout(220);
        run.drawerBack = await page.evaluate(() => {
          const btn = document.querySelector('.navdrawer__expand[aria-label="资源库"]');
          const id = btn?.getAttribute("aria-controls");
          const panel = id ? document.getElementById(id) : null;
          return {
            expanded: btn?.getAttribute("aria-expanded"),
            hidden: panel ? panel.hasAttribute("hidden") : null,
          };
        });
        if (run.drawerBack.expanded !== "false" || !run.drawerBack.hidden) {
          failures.push(
            `${theme}@${w} 抽屉折回失败（${JSON.stringify(run.drawerBack)}）`,
          );
        }
        await shot(page, `${theme}-${w}-drawer-back`);
      }
      await page.keyboard.press("Escape");
      await page.waitForTimeout(200);
      run.drawerClosed = await page.evaluate(
        () => !document.querySelector(".navdrawer"),
      );
      if (!run.drawerClosed) failures.push(`${theme}@${w} 抽屉 Esc 未关闭`);
    }

    await shot(page, `${theme}-${w}-home`);
    if (errors.length) failures.push(`${theme}@${w} 页面错误：${errors[0]}`);
    report.runs.push(run);
    } catch (e) {
      const msg = String(e?.message ?? e).split("\n")[0];
      failures.push(`${theme}@${w} 脚本异常：${msg}`);
      await shot(page, `${theme}-${w}-error`).catch(() => {});
    } finally {
      await context.close();
    }
  }
}

report.failures = failures;
fs.writeFileSync(path.join(OUT, "nav-verify.json"), JSON.stringify(report, null, 2));
await browser.close();
console.log(
  JSON.stringify(
    { runs: report.runs.length, failures: failures.length, failuresList: failures },
    null,
    2,
  ),
);
if (failures.length) process.exitCode = 1;

