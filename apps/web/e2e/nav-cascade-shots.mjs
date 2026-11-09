// 级联子菜单真实展开态截图（2026-11-09 补拍）
// 触发方式说明：
//   · 桌面级联：page.mouse.move 到目标中心 → 命中 React onPointerEnter，
//     走的是用户真实 hover 路径（不注入类名、不预置 data-open）。
//   · 抽屉：点击汉堡触发条 → 点击分组 chevron（真实 click 事件）。
//   · 山海经轨态浮层：mouse.move 到分组标题 → onMouseEnter。
//   · 侧栏钻取：点击「浏览学段」与钻取行（真实 click）。
// 每张图附面板矩形/裁切/文字截断实测，写入 nav-cascade.json。
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

// 滚动条证据：headless 与 Edge 默认都是覆盖式滚动条，截图里不出现轨道。
// 长列表两张单独用有头模式（Windows 经典 17px 滚动条）拍，其余保持 headless。
const browser = await chromium.launch({
  channel: "msedge",
  headless: true,
  args: ["--disable-features=OverlayScrollbar,FluentOverlayScrollbars"],
});
const HEADED_ARGS = {
  channel: "msedge",
  headless: false,
  args: ["--disable-features=OverlayScrollbar,FluentOverlayScrollbars"],
};
let lastMouse = { x: 0, y: 0 };
const report = { base: BASE, generatedAt: new Date().toISOString(), shots: [], issues: [] };

async function open(theme, width, height, path = "/", br = browser) {
  lastMouse = { x: 4, y: 4 };
  const context = await br.newContext({ viewport: { width, height } });
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
  page.setDefaultTimeout(20000);
  await page.goto(`${BASE}${path}`, { waitUntil: "domcontentloaded" });
  await page.waitForSelector(".site-shell, main", { state: "visible", timeout: 45000 });
  await page.waitForTimeout(700);
  return { context, page };
}

/** 按可见文本找元素中心（只看 visibility 可见的 .nd-list 内行，
 *  避免命中隐藏层里同名节点）。 */
async function pointOf(page, selector, label, index = 0) {
  const p = await page.evaluate(
    ({ selector, label, index }) => {
      const norm = (s) => (s || "").replace(/[▾›‹▸✕]/g, "").trim();
      const shown = (el) => {
        const cs = getComputedStyle(el);
        if (cs.display === "none" || cs.visibility === "hidden") return false;
        const list = el.closest(".nd-list");
        if (list) {
          const ls = getComputedStyle(list);
          if (ls.visibility === "hidden" || ls.display === "none") return false;
        }
        return el.getBoundingClientRect().width > 0;
      };
      const els = Array.from(document.querySelectorAll(selector)).filter(shown);
      const hits = els.filter((e) => norm(e.textContent) === label);
      const el = hits[index];
      if (!el) return { error: "not-found", labels: els.map((e) => norm(e.textContent)) };
      const r = el.getBoundingClientRect();
      const lists = Array.from(document.querySelectorAll(".nd-list")).filter(shown);
      const list = el.closest(".nd-list");
      const lr = list ? list.getBoundingClientRect() : { left: 0, width: 0 };
      return {
        x: r.left + r.width / 2,
        y: r.top + r.height / 2,
        listIndex: list ? lists.indexOf(list) : -1,
        listLeft: lr.left,
        listWidth: lr.width,
      };
    },
    { selector, label, index },
  );
  if (p.error) throw new Error(`找不到 ${selector} 「${label}」；可见项=${p.labels.join("/")}`);
  return p;
}

/** 真实 hover，但分两段走位，避免斜穿父面板右缘时误触相邻行：
 *  第一层：先竖直下落（指针始终在触发条内），再水平到行中心；
 *  飞入层：先水平进入本层左缘（保持父行高度），再竖直，再水平到目标。
 *  仍是 page.mouse.move 的真实指针路径，不注入任何类名。 */
async function hover(page, selector, label, index = 0) {
  const p = await pointOf(page, selector, label, index);
  if (p.listIndex <= 0) {
    await page.mouse.move(lastMouse.x, p.y, { steps: 8 });
    await page.mouse.move(p.x, p.y, { steps: 8 });
  } else {
    const entryX = p.listLeft + Math.min(20, p.listWidth / 2);
    await page.mouse.move(entryX, lastMouse.y, { steps: 8 });
    await page.mouse.move(entryX, p.y, { steps: 8 });
    await page.mouse.move(p.x, p.y, { steps: 8 });
  }
  lastMouse = { x: p.x, y: p.y };
}

