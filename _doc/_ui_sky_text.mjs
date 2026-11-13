/**
 * 把「文字色」用途的 var(--sky) 换成 var(--sky-text)（AA 文本令牌）。
 * 只替换 color: 属性值；background/border-color/box-shadow 保持装饰色。
 */
import { readFileSync, writeFileSync } from "node:fs";
import { globCompat as globSync } from "../scripts/lib/glob_compat.mjs";
import path from "node:path";
const ROOT = path.resolve(import.meta.dirname, "..");
const files = globSync("apps/web/app/styles/*.css", { cwd: ROOT });
let n = 0; const touched = [];
for (const rel of files) {
  const p = path.join(ROOT, rel);
  const src = readFileSync(p, "utf8");
  let s = src;
  // color: var(--sky)  （含 color: var(--sky) !important）
  s = s.replace(/color:\s*var\(--sky\)/g, () => { n++; return "color: var(--sky-text)"; });
  if (s !== src) { touched.push(rel); writeFileSync(p, s); }
}
console.log("color -> sky-text: " + n + " files=" + touched.length);
