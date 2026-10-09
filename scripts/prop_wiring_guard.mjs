#!/usr/bin/env node
// 详情页 prop 接线守门（2026-10-10）：aggregate 供得到的数据，组件声明要得到，
// 但**调用点没传**时既不会红 tsc（可选 prop）也不会红页面（板块照渲），只有
// 渲染层看得见——50ba22b 的 message 声称「三处一次接齐」，实际 `ripLogs` 从未
// 进过 page.tsx，日志分徽标因此一直是死的。view_layout_guard 盯的是「段落键
// 存得进但页面不认」，这一条盯的是「数据接得上但组件没喂」，两个方向各管一段。
//
//   node scripts/prop_wiring_guard.mjs              # CI 用；exit 1 = 有断线
//   node scripts/prop_wiring_guard.mjs --strip NAME # 自检：模拟 NAME 这个 prop
//                                                   # 漏接，必须报红并点名它
//
// 豁免：EXEMPT 里的 `组件.prop` 视为「有意不传」，必须写明理由，且只许减不许加。
import { readFileSync, readdirSync } from "node:fs";
import path from "node:path";

const ROOT = path.resolve(import.meta.dirname, "..");
const R = (p) => path.join(ROOT, p);

// 受管页面：详情页首屏 + 尾块（其它页面尚无「聚合喂组件」的形态，先不铺开）
const PAGES = [
  "apps/web/app/(main)/torrent/[id]/page.tsx",
  "apps/web/app/(main)/torrent/[id]/_parts/torrent-detail-tail.tsx",
];
// 组件索引扫描目录
const COMP_DIRS = [
  "apps/web/components",
  "apps/web/app/(main)/torrent/[id]/_parts",
];
// 有意不传的 prop（当前应为空；加条目必须写理由）
const EXEMPT = new Set([]);

const strip = process.argv.includes("--strip")
  ? process.argv[process.argv.indexOf("--strip") + 1]
  : null;

const raw = PAGES.map((p) => readFileSync(R(p), "utf8")).join("\n");
// 自检用：把 `NAME={…}` 从 JSX 里抹掉，模拟一次真实漏接
const src = strip
  ? raw.replace(new RegExp(`\\s*${strip}={[\\s\\S]{0,120}?}`, "g"), "")
  : raw;

const stripComments = (t) =>
  t.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");

// 「导出组件名 → 文件」索引：组件文件名与组件名不同源，靠命名猜测会漏
const INDEX = new Map();
for (const dir of COMP_DIRS) {
  let entries = [];
  try {
    entries = readdirSync(R(dir), { recursive: true });
  } catch {
    continue;
  }
  for (const e of entries) {
    const p = `${dir}/${e}`;
    if (!/\.tsx?$/.test(p)) continue;
    let t;
    try {
      t = readFileSync(R(p), "utf8");
    } catch {
      continue;
    }
    for (const m of t.matchAll(/export function ([A-Z]\w*)/g)) {
      if (!INDEX.has(m[1])) INDEX.set(m[1], p);
    }
  }
}

function declaredProps(file) {
  const t = stripComments(readFileSync(R(file), "utf8"));
  const m = t.match(/export function \w+\(\s*\{([^}]*)\}/);
  if (!m) return null;
  return new Set(
    m[1]
      .split(",")
      .map((s) => s.trim().split(":")[0].trim())
      .filter((s) => /^[a-zA-Z_$][\w$]*$/.test(s)),
  );
}

// 调用点属性：深度只按 { ( 计（表达式里的 `>` 不是结束符），
// 且只取深度 0 处的 `ident=`（嵌套组件的属性不算本组件已传）。
function passedAttrs(name) {
  const out = new Set();
  let from = 0;
  while ((from = src.indexOf(`<${name}`, from)) >= 0) {
    const start = from + name.length + 1;
    let d = 0;
    let i = start;
    for (; i < src.length; i++) {
      const ch = src[i];
      if (ch === "{" || ch === "(") d++;
      else if (ch === "}" || ch === ")") d--;
      else if (ch === ">" && d === 0) break;
    }
    const body = src.slice(start, i);
    const depthAt = [0];
    let k2 = 0;
    for (let k = 0; k < body.length; k++) {
      const ch = body[k];
      if (ch === "{" || ch === "(") k2++;
      else if (ch === "}" || ch === ")") k2--;
      depthAt[k] = k2;
    }
    for (const m of body.matchAll(/([a-zA-Z_$][\w$]*)\s*=\s*[{"]/g)) {
      if (depthAt[m.index] === 0) out.add(m[1]);
    }
    from = i;
  }
  return out;
}

const comps = [
  ...new Set(
    [...src.matchAll(/import \{ ([A-Z]\w*) \} from "@\/(components|app)/g)].map(
      (m) => m[1],
    ),
  ),
].sort();

let bad = [];
let checked = 0;
for (const c of comps) {
  const file = INDEX.get(c);
  if (!file) continue;
  const decl = declaredProps(file);
  const passed = passedAttrs(c);
  if (!decl || decl.size === 0 || passed.size === 0) continue;
  checked++;
  for (const p of [...decl].filter((x) => !passed.has(x))) {
    if (EXEMPT.has(`${c}.${p}`)) continue;
    bad.push(`${c}.${p}（组件声明了，详情页调用点没传）`);
  }
}

if (bad.length) {
  console.log(
    `prop 接线断裂 ${bad.length} 处（aggregate 供得到、组件收得到、JSX 没喂；` +
      `可选 prop 漏传不会红 tsc，只会让那块 UI 静默死掉）：`,
  );
  for (const b of bad) console.log("  " + b);
  console.log("要么在调用点补上，要么把 组件.prop 加进 EXEMPT 并写明理由。");
  process.exit(1);
}
console.log(
  `OK: 详情页 ${checked} 个组件的声明 prop 全部有传${
    strip ? "（--strip " + strip + " 自检未报红 = 自检失效）" : ""
  }`,
);
