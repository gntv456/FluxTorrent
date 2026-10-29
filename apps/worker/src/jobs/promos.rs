//! 促销到期/魔力池/保种退出结算。
//! 从 jobs.rs 按域拆出。

use chrono::Datelike;
use sqlx::PgPool;

/// 促销到期回收（M06：到期自动回收，无残留）。
/// 审计修复：保留一年历史窗口——hr_enforce 建 H&R 快照时要按 completed_at 时点
/// 回查「当时是否处于免费窗口」豁免，物理删掉近期促销会让回查失明（误判违规）。
/// 计费查询全部走 `starts_at <= now() < ends_at` 生效窗口，不受历史保留影响。
/// 口径对齐（P2）：边界日含当日删除（`<=`），与 API 侧
/// apps/api/src/staff_http/stats.rs do_cleanup 完全一致。
pub async fn expire_promotions(db: &PgPool) -> anyhow::Result<u64> {
    let res = sqlx::query(
        "DELETE FROM promotions WHERE ends_at <= now() - interval '365 days'",
    )
    .execute(db)
    .await?;
    Ok(res.rows_affected())
}

/// 魔法池双免联动（原实现为死功能：promo_started 全库无写入点）。
/// 站点时区 UTC+8 每月 1-3 号检查上月是否达标（donated_total >= goal），
/// 达标则开全站 scope 双免促销（x2free，3 天），幂等靠 promo_started 标记。
pub async fn magic_pool_promo(db: &PgPool) -> anyhow::Result<u64> {
    let now_site = chrono::Utc::now() + chrono::Duration::hours(8);
    if !(1..=3).contains(&now_site.day()) {
        return Ok(0);
    }
    let res = sqlx::query(
        r#"
        WITH prev AS (
            SELECT to_char(date_trunc('month', $1::date) 
                   - interval '1 month', 'YYYY-MM') AS month
        ),
        pool AS (
            SELECT mp.month, mp.promo_started
            FROM magic_pool mp, prev
            WHERE mp.month = prev.month 
              AND mp.donated_total >= mp.goal 
              AND NOT mp.promo_started
        ),
        started AS (
            UPDATE magic_pool mp SET promo_started = TRUE
            FROM pool WHERE mp.month = pool.month
            RETURNING mp.month
        )
        INSERT INTO promotions 
            (scope, torrent_id, kind, starts_at, ends_at, source)
        SELECT 'global', NULL, 'x2free', now(), 
               now() + interval '3 days', 'magic_pool'
        FROM started
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(now_site)
    .execute(db)
    .await?;
    let n = res.rows_affected();
    if n > 0 {
        tracing::info!(n, "magic_pool promo started (x2free, 3d)");
    }
    Ok(n)
}

/// 全站促销开播播报（run.rs:164 的 job:announce_site_promos）。
///
/// 语义：检测「已生效但尚未播报过」的全站（scope=global）促销，向聊天室
/// （shoutbox）发一条系统播报——免费/双倍活动对散客的触达短板就在
/// 「开了没人知道」（产品分析 P0：促销触达）。幂等锚 promotions.id：
/// 播报过的促销在备注里带 id，重跑不重发（shoutbox 无删除通道，
/// 防刷屏是硬约束）。60s 一轮与调度注册的 cadence 一致。
/// 播报身份取在场最高等级职员（缺省 id=1）；无新促销时零查询副作用。
///
/// E-2 补齐（0423② 半成品）：promotions.announced_at 列自 0423 加上后
/// 零消费——「全员站内信」只在迁移注释里存在。现在 global 促销在
/// shoutbox 播报的同时给活跃用户（90 天内登录）发一封站内信，
/// 幂等改锚 announced_at（UPDATE ... WHERE announced_at IS NULL），
/// 与 shoutbox 的 LIKE 判据双保险。单种/分类促销仍不进全员信箱
/// （防刷屏），触达走种子页促销标记与愿望单推送等既有通道。
pub async fn announce_site_promos(db: &PgPool) -> anyhow::Result<u64> {
    // ① 站内信（announced_at 幂等；一次一条促销一条信，收件=活跃用户集）
    let pm = sqlx::query(
        r#"
        WITH fresh AS (
            SELECT id, kind, ends_at FROM promotions
            WHERE scope = 'global' AND starts_at <= now() AND ends_at > now()
              AND announced_at IS NULL
            ORDER BY id LIMIT 1
        ),
        claimed AS (
            UPDATE promotions SET announced_at = now()
            WHERE id IN (SELECT id FROM fresh)
              -- 与头注「UPDATE ... WHERE announced_at IS NULL」对齐：
              -- 缺这句多副本会各发一轮全员信（第二个 UPDATE 不跳行）。
              AND announced_at IS NULL
            RETURNING id, kind, ends_at
        ),
        staff AS (
            SELECT id FROM users WHERE status < 2 AND class_id >= 99
            ORDER BY id LIMIT 1
        )
        INSERT INTO messages (sender_id, receiver_id, subject, body, kind)
        SELECT staff.id, u.id,
               CASE claimed.kind
                   WHEN 'x2free' THEN '促销开启：全站免费 + 双倍上传'
                   WHEN 'free'   THEN '促销开启：全站免费'
                   WHEN 'half'   THEN '促销开启：全站半量下载'
                   WHEN 'x2'     THEN '促销开启：全站双倍上传'
                   WHEN 'x2half' THEN '促销开启：全站双倍上传 + 半量下载'
                   ELSE '促销开启：全站促销活动'
               END,
               '促销已开启，截止 ' 
               || to_char(claimed.ends_at AT TIME ZONE 'Asia/Shanghai',
                                               'MM-DD HH24:mi') || '，抓紧时间下载！',
               'system'
        FROM claimed
        CROSS JOIN users u
        -- staff 为空时也要发：LEFT JOIN 让 sender_id 落 NULL（系统信），
        -- 否则 staff.id 不在 FROM 里＝运行期 missing FROM-clause，整条 job 失败。
        LEFT JOIN staff ON TRUE
        WHERE u.status < 2
          AND u.last_seen_at > now() - interval '90 days'
          -- 与欢迎信/违规信同口径：系统通知必须过收件人偏好闸
          AND u_notice_enabled(u.id, 'system')
          AND NOT EXISTS (SELECT 1 FROM users WHERE id = u.id AND id = staff.id)
        "#,
    )
    .execute(db)
    .await?;
    let pm_n = pm.rows_affected();
    if pm_n > 0 {
        tracing::info!(pm_n, "site promo announced via site mail");
    }
    // ② shoutbox 播报（原有通道，LIKE 判据保留）
    let res = sqlx::query(
        r#"
        WITH fresh AS (
            SELECT id, kind, ends_at FROM promotions
            WHERE scope = 'global' AND starts_at <= now() AND ends_at > now()
              AND NOT EXISTS (
                  SELECT 1 FROM shoutbox sb
                  WHERE sb.message LIKE '%promo:' || promotions.id || '%'
              )
            ORDER BY id LIMIT 3
        ),
        staff AS (
            SELECT id FROM users WHERE status < 2 AND class_id >= 99
            ORDER BY id LIMIT 1
        )
        INSERT INTO shoutbox (user_id, message)
        SELECT staff.id,
               CASE fresh.kind
                   WHEN 'x2free' THEN '🎉 全站 FREE + 双倍上传已开启！'
                   WHEN 'free'   THEN '🎉 全站 FREE 已开启！'
                   WHEN 'half'   THEN '🎉 全站 50% 下载已开启！'
                   WHEN 'x2'     THEN '🎉 全站双倍上传已开启！'
                   WHEN 'x2half' THEN '🎉 全站 2x上传+50%下载已开启！'
                   ELSE '🎉 全站促销活动进行中！'
               END
               || ' 截止 ' 
               || to_char(fresh.ends_at AT TIME ZONE 'Asia/Shanghai', 
                          'MM-DD HH24:mi')
               || '（promo:' || fresh.id || '）'
        FROM fresh CROSS JOIN staff
        "#,
    )
    .execute(db)
    .await?;
    let n = res.rows_affected();
    if n > 0 {
        tracing::info!(n, "site promo announced to shoutbox");
    }
    Ok(n)
}
