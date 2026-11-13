/**
 * Promote first header row inside <tbody> to <thead> (2026-11-13 audit).
 * Only when first <tr> has >=2 <th scope="col"> and no <td>.
 * Usage: node _doc/_ui_thead.mjs [--dry]
 */
import { readFileSync, writeFileSync } from "node:fs";
import path from "node:path";

const ROOT = path.resolve(import.meta.dirname, "../apps/web");
const DRY = process.argv.includes("--dry");
const files = [
  "app/(main)/admin/_parts/admin-tool-panels.tsx",
  "app/(main)/donate/_inner-ledger.tsx",
  "app/(main)/jixiao/_inner.tsx",
  "app/(main)/my-spark/_parts/ledger-panel.tsx",
  "app/(main)/my-spark/_parts/merkle-panel.tsx",
  "app/(main)/offers/_inner.tsx",
  "app/rules/page.tsx",
  "components/admin-ops-jobs.tsx",
  "components/admin-trumps.tsx",
  "components/appeal-records.tsx",
  "components/ban-log-list.tsx",
  "components/content-manage-tabs.tsx",
  "components/content-manage.tsx",
  "components/funding-panel-parts.tsx",
  "components/invite-invitees.tsx",
  "components/invite-manager-parts.tsx",
  "components/login-history.tsx",
  "components/my-hr.tsx",
  "components/my-orders-table.tsx",
  "components/plugins.tsx",
];
let done = 0, skip = 0;
for (const rel of files) {
  const p = path.join(ROOT, rel);
  const src = readFileSync(p, "utf8");
  const m = src.match(/(<tbody>\s*)(<tr[^>]*>[\s\S]*?<\/tr>)(\s*)/);
  if (!m) { console.log("SKIP no tbody tr: " + rel); skip++; continue; }
  const row = m[2];
  const ths = (row.match(/<th\s+scope="col"/g) || []).length;
  if (ths < 2) { console.log("SKIP th<2: " + rel); skip++; continue; }
  if (/<td[\s>]/.test(row)) { console.log("SKIP has td: " + rel); skip++; continue; }
  const tailIdx = m.index + m[1].length + row.length + m[3].length;
  const out = src.slice(0, m.index) + m[1] +
    "<thead>\n" + row + "\n</thead>" + m[3] + src.slice(tailIdx);
  if (!DRY) writeFileSync(p, out);
  done++;
  console.log("OK: " + rel);
}
console.log("\nthead done=" + done + " skip=" + skip + (DRY ? " (dry)" : ""));
