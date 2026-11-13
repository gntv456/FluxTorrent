/**
 * 全站 UI 审查（2026-11-13）CSS 存量债一次性迁移，可重跑。
 *   1. min-height < 44px -> var(--tap-min)（项目自定触摸目标令牌）
 *   2. font-size 9/10/10.5/11px -> 12px（skill ui-ux-pro-max 正文下限）
 *   3. ui.css 的裸 0.15s 过渡 -> var(--motion-fast)
 * 用法：node _doc/_ui_css_migrate.mjs [--dry]
 */
import { readFileSync, writeFileSync, readdirSync } from "node:fs";
import path from "node:path";

const DIR = path.resolve(import.meta.dirname,
  "../apps/web/app/styles");
const DRY = process.argv.includes("--dry");
const files = readdirSync(DIR).filter((f) => f.endsWith(".css"));
let nTap = 0, nFont = 0, nMotion = 0;
const touched = [];

for (const f of files) {
  const p = path.join(DIR, f);
  const before = readFileSync(p, "utf8");
  let s = before;
  s = s.replace(/min-height:\s*(\d+)px/g, (m, n) => {
    if (parseInt(n, 10) >= 44) return m;
    nTap++;
    return "min-height: var(--tap-min)";
  });
  s = s.replace(/font-size:\s*(?:9|10|10\.5|11)px/g, () => {
    nFont++;
    return "font-size: 12px";
  });
  s = s.replace(
    "transition:\n    color 0.15s,\n    border-color 0.15s;",
    () => {
      nMotion++;
      return "transition:\n    color var(--motion-fast)," +
        "\n    border-color var(--motion-fast);";
    },
  );
  if (s !== before) {
    touched.push(f);
    if (!DRY) writeFileSync(p, s);
  }
}
console.log("min-height -> var(--tap-min):", nTap);
console.log("font-size <12px -> 12px:", nFont);
console.log("transition -> motion token:", nMotion);
console.log("files touched:", touched.length);
console.log(touched.join(", "));
if (DRY) console.log("(dry run, no files written)");