async function waitLists(page, n) {
  await page.waitForFunction(
    (n) =>
      Array.from(document.querySelectorAll(".nd-list")).filter(
        (el) => getComputedStyle(el).visibility === "visible",
      ).length >= n,
    n,
    { timeout: 5000 },
  );
  // .nd-list 过渡 140ms，等稳定再截
  await page.waitForTimeout(240);
}

/** 当前同框的每一层行文本（用于证明展开的是预期链路而非错层） */
function visibleChain(page) {
  return page.evaluate(() =>
    Array.from(document.querySelectorAll(".nd-list"))
      .filter((el) => getComputedStyle(el).visibility === "visible")
      .map((ul) =>
        Array.from(ul.querySelectorAll(":scope > li > .nd-row"))
          .map((r) => (r.querySelector(".nd-label")?.textContent || "").trim())
          .join("/"),
      ),
  );
}

const CLIP_MARK = (rects) => {
  for (const r of rects) {
    const d = document.createElement("div");
    const l = Math.max(0, r.left);
    const t = Math.max(8, r.top);
    d.style.cssText =
      `position:fixed;left:${l}px;top:${t}px;width:${r.width}px;height:${r.height}px;` +
      "border:2px dashed #e11d48;z-index:2147483646;pointer-events:none;";
    const tag = document.createElement("span");
    tag.textContent = "面板超出视口";
    tag.style.cssText =
      "position:absolute;top:-19px;left:0;background:#e11d48;color:#fff;" +
      "font:12px/16px sans-serif;padding:2px 6px;border-radius:3px;white-space:nowrap;";
    d.appendChild(tag);
    document.body.appendChild(d);
  }
};

async function measure(page) {
  return page.evaluate(() => {
    const vis = (el) => {
      const cs = getComputedStyle(el);
      return cs.display !== "none" && cs.visibility === "visible";
    };
    const lists = Array.from(document.querySelectorAll(".nd-list"))
      .filter(vis)
      .map((ul) => {
        const r = ul.getBoundingClientRect();
        const cs = getComputedStyle(ul);
        return {
          left: Math.round(r.left),
          top: Math.round(r.top),
          right: Math.round(r.right),
          bottom: Math.round(r.bottom),
          width: Math.round(r.width),
          height: Math.round(r.height),
          rows: ul.querySelectorAll(":scope > li").length,
          overflowY: cs.overflowY,
          barWidth: ul.offsetWidth - ul.clientWidth,
          scroll: {
            top: Math.round(ul.scrollTop),
            height: ul.scrollHeight,
            client: ul.clientHeight,
            scrollable: ul.scrollHeight > ul.clientHeight + 1,
          },
        };
      });
    const trunc = Array.from(document.querySelectorAll(".nd-label"))
      .filter((el) => vis(el) && el.scrollWidth > el.clientWidth + 1)
      .map((el) => el.textContent.trim());
    return {
      lists,
      trunc,
      viewport: { w: innerWidth, h: innerHeight },
      pageOverflowX: document.documentElement.scrollWidth > innerWidth + 1,
      docScrollWidth: document.documentElement.scrollWidth,
    };
  });
}

async function shot(page, name, meta) {
  const m = await measure(page);
  const clipped = m.lists.filter(
    (l) => l.left < 0 || l.right > m.viewport.w || l.top < 0 || l.bottom > m.viewport.h,
  );
  if (clipped.length) {
    await page.evaluate(CLIP_MARK, clipped);
    await page.waitForTimeout(80);
  }
  await page.screenshot({ path: resolve(OUT, `${name}.png`) });
  if (clipped.length) {
    await page.evaluate(() =>
      document.querySelectorAll('[style*="2147483646"]').forEach((el) => el.remove()),
    );
  }
  const rec = { name, file: `${name}.png`, ...meta, ...m, clipped };
  report.shots.push(rec);
  for (const c of clipped) {
    report.issues.push(`${name}: 面板 left=${c.left} right=${c.right} top=${c.top} bottom=${c.bottom} 超出视口`);
  }
  if (m.trunc.length) {
    report.issues.push(`${name}: 文字截断 ${m.trunc.join(" / ")}`);
  }
  console.log(
    `📸 ${name} · 面板 ${m.lists.length} 层 · ` +
      m.lists.map((l) => `${l.left}-${l.right}${l.scroll.scrollable ? `(滚${l.scroll.top})` : ""}`).join(" | ") +
      (clipped.length ? " ⚠️ 裁切" : ""),
  );
}

