# Worker + Tracker + 基础设施审计报告（agent_c05d78b4）

## 缺陷清单

### [P0] announce 计费链路整体死锁（consume_announce）
- 位置：apps/worker/src/jobs.rs:645-681
- 证据：worker 日志每分钟 ERROR "bind message supplies 0 parameters, but prepared statement sqlx_s_23 requires 1"；Redis 游标 flux:announce:cursor 自 06:06 冻结，stream XLEN 持续增长（1800→1806）；测试 announce 全部滞留，snatches/traffic_ledger/users 均未落账（traffic_ledger 最新 2026-07-11）。
- 根因：schema 漂移——0071 迁移要 connectable SMALLINT，但 0032 已有 BOOLEAN 列，ADD COLUMN IF NOT EXISTS 静默 no-op；worker 绑 Option<i16> 协议失败。
- 修法：ALTER TABLE snatches ALTER COLUMN connectable TYPE smallint USING CASE WHEN connectable THEN 1 ELSE 0 END（或 worker 改绑 boolean）。修后游标自动续跑。

### [P0] nexusphp-scheduler/cleanup/queue 三容器 crash-loop 空转
- 证据：restarts=291，日志循环 "Waiting for MySQL... MySQL not available after 30s"（本站 PostgreSQL）。与 flux-worker 零重叠，纯资源空转。
- 修法：compose 移除三服务。

### [P1] seeding_reward 每轮失败（boolean = integer）
- 位置：jobs.rs:338 `NOT (s.connectable = 0 AND s.uploaded = 0)`——同一 schema 漂移姊妹症状。
- 后果：①做种收益停发（最后一条 seeding_reward 09-13）；②尾部 spark_balance 全量对账一并跳过，无兜底。
- 修法：随列类型迁移解决；建议把 spark_balance 对账挪进 6h reconcile_snapshots。

### [P1] achievement_grant RETURNING 引用目标表不存在列 → 成就系统从未授予
- 位置：jobs.rs:1179-1182：RETURNING code, reward_sparks 在 user_achievements 不存在；achievement_defs 9 条、user_achievements 0 行。
- 修法：WITH ins AS (INSERT ... RETURNING user_id, def_id) SELECT ... JOIN achievement_defs。

### [P1] ratio_watch make_interval(bigint) 不存在 → Ratio Watch 从未生效
- 位置：jobs.rs:1405：$2 绑 i64，PG16 无该重载；users.ratio_watch_until 全库 0 行。
- 修法：$2::int。同型炸弹：jobs.rs:1529-1533 funding_settle make_interval(hours=>$2) 也绑 i64，首个达标众筹结算必炸；hr_enforce:906 已正确 ::int 可对照。

### [P1] wishlist_notify 第二条 SQL 引用上一条的 CTE → 24h 节流失效
- 位置：jobs.rs:856-865：UPDATE ... FROM recent r 报 relation "recent" does not exist；notified_at 永不推进，命中窗口每小时重复发信（当前 wishlist 0 行未实际发生）。
- 修法：UPDATE 并入第一条 CTE 或内联子查询重算。

### [P2] 杂项
- peers.rs:320 / main.rs:517：scrape/announce 的 downloaded 恒 0（硬编码），客户端完成数失真。
- main.rs:277-282：限流键仅 INCR=1 时 EXPIRE，进程中途挂掉留永生键（低概率）。
- peers.rs:240-246：无 announce 的死 swarm 仅靠 /metrics 触发 gc_all，Prometheus 不在时无上限驻留。
- redis appendonly=no：崩溃丢游标后 announce 重放天然幂等（安全），agent_block hits 会重复 +1。
- seeding_reward 幂等键用 UTC 小时，UTC+8 口径结账时刻错位 8h（不影响金额）。
- 除 bank_settle_runs 外任务无 last_run 落库，健康只能靠日志反推。

## 已验证闭环（实测）
- tracker HTTP 全链路：compact 响应（interval=1800/min=90/6字节peers/BEP-7 peers6）、left 判定、completed 翻转、双 peer scrape、stopped 归零、伪造 passkey 拒绝、坏 peer_id 拒绝、XADD 事件完整入流。
- 银行结算：每站点日恰一行，子任务幂等键+日期游标齐全，UTC+8 日界幂等无重发漏发，init_bank_day 重启基线正确。
- 计量同源：tracker→Redis Stream→worker→traffic_ledger→users 单一账本链，admin 对账同公式。
- 幂等/崩溃恢复：task_settle CAS、resurrection/funding/achievement/class_promo、seeding、consume_announce 差量化重放——均合格。

## 任务健康表（06:04Z 启动至今）
| 任务 | 频率 | 状态 |
|---|---|---|
| consume_announce | 60s | 死锁(P0) |
| seeding_reward | 1h | 每轮失败(P1) |
| ratio_watch | 1h | 每轮失败(P1) |
| wishlist_notify | 1h | 每轮失败(P1) |
| achievement_grant | 1h | 每轮失败(P1) |
| 其余 20+ 任务（consume_agent_blocks/expire_promotions/magic_pool/preserve_exit/backfill_pieces_hash/sweep_stale_peers/collect_milestones/hr_enforce/class_auto_adjust/task_settle/bank_daily/purge_old_login_events/dormant_mark/highspeed_tag/resurrection_settle/funding_settle/refundable_settle/cheat_audit/multi_ip_check/leak_scan/reconcile_snapshots/ensure_partitions） | — | 正常 |
| nexusphp ×3 | — | crash-loop ×291(P0) |

核心结论：tracker 本身健康；worker 计费入口+4 个小时级经济任务全部瘫痪，共享两颗根因（connectable 列类型漂移、make_interval 参数类型），修复量小收益大。
