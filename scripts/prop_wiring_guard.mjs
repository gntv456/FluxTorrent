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
// 有意不传的 prop（加条目必须写理由，且只许减不许加）：
// - Fold.open：折叠区默认收起是有意行为（详情页只有简介默认展开）
// - Spec.num：num=true 走等宽数字排版，仅数值型格子用（size/评分等已在传）
const EXEMPT = new Set(["Fold.open", "Spec.num"]);

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

// 「组件名 → 该组件的 prop 表」索引：一个文件可导出多个组件
// （torrent-detail-parts.tsx 里 Descr/Spec/Fold/PosterBlock 四件），
// prop 表必须按组件名取各自的解构，拿「文件第一个导出」会给整文件
// 所有组件套同一个表——漏检时是假绿，修完 import 收集后会变误报源。
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
    const clean = stripComments(t);
    for (const m of clean.matchAll(
      /export function ([A-Z]\w*)\s*\(\s*\{([^}]*)\}/g,
    )) {
      const props = new Set(
        m[2]
          .split(",")
          .map((s) => s.trim().split(":")[0].trim())
          .filter((s) => /^[a-zA-Z_$][\w$]*$/.test(s)),
      );
      if (!INDEX.has(m[1])) INDEX.set(m[1], { file: p, props });
    }
  }
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
    // JSX 子内容即 children 传参（<Fold>…</Fold> 不会出现 children= 属性）：
    // 标签体非空（去掉纯空白/自闭合）视为 children 已传
    const selfClosing = src.slice(start, i).trimEnd().endsWith("/");
    if (!selfClosing && body.trim() !== "") out.add("children");
    // 裸 prop（`embedded` / `bare` 这类无值写法）也算已传：没有 `=`，
    // 下面的属性正则抓不到——实测漏抓把真传了的 Comments.embedded、
    // TorrentSubtitles.bare 误报成断裂。要求裸名是真标识符且不吃前缀
    // （排除 `...spread`），尾部可带换行。
    for (const m of body.matchAll(/(?:^|[\s>])([a-zA-Z_$][\w$]*)(?=[\s/>}]|$)/g)) {
      out.add(m[1]);
    }
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

// 调用点组件收集：解析页面 import 的**全部**命名绑定。旧正则只认
// `import { X } from "@/…"`（花括号内恰一个标识符），多成员导入
// （`import { TorrentTags, type TagPayload }`）整条不命中——实测
// 18 个受管组件里 7 个（Descr/Spec/Fold/Comments/Thankers/…）在守门
// 范围外，而多导出共享文件恰是最易断线的地方。剥掉 `type X` 与 `as X`。
const comps = [
  ...new Set(
    [
      ...src.matchAll(
        /import \{([^}]*)\} from ["']@\/(components|app)/g,
      ),
    ].flatMap((m) =>
      m[1]
        .split(",")
        .map((s) =>
          s
            .trim()
            .replace(/^type\s+/, "")
            .split(/\s+as\s+/)[0]
            .trim(),
        )
        .filter((s) => /^[A-Z]\w*$/.test(s)),
    ),
  ),
].sort();

let bad = [];
let checked = 0;
for (const c of comps) {
  const entry = INDEX.get(c);
  if (!entry) continue;
  const decl = entry.props;
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
