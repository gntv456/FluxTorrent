import { chromium } from "@playwright/test";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const BASE = process.env.E2E_BASE_URL ?? "http://localhost:3000";
const HERE = dirname(fileURLToPath(import.meta.url));
const OUT = resolve(HERE, "../../../design/_shots/nav-2026-11-09");
mkdirSync(OUT, { recursive: true });

const THEMES = ["brand", "brand-night", "shanhaijing", "shanhaijing-light"];

const login = await fetch(`${BASE}/api/v1/auth/login`, {
  method: "POST",
  headers: { "content-type": "application/json" },
  body: JSON.stringify({ username: "root", password: "password123" }),
}).then((r) => r.json());
const token = login.data.token;

const browser = await chromium.launch({ channel: "msedge" });
const report = { base: BASE, runs: [], failures: [] };

async function open(theme, width, height, path = "/") {
  const context = await browser.newContext({ viewport: { width, height } });
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
  await page.goto(`${BASE}${path}`, { waitUntil: "domcontentloaded" });
  await page.waitForSelector(".site-shell, main", { state: "visible", timeout: 45000 });
  await page.waitForTimeout(700);
  return { context, page };
}

/** 读焦点环实测值。先按一次 Tab 把「最近输入＝键盘」定下来，
 *  再脚本聚焦目标——Chromium 据此让 :focus-visible 命中。 */
async function ring(page, selector) {
  await page.keyboard.press("Tab");
  return page.evaluate((sel) => {
    const el = document.querySelector(sel);
    if (!el) return null;
    el.focus({ focusVisible: true });
    const cs = getComputedStyle(el);
    return {
      selector: sel,
      focusVisible: el.matches(":focus-visible"),
      outline: `${cs.outlineWidth} ${cs.outlineStyle} ${cs.outlineColor}`,
      outlineOffset: cs.outlineOffset,
      boxShadow: cs.boxShadow,
      token: getComputedStyle(document.documentElement).getPropertyValue("--focus-ring").trim(),
    };
  }, selector);
}

