-- 0407_hr_warn_repair.sql
-- 修复 H&R 警告阶梯的两处存量伤害（代码侧同步修在
-- apps/worker/src/jobs/hr_warn_ladder.rs）：
--
-- ① 同一批违规每轮重算：`decide` 的顺延起点是警告自身的 expires_at，
--    worker 每分钟跑一次，就把到期时刻再加一档。现网 41 行被推到
--    2026-11 ~ 2028-01（其中 32 行已无未解决违规），等于「到期自动
--    恢复」永远不来；hr_warn_history 也按分钟灌了 3.4 万行。
-- ② 选档把「本次违规」算进了「本次之前」的条数：count=1 命中
--    step_for(1)，第一次警告直接变成第 2 档（限下），0388 的
--    「第 1 档只警告」从未生效。
--
-- 本迁移按当前未解决违规数重算存量行：
--   * 无未解决违规 → 清零警告（并解 H&R 自己下的下载锁）；
--   * 仍有违规 → step = min(count, 4)，动作位按 LADDER 重算；
--   * 到期明显超出「新档天数 + 2 天」才重置为 now + 新档天数
--     （幂等：重跑不会再次顺延）。
-- 历史行保留（申诉要看），只停止继续增长。

BEGIN;

-- ① 违规已处理完的人：警告应随「违规消除」结束，不再被 2027 年的
--    假到期时刻拖着。先解锁再清零（只解 download_locked_by='hr'）。
UPDATE users u
SET download_enabled = TRUE, download_locked_by = NULL
FROM hr_warn_states w
WHERE w.user_id = u.id
  AND w.step > 0
  AND u.status < 2
  AND NOT u.download_enabled
  AND u.download_locked_by = 'hr'
  AND NOT EXISTS (
      SELECT 1 FROM hr_violations v
      WHERE v.user_id = u.id AND v.resolved_at IS NULL
  );

UPDATE hr_warn_states w
SET step = 0,
    expires_at = NULL,
    shifted_days = 0,
    hit_cap = FALSE,
    restrict_download = FALSE,
    block_upload = FALSE,
    block_invite = FALSE,
    updated_at = now()
WHERE w.step > 0
  AND NOT EXISTS (
      SELECT 1 FROM hr_violations v
      WHERE v.user_id = w.user_id AND v.resolved_at IS NULL
  );

-- ② 仍有未解决违规的人：按当前条数重算档位与动作。
--    count=1 → 第 1 档（只警告）；count=2 → 限下；3 → 禁传；4+ → 禁邀。
WITH live AS (
    SELECT w.user_id,
           LEAST(count(v.id), 4)::int AS lvl,
           w.base_days
    FROM hr_warn_states w
    JOIN hr_violations v
      ON v.user_id = w.user_id AND v.resolved_at IS NULL
    WHERE w.step > 0
    GROUP BY w.user_id, w.base_days
), want AS (
    SELECT l.user_id, l.lvl, l.base_days,
           GREATEST(
               CASE l.lvl
                   WHEN 1 THEN 7
                   WHEN 2 THEN 14
                   WHEN 3 THEN 30
                   ELSE 90
               END,
               l.base_days
           ) AS days
    FROM live l
)
UPDATE hr_warn_states w
SET step = want.lvl,
    restrict_download = want.lvl >= 2,
    block_upload = want.lvl >= 3,
    block_invite = want.lvl >= 4,
    shifted_days = CASE
        WHEN w.expires_at >
             now() + make_interval(days => want.days) + interval '2 days'
        THEN 0 ELSE w.shifted_days END,
    hit_cap = CASE
        WHEN w.expires_at >
             now() + make_interval(days => want.days) + interval '2 days'
        THEN FALSE ELSE w.hit_cap END,
    expires_at = CASE
        WHEN w.expires_at >
             now() + make_interval(days => want.days) + interval '2 days'
        THEN now() + make_interval(days => want.days)
        ELSE w.expires_at END,
    updated_at = now()
FROM want
WHERE w.user_id = want.user_id;

-- ③ 补发阶梯通知：旧 worker 只落状态、从未发过「第 N 档」信；新 worker
--    对同档用户会跳过，不补则这批人永远只知道自己违规、不知道被限了
--    什么、何时恢复。只在从未收到过阶梯信时补一次（幂等）。
INSERT INTO messages (sender_id, receiver_id, subject, body, kind)
SELECT NULL, w.user_id,
       'H&R 警告（第 ' || w.step || ' 档）',
       '您当前有 ' || c.n || ' 条未解决的 H&R 违规，站点已按警告阶梯'
       || '记为第 ' || w.step || ' 档（共 4 档）。' || chr(10)
       || '本次处置：' || CASE
           WHEN w.step = 1 THEN '本次仅警告，暂不限制任何权限'
           ELSE '已暂停：' || concat_ws('、',
               CASE WHEN w.restrict_download THEN '下载' END,
               CASE WHEN w.block_upload THEN '发布新种' END,
               CASE WHEN w.block_invite THEN '生成邀请码' END)
       END || '。' || chr(10)
       || '本档警告到期时间：'
       || to_char(w.expires_at AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI')
       || '（UTC）。' || chr(10)
       || '恢复方式：补足做种时长达标后违规会自动消除；也可在'
       || '「我的 H&R」用魔力自助免罪。未解决违规全部处理完毕且'
       || '本档警告到期后，权限自动恢复。',
       'hr_violation'
FROM hr_warn_states w
JOIN (
    SELECT user_id, count(*)::int AS n
    FROM hr_violations
    WHERE resolved_at IS NULL
    GROUP BY user_id
) c ON c.user_id = w.user_id
WHERE w.step > 0
  AND w.expires_at IS NOT NULL
  AND u_notice_enabled(w.user_id, 'hr_violation')
  AND NOT EXISTS (
      SELECT 1 FROM messages m
      WHERE m.receiver_id = w.user_id
        AND m.kind = 'hr_violation'
        AND m.body LIKE '%警告阶梯%'
  );

COMMIT;
