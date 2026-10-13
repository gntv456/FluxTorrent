// 路由清单防漂移门禁（W-12 / FRAME-PLAN-01 §6 门禁家族）：重跑提取器与
// 已入库的 docs/api/route-manifest.json 对账，新增/删除路由或 handler 改名
// 未同步清单即 exit 1。提取逻辑全部复用 route_manifest.mjs，本文件只做
// 门禁呈现——单一真源，两个入口不会口径漂移。
//
// 与其他六件门禁同哲学：exit 0 = 通过，exit 1 = 挂（CI 步骤）。
// 红测方式：临时注释任意一条路由宏再跑本脚本，应被拦下；测完还原。
//
// 用法：node scripts/route_manifest_guard.mjs
import { compareWithStored, OUT_REL, HINT } from "./route_manifest.mjs";

const r = compareWithStored();
if (r.missing) {
  console.error(`ROUTE_MANIFEST_GUARD_FAIL ${OUT_REL} 不存在；${HINT}`);
  process.exit(1);
}
if (r.ok) {
  console.log(`ROUTE_MANIFEST_GUARD_OK ${r.total} 条路由与清单一致`);
  process.exit(0);
}
const MAX = 20; // 每类最多列 20 条，全量几百条会淹没 CI 日志
console.error(
  `ROUTE_MANIFEST_GUARD_FAIL 路由变更未同步 ${OUT_REL}：库中 ` +
  `${r.storedTotal} 条，源码 ${r.total} 条（新增 ${r.added.length} / ` +
  `移除 ${r.removed.length} / handler 改名 ${r.changed.length}）`,
);
for (const [tag, list] of [
  ["+", r.added],
  ["-", r.removed],
  ["~", r.changed],
])
  list.slice(0, MAX).forEach((k) => console.error(`  ${tag} ${k}`));
console.error(`修复：${HINT}`);
process.exit(1);