for (const theme of THEMES) {
  for (const width of [1440, 375]) {
    console.log(`▶ a11y ${theme}@${width}`);
    const height = width === 1440 ? 900 : 844;
    const { context, page } = await open(theme, width, height);
    const run = { theme, width, themeApplied: await page.evaluate(() => document.documentElement.dataset.theme) };
    try {
      if (width === 1440) {
        run.targetHeight = await page.evaluate(() => {
          const a = Array.from(document.querySelectorAll(".mainmenu-link")).find((x) =>
            x.textContent.includes("资源库"),
          );
          if (!a) return null;
          const r = a.getBoundingClientRect();
          return { h: Math.round(r.height), after: getComputedStyle(a, "::after").height };
        });
        run.ringMenu = await ring(page, ".mainmenu-link");
        const lib = page.locator(".mainmenu-link", { hasText: "资源库" }).first();
        await lib.hover();
        await page.waitForTimeout(220);
        run.hover = await page.evaluate(() => {
          const a = Array.from(document.querySelectorAll(".mainmenu-link")).find((x) =>
            x.textContent.includes("资源库"),
          );
          const cs = getComputedStyle(a);
          return { bg: cs.backgroundColor, color: cs.color, expanded: a.getAttribute("aria-expanded") };
        });
        run.ringRow = await ring(page, '.mainmenu > li[data-open="true"] .nd-row');
        run.rowTap = await page.evaluate(() => {
          const row = document.querySelector(".nd-list .nd-row");
          return row ? Math.round(row.getBoundingClientRect().height) : null;
        });
        // 键盘：焦点进触发条 → aria-expanded=true → Esc 收起并回到触发条
        await page.mouse.move(width - 4, height - 4);
        await page.evaluate(() =>
          Array.from(document.querySelectorAll(".mainmenu-link"))
            .find((x) => x.textContent.includes("资源库"))
            ?.focus(),
        );
        await page.waitForTimeout(150);
        run.kbBefore = await page.evaluate(() => {
          const a = document.activeElement;
          return { expanded: a?.getAttribute("aria-expanded"), tag: a?.tagName };
        });
        await page.keyboard.press("Escape");
        await page.waitForTimeout(180);
        run.kbAfterEsc = await page.evaluate(() => {
          const a = document.activeElement;
          return {
            tag: a?.tagName,
            isTrigger: !!a?.hasAttribute("data-nav-trigger"),
            expanded: a?.getAttribute("aria-expanded"),
            panelVisible: Array.from(document.querySelectorAll(".nd-list")).some(
              (el) => getComputedStyle(el).visibility === "visible",
            ),
          };
        });
        // Tab 顺序（重新载入，从页面顶部开始；记录前 24 个可聚焦节点）
        await page.goto(`${BASE}/`, { waitUntil: "domcontentloaded" });
        await page.waitForSelector(".mainmenu-link", { state: "visible" });
        await page.waitForTimeout(400);
        await page.evaluate(() => document.body.focus());
        run.tabOrder = [];
        for (let i = 0; i < 24; i++) {
          await page.keyboard.press("Tab");
          run.tabOrder.push(
            await page.evaluate(() => {
              const a = document.activeElement;
              if (!a) return "";
              return `${a.tagName}:${(a.getAttribute("aria-label") || a.textContent || "").trim().slice(0, 12)}`;
            }),
          );
        }
        // 选中态：/torrents 下 aria-current 与配色
        await page.goto(`${BASE}/torrents`, { waitUntil: "domcontentloaded" });
        await page.waitForSelector(".mainmenu-link", { state: "visible" });
        await page.waitForTimeout(500);
        run.active = await page.evaluate(() => {
          const a = document.querySelector(".mainmenu-link[aria-current='page']");
          if (!a) return null;
          const cs = getComputedStyle(a);
          return {
            text: a.textContent.trim(),
            bg: cs.backgroundColor,
            color: cs.color,
            bar: getComputedStyle(a, "::after").backgroundColor,
          };
        });
        run.ringActive = await ring(page, ".mainmenu-link[aria-current='page']");
        // eduedit（仅展开态可见）
        run.eduEdit = await page.evaluate(() => {
          const b = document.querySelector(".shj-atlas__eduedit");
          if (!b) return null;
          const cs = getComputedStyle(b);
          return { h: Math.round(b.getBoundingClientRect().height), display: cs.display };
        });
      } else {
        // 抽屉键盘：触发条 Enter 打开 → 焦点进陷阱 → Tab 不逃逸 → Esc 关闭并归还焦点
        const trig = page.locator(".navdrawer-trigger").first();
        await trig.focus();
        await page.keyboard.press("Enter");
        await page.waitForTimeout(450);
        run.drawerOpened = await page.evaluate(() => {
          const panel = document.querySelector(".navdrawer");
          return {
            open: !!panel,
            focusInside: !!document.activeElement?.closest(".navdrawer"),
            role: document.activeElement?.getAttribute("role"),
          };
        });
        // 展开资源库并核对 aria-expanded / aria-controls 同步
        const synced = await page.evaluate(() => {
          const btn = document.querySelector('.navdrawer__expand[aria-label="资源库"]');
          if (!btn) return false;
          btn.click();
          return true;
        });
        await page.waitForTimeout(250);
        run.drawerAria = await page.evaluate(() => {
          const btn = document.querySelector('.navdrawer__expand[aria-label="资源库"]');
          const id = btn?.getAttribute("aria-controls");
          const panel = id ? document.getElementById(id) : null;
          return {
            expanded: btn?.getAttribute("aria-expanded"),
            controls: !!id,
            panelHidden: panel?.hasAttribute("hidden"),
          };
        });
        if (!synced || run.drawerAria.expanded !== "true" || run.drawerAria.panelHidden) {
          report.failures.push(`${theme}@${width} 抽屉 aria-expanded 未同步`);
        }
        run.drawerRowTap = await page.evaluate(() => {
          const row = document.querySelector(".navdrawer__row");
          return row ? Math.round(row.getBoundingClientRect().height) : null;
        });
        run.ringDrawer = await ring(page, ".navdrawer .navdrawer__link");
        // Tab 30 次：焦点始终在抽屉内（焦点陷阱）
        let escaped = false;
        for (let i = 0; i < 30; i++) {
          await page.keyboard.press("Tab");
          const inside = await page.evaluate(() => !!document.activeElement?.closest(".navdrawer"));
          if (!inside) {
            escaped = true;
            break;
          }
        }
        run.drawerTrap = { escaped };
        if (escaped) report.failures.push(`${theme}@${width} 抽屉焦点逃逸`);
        await page.keyboard.press("Escape");
        await page.waitForTimeout(250);
        run.drawerAfterEsc = await page.evaluate(() => ({
          closed: !document.querySelector(".navdrawer"),
          focusTag: document.activeElement?.tagName,
          focusedTrigger: document.activeElement?.classList.contains("navdrawer-trigger"),
        }));
        if (!run.drawerAfterEsc.closed) report.failures.push(`${theme}@${width} 抽屉 Esc 未关闭`);
        if (!run.drawerAfterEsc.focusedTrigger) {
          report.failures.push(`${theme}@${width} 抽屉关闭后焦点未归还触发条`);
        }
      }
    } catch (e) {
      report.failures.push(`${theme}@${width} 脚本异常：${String(e?.message ?? e).split("\n")[0]}`);
    } finally {
      report.runs.push(run);
      await context.close();
    }
  }
}

