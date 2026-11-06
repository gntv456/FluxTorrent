-- 0465: 修正 balance_baseline.base_spark 口径（旧站全历史流水 vs 期末快照）
--
-- 现象（2026-11-06 对账告警）：
--   spark 漂移 2,959 户（drift_with_ledger=2959，diff 3.28 亿），
--   uploaded 漂 3 户、downloaded 漂 1 户。
--
-- 根因：迁站把旧站**全历史**流水（spark_ledger 里 983,436 笔 legacy_bt*，
-- 含负额/透支）整体搬进新站，而 users.spark_balance 是旧站的**期末快照**
-- （迁站脚本按 seed_points 落库）。两者不是「期初 + 增量」关系：
--   权威值 = base_spark(=期末快照) + Σledger(=全历史流水) ⇒ 同一笔钱算两遍，
--   权威值随历史长度漂得越来越远，reconcile 每轮都报漂移。
--
-- 修法：基线改成**残差口径**——base_spark := 当前快照 − Σledger。
--   修后权威值 = (快照 − Σledger) + Σledger = 快照，漂移归零；
--   此后新发生的赚取/消费照常进 ledger，worker 重算（base + Σledger）
--   得到同一个值 ⇒ 长期自洽，且**用户余额数值不变**（残差法只改基线口径）。
--   幂等：重跑时快照已被重算成 base+Σledger，残差还原为原 base ⇒ 不变。
--
-- 只改 spark：base_up/base_down 无此问题——traffic_ledger 里**没有**迁站
-- 导入行（只有 promo 与匿名 reason 两类），旧站流量本就只落在基线上，
-- 口径正确，那 3 户漂移由 reconcile_snapshots 的回写自然收敛。
--
-- 回滚：UPDATE balance_baseline b SET base_spark = b.base_spark
--        （按需用旧值回填；备份见 balance_baseline_bak_* 系列）。

UPDATE balance_baseline b
   SET base_spark = COALESCE(u.spark_balance, 0) - COALESCE(l.sa, 0),
       updated_at = now()
  FROM users u
  LEFT JOIN (
        SELECT user_id, sum(amount) AS sa
          FROM spark_ledger
         GROUP BY user_id
  ) l ON l.user_id = u.id
 WHERE u.id = b.user_id;
