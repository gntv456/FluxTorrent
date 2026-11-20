#!/usr/bin/env node
// 五问评审整改门禁（2026-11-20 UI 五问评审沉淀）。
//
// 背景：本轮人工评审（信息层级 / 行动引导 / 操作反馈 / 后续操作 / 容错设计）
// 挖出的缺陷，**大部分都是机器能拦的**——只是仓里没有对应门禁，于是同款缺陷
// 一再长回来。这份脚本把评审结论固化成六条规则：
//
//   R1 行级语义类必须有样式
//      torrent-dead / torrent-official / torrent-sticky / pro_* 这类「行级语义
//      类」是列表页的信息层级载体（断种 / 官种 / 置顶 / 促销）。评审实测：代码
//      里加了三处 torrent-dead，**45 个 css 文件里一条规则都没有** —— 断种种子
//      在列表里与活种长得一模一样，玩家判不出「还有没有人在做种」。
//      规则：tsx 里出现的语义类，css 里必须能查到对应选择器。
//
//   R2 正文禁用装饰色令牌
//      --sky 是品牌装饰色（白底 2.57:1，AA 不达标）；base.css 注释自己写着
//      「正文用 --sky-text（5.53:1）」。评审实测 29 处正文仍在用装饰色。
//      规则：tsx 里不得出现 text-[var(--sky)]（--sky-text / --sky-deep 合法）。
//
//   R3 aria-label 不得硬编码
//      读屏用户听到的是界面语言。中文站里冒一句英文 aria-label 等于对读屏
//      用户说外语。规则：aria-label 的字面量必须走字典（唯一豁免见 ALLOW）。
//
//   R4 失败反馈必须可诊断
//      toast 协议写明「失败可诊断：request_id 芯片（必附）」，实测 59 处
//      toast.error 只有 5 处带 requestId。规则：裸 toast.error( 的处数只许
//      减少不许增加（棘轮）。
//
//   R5 路由级错误边界
//      只有根级 error.tsx 时，任意页面运行时异常会连 (main)/layout 一起替换
//      掉 —— 玩家被丢到没有顶栏 / 导航 / 底部 Tab 的裸页。规则：每个 (main)
//      顶级段要有 error.tsx，根目录要有 global-error.tsx。
//
//   R6 表单原语采用率下限
//      806 个原生表单控件里只有 13 处走 <FormField>（1.6%），字段级错误
//      与 aria-describedby 关联基本全站缺席。规则：四项原语的使用数不得低于
//      基线（棘轮，只许升）。
//
// 用法：
//   node scripts/ui_review_guard.mjs            # 门禁（CI 一步）
//   node scripts/ui_review_guard.mjs --update   # 收紧基线（整改批次收尾时跑）
//   exit 0 = 通过；exit 1 = 有违规。
import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { globCompat as globSync } from "./lib/glob_compat.mjs";
import path from "node:path";

const ROOT = path.resolve(import.meta.dirname, "..");
const BASELINE = path.resolve(import.meta.dirname, "ui_review_baseline.json");
const UPDATE = process.argv.includes("--update");

const read = (rel) => readFileSync(path.join(ROOT, rel), "utf8");
const readOpt = (rel) => (existsSync(path.join(ROOT, rel)) ? read(rel) : "");

const base = existsSync(BASELINE)
  ? JSON.parse(readFileSync(BASELINE, "utf8"))
  : {};

let fails = 0;
const fail = (rule, msg, detail) => {
  fails++;
  console.error(`  FAIL [${rule}] ${msg}`);
  for (const d of detail ?? []) console.error(`    ${d}`);
};

const tsxFiles = globSync("apps/web/{app,components}/**/*.tsx", { cwd: ROOT })
  .map((f) => f.replace(/\\/g, "/"))
  .filter((f) => !f.includes("/.next/"));
