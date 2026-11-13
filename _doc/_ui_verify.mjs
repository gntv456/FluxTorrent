/**
 * 全站 UI 审查修复前后量化对比（2026-11-13）。
 * 用法：node _doc/_ui_verify.mjs
 * 读 _doc/ui-ux-fix-baseline.json，重新扫描当前源码并输出 before/after。
 */
import { readFileSync, readdirSync, statSync } from "node:fs";
import path from "node:path";

const ROOT = path.resolve(import.meta.dirname, "..");
const WEB = path.join(ROOT, "apps/web");
const files = [];
function walk(d) {
  for (const e of readdirSync(d, { withFileTypes: true })) {
    const n = e.name;
    if (["node_modules", ".next", "coverage", "_shots"].includes(n)) {
      continue;
    }
    const p = path.join(d, n);
    if (e.isDirectory()) walk(p);
    else if (/\.(tsx|ts|css)$/.test(n)) files.push(p);
  }
}
walk(WEB);
const tsx = files.filter((f) => f.endsWith(".tsx"));
const css = files.filter((f) => f.endsWith(".css"));
function count(list, re) {
  let n = 0;
  for (const f of list) {
    n += (readFileSync(f, "utf8").match(re) || []).length;
  }
  return n;
}
const now = {
  colhead: count(tsx, /colhead/g),
  thScope: count(tsx, /scope=["'](?:col|row)["']/g),
  caption: count(tsx, /<caption/g),
  smallTap: count(tsx,
    /(?:min-h|min-w|h|w)-\[(?:2[0-9]|3[0-9]|4[0-3])px\]/g),
  tinyTextTsx: count(tsx, /text-\[(?:9|10|10\.5|11)px\]/g),
  tinyTextCss: count(css, /font-size:\s*(?:9|10|10\.5|11)px/g),
  ariaLive: count(tsx, /aria-live|aria-busy/g),
  roleAlert: count(tsx, /role="alert"/g),
  ariaLabel: count(tsx, /aria-label/g),
  ariaCurrent: count(tsx, /aria-current/g),
  ariaPressed: count(tsx, /aria-pressed/g),
  htmlImg: count(tsx, /<img[\s>]/g),
  tapMin: count(tsx, /--tap-min/g),
  reducedMotion: count(css, /prefers-reduced-motion/g),
};
const base = JSON.parse(readFileSync(
  path.join(ROOT, "_doc/ui-ux-fix-baseline.json"), "utf8"));
const rows = [
  ["colhead（td 表头）", base.colhead, now.colhead],
  ["th scope=col/row", base.thScope, now.thScope],
  ["<caption>", base.caption, now.caption],
  ["<44px 小热区(tsx)", base.smallTap, now.smallTap],
  ["<12px 字号(tsx)", base.tinyTextTsx, now.tinyTextTsx],
  ["<12px 字号(css)", base.tinyTextCss, now.tinyTextCss],
  ["aria-live/aria-busy", base.ariaLive, now.ariaLive],
  ["role=alert", base.roleAlert, now.roleAlert],
  ["aria-label", base.ariaLabel, now.ariaLabel],
  ["aria-current", 0, now.ariaCurrent],
  ["aria-pressed", 0, now.ariaPressed],
  ["<img> 原生标签", base.htmlImg, now.htmlImg],
  ["prefers-reduced-motion", base.reducedMotion, now.reducedMotion],
];
console.log("指标".padEnd(26) + "修复前".padStart(9) +
  "修复后".padStart(9) + "  变化");
console.log("-".repeat(56));
for (const [name, before, after] of rows) {
  const delta = after - before;
  const sign = delta > 0 ? "+" : "";
  console.log(name.padEnd(26) + String(before).padStart(9) +
    String(after).padStart(9) + "  " + sign + delta);
}
console.log("\n扫描文件：tsx " + tsx.length + " / css " + css.length);
