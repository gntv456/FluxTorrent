#!/usr/bin/env bash
# 本地等价闸门：CI 黑掉时（private 仓库分钟数耗尽、runner 配额、网络）也能一条命令
# 跑完 build-test + e2e-smoke 的核心门。2026-09-25 四审复验期间 master CI 已连续
# 数日 4~5 秒判失败、无日志，而所谓「e2e 全绿」其实只在本地成立——这个脚本就是
# 把那份「本地成立」变成可交接、可复跑的闸门。
#
#   bash scripts/ci_local.sh                # 编译/测试/类型/门禁脚本
#   bash scripts/ci_local.sh --with-install # 追加空库装站链（会清库！仅测试实例）
#   bash scripts/ci_local.sh --with-pt      # 追加 PT 经济/安全不变式（需栈在跑）
set -euo pipefail
cd "$(dirname "$0")/.."

step() { printf '\n=== %s ===\n' "$*"; }

step "cargo fmt --check"
cargo fmt --all --check

step "cargo check --workspace --all-targets"
cargo check --workspace --all-targets

step "cargo test --workspace"
cargo test --workspace

step "web: tsc --noEmit"
(cd apps/web && npx tsc --noEmit)

step "仓库门禁脚本（行数/宽度、i18n 基线、模块键/首页板块/术语规则单源、类型契约、迁移校验和）"
node scripts/line_limit_guard.mjs
node scripts/i18n_guard.mjs
node scripts/home_sections_guard.mjs
node scripts/module_keys_guard.mjs
node scripts/gacha_math_guard.mjs
node scripts/gacha_pack_guard.mjs
node scripts/terms_guard.mjs
node scripts/check_type_drift.mjs
python3 scripts/validation_i18n_guard.py
python3 scripts/audit_migration_checksums.py

if [ "${1:-}" = "--with-install" ]; then
  step "空库装站闸门（会 down -v 清掉本地数据卷）"
  (cd docker && docker compose down -v >/dev/null && docker compose up -d >/dev/null)
  python scripts/install_e2e.py
fi

if [ "${1:-}" = "--with-pt" ]; then
  # 需要栈已在跑（api/worker/tracker/postgres/redis），不清库；探针自清
  step "PT 经济与安全不变式（0285：announce 回放/补量持久/passkey/审计/举报去重）"
  python scripts/e2e_pt_economy_invariants.py
  step "对外接口面与经济玩法不变式（0285 二批：缓存越权/RSS 匿名/禁言/令牌撤销/商店重放）"
  python scripts/e2e_surface_economy_invariants.py
  step "H&R 可信度与运维不变式（0286：手动冻结不被自动放开 / enable_hr 真生效 / 促销落账）"
  python scripts/e2e_hr_ops_invariants.py
  step "发种/审种动线不变式（0288：审核通知/队列分页/被拒自助/校验时序/结构校验/可见性闸门/批量裁决/最低标准）"
  python scripts/pt_audit_d_review_flow.py
fi

printf '\n本地闸门通过。\n'
