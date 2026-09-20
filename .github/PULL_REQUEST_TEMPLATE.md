## 变更说明

## 自查清单
- [ ] `cargo fmt --all --check` / `cargo check`（零警告）/ `cargo test` 通过
- [ ] web：`tsc --noEmit` + `next build` 通过
- [ ] 涉及迁移：新增文件（未改动已有迁移），且幂等可重跑
- [ ] 涉及资金动账：走 spend/earn 管线，`SpendOutcome` 已处理，扣款与副作用同事务
- [ ] 新端点：require_auth/staff 守卫已加；e2e-smoke probe 已补
- [ ] 涉及凭证：无 token/passkey 进日志或 URL query
