/**
 * Wrap >80 chars JSX lines introduced by the batch migration (2026-11-13).
 * Splits on a single JSX attribute boundary, preserving indentation.
 * Usage: node _doc/_ui_wrap.mjs [--dry]
 */
import { readFileSync, writeFileSync } from "node:fs";
import { globCompat as globSync } from "../scripts/lib/glob_compat.mjs";
import path from "node:path";

const ROOT = path.resolve(import.meta.dirname, "..");
const DRY = process.argv.includes("--dry");
const files = globSync("apps/web/{app,components}/**/*.tsx", { cwd: ROOT });
let changed = 0, total = 0;
for (const rel of files) {
  const p = path.join(ROOT, rel);
  const src = readFileSync(p, "utf8");
  const arr = src.split("\n");
  let dirty = false;
  const out = [];
  for (const line of arr) {
    if (line.length <= 80) { out.push(line); continue; }
    // only touch lines with a JSX attribute boundary
    const m = line.match(/^(\s*)(.*?\S)\s+(\S+=.*)$/);
    if (!m) { out.push(line); continue; }
    const indent = m[1];
    const head = m[2];
    const restv = m[3];
    // avoid splitting plain text/strings mid-token
    if (!/[<{]/.test(head) && !/^[a-zA-Z-]+=/.test(restv)) {
      out.push(line);
      continue;
    }
    out.push(indent + head);
    out.push(indent + "  " + restv);
    dirty = true;
    total++;
  }
  if (dirty) {
    changed++;
    if (!DRY) writeFileSync(p, out.join("\n"));
  }
}
console.log("lines wrapped=" + total + " files=" + changed + (DRY ? " (dry)" : ""));
