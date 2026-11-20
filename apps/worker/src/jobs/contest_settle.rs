//! 竞赛结算（0503 运营补全 #2 后半）：到期赛按 prizes JSONB 发奖。
//!
//! 计分口径 = contests.metric 赛制（0506 参数化，此前硬编码上传竞速）：
//!   · uploads   窗口内过审种数（0503 既有口径，缺省）
//!   · downloads 窗口内完成下载数（snatches.completed_at 落窗）
//!   · seedtime  窗口内做种秒增量（按 announce 刷出的 seeded_seconds
//!               快照差不可回溯，落窗口径=窗口末仍在做种的行各记其
//!               seeded_seconds，赛制说明里向站长言明是「榜时点存量」）
//! 全部现有列可算。分数回写 contest_entries.score 供前台榜单展示；
//! 奖励走 prizes [{"rank":N,"sparks":M},...] 名次制，幂等键锚
//! contest-prize:{id}:{uid}（重跑天然去重）。settled_at 置位后不再
//! 进入候选（is_active 同时置 FALSE）。

use sqlx::PgPool;

pub(crate) async fn contest_settle(db: &PgPool) -> anyhow::Result<u64> {
    // 候选：已到期、未结算的竞赛（含被手动关闭且已到期的）
    let contests: Vec<(i32, serde_json::Value, String)> = sqlx::query_as(
        "SELECT id, prizes, metric FROM contests \
         WHERE settled_at IS NULL AND ends_at <= now()",
    )
    .fetch_all(db)
    .await?;
    let mut acted = 0u64;
    for (cid, prizes, metric) in contests {
        // 赛榜：窗口内按赛制计分 TOP20（含未报名者也可上榜——
        // 报名只是参与资格的仪式性动作，奖励看成绩）
        let board: Vec<(i64, i64)> = match metric.as_str() {
            "downloads" => sqlx::query_as(
                "SELECT s.user_id, count(*)::bigint FROM snatches s \
                 WHERE s.completed_at IS NOT NULL AND s.user_id IN \
                   (SELECT user_id FROM contest_entries \
                    WHERE contest_id = $1) \
                   AND s.completed_at >= \
                     (SELECT starts_at FROM contests WHERE id = $1) \
                   AND s.completed_at < \
                     (SELECT ends_at FROM contests WHERE id = $1) \
                 GROUP BY s.user_id ORDER BY count(*) DESC LIMIT 20",
            )
            .bind(cid)
            .fetch_all(db)
            .await
            .unwrap_or_default(),
            "seedtime" => sqlx::query_as(
                "SELECT s.user_id, s.seeded_seconds::bigint FROM snatches s \
                 WHERE s.seeded_seconds > 0 AND s.user_id IN \
                   (SELECT user_id FROM contest_entries \
                    WHERE contest_id = $1) \
                   AND s.last_seen_at >= \
                     (SELECT starts_at FROM contests WHERE id = $1) \
                   AND s.last_seen_at < \
                     (SELECT ends_at FROM contests WHERE id = $1) \
                 ORDER BY s.seeded_seconds DESC LIMIT 20",
            )
            .bind(cid)
            .fetch_all(db)
            .await
            .unwrap_or_default(),
            _ => sqlx::query_as(
                "SELECT owner_id, count(*)::bigint FROM torrents \
                 WHERE approval_status = 1 AND owner_id IN \
                   (SELECT user_id FROM contest_entries \
                    WHERE contest_id = $1) \
                   AND created_at >= \
                     (SELECT starts_at FROM contests WHERE id = $1) \
                   AND created_at < \
                     (SELECT ends_at FROM contests WHERE id = $1) \
                 GROUP BY owner_id ORDER BY count(*) DESC LIMIT 20",
            )
            .bind(cid)
            .fetch_all(db)
            .await
            .unwrap_or_default(),
        };
        // 分数回写（展示用）
        for (uid, n) in &board {
            let _ = sqlx::query(
                "UPDATE contest_entries SET score = $2 \
                 WHERE contest_id = $1 AND user_id = $3",
            )
            .bind(cid)
            .bind(*n)
            .bind(uid)
            .execute(db)
            .await;
        }
        // prizes JSONB → [(rank, sparks)]
        let mut prize_list: Vec<(i64, i64)> = Vec::new();
        if let Some(arr) = prizes.as_array() {
            for p in arr {
                let rank = p.get("rank").and_then(|v| v.as_i64()).unwrap_or(0);
                let sparks =
                    p.get("sparks").and_then(|v| v.as_i64()).unwrap_or(0);
                if rank > 0 && sparks > 0 {
                    prize_list.push((rank, sparks));
                }
            }
        }
        for (rank, sparks) in &prize_list {
            if let Some((uid, _score)) = board.get((*rank - 1).max(0) as usize)
            {
                let idem = format!("contest-prize:{cid}:{uid}");
                // worker 侧无 economy_http——照 achv_dormant 先例直接 SQL
                // 入账（流水+余额同事务，幂等护栏装 INSERT 上）
                let mut tx = match db.begin().await {
                    Ok(t) => t,
                    Err(e) => {
                        tracing::warn!(?e, cid, uid, "竞赛奖励事务开启失败");
                        continue;
                    }
                };
                let credited = match sqlx::query(
                    "INSERT INTO spark_ledger \
                        (id, user_id, amount, kind, idempotency_key, \
                         balance_after) \
                      SELECT nextval('spark_ledger_id_seq'), $1, $2, \
                        'contest_prize', $3, u.spark_balance + $2 \
                      FROM users u WHERE u.id = $1 \
                      AND NOT EXISTS (SELECT 1 FROM spark_ledger \
                                      WHERE idempotency_key = $3)",
                )
                .bind(uid)
                .bind(sparks)
                .bind(&idem)
                .execute(&mut *tx)
                .await
                {
                    Ok(r) => r.rows_affected() > 0,
                    Err(e) => {
                        tracing::warn!(?e, cid, uid, "竞赛奖励流水写入失败");
                        let _ = tx.rollback().await;
                        continue;
                    }
                };
                if credited {
                    if let Err(e) = sqlx::query(
                        "UPDATE users SET spark_balance = \
                         spark_balance + $2 WHERE id = $1",
                    )
                    .bind(uid)
                    .bind(sparks)
                    .execute(&mut *tx)
                    .await
                    {
                        tracing::warn!(?e, cid, uid, "竞赛奖励余额更新失败");
                        let _ = tx.rollback().await;
                        continue;
                    }
                }
                let _ = tx.commit().await;
                let _ = sqlx::query(
                    "INSERT INTO messages \
                     (sender_id, receiver_id, subject, body) \\
                     VALUES (NULL, $1, $2, $3)",
                )
                .bind(uid)
                .bind(format!("竞赛「#{cid}」名次奖励"))
                .bind(format!(
                    "竞赛结算：你以第 {rank} 名完成比赛，奖励 {sparks} 魔力已入账。\\
                     感谢参与——下一场竞赛见。"
                ))
                .execute(db)
                .await;
            }
        }
        let _ = sqlx::query(
            "UPDATE contests SET settled_at = now(), \
             is_active = FALSE WHERE id = $1",
        )
        .bind(cid)
        .execute(db)
        .await;
        acted += 1;
    }
    Ok(acted)
}
