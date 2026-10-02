# 任务面板（后台任务）

## 架构

45 个定时任务全部在 **worker 容器内调度**（无需系统 crontab）：advisory lock 防多实例抢跑、900s 超时、四档节奏（60s / 10m / 30m / 6h / 24h）。多实例部署时任务按实例分片认领（SKIP LOCKED）。任务数随版本增减，以后台「任务面板」实际列表为准；所属模块关闭的任务不会执行，面板显示「未运行」属正常。

## 代表性任务

| job | 作用 |
| :--- | :--- |
| consume_announce | announce → 计费（Redis Stream 消费，死信进 DLQ） |
| expire_promotions | 促销到期回退 |
| hr_enforce / hr_punish | H&R 达标判定与惩罚 |
| class_auto_adjust | 等级自动升降（升降均发站内信） |
| exam_assign / task_settle | 考核派发与结算 |
| seeding_reward / bank_daily | 做种收益、银行结息 |
| dormant_mark | 休眠账号打标 |
| cheat_audit / multi_ip_check / highspeed_tag / leak_scan | 风控四件套 |
| reconcile_diff_alert / reconcile_snapshots | 账本对账：前者每 6h 比对流水合计 vs 余额快照（超阈值告警，先留证）；后者静默收敛快照漂移。删除用户自 0266 起自动补 `user_purge` 流水，正常运营不应再触发告警——若仍告警说明存在新的流水外动账路径，按告警排查 |
| dlq_watch | 死信看门狗（自动重试） |
| purge_runtime_logs | 运行日志清理 |

## 手动触发与观测

- 后台「任务面板」：全量任务清单、上次执行时间/结果、**手动触发**（0218 起）、`executed_by` 实例列（多机可见是哪个实例跑的，0225 起）；
- 失败排查：任务失败落 runtime_logs，后台「运行日志」可查；重复失败先看依赖（DB/Redis/外部 SMTP）。

## 升级注意

新增任务随代码发布自动注册进面板；任务名即幂等键，手动重跑安全。
