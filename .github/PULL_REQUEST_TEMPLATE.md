## 变更说明

## 自查清单
- [ ] `cargo fmt --all --check` / `cargo check`（零警告）/ `cargo test` 通过
- [ ] web：`tsc --noEmit` + `next build` 通过
- [ ] 涉及迁移：新增文件（未改动已有迁移），且幂等可重跑
- [ ] 涉及资金动账：走 spend/earn 管线，`SpendOutcome` 已处理，扣款与副作用同事务
- [ ] 新端点：require_auth/staff 守卫已加；e2e-smoke probe 已补
- [ ] 涉及凭证：无 token/passkey 进日志或 URL query

## UI/UX 自查（涉及界面改动时必填，取自 ui-ux-pro-max Pre-Delivery Checklist）
- [ ] 375px 与横屏实测；`prefers-reduced-motion` 与系统最大字号实测
- [ ] 暗色对比度独立测（不假设亮色值可沿用）+ `check_light_contrast.mjs --strict`
- [ ] 可点元素命中区 ≥ `var(--tap-min)`；无 emoji 作结构图标；语义 token 无硬编码 hex
- [ ] 数据四态（loading/empty/error/success）齐全，失败态带 `request_id` 与重试
- [ ] 写操作反馈走 `toast` / `ActionFeedback`；危险操作 busy 禁用且失败就地反馈
- [ ] 文案三语齐；表头用 `<th scope="col">`；非 `(main)` 布局页有 `id="main-content"`
- [ ] `check_tap_target` / `check_tiny_text` / `check_light_contrast` 三门禁未恶化
