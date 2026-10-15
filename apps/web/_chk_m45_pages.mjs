// M4/M5 前端面板实机校验（headless chromium，不碰真实鼠标键盘）
//
// 用法（在仓库根跑，截图落根目录 _tmp_m45_shots/）：
//   FLUX_TOKEN=$(python _mint_jwt.py) node apps/web/_chk_m45_pages.mjs
// 凭据 cookie：flux_token（服务端转 Bearer）+ flux.session=1（middleware 闸）
//   + flux.locale。
//
// 覆盖：?tool=emaildomains（M4 域名库）/ ?tool=regquiz（M5 题库）/
//   /cheaterbox（M5 规则准确率）/ /register（注册页闸门块）。
// 注：仓里装的是 @playwright/test（无独立 playwright 包），chromium
//   从其 re-export 取。
import { chromium } from "@playwright/test";

const BASE = process.env.FLUX_BASE ?? "http://localhost:3000";
const TOKEN = process.env.FLUX_TOKEN ?? "";
const SHOTS = process.env.FLUX_SHOTS ?? "_tmp_m45_shots";

const results = [];
function check(name, ok, extra = "") {
  results.push({ name, ok });
  console.log(`  ${ok ? "✓" : "✗"} ${name}${extra ? ` — ${extra}` : ""}`);
}

function ctxOpts() {
  return { cookies: [] };
}

// 本机 ms-playwright 里的 chromium revision 与 @playwright/test 期望不符
// （要 1243 只有 1234）——不下载浏览器，直接用系统 Edge（channel=msedge）。
const browser = await chromium.launch({ headless: true, channel: "msedge" });
const ctx = await browser.newContext(ctxOpts());
// ⚠️ newContext({cookies}) 在本版本不生效（会话落空 → 每个页都跳登录）；
// 必须 ctx.addCookies 且用 url 字段（domain:'localhost' 不生效）。
await ctx.addCookies([
  { name: "flux_token", value: TOKEN, url: BASE },
  { name: "flux.session", value: "1", url: BASE },
  { name: "flux.locale", value: "zh-CN", url: BASE },
]);
const page = await ctx.newPage();
const errors = [];
page.on("pageerror", (e) => errors.push(String(e).slice(0, 160)));

// ---------- M4：一次性邮箱域名库 ----------
await page.goto(`${BASE}/admin?tool=emaildomains`, { waitUntil: "load" });
await page.waitForSelector('h2:has-text("导入域名")', { timeout: 15000 });
check("M4 面板挂载（导入域名 h2）", true);
await page.waitForSelector('table:has-text("temp-mail.org")', {
  timeout: 15000,
});
const m4rows = await page.locator("table tbody tr").count();
check("M4 内置域名库已渲染", m4rows > 3, `${m4rows} 行可见`);
const totalTxt = await page.getByText(/共 \d+ 条/).first().innerText();
check("M4 总数文案", /共 \d+ 条/.test(totalTxt), totalTxt);
await page.screenshot({ path: `${SHOTS}/m4-emaildomains.png`, fullPage: true });

// 导入交互：粘贴两行 → 导入 → 出现新域名
const impTxt = "m4ui-check.example\nm4ui2.example";
await page.locator("textarea").first().fill(impTxt);
await page.locator('button:has-text("导入")').first().click();
await page.waitForSelector('table:has-text("m4ui-check.example")', {
  timeout: 20000,
});
check("M4 导入行已入列表", true);
// 清理：删掉两行
for (const d of ["m4ui-check.example", "m4ui2.example"]) {
  const row = page.locator(`tr:has-text("${d}")`).first();
  await row.locator('button:has-text("删除")').first().click();
  await page.waitForTimeout(600);
}
await page.waitForSelector('table:not(:has-text("m4ui-check.example"))', {
  timeout: 15000,
});
check("M4 删除行已消失", true);

// ---------- M5：注册答题题库 ----------
await page.goto(`${BASE}/admin?tool=regquiz`, { waitUntil: "load" });
await page.waitForSelector('h2:has-text("注册答题题库")', { timeout: 15000 });
check("M5 题库面板挂载", true);
await page.waitForSelector('table:has-text("分享率")', { timeout: 15000 });
const m5rows = await page.locator("table tbody tr").count();
check("M5 内置题库已渲染", m5rows >= 14, `${m5rows} 行可见`);
const statsTxt = await page.getByText(/题目 \d+ · 近/).first().innerText();
check("M5 统计条", /题目 \d+ · 近 \d+ 天答题 \d+ · 通过率 \d+%/.test(statsTxt),
  statsTxt);
await page.screenshot({ path: `${SHOTS}/m5-regquiz.png`, fullPage: true });

// ---------- M5：规则准确率（cheaterbox） ----------
await page.goto(`${BASE}/cheaterbox`, { waitUntil: "load" });
await page.waitForSelector('h2:has-text("规则准确率")', { timeout: 15000 });
check("M5 规则准确率面板挂载", true);
await page.waitForSelector('th:has-text("准确率")', { timeout: 15000 });
check("M5 准确率表头", true);
await page.screenshot({ path: `${SHOTS}/m5-accuracy.png`, fullPage: true });

// ---------- 注册页闸门块 ----------
await page.goto(`${BASE}/register`, { waitUntil: "load" });
await page.waitForSelector('input[inputmode="numeric"]', { timeout: 15000 });
check("注册页算术验证码块", true);
const qFld = await page.locator('form >> text=站规答题').count();
check("注册页答题块按开关隐藏（quiz 关闭）", qFld === 0, `命中 ${qFld}`);
await page.screenshot({ path: `${SHOTS}/register.png`, fullPage: true });

check("无页面级 JS 异常", errors.length === 0, errors.join(" | "));

await browser.close();
const bad = results.filter((r) => !r.ok);
const n = results.length;
console.log();
console.log(bad.length ? `✗ ${bad.length}/${n} 项失败` : `✓ ${n}/${n} 全部通过`);
process.exit(bad.length ? 1 : 0);