// ── 1. 桌面级联：四主题 × L1/L2/L3（1440） ────────────────────────────
for (const theme of THEMES) {
  console.log(`\n▶ 桌面级联 ${theme}@1440`);
  const { context, page } = await open(theme, 1440, 1000);

  // L1：hover 顶栏一级「资源库」→ 第一列（全部/资源/官种）
  await hover(page, ".mainmenu-link", "资源库");
  await waitLists(page, 1);
  await shot(page, `cascade-L1-library-${theme}-1440`, {
    theme,
    width: 1440,
    path: "/",
    trigger: "hover",
    interaction: "鼠标悬停顶栏一级「资源库」，第一列正下弹出",
  });

  // L2：hover 第一列「资源」→ 右侧飞出分类叶子（16 行）
  await hover(page, ".nd-list .nd-row", "资源");
  await waitLists(page, 2);
  await shot(page, `cascade-L2-library-${theme}-1440`, {
    theme,
    width: 1440,
    path: "/",
    trigger: "hover",
    interaction: "一级面板内悬停「资源」，右飞第二列分类",
  });

  // L3：教育资源是全站最深（学段→年级→科目）；资源库只有两列。
  await hover(page, ".mainmenu-link", "教育资源");
  await waitLists(page, 1);
  await hover(page, ".nd-list .nd-row", "初中");
  await waitLists(page, 2);
  await hover(page, ".nd-list .nd-row", "初二");
  await waitLists(page, 3);
  const chain = await visibleChain(page);
  if (!chain[0]?.includes("初中") || !chain[1]?.includes("初二")) {
    throw new Error(`${theme} 展开链路错层：${JSON.stringify(chain)}`);
  }
  await shot(page, `cascade-L3-edu-${theme}-1440`, {
    theme,
    width: 1440,
    path: "/",
    trigger: "hover",
    interaction: "悬停「教育资源」→ 初中 → 初二，三列同时展开（顶栏+三级=四级）",
    chain,
  });

  await context.close();
}

// ── 2. 长列表内层滚动（brand / shanhaijing；有头模式拍经典滚动条） ──────
const headed = await chromium.launch(HEADED_ARGS);
for (const theme of ["brand", "shanhaijing"]) {
  console.log(`\n▶ 长列表滚动 ${theme}@1440`);
  const { context, page } = await open(theme, 1440, 1000, "/", headed);
  await hover(page, ".mainmenu-link", "资源库");
  await waitLists(page, 1);
  await hover(page, ".nd-list .nd-row", "资源");
  await waitLists(page, 2);
  const c = await page.evaluate(() => {
    const el = Array.from(document.querySelectorAll(".nd-list--scroll")).find(
      (x) => getComputedStyle(x).visibility === "visible",
    );
    const r = el.getBoundingClientRect();
    return { x: r.left + r.width / 2, y: r.top + 80 };
  });
  await page.mouse.move(c.x, c.y, { steps: 4 });
  lastMouse = c;
  await page.mouse.wheel(0, 320);
  // 有头模式经典滚动条常驻；先让出一帧确保滚动位置已提交再截
  await page.evaluate(() => new Promise((r) => requestAnimationFrame(() => r())));
  await shot(page, `cascade-L2-library-scroll-${theme}-1440`, {
    theme,
    width: 1440,
    path: "/",
    trigger: "hover + wheel",
    interaction: "展开「资源」16 行分类面板后，指针在面板内滚轮下滑 320px",
  });
  await context.close();
}
await headed.close();

