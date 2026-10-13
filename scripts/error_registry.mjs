// 错误码注册表门禁（W-13）：本脚本已退役为兼容垫片。
//
// 演进（FRAME-PLAN-01 F1 步②）：注册表生成与对拍已拆成两处、语义超集——
//   - scripts/error_codes_gen.mjs：errors.rs code()（变体→码→触发条件注释）
//     + status()（HTTP 态）↔ i18n.rs MESSAGES（三语文案）join 生成
//     docs/api/error-codes.md 与 error-codes.generated.ts（--check 防过期）。
//     本脚本独有的「Rust 有而 i18n 无 → FAIL」已由它承接（KNOWN_GAPS 同口径）。
//   - scripts/check_type_drift.mjs 第 1/4 维：Rust→TS 缺常量 FAIL、
//     生成物 ↔ 手写 ErrorCode 值集差 WARN（原 RESERVED 白名单口径并入）。
//
// CI 仍以本路径调用（build-test.yml 的 Error registry guard 一步不改）：
// 透传参数与退出码，行为等价于直接跑 error_codes_gen.mjs。
import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const args = process.argv.slice(2);
if (args.length === 0) args.push("--update"); // 原默认行为：生成/刷新
console.log(
  "note: error_registry.mjs 已由 error_codes_gen.mjs 接管（透传本次调用）",
);
const r = spawnSync(
  process.execPath,
  [path.join(here, "error_codes_gen.mjs"), ...args],
  { stdio: "inherit" },
);
process.exit(r.status ?? 1);