// 山海经轨态：分组浮层键盘可达 + 焦点陷阱 + Esc 关闭归还焦点
for (const theme of ["shanhaijing", "shanhaijing-light"]) {
  console.log(`▶ a11y rail ${theme}@1024`);
  const { context, page } = await open(theme, 1024, 900);
  const run = { theme, width: 1024, rail: true };
  try {
    run.ringGroup = await ring(page, ".shj-atlas__gtitle");
    // ring() 的聚焦会把浮层带开（焦点即展开）；先 Esc 归零，再用纯键盘 Enter 复现
    await page.keyboard.press("Escape");
    await page.waitForTimeout(200);
    await page.evaluate(() => document.querySelector(".shj-atlas__gtitle")?.focus());
    await page.waitForTimeout(120);
    await page.keyboard.press("Enter");
    await page.waitForTimeout(350);
    run.flyout = await page.evaluate(() => {
      const f = document.querySelector(".shj-atlas__flyout");
      if (!f) return null;
      const g = document.querySelector(".shj-atlas__gtitle");
      const r = f.getBoundingClientRect();
      return {
        items: f.querySelectorAll(".shj-atlas__item").length,
        right: Math.round(r.right),
        expanded: g?.getAttribute("aria-expanded"),
        focusInside: !!document.activeElement?.closest(".shj-atlas__flyout"),
      };
    });
    if (!run.flyout) report.failures.push(`${theme}@1024 轨态浮层未展开`);
    else if (run.flyout.right > 1024) report.failures.push(`${theme}@1024 浮层出屏`);
    else if (!run.flyout.focusInside) report.failures.push(`${theme}@1024 浮层未接管焦点`);
    let escaped = false;
    for (let i = 0; i < 20; i++) {
      await page.keyboard.press("Tab");
      const inside = await page.evaluate(() => !!document.activeElement?.closest(".shj-atlas__flyout"));
      if (!inside) {
        escaped = true;
        break;
      }
    }
    run.trap = { escaped };
    if (escaped) report.failures.push(`${theme}@1024 浮层焦点逃逸`);
    await page.keyboard.press("Escape");
    await page.waitForTimeout(250);
    run.afterEsc = await page.evaluate(() => ({
      closed: !document.querySelector(".shj-atlas__flyout"),
      onGroup: !!document.activeElement?.classList.contains("shj-atlas__gtitle"),
    }));
    if (!run.afterEsc.closed) report.failures.push(`${theme}@1024 浮层 Esc 未关闭`);
    if (!run.afterEsc.onGroup) report.failures.push(`${theme}@1024 Esc 后焦点未回到分组`);
  } catch (e) {
    report.failures.push(`${theme}@1024 脚本异常：${String(e?.message ?? e).split("\n")[0]}`);
  } finally {
    report.runs.push(run);
    await context.close();
  }
}

await browser.close();
writeFileSync(resolve(OUT, "nav-a11y.json"), JSON.stringify(report, null, 2));
console.log("failures", report.failures.length);
for (const f of report.failures) console.log(" ✗", f);
