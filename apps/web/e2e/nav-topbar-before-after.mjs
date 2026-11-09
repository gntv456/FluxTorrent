import { chromium } from "@playwright/test";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

/**
 * 三套主题顶栏「胶囊/透明胶囊 → 56px 平铺 + 底部 2px 标尺」前后对照。
 *
 * before 的还原方式：把 HEAD 版 layout.css / theme-tide.css 中与
 * .mainmenu / .mainmenu-link 相关的规则原样注入（下方 OLD_CSS 逐条注明
 * 出处行号），不做臆造。after 即当前工作区真实渲染。
 */
const BASE = process.env.E2E_BASE_URL ?? "http://localhost:3000";
const HERE = dirname(fileURLToPath(import.meta.url));
const OUT = resolve(HERE, "../../../design/_shots/nav-2026-11-09");
mkdirSync(OUT, { recursive: true });

const OLD_CSS = `
/* —— HEAD: apps/web/app/styles/layout.css L37-76 —— */
.mainmenu { display:flex; flex-wrap:wrap; align-items:center; gap:8px; min-height:56px; list-style:none; }
.mainmenu-link {
  display:flex; min-height:42px; flex:0 0 auto; align-items:center;
  padding:6px 14px; color:var(--compat-ink); font-size:15px; font-weight:700;
  white-space:nowrap; border:1px solid var(--compat-line); border-radius:10px;
  background:var(--row-veil-soft);
  box-shadow:0 2px 0 rgba(70,110,165,.04);
}
.mainmenu-link:hover {
  color:var(--compat-orange); border-color:var(--compat-orange);
  background:var(--surface-raised); transform:translateY(-1px);
}
.mainmenu-link[data-active="true"] {
  color:var(--text-on-accent); border-color:transparent;
  background:var(--grad-aurora); box-shadow:0 5px 13px var(--accent-shadow);
}
/* —— HEAD: apps/web/app/styles/theme-tide.css L503-532 —— */
.tide-header .mainmenu-link {
  border-radius:8px; border-color:transparent; background:transparent; font-weight:600;
}
.tide-header .mainmenu-link:hover {
  background:var(--surface-hover); color:var(--tide-deep); transform:translateY(-1px);
}
.tide-header .mainmenu-link[data-active="true"] {
  position:relative; background:var(--sky-soft); color:var(--tide-deep);
  border-color:transparent; box-shadow:none; font-weight:700;
}
.tide-header .mainmenu-link[data-active="true"]::before {
  content:""; position:absolute; left:10px; right:10px; top:3px;
  height:2px; border-radius:2px; background:var(--tide-sea);
}
/* 新版底部标尺在 before 里不存在（顶栏未改前的形态） */
.mainmenu-link::after { content:none !important; }
`;

const THEMES = ["brand", "brand-night", "shanhaijing"];
const login = await fetch(`${BASE}/api/v1/auth/login`, {
  method: "POST",
  headers: { "content-type": "application/json" },
  body: JSON.stringify({ username: "root", password: "password123" }),
}).then((r) => r.json());
const token = login.data.token;

const browser = await chromium.launch({ channel: "msedge" });
const report = { shots: [], style: [] };

for (const theme of THEMES) {
  const context = await browser.newContext({ viewport: { width: 1440, height: 900 } });
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
  await page.goto(`${BASE}/torrents`, { waitUntil: "domcontentloaded" });
  await page.waitForSelector(".mainmenu-link", { state: "visible" });
  await page.waitForTimeout(600);
  await page.mouse.move(4, 800);
  await page.waitForTimeout(150);

  const header = page.locator("header").first();
  const readStyle = () =>
    page.evaluate(() => {
      const a = document.querySelector(".mainmenu-link[aria-current='page']") ||
        document.querySelector(".mainmenu-link");
      const cs = getComputedStyle(a);
      return {
        height: Math.round(a.getBoundingClientRect().height),
        minHeight: cs.minHeight,
        radius: cs.borderRadius,
        border: `${cs.borderTopWidth} ${cs.borderTopStyle} ${cs.borderTopColor}`,
        background: cs.backgroundColor,
        fontWeight: cs.fontWeight,
        after: getComputedStyle(a, "::after").content,
        afterH: getComputedStyle(a, "::after").height,
        before: getComputedStyle(a, "::before").content,
        menuGap: getComputedStyle(document.querySelector(".mainmenu")).gap,
      };
    });

  report.style.push({ theme, after: await readStyle(), themeApplied: await page.evaluate(() => document.documentElement.dataset.theme) });
  await header.screenshot({ path: resolve(OUT, `topbar-${theme}-after.png`) });
  report.shots.push(`topbar-${theme}-after.png`);

  await page.addStyleTag({ content: OLD_CSS });
  await page.waitForTimeout(250);
  report.style.at(-1).before = await readStyle();
  await header.screenshot({ path: resolve(OUT, `topbar-${theme}-before.png`) });
  report.shots.push(`topbar-${theme}-before.png`);

  await context.close();
}

await browser.close();
writeFileSync(resolve(OUT, "topbar-before-after.json"), JSON.stringify(report, null, 2));
console.log(JSON.stringify(report.style, null, 1));
