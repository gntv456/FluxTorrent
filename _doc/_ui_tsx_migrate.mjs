/**
 * 全站 UI 审查（2026-11-13）TSX 存量债批量迁移，可重跑、可 dry-run。
 *   1. <td className="colhead..."> -> <th scope="col" className="colhead...">
 *      （680 处；闭合标签同步改，保留 class 让既有 CSS 继续生效）
 *   2. min-h/h/w-[28..43px] -> [var(--tap-min)]（项目自定 44px 令牌）
 *   3. text-[9|10|10.5|11px] -> text-xs（12px，skill 正文下限）
 * 不做：emoji->SVG / truncate title / aria 语义（需逐处判断，由分组修复处理）
 * 用法：node _doc/_ui_tsx_migrate.mjs [--dry]
 */
import { readFileSync, writeFileSync } from "node:fs";
import { globCompat as globSync } from "../scripts/lib/glob_compat.mjs";
import path from "node:path";

const ROOT = path.resolve(import.meta.dirname, "..");
const DRY = process.argv.includes("--dry");
const files = globSync(
  "apps/web/{app,components,lib}/**/*.tsx",
  { cwd: ROOT },
).sort();
let nHead = 0, nTap = 0, nFont = 0;
const touched = [];
const colheadNoThead = [];

for (const rel of files) {
  const p = path.join(ROOT, rel);
  const before = readFileSync(p, "utf8");
  let s = before;
  // 1. 表头语义（逐行，避免跨行误匹配）
  s = s.split("\n").map((line) => {
    return line.replace(
      /<td className="colhead([^"]*)"([^>]*)>(.*?)<\/td>/g,
      (m, cls, rest, body) => {
        nHead++;
        return "<th scope=\"col\" className=\"colhead" + cls +
          "\"" + rest + ">" + body + "</th>";
      },
    );
  }).join("\n");
  // 2. 小热区 -> 令牌
  s = s.replace(
    /((?:min-h|min-w|h|w)-\[)(?:2[0-9]|3[0-9]|4[0-3])px\]/g,
    (m, prefix) => { nTap++; return prefix + "var(--tap-min)]"; },
  );
  // 3. <12px 字号
  s = s.replace(/text-\[(?:9|10|10\.5|11)px\]/g, () => {
    nFont++;
    return "text-xs";
  });
  if (s !== before) {
    touched.push(rel);
    if (s.includes("colhead") && !s.includes("<thead")) {
      colheadNoThead.push(rel);
    }
    if (!DRY) writeFileSync(p, s);
  }
}
console.log("colhead -> th scope=col:", nHead);
console.log("small tap -> var(--tap-min):", nTap);
console.log("tiny text -> text-xs:", nFont);
console.log("files touched:", touched.length);
console.log("colhead without <thead> (needs manual):",
  colheadNoThead.length);
for (const f of colheadNoThead.slice(0, 20)) console.log("  " + f);
if (DRY) console.log("(dry run, no files written)");
