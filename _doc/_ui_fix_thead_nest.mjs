/**
 * 修复 thead 误嵌在 tbody 内（非法 HTML）：把 <tbody>…<thead>…</thead>
 * 改为 <thead>…</thead><tbody>，并规整缩进。
 * 用法：node _doc/_ui_fix_thead_nest.mjs [--dry]
 */
import { readFileSync, writeFileSync } from "node:fs";
import { globCompat as globSync } from "../scripts/lib/glob_compat.mjs";
import path from "node:path";

const ROOT = path.resolve(import.meta.dirname, "..");
const DRY = process.argv.includes("--dry");
const files = globSync("apps/web/{app,components}/**/*.tsx", { cwd: ROOT });
let fixed = 0; const touched = [];

for (const rel of files) {
  const p = path.join(ROOT, rel);
  const src = readFileSync(p, "utf8");
  // <tbody> [可选表达式行] <thead> ... </thead>
  const re = /(<tbody>)(\s*(?:\{[^{}]*\}\s*)?)(<thead>[\s\S]*?<\/thead>)/;
  if (!re.test(src)) continue;
  const out = src.replace(re, (m, tb, mid, thead) => {
    // 规整 thead 内部缩进：以 <thead> 行为基准
    const indent = "        ";
    const body = thead
      .split("\n")
      .map((l, i) => {
        if (i === 0) return l.trim();
        return indent + l.trim();
      })
      .join("\n");
    return body + "\n" + indent.slice(0, -2) + "<tbody>" + mid.replace(/\s+$/, "");
  });
  if (out !== src) {
    fixed++; touched.push(rel);
    if (!DRY) writeFileSync(p, out);
  }
}
console.log("fixed=" + fixed + (DRY ? " (dry)" : ""));
for (const t of touched.slice(0, 8)) console.log("  " + t);