const cssFiles = globSync("apps/web/**/*.css", { cwd: ROOT })
  .map((f) => f.replace(/\\/g, "/"))
  .filter((f) => !f.includes("/.next/") && !f.includes("node_modules"));

// ============ R1 行级语义类必须有样式 ============
{
  const SEMANTIC = [
    "torrent-dead",
    "torrent-official",
    "torrent-sticky",
    "pro_free",
    "pro_2up",
    "pro_50pct",
    "pro_30pct",
  ];
  const cssBlob = cssFiles.map((f) => readOpt(f)).join("\n");
  const missing = [];
  for (const cls of SEMANTIC) {
    const inTsx = tsxFiles.filter((f) => readOpt(f).includes(cls));
    if (!inTsx.length) continue;
    const inCss = new RegExp(`\\.${cls}\\b`).test(cssBlob);
    if (!inCss) {
      missing.push(
        `${cls}：${inTsx.length} 个 tsx 在用，css 里查不到选择器`,
      );
    }
  }
  if (missing.length) {
    fail("R1", `${missing.length} 个行级语义类没有对应样式`, missing);
  } else {
    console.log("  OK R1：行级语义类均有样式");
  }
}

// ============ R2 正文禁用装饰色 ============
{
  const hits = [];
  for (const f of tsxFiles) {
    readOpt(f)
      .split("\n")
      .forEach((l, i) => {
        if (/text-\[var\(--sky\)\]/.test(l)) {
          hits.push(`${f}:${i + 1}  ${l.trim().slice(0, 90)}`);
        }
      });
  }
  if (hits.length) {
    fail("R2", `${hits.length} 处正文用了装饰色 --sky（白底 2.57:1，AA 不达标）`, hits);
  } else {
    console.log("  OK R2：正文未使用装饰色 --sky");
  }
}

// ============ R3 aria-label 不得硬编码 ============
{
  // 纯符号/语言中立的白名单：字母索引（A-Z）、省略号（⋯）
  const ALLOW = new Set(["A-Z", "⋯"]);
  const hits = [];
  for (const f of tsxFiles) {
    readOpt(f)
      .split("\n")
      .forEach((l, i) => {
        for (const m of l.matchAll(/aria-label="([^"]*)"/g)) {
          const v = m[1].trim();
          if (!v || ALLOW.has(v)) continue;
          if (/[A-Za-z\u4e00-\u9fff]/.test(v)) {
            hits.push(`${f}:${i + 1}  aria-label="${v}"`);
          }
        }
      });
  }
  if (hits.length) {
    fail("R3", `${hits.length} 处 aria-label 硬编码（读屏会念外语/未本地化）`, hits);
  } else {
    console.log("  OK R3：aria-label 全部走字典");
  }
}

