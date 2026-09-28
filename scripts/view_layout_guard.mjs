#!/usr/bin/env node
// E6 视图布局守门（照抄 home_sections_guard 范式）：
// Rust 侧 view_layout.rs 的 TORRENT_COLUMNS / DETAIL_SECTIONS 单源清单
// vs 前端消费点键集，双向差集报错——防「后台存得进但页面不认」漂移。
// · 列：TorrentsTable/TorrentTr/行卡片用 hd.has("<key>") 消费
// · 段落：详情页 sectOn("<key>") 消费
import { readFileSync } from "node:fs";
import path from "node:path";

const ROOT = path.resolve(import.meta.dirname, "..");
const rs = readFileSync(
  path.join(ROOT, "apps/api/src/http/view_layout.rs"), "utf8");
const table = readFileSync(
  path.join(ROOT, "apps/web/app/(main)/torrents/_parts/torrents-table.tsx"),
  "utf8");
const row = readFileSync(
  path.join(ROOT, "apps/web/components/torrent-table.tsx"), "utf8");
const rowCard = readFileSync(
  path.join(ROOT, "apps/web/components/torrent-row-card.tsx"), "utf8");
const detailMain = readFileSync(
  path.join(ROOT, "apps/web/app/(main)/torrent/[id]/page.tsx"), "utf8");
const detailTail = readFileSync(
  path.join(
    ROOT,
    "apps/web/app/(main)/torrent/[id]/_parts/torrent-detail-tail.tsx",
  ),
  "utf8");
const detail = detailMain + detailTail;

function rsKeys(constName) {
  const seg = rs.split(`pub const ${constName}`)[1]?.split("];")[0] ?? "";
  return new Set([...seg.matchAll(/"([a-z_]+)"/g)].map((m) => m[1]));
}
const cols = rsKeys("TORRENT_COLUMNS");
const sects = rsKeys("DETAIL_SECTIONS");

// 前端消费键：表格/行/行卡片三处 hd.has("…")；title 恒渲染属约定（守门豁免注释即可）
const colRefs = new Set(
  [...(table + row + rowCard).matchAll(/hd\.has\("([a-z_]+)"\)/g)].map(
    (m) => m[1]));
// 段落消费键：详情页 sectOn("…")
const sectRefs = new Set(
  [...detail.matchAll(/sectOn\("([a-z_]+)"\)/g)].map((m) => m[1]));

let fail = 0;
for (const [name, defs, refs] of [
  ["TORRENT_COLUMNS", cols, colRefs],
  ["DETAIL_SECTIONS", sects, sectRefs],
]) {
  // title 列不可隐藏（后端校验+恒渲染），从对账中豁免
  const onlyDef = [...defs].filter((k) => !refs.has(k) && k !== "title");
  const onlyRef = [...refs].filter((k) => !defs.has(k));
  if (onlyDef.length) {
    console.error(`view_layout 守门：${name} 仅后端有（存得进但页面不认）：` +
      onlyDef.join(","));
    fail = 1;
  }
  if (onlyRef.length) {
    console.error(`view_layout 守门：${name} 仅前端有（永远写不进）：` +
      onlyRef.join(","));
    fail = 1;
  }
}
if (!fail) console.log("view_layout 守门：列/段落键前后端一致");
process.exit(fail);
