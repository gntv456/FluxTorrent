/**
 * 把 className 里的 min-h-[var(--tap-min)] 换成等值 min-h-11（44px），
 * 缩短字符串以避免 Prettier 折行导致的文件变长（tap-min=44px 见 base.css:592）。
 * 仅替换 className 字符串内出现，不动 CSS。
 */
import { readFileSync, writeFileSync } from "node:fs";
import { globCompat as globSync } from "../scripts/lib/glob_compat.mjs";
import path from "node:path";
const ROOT = path.resolve(import.meta.dirname, "..");
const files = globSync("apps/web/{app,components}/**/*.tsx", { cwd: ROOT });
let n = 0;
const touched = [];
for (const rel of files) {
  const p = path.join(ROOT, rel);
  const src = readFileSync(p, "utf8");
  let s = src;
  s = s.replace(/min-h-\[var\(--tap-min\)\]/g, () => { n++; return "min-h-11"; });
  s = s.replace(/min-w-\[var\(--tap-min\)\]/g, () => { n++; return "min-w-11"; });
  if (s !== src) { touched.push(rel); writeFileSync(p, s); }
}
console.log("replaced=" + n + " files=" + touched.length);
