# 贡献指南（CONTRIBUTING）

感谢关注 FluxTorrent！本文说明开发环境、提交规范与质量门禁。

## 开发环境

```bash
# 基础设施
docker compose -f docker/docker-compose.yml up -d postgres redis

# API（自动执行迁移）
DATABASE_URL="postgres://flux:<密码>@127.0.0.1:5432/fluxtorrent" \
REDIS_URL="redis://:<密码>@127.0.0.1:6379" \
JWT_SECRET="dev_secret_change_me_at_least_32_bytes!" \
cargo run -p flux-api

# Web
pnpm install && pnpm --filter @fluxtorrent/web dev
```

## 质量门禁（PR 必须全绿，与 CI 一致）

```bash
cargo fmt --all --check     # 格式
cargo check --workspace --all-targets   # 零警告基线
cargo test --workspace      # 单测
pnpm lint                   # ESLint（error 阻断；warning 存量 7 处不阻断）
node scripts/line_limit_guard.mjs    # 行数门禁（见纪律第 9 条）
pnpm --filter @fluxtorrent/web exec tsc --noEmit   # 前端类型
pnpm --filter @fluxtorrent/web exec next build     # 前端构建
node scripts/i18n_guard.mjs # i18n 防新增硬编码中文
```

## 关键工程纪律（违反会被 review 拒绝）

1. **迁移文件不可变**：`apps/api/migrations/` 中已合并的迁移禁止改名/改号/改内容——
   sqlx 按版本号 + checksum 记账，任何改动都会让存量部署启动失败。撞号时把自己的
   迁移挪到更大的号（如 0143+），永远不要让已存在的编号让位。
2. **资金动账必须走统一管线**：`spend_spark`/`earn_spark`（或 `_tx` 事务版），
   禁止直接 UPDATE 余额；`SpendOutcome` 是 `#[must_use]`——丢弃返回值继续执行
   副作用等于打开幂等重放印钞口。
3. **扣款与副作用同事务**：参照 `bank_deposit` 现版实现；禁止「先扣款提交、
   再写业务行、失败靠补偿」的两段提交。
4. **快照与流水双写**：users 上的余额/流量是快照，权威在 `spark_ledger`/
   `traffic_ledger`；只改快照不落流水的变更会被 6h 对账静默回滚。
5. **handler 不写裸 SQL 拼接**：参数一律 bind；动态表名/排序字段走白名单 match。
6. **新端点必须有守卫**：受保护资源第一步 `require_auth`；staff 端点用 `staff()`
   （authz.rs）。e2e-smoke 的匿名列矩阵会拦住漏守卫的端点。
7. **错误走信封**：所有响应 `{code, message, data, request_id}`；不要在 handler
   里手搓 HttpResponse 绕过 `ok()`/`DomainError`。
8. **组件内禁止裸 fetch**：前端统一走 `lib/api-client.ts`；凭证是 HttpOnly cookie，
   不要把 token 放进 localStorage 或 JS 可读的 cookie。
9. **文件行数只许瘦不许胖**：源码软上限 Rust/TS/TSX/JS/MJS/Python 500 行、
   CSS 3000 行（`scripts/line_limit_guard.mjs` 门禁；例外=迁移/i18n 字典/锁/
   生成物/domain-types 契约）。新文件超限直接拒；存量超限文件（见
   `scripts/line_limit_baseline.json`）可以改、鼓励拆分变短，但比基线更长即拒——
   拆分落地后跑 `node scripts/line_limit_guard.mjs --update` 收缩基线，
   基线只许变短（重新 --update 前先 review diff）。

## 提交规范

- 格式：`type(scope): 摘要`（中文摘要可），type ∈ feat/fix/docs/refactor/test/chore/ci，
  scope 参考：api/web/worker/tracker/db/ci/ops/docs。
- 一次提交做一件事；修复与重构分开。
- 涉及迁移的提交必须在 commit message 里说明幂等性（重跑是否安全）。

## 测试期望

- 纯函数（概率/计费/解析）必须有单测；经济类改动补 EV 或幂等断言。
- 新端点在 e2e-smoke 的冒烟清单里加一行 probe。
- 修 bug 先写会失败的测试（能自动化时），再修。
