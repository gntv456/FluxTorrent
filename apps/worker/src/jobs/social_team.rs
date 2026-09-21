//! 组队契约结算与到期。
//! 从 jobs.rs 按域拆出。

use sqlx::PgPool;

/// 组队契约结算（0102 社交层）：团队成员在契约期内的做种增量合计达标 →
/// 按贡献占比分账 + 写拯救荣誉 + 推进状态。
///
/// 与单人复活任务**严格隔离**：resurrection_settle 只处理 `team_id IS NULL` 的行，
/// 否则队长会按 `r.user_id` 再拿一份全额奖励（双发）。
///
/// 判据：团队总增量 ≥ required_hours × 3600，且至少一名成员当前仍在做种。
/// 贡献口径：snatches 增量（`seeded_seconds - 基线`），与 seed_preserve 同款 —— 精度到单资源，
/// 杜绝「挂无关种子刷契约贡献」；也不用 spark_ledger 反查（seeding_reward 按用户逐小时聚合、无 ref_id）。
///
/// 可靠性：以 `social_team.settled_at IS NULL` 作为「未处理」标记（而非纯状态 CAS），
/// 配合 spark_ledger 幂等键 `social:team:{team_id}:{uid}` —— 中途崩溃重跑既不丢奖也不重发。
pub(crate) async fn social_team_settle(db: &PgPool) -> anyhow::Result<u64> {
    let teams: Vec<(i64, i64, i64, i64, i32)> = sqlx::query_as(
        r#"
        SELECT st.id, r.id, r.torrent_id, r.reward_sparks, r.required_hours
        FROM social_team st
        JOIN resurrections r ON r.team_id = st.id AND r.status = 'open'
        WHERE st.status IN (0, 1) AND st.settled_at IS NULL
          AND EXISTS (
              SELECT 1 FROM social_team_member m
              JOIN snatches s ON s.user_id = m.uid AND s.torrent_id = r.torrent_id
              WHERE m.team_id = st.id AND m.join_status IN (0, 1) AND s.seeding)
          AND (
              SELECT COALESCE(sum(GREATEST(COALESCE(s.seeded_seconds, 0)::bigint - m.seed_seconds_begin, 0)), 0)
              FROM social_team_member m
              LEFT JOIN snatches s ON s.user_id = m.uid AND s.torrent_id = r.torrent_id
              WHERE m.team_id = st.id AND m.join_status IN (0, 1)
          ) >= r.required_hours::bigint * 3600
        "#,
    )
    .fetch_all(db)
    .await?;

    if teams.is_empty() {
        return Ok(0);
    }

    // 信誉参数：完成/失败/退出都由系统判定，不可伪造（队友评价不参与主流程，防互刷）
    let rep_gain: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value::bigint FROM site_settings \
         WHERE name = 'social_rep_on_fulfilled'), 20)",
    )
    .fetch_one(db)
    .await
    .unwrap_or(20);
    let rep_min: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value::bigint FROM site_settings \
         WHERE name = 'social_rep_min'), 0)",
    )
    .fetch_one(db)
    .await
    .unwrap_or(0);
    let rep_max: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value::bigint FROM site_settings \
         WHERE name = 'social_rep_max'), 2000)",
    )
    .fetch_one(db)
    .await
    .unwrap_or(2000);

    let mut done = 0u64;
    for (team_id, rid, tid, reward, _hours) in &teams {
        let (team_id, rid, tid, reward) = (*team_id, *rid, *tid, *reward);
        let mut tx = db.begin().await?;

        let members: Vec<(i64, i64)> = sqlx::query_as(
            "SELECT m.uid, GREATEST(COALESCE(s.seeded_seconds, 0)::bigint - m.seed_seconds_begin, 0) \
             FROM social_team_member m \
             LEFT JOIN snatches s ON s.user_id = m.uid AND s.torrent_id = $2 \
             WHERE m.team_id = $1 AND m.join_status IN (0, 1)",
        )
        .bind(team_id)
        .bind(tid)
        .fetch_all(&mut *tx)
        .await?;

        let total: i64 = members.iter().map(|m| m.1).sum();
        if total <= 0 {
            continue;
        }

        // 按增量占比分账；整数除法的余数落在排序末位（贡献最少者）身上，
        // 保证分配总额恰好等于 reward（实测 5000 → 2777 + 2223）
        let mut ordered = members.clone();
        ordered.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        let mut remaining = reward;
        for (i, (uid, delta)) in ordered.iter().enumerate() {
            let amount = if i == ordered.len() - 1 {
                remaining
            } else {
                (reward * delta) / total
            };
            remaining -= amount;
            if amount > 0 {
                let idem = format!("social:team:{team_id}:{uid}");
                sqlx::query(
                    "INSERT INTO spark_ledger (id, user_id, amount, kind, idempotency_key) \
                     SELECT nextval('spark_ledger_id_seq'), $1, $2, 'social_team_reward', $3 \
                     WHERE NOT EXISTS (SELECT 1 FROM spark_ledger WHERE idempotency_key = $3)",
                )
                .bind(*uid)
                .bind(amount)
                .bind(&idem)
                .execute(&mut *tx)
                .await?;
                sqlx::query(
                    "UPDATE users SET spark_balance = spark_balance + $2 WHERE id = $1 \
                     AND NOT EXISTS (SELECT 1 FROM spark_ledger WHERE idempotency_key = $3)",
                )
                .bind(*uid)
                .bind(amount)
                .bind(&idem)
                .execute(&mut *tx)
                .await?;
            }
            sqlx::query(
                "UPDATE social_team_member SET contributed_sec = $3, settled_amount = $4, join_status = 4 \
                 WHERE team_id = $1 AND uid = $2",
            )
            .bind(team_id)
            .bind(*uid)
            .bind(delta)
            .bind(amount)
            .execute(&mut *tx)
            .await?;
        }

        // 拯救荣誉（永久留存，不随赛季重置）
        let uids: Vec<i64> = members.iter().map(|m| m.0).collect();
        sqlx::query(
            "INSERT INTO rescue_honor (torrent_id, info_hash, team_id, uids, total_sec, seeders_before, seeders_after) \
             SELECT $1, t.info_hash, $2, $3, $4, 0, COALESCE(t.seeders, 0) FROM torrents t WHERE t.id = $1",
        )
        .bind(tid)
        .bind(team_id)
        .bind(&uids)
        .bind(total)
        .execute(&mut *tx)
        .await?;

        // 状态推进：settled_at 是「未处理」标记，必须与发奖同事务置位
        sqlx::query(
            "UPDATE social_team SET status = 2, \
         settled_at = now() WHERE id = $1",
        )
        .bind(team_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE resurrections SET status = 'done', \
         finished_at = now() WHERE id = $1",
        )
        .bind(rid)
        .execute(&mut *tx)
        .await?;

        for (uid, _) in &members {
            // 信誉 +（契约完成是系统判定的事实，不可伪造）
            sqlx::query(
                "INSERT INTO social_reputation (uid, score, fulfilled_count, updated_at) \
                 VALUES ($1, 1000 + $2, 1, now()) \
                 ON CONFLICT (uid) DO UPDATE SET \
                   score = LEAST($3, GREATEST($4, social_reputation.score + $2)), \
                   fulfilled_count = social_reputation.fulfilled_count + 1, \
                   updated_at = now()",
            )
            .bind(*uid)
            .bind(rep_gain)
            .bind(rep_max)
            .bind(rep_min)
            .execute(&mut *tx)
            .await?;
            let _ = sqlx::query(
                                "INSERT INTO messages (sender_id, receiver_id, \
                 subject, body) VALUES (NULL, $1, $2, $3)",
            )
            .bind(*uid)
            .bind("保种协作完成")
            .bind(format!(
                "你们小队协作保种的资源 #{tid} 已验收，奖励按贡献分摊到账。感谢你为保种出的力！"
            ))
            .execute(&mut *tx)
            .await;
        }

        tx.commit().await?;
        done += 1;
    }

    if done > 0 {
        tracing::info!(n = done, "social teams settled");
    }
    Ok(done)
}
