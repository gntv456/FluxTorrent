/**
 * 把「<tbody><tr> 首行含 >=2 个 <th scope="col"> 的表头行」提升为 <thead>，
 * 同行的 <td className="colhead..."> 一并转为 <th scope="col">。
 * 用法：node _doc/_ui_thead2.mjs [--dry]
 */
import { readFileSync, writeFileSync } from "node:fs";
import { globCompat as globSync } from "../scripts/lib/glob_compat.mjs";
import path from "node:path";

const ROOT = path.resolve(import.meta.dirname, "..");
const DRY = process.argv.includes("--dry");
const files = globSync("apps/web/{app,components}/**/*.tsx", { cwd: ROOT });
let done = 0, skip = 0;
const touched = [];

for (const rel of files) {
  const p = path.join(ROOT, rel);
  const src = readFileSync(p, "utf8");
  if (src.includes("<thead")) continue;
  const m = src.match(/(<tbody>\s*)(<tr[^>]*>[\s\S]*?<\/tr>)/);
  if (!m) continue;
  const row = m[2];
  const ths = (row.match(/<th\s+scope="col"/g) || []).length;
  if (ths < 2) { skip++; continue; }
  // 同行 td.colhead -> th scope=col
  const fixed = row.replace(
    /<td(\s+className="colhead[^"]*")(\s*)>/g,
    (mm, attrs) => `<th scope="col"${attrs}>`,
  ).replace(
    /(<th scope="col"[^>]*>[\s\S]*?)<\/td>/g,
    (mm, head) => `${head}</th>`,
  );
  const out = src.slice(0, m.index) + m[1] +
    "<thead>\n" + fixed + "\n</thead>" + src.slice(m.index + m[1].length + row.length);
  if (!DRY) writeFileSync(p, out);
  done++; touched.push(rel);
}
console.log("promoted=" + done + " skip(<2th)=" + skip + (DRY ? " (dry)" : ""));
for (const t of touched.slice(0, 25)) console.log("  " + t);
