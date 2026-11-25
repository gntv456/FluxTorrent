# 固定抽签（FluxTorrent 0520）深度测试与修复报告

日期：2026-11-25 · 范围：报名 → 开奖 → 履约 → 领取审核全链路（API / worker / web）
方法：代码走查 + 状态机推演 + 并发竞态分析 + 纯函数/UI 单测 + 质量门禁

## 测试矩阵

| 层 | 覆盖点 | 结果 |
| :-- | :-- | :-- |
| 领域纯函数 | HMAC bottom-k、快照哈希、槽位分配、流拍、均匀性冒烟 | 10/10 通过（原有） |
| 服务层 | 领取资料 schema 校验（缺字段/非对象/字段数/超长） | 5/5 通过（新增） |
| 前端 | 自定义奖领取入口可见性、状态文案、审核后锁定、超期锁定 | 5/5 通过（新增） |
| 编译 | `cargo check -p flux-worker --all-targets` | 通过 |
| 类型 | `tsc --noEmit` | 通过 |
| 风控 | 行数/行宽、i18n、design 门禁 | 本模块无新增违规（存量见末尾） |

## 缺陷与修复

| # | 级别 | 现象（用户视角） | 根因 | 修复 |
| :- | :-- | :-- | :-- | :-- |
| 1 | P0 | 中了自定义奖却永远看不到「填写领取资料」入口 | 履约把自定义奖置为 `satisfied`，前端却只在 `claimed` 时渲染表单；且 `rewardKind` 从 `reward_config` 里取了不存在的键 | 新增 `claimFormVisible` / `prizeStatusText` 纯函数；API 回传真实 `reward_kind`、`claimStatus`、`rejectReason` |
| 2 | P0 | 领取期限形同虚设，超期仍能提交/永不过期 | 过期 SQL 只匹配 `fulfilled`（实际终态是 `satisfied`）；函数没有任何调度调用点 | 修正状态口径并区分「已提交待审」豁免；拆独立模块并接入 60s tick + 手动任务 `flx_claim_expire`，API/worker 双份同步 |
| 3 | P1 | 未达注册时长的人可付费报名，开奖时被静默剔除 | 报名侧 `check_eligibility` 漏了 `registered_days_min`，与开奖 SQL 不同尺 | 补 `repo::registration_ok`，报名前按同一口径拦截 |
| 4 | P1 | 管理员已通过审核后，用户重交可把结果改回待审 | SELECT 与 UPSERT 之间无原子性 | 条件 UPSERT（`WHERE status <> 'approved'`）做库级 CAS，0 行即拒绝 |
| 5 | P1 | 创建定时活动时不开奖时间、人数达标活动无阈值/保底，活动永久卡在 open | 创建口只校验标题和奖项 | 补触发方式、阈值、保底时间、奖项名、奖励类型白名单校验；领取期限 clamp 1–365 天 |
| 6 | P2 | 领取审核查不到是谁批的 | `reviewed_by` 写死 NULL，审计 actor 也是 NULL | 审核人 id 从 HTTP 层透传，落库并写进审计 |
| 7 | P2 | worker 自动开奖的 `draw_meta` 与 API 手动开奖不一致，复算凭证失信 | `prize_slot_count` 取 `Vec::capacity()` 而非名额总数；缺 `seed_hash` | 两边统一为名额总数并补 seed_hash |
| 8 | P2 | 领取资料可塞任意多字段/超长文本 | `validate_fields` 只查必填项 | 字段数 ≤10、单值 ≤500 字符 |
| 9 | P2 | 前端 JSON 写错时静默提交空表单，用户以为已提交 | `catch` 里把 fields 置空后继续 POST | 解析失败直接提示 `claimInvalidJson` 并中断（三语文案已补） |
| 10 | P3 | 运营面板存在一整段重复的创建表单状态与函数 | 拆件时旧实现未删 | 删除死代码，创建统一走 `AdminLotteryCreate` |

## 回归资产

- `apps/api/src/services/fixed_lottery/claim.rs` — 5 个 Rust 单测
- `apps/web/tests/lottery-claim-form.test.ts` — 5 个 Vitest 单测
- `apps/api/src/services/fixed_lottery/db_tests.rs` — 4 个 `#[ignore]` DB 回归（第二轮新增）
- 领域层原 10 个测试保持通过；最终 `cargo test -p flux-api --bin flux-api fixed_lottery`：15 passed / 0 failed

## 第二轮续修（用户要求「继续」后完成）

| # | 级别 | 现象 | 根因 | 修复 |
| :- | :-- | :-- | :-- | :-- |
| 11 | P1 | 开奖瞬间的报名可能付了钱却不在中奖池 | `draw()` 先读候选集、后做 CAS，中间的报名事务能挤进快照；报名口也没有对活动行加锁 | 开奖改为**事务首步 CAS 持锁、锁后在同一事务内冻结候选**；报名 `join_tx` 加 `FOR SHARE` 与 CAS 的 `FOR UPDATE` 互斥。API 与 worker 双份同步，worker 候选查询拆独立模块 |
| 12 | P2 | 奖励配置写错（金额/物品键/勋章 ID）要等履约重试 5 次才报错 | 创建口只校验类型白名单 | 按 reward_kind 校验必填键与取值域，错误在创建时暴露 |
| 13 | P2 | 领取状态机只有纯函数测试，库上 CAS/过期 SQL 无证据 | 缺 DB 级回归 | 新增 4 个 `#[ignore]` DB 测试：开奖后报名被拒、CAS 后候选快照完整、审核通过后重交被拒且 submission 不变、超期扫描区分 satisfied/已驳回/待审 |

第二轮验证：

- `cargo test -p flux-api --bin flux-api fixed_lottery` → 15 passed / 0 failed / 4 ignored
- `cargo test -p flux-api db_tests -- --ignored --test-threads=1` → 17 passed / 0 failed（含本模块 4 个）
- `cargo check -p flux-worker --all-targets` → Finished
- Vitest 5 passed；`tsc --noEmit` 通过；行数门禁本模块无新增违规

## 遗留风险与建议（按优先级）

1. **P2 设计门禁**：`lottery/_inner.tsx` 两处语义色直写（bg-rose-500/10、bg-sky-500/10）在新增文件基线下被判违规，建议按 Badge tone / var(--tone-*) 迁移。
2. **P3 存量门禁债（非本次引入）**：`catalog_jobs.rs` 335>329、`run.rs` 332>328、`repo/mod.rs` 305>302、`sfx.ts` 1 行超宽、`overview.rs` 宽行基线需收缩；i18n 门禁报 doudizhu 三处硬编码中文。建议单独工单清理，勿与本次修复混提。

## 验证命令

```bash
cargo test -p flux-api --bin flux-api fixed_lottery     # 15 passed / 4 ignored
cargo check -p flux-worker --all-targets               # Finished
pnpm --filter @fluxtorrent/web exec vitest run tests/lottery-claim-form.test.ts  # 5 passed
cargo test -p flux-api db_tests -- --ignored --test-threads=1  # 17 passed
pnpm --filter @fluxtorrent/web exec tsc --noEmit        # exit 0
node scripts/line_limit_guard.mjs                      # 本模块无新增违规
node scripts/i18n_guard.mjs                            # 本模块无新增硬编码中文
```
