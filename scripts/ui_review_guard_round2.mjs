// 五问评审门禁·第二轮规则（R7–R12）——从 ui_review_guard.mjs 拆出
//（2026-11-20：主件 357 行超 300 行门禁，按守卫拆件先例分家）。
//
// 共享上下文由主件经 uiReviewCtx() 注入：ROOT/fail/tsxFiles/readOpt/base/UPDATE。
// 本文件只导出 runRound2(ctx)；基线对象 base 由主件统一写盘。
import path from "node:path";

export function uiReviewCtx() {
  const ROOT = path.resolve(import.meta.dirname, "..");
  return {
    ROOT,
    BASELINE: path.resolve(ROOT, "scripts/ui_review_baseline.json"),
    UPDATE: process.argv.includes("--update"),
    fails: 0,
    fail(rule, msg, detail) {
      this.fails++;
      console.error(`  FAIL [${rule}] ${msg}`);
      for (const d of detail ?? []) console.error(`    ${d}`);
    },
  };
}

export function runRound2({ fail, tsxFiles, readOpt, base, UPDATE }) {
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
      const re =
        /catch\s*\([^)]*\)\s*\{[\s\S]{0,180}?(clientUuid\(\)|Date\.now\(\))/g;
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
      const re =
        /catch\s*(?:\([^)]*\))?\s*\{[\s\S]{0,200}?set[A-Z]\w+\(\[\]\)/g;
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
    if (hits.length > prev) {
      fail(
        "R10",
        `catch 置空数组（无错误标识）增至 ${hits.length}（基线 ${prev}）`,
        hits.slice(0, 15),
      );
    } else {
      console.log(`  OK R10：catch 置空数组 ${hits.length} 处（基线 ${prev}）`);
    }
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
    const toastLow =
      /body:has\([^)]*scrim[^)]*\)\s*\.ui-toast__vp[\s\S]{0,120}bottom:/.test(
        ui,
      );
    if (!toastLow) fail("R12", "移动端 toast 与弹窗关闭钮重叠");
    else console.log("  OK R12：弹窗打开时 toast 移到底部");
  }
}
