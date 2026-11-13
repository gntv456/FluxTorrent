/**
 * TSX 存量债迁移（紧凑输出，避免 Prettier 展开单行导致行数膨胀）。
 *  1. <td className="colhead...">x</td> -> <th scope="col" className="colhead...">x</th>
 *  2. min-h/h/w-[28..43px] -> [var(--tap-min)]
 *  3. text-[9|10|10.5|11px] -> text-xs
 * 用法：node _doc/_ui_tsx_migrate2.mjs [--dry]
 */
import { readFileSync, writeFileSync } from "node:fs";
import { globCompat as globSync } from "../scripts/lib/glob_compat.mjs";
import path from "node:path";

const ROOT = path.resolve(import.meta.dirname, "..");
const DRY = process.argv.includes("--dry");
const files = globSync("apps/web/{app,components,lib}/**/*.tsx", {
  cwd: ROOT,
}).sort();
let nHead = 0, nTap = 0, nFont = 0;
const touched = [];

for (const rel of files) {
  const p = path.join(ROOT, rel);
  const before = readFileSync(p, "utf8");
  let s = before;
  // 1) 表头语义：保持单行紧凑（属性顺序固定，长度可控）
  s = s.split("\n").map((line) =>
    line.replace(
      /<(td|th)(\s+className="colhead[^"]*")(\s*)(?:scope="col"\s*)?>(.*?)<\/(td|th)>/g,
      (m, _tag, cls, _sp, body) => {
        nHead++;
        return `<th scope="col"${cls}>${body}</th>`;
      },
    ),
  ).join("\n");
  // 2) 小热区
  s = s.replace(
    /((?:min-h|min-w|h|w)-\[)(?:2[0-9]|3[0-9]|4[0-3])px\]/g,
    (m, pre) => { nTap++; return `${pre}var(--tap-min)]`; },
  );
  // 3) 字号
  s = s.replace(/text-\[(?:9|10|10\.5|11)px\]/g, () => {
    nFont++;
    return "text-xs";
  });
  if (s !== before) {
    touched.push(rel);
    if (!DRY) writeFileSync(p, s);
  }
}
console.log("colhead -> th: " + nHead);
console.log("tap -> var(--tap-min): " + nTap);
console.log("tiny -> text-xs: " + nFont);
console.log("files: " + touched.length);
if (DRY) console.log("(dry)");