// ── 3. 深层页父链上溯点亮（brand 1440） ───────────────────────────────
{
  console.log("\n▶ 父链点亮 brand@1440");
  const path = "/torrents?category_id=103&grade_id=8&sec_subject=100";
  const { context, page } = await open("brand", 1440, 1000, path);
  await hover(page, ".mainmenu-link", "教育资源");
  await waitLists(page, 1);
  await hover(page, ".nd-list .nd-row", "初中");
  await waitLists(page, 2);
  await hover(page, ".nd-list .nd-row", "初二");
  await waitLists(page, 3);
  const lit = await page.evaluate(() => ({
    top: Array.from(document.querySelectorAll('.mainmenu-link[data-active="true"]')).map((e) =>
      e.textContent.replace(/[▾]/g, "").trim(),
    ),
    rows: Array.from(document.querySelectorAll(".nd-row.is-lit")).map((e) =>
      e.textContent.replace(/[›]/g, "").trim(),
    ),
    current: Array.from(document.querySelectorAll('[aria-current="page"]')).map((e) =>
      (e.textContent || "").trim().slice(0, 20),
    ),
  }));
  console.log(`  点亮：顶栏 ${lit.top.join("/")} ｜ 行 ${lit.rows.join(" > ")}`);
  await shot(page, "active-chain-edu-brand-1440", {
    theme: "brand",
    width: 1440,
    path,
    trigger: "hover",
    interaction: "直接访问「初中二年级 · 数学」筛选 URL，再悬停展开到该叶子，查看父链点亮",
    lit,
    chain: await visibleChain(page),
  });
  await context.close();
}

// ── 4. 窄屏：375 抽屉逐级展开（brand） ───────────────────────────────
{
  console.log("\n▶ 抽屉 brand@375");
  const { context, page } = await open("brand", 375, 844);
  await page.evaluate(() => {
    const b = Array.from(document.querySelectorAll(".navdrawer-trigger")).find(
      (x) => x.getBoundingClientRect().width > 0,
    );
    b?.click();
  });
  await page.waitForSelector(".navdrawer", { state: "visible", timeout: 8000 });
  await page.waitForTimeout(400);

  const expand = async (label) => {
    const ok = await page.evaluate((label) => {
      const b = document.querySelector(`.navdrawer__expand[aria-label="${label}"]`);
      if (!b) return null;
      b.click();
      return b.getAttribute("aria-controls");
    }, label);
    if (!ok) throw new Error(`抽屉里找不到展开钮 ${label}`);
    await page.waitForFunction(
      (id) => {
        const p = document.getElementById(id);
        return p && !p.hidden;
      },
      ok,
      { timeout: 4000 },
    );
    await page.waitForTimeout(260);
  };

  await expand("资源库");
  await shot(page, "drawer-L1-library-brand-375", {
    theme: "brand",
    width: 375,
    path: "/",
    trigger: "click",
    interaction: "汉堡按钮打开抽屉 → 点「资源库」右侧 chevron，展开第一层（全部/资源/官种）",
  });

  await expand("资源");
  const drawer = await page.evaluate(() => {
    const rows = Array.from(document.querySelectorAll(".navdrawer__tree .navdrawer__row"));
    return {
      widths: Array.from(document.querySelectorAll(".navdrawer__row"))
        .slice(0, 6)
        .map((r) => Math.round(r.getBoundingClientRect().height)),
      indent: rows.slice(0, 8).map((r) => r.style.paddingInlineStart),
      expanded: Array.from(document.querySelectorAll(".navdrawer__expand"))
        .filter((b) => b.getAttribute("aria-expanded") === "true")
        .map((b) => b.getAttribute("aria-label")),
      visibleLeaves: Array.from(document.querySelectorAll(".navdrawer__sub:not([hidden]) > li > a"))
        .length,
    };
  });
  await shot(page, "drawer-L2-library-brand-375", {
    theme: "brand",
    width: 375,
    path: "/",
    trigger: "click",
    interaction: "在第一层点「资源」chevron，展开第二层分类叶子；逐级缩进 + 回到上层可收起",
    drawer,
  });
  await context.close();
}

// ── 5. 1024 右飞不出屏实证（brand） ──────────────────────────────────
{
  console.log("\n▶ 1024 级联 brand");
  const { context, page } = await open("brand", 1024, 900);
  await hover(page, ".mainmenu-link", "资源库");
  await waitLists(page, 1);
  await hover(page, ".nd-list .nd-row", "资源");
  await waitLists(page, 2);
  await shot(page, "cascade-L2-library-brand-1024", {
    theme: "brand",
    width: 1024,
    path: "/",
    trigger: "hover",
    interaction: "1024 视口悬停「资源库 → 资源」，右飞第二列，验证右缘不出屏",
  });

  await hover(page, ".nd-list .nd-row", "官种");
  await waitLists(page, 2);
  await shot(page, "cascade-L2-lastrow-brand-1024", {
    theme: "brand",
    width: 1024,
    path: "/",
    trigger: "hover",
    interaction: "1024 视口悬停最后一行「官种」（≤1279 规则下子层向左飞），验证左缘是否出屏",
  });
  await context.close();
}

