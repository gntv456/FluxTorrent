#!/usr/bin/env bash
# 本地等价闸门：CI 黑掉时（private 仓库分钟数耗尽、runner 配额、网络）也能一条命令
# 跑完 build-test + e2e-smoke 的核心门。2026-09-25 四审复验期间 master CI 已连续
# 数日 4~5 秒判失败、无日志，而所谓「e2e 全绿」其实只在本地成立——这个脚本就是
# 把那份「本地成立」变成可交接、可复跑的闸门。
#
#   bash scripts/ci_local.sh                # 编译/测试/类型/门禁脚本
#   bash scripts/ci_local.sh --with-install # 追加空库装站链（会清库！仅测试实例）
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

step "仓库门禁脚本（行数/宽度、i18n 基线、模块键与首页板块单源、类型契约、迁移校验和）"
node scripts/line_limit_guard.mjs
node scripts/i18n_guard.mjs
node scripts/home_sections_guard.mjs
node scripts/module_keys_guard.mjs
node scripts/check_type_drift.mjs
python scripts/audit_migration_checksums.py

if [ "${1:-}" = "--with-install" ]; then
  step "空库装站闸门（会 down -v 清掉本地数据卷）"
  (cd docker && docker compose down -v >/dev/null && docker compose up -d >/dev/null)
  python scripts/install_e2e.py
fi

printf '\n本地闸门通过。\n'
