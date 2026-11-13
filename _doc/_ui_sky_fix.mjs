/** 把误替换的 border-color/outline 类 var(--sky-text) 还原为 var(--sky)。 */
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
  s = s.replace(/border-color:\s*var\(--sky-text\)/g, () => { n++; return "border-color: var(--sky)"; });
  s = s.replace(/border-bottom-color:\s*var\(--sky-text\)/g, () => { n++; return "border-bottom-color: var(--sky)"; });
  if (s !== src) { touched.push(rel); writeFileSync(p, s); }
}
console.log("border restored=" + n + " files=" + touched.length);