// ============ R4 失败反馈必须可诊断 ============
{
  let bare = 0;
  const detail = [];
  for (const f of tsxFiles) {
    readOpt(f)
      .split("\n")
      .forEach((l, i) => {
        if (!l.includes("toast.error(")) return;
        // 同行内出现 requestId 视为合规（多行调用由下方的跨行粗判兜底）
        if (/requestId/.test(l)) return;
        bare++;
        detail.push(`${f}:${i + 1}  ${l.trim().slice(0, 90)}`);
      });
  }
  // 跨行粗判：toast.error( 之后 220 字符内没有 requestId 也算裸调用
  for (const f of tsxFiles) {
    const src = readOpt(f);
    const re = /toast\.error\(/g;
    let m;
    while ((m = re.exec(src))) {
      const win = src.slice(m.index, m.index + 220);
      if (win.includes("requestId")) continue;
      if (win.includes("toast.error(")) continue;
      const line = src.slice(0, m.index).split("\n").length;
      bare++;
      detail.push(`${f}:${line}  toast.error( 跨行调用未带 requestId`);
    }
  }
  const prev = base.bareToastError ?? bare;
  if (bare > prev) {
    fail("R4", `裸 toast.error 增加到 ${bare} 处（基线 ${prev}）`, detail.slice(0, 20));
  } else {
    console.log(
      `  OK R4：裸 toast.error ${bare} 处（基线 ${prev}` +
        (bare < prev ? `，减少 ${prev - bare}` : "") + "）",
    );
  }
  base.bareToastError = UPDATE ? bare : prev;
}

// ============ R5 路由级错误边界 ============
{
  const missing = [];
  const mainDirs = globSync("apps/web/app/(main)/*/", { cwd: ROOT, nodir: false })
    .map((f) => f.replace(/\\/g, "/"))
    .filter((f) => f.endsWith("/"))
    .map((f) => f.replace(/\/$/, ""));
  for (const d of mainDirs) {
    // 纯参数段（[id]）不是独立段，跳过
    const seg = d.split("/").pop();
    if (!seg || seg.startsWith("[") || seg.startsWith("(")) continue;
    if (!existsSync(path.join(ROOT, d, "error.tsx"))) {
      missing.push(`${d}/error.tsx`);
    }
  }
  if (!existsSync(path.join(ROOT, "apps/web/app/global-error.tsx"))) {
    missing.push("apps/web/app/global-error.tsx");
  }
  if (missing.length) {
    fail("R5", `${missing.length} 处缺路由级错误边界（页面异常会打掉整站外壳）`, missing);
  } else {
    console.log("  OK R5：路由级错误边界齐备");
  }
}

// ============ R6 表单原语采用率下限 ============
{
  const PRIMS = ["<FormField", "<FieldErrors", "<SubmitButton", "<StatePanel", "<ErrorPanel"];
  const counts = {};
  for (const p of PRIMS) {
    let n = 0;
    for (const f of tsxFiles) {
      const c = readOpt(f).split(p).length - 1;
      n += c;
    }
    counts[p] = n;
  }
  const floors = base.primitiveFloors ?? {};
  const regress = [];
  for (const [p, n] of Object.entries(counts)) {
    const prev = floors[p];
    if (prev === undefined) continue;
    if (n < prev) {
      regress.push(`${p}：${n} 处（基线 ${prev}，少 ${prev - n}）`);
    }
  }
  if (regress.length) {
    fail("R6", "表单原语采用率回退", regress);
  } else {
    console.log(
      "  OK R6：原语采用率未回退 " +
        Object.entries(counts)
          .map(([p, n]) => `${p}=${n}`)
          .join(" "),
    );
  }
  base.primitiveFloors = UPDATE ? counts : floors;
}


// ============ 第二轮评审门禁（R7–R12） ============

// R7：赌博/抽奖页不得硬编码与后端矛盾的规则数字（赔率表曾写 1-100 数域，
// 实际是三骰 3-10/11-18/豹子判负）。检测「1-49」「52-100」「50 / 51」这类旧口径。
{
  const banned = ["1-49", "52-100", "50 / 51", "50·51", "平局返本"];
  const hits = [];
  for (const f of tsxFiles) {
    if (!f.includes("/games/") && !f.includes("gacha")) continue;
    const src = readOpt(f);
    banned.forEach((pat, idx) => {
      if (src.includes(pat)) {
        const line = src.slice(0, src.indexOf(pat)).split("\n").length;
        hits.push(`${f}:${line} 硬编码旧规则「${pat}」(R7#${idx})`);
      }
    });
  }
  if (hits.length) fail("R7", "赌博/抽奖页出现与后端矛盾的规则文案", hits.slice(0, 15));
  else console.log("  OK R7：赌博页无硬编码旧规则文案");
}

// R8：网络错误 catch 分支不得换新幂等键（响应丢失时重试=二次扣款）。
{
  const hits = [];
  for (const f of tsxFiles) {
    const src = readOpt(f);
    if (!src.includes("idempotency_key")) continue;
    // 检测 catch 之后立刻出现 clientUuid()/Date.now() 生成新键
    const re = /catch\s*\([^)]*\)\s*\{[\s\S]{0,180}?(clientUuid\(\)|Date\.now\(\))/g;
    let m;
    while ((m = re.exec(src))) {
      const line = src.slice(0, m.index).split("\n").length;
      hits.push(`${f}:${line} catch 分支可能重生成幂等键`);
    }
  }
  if (hits.length) fail("R8", "网络错误后重生成幂等键（重复扣款风险）", hits.slice(0, 15));
  else console.log("  OK R8：无 catch 分支重生成幂等键");
}

// R9：危险操作（全员/批量/封禁/裁决）必须接 ConfirmDialog。
{
  const PRIMS = ["ConfirmDialog", "AskDialog", "useConfirm"];
  const counts = {};
  for (const p of PRIMS) {
    let n = 0;
    for (const f of tsxFiles) n += readOpt(f).split(p).length - 1;
    counts[p] = n;
  }
  const floors = base.confirmFloors ?? {};
  const regress = [];
  for (const [p, n] of Object.entries(counts)) {
    if (floors[p] !== undefined && n < floors[p]) {
      regress.push(`${p}：${n} < 基线 ${floors[p]}`);
    }
  }
  if (regress.length) fail("R9", "二次确认原语采用率回退", regress);
  else console.log("  OK R9：二次确认原语未回退");
  base.confirmFloors = UPDATE ? counts : floors;
}

// R10：catch 中禁止 setX([]) 而无 failed 标识（错误被当空态）。
{
  const hits = [];
  for (const f of tsxFiles) {
    const src = readOpt(f);
    const re = /catch\s*(?:\([^)]*\))?\s*\{[\s\S]{0,200}?set[A-Z]\w+\(\[\]\)/g;
    let m;
    while ((m = re.exec(src))) {
      const win = src.slice(m.index, m.index + 500);
      if (!/failed|setErr|setFailed/.test(win)) {
        const line = src.slice(0, m.index).split("\n").length;
        hits.push(`${f}:${line} catch 置空数组且无 failed 标识`);
      }
    }
  }
  const prev = base.bareCatchEmpty ?? hits.length;
  if (hits.length > prev) fail("R10", `catch 置空数组（无错误标识）增至 ${hits.length}（基线 ${prev}）`, hits.slice(0, 15));
  else console.log(`  OK R10：catch 置空数组 ${hits.length} 处（基线 ${prev}）`);
  base.bareCatchEmpty = UPDATE ? hits.length : prev;
}

// R11：请登录提示必须带登录入口（死胡同检测）。
{
  const hits = [];
  for (const f of tsxFiles) {
    const src = readOpt(f);
    if (!src.includes("pleaseLogin")) continue;
    // pleaseLogin 附近 400 字符内应有 /login 链接
    const i = src.indexOf("pleaseLogin");
    const win = src.slice(Math.max(0, i - 400), i + 400);
    if (!win.includes("/login")) hits.push(`${f} 的 pleaseLogin 无登录入口`);
  }
  if (hits.length) fail("R11", "「请登录」无补救路径", hits);
  else console.log("  OK R11：请登录提示均有登录入口");
}

// R12：移动端 toast 不得在弹窗打开时停留顶部（遮挡关闭钮）。
{
  const ui = readOpt("apps/web/app/styles/ui.css");
  const ok = /body:has\([^)]*scrim[^)]*\)\s*\.ui-toast__vp[\s\S]{0,120}bottom:/.test(ui);
  if (!ok) fail("R12", "移动端 toast 与弹窗关闭钮重叠");
  else console.log("  OK R12：弹窗打开时 toast 移到底部");
}

// ============ 收口 ============
writeFileSync(BASELINE, JSON.stringify(base, null, 2) + "\n");
if (fails) {
  console.error(`\n五问评审门禁：${fails} 条规则未通过`);
  process.exit(1);
}
console.log("\n五问评审门禁通过");