// ── 6. 768 山海经轨态分组浮层 ────────────────────────────────────────
{
  console.log("\n▶ 轨态浮层 shanhaijing@768");
  const { context, page } = await open("shanhaijing", 768, 900);
  await page.waitForSelector(".shj-atlas", { state: "visible", timeout: 8000 });
  const p = await page.evaluate(() => {
    const g = document.querySelector(".shj-atlas__gtitle");
    const r = g.getBoundingClientRect();
    return { x: r.left + r.width / 2, y: r.top + r.height / 2 };
  });
  await page.mouse.move(p.x, p.y, { steps: 10 });
  await page.waitForSelector(".shj-atlas__flyout", { state: "visible", timeout: 5000 });
  await page.waitForTimeout(260);
  const m = await page.evaluate(() => {
    const a = document.querySelector(".shj-atlas").getBoundingClientRect();
    const f = document.querySelector(".shj-atlas__flyout").getBoundingClientRect();
    return {
      atlas: { left: Math.round(a.left), width: Math.round(a.width) },
      flyout: { left: Math.round(f.left), right: Math.round(f.right), width: Math.round(f.width) },
      entries: document.querySelectorAll(".shj-atlas__flyout .shj-atlas__item").length,
    };
  });
  await shot(page, "rail-flyout-discover-shanhaijing-768", {
    theme: "shanhaijing",
    width: 768,
    path: "/",
    trigger: "hover",
    interaction: "768 轨态（64px）悬停分组标题「资源发现」，右侧浮层带文字条目",
    rail: m,
  });
  await context.close();
}

// ── 7. 山海经侧栏教育资源钻取（1440） ────────────────────────────────
{
  console.log("\n▶ 侧栏钻取 shanhaijing@1440");
  const { context, page } = await open("shanhaijing", 1440, 1000);
  await page.waitForSelector(".shj-atlas__eduedit", { state: "visible", timeout: 8000 });
  await page.evaluate(() => document.querySelector(".shj-atlas__eduedit").click());
  await page.waitForSelector(".shj-atlas__drill", { state: "visible" });
  await page.waitForTimeout(260);
  await shot(page, "sidebar-drill-stage-shanhaijing-1440", {
    theme: "shanhaijing",
    width: 1440,
    path: "/",
    trigger: "click",
    interaction: "侧栏「教育资源」下的「浏览学段」→ 第 1 页学段列表（返回条：‹ 返回 教育资源）",
    level: "学段",
  });

  const drill = async (label) => {
    await page.evaluate((label) => {
      const b = Array.from(document.querySelectorAll(".shj-atlas__drillrow")).find(
        (x) => (x.textContent || "").includes(label),
      );
      b?.click();
    }, label);
    await page.waitForTimeout(300);
  };
  await drill("小学");
  const gradeInfo = await page.evaluate(() => ({
    back: document.querySelector(".shj-atlas__drillback")?.textContent.replace(/\s+/g, " ").trim(),
    rows: document.querySelectorAll(".shj-atlas__drill .shj-atlas__list > li").length,
  }));
  await shot(page, "sidebar-drill-grade-shanhaijing-1440", {
    theme: "shanhaijing",
    width: 1440,
    path: "/",
    trigger: "click",
    interaction: "学段页点「小学」→ 第 2 页年级列表（返回条：‹ 返回 小学）",
    level: "年级",
    info: gradeInfo,
  });

  await drill("一年级");
  const subjectInfo = await page.evaluate(() => ({
    back: document.querySelector(".shj-atlas__drillback")?.textContent.replace(/\s+/g, " ").trim(),
    rows: document.querySelectorAll(".shj-atlas__drill .shj-atlas__list > li").length,
  }));
  await shot(page, "sidebar-drill-subject-shanhaijing-1440", {
    theme: "shanhaijing",
    width: 1440,
    path: "/",
    trigger: "click",
    interaction: "年级页点「一年级」→ 第 3 页科目列表（返回条：‹ 返回 一年级）",
    level: "科目",
    info: subjectInfo,
  });
  await context.close();
}

writeFileSync(resolve(OUT, "nav-cascade.json"), JSON.stringify(report, null, 1));
console.log(`\n完成 ${report.shots.length} 张 → ${OUT}`);
console.log(`异常 ${report.issues.length} 条`);
for (const i of report.issues) console.log(`  ⚠️ ${i}`);
await browser.close();
