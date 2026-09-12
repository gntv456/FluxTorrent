//! 任务系统结算（0051）：领取时记指标基线，达标发奖、超时失败。
//! 口径（对齐好学站 UserTaskRecord 的 base + delta）：
//!   upload_delta      = users.uploaded - base_uploaded
//!   download_delta    = users.downloaded - base_downloaded（仅用于 Master 系列等级任务的总流量口径）
//!   seed_seconds_delta= sum(snatches.seeded_seconds) - base_seed_seconds（做种时长）
//!   uploads           = 发布种子数 - base_uploads（领取后新发布）
//! metric JSON 里出现的键参与判定：全部达标 → 完成；任务到期/超时限 → 失败。
//! 注：等级系列任务（Master/Ultimate…）metric 是累计门槛而非增量，
//!     此类任务 base 记 0 且用 users 现值直接比对（tier 非空视为累计口径）。

use sqlx::PgPool;

#[derive(serde::Deserialize, Default)]
struct TaskMetric {
    #[serde(default)]
    upload_delta: Option<i64>,
    #[serde(default)]
    download_delta: Option<i64>,
    #[serde(default)]
    seed_points_delta: Option<i64>, // 做种积分 = 做种时长秒（1 积分/小时 × 3600 折算前的秒）
    #[serde(default)]
    uploads: Option<i64>,
    #[serde(default)]
    subtitles: Option<i64>,
}

#[derive(sqlx::FromRow)]
struct OpenClaim {
    id: i64,
    task_id: i64,
    user_id: i64,
    reward: i64,
    penalty: i64,
    duration_days: i32,
    tier: Option<String>,
    metric: serde_json::Value,
    claimed_at: chrono::DateTime<chrono::Utc>,
    base_uploaded: i64,
    base_seed_seconds: i64,
    base_uploads: i64,
}

/// 结算一次所有进行中的认领。worker 分钟级调用（幂等：状态推进用条件更新 + 发奖幂等键）。
pub async fn task_settle(db: &PgPool) -> anyhow::Result<u64> {
    let claims: Vec<OpenClaim> = sqlx::query_as(
        "SELECT c.id, c.task_id, c.user_id, t.reward, t.penalty, t.duration_days, t.tier, \
                t.metric, c.claimed_at, c.base_uploaded, c.base_seed_seconds, c.base_uploads \
         FROM task_claims c JOIN tasks t ON t.id = c.task_id \
         WHERE c.status = 0 LIMIT 500",
    )
    .fetch_all(db)
    .await?;

    let mut done = 0u64;
    for c in claims {
        let metric: TaskMetric = serde_json::from_value(c.metric.clone()).unwrap_or_default();
        let now = chrono::Utc::now();

        // 用户当前指标
        let cur: Option<(i64, i64)> = sqlx::query_as(
            "SELECT uploaded, downloaded FROM users WHERE id = $1",
        )
        .bind(c.user_id)
        .fetch_optional(db)
        .await?;
        let Some((uploaded, downloaded)) = cur else { continue }; // 用户已删，跳过

        let seed_seconds: i64 = sqlx::query_scalar(
            "SELECT COALESCE(sum(seeded_seconds), 0) FROM snatches WHERE user_id = $1",
        )
        .bind(c.user_id)
        .fetch_one(db)
        .await
        .unwrap_or(0);
        let uploads_now: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM torrents WHERE owner_id = $1 AND approval_status = 1",
        )
        .bind(c.user_id)
        .fetch_one(db)
        .await
        .unwrap_or(0);
        let subtitles_now: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM subtitles WHERE uploader_id = $1",
        )
        .bind(c.user_id)
        .fetch_one(db)
        .await
        .unwrap_or(0);

        // 累计口径（tier 任务）：base 视为 0，用现值直接比
        let cumulative = c.tier.is_some();
        let (up_d, seed_d, uploads_d) = if cumulative {
            (uploaded, seed_seconds, uploads_now)
        } else {
            (
                uploaded - c.base_uploaded,
                seed_seconds - c.base_seed_seconds,
                uploads_now - c.base_uploads,
            )
        };

        // 达标判定：metric 里出现的键全部满足
        let mut met = true;
        let mut has_target = false;
        if let Some(v) = metric.upload_delta {
            has_target = true;
            met &= up_d >= v;
        }
        if let Some(v) = metric.download_delta {
            has_target = true;
            met &= downloaded >= v;
        }
        if let Some(v) = metric.seed_points_delta {
            has_target = true;
            // 站内 seed_points 近似：做种数 × 100；但任务口径用做种时长更稳（小时 → 折算）
            met &= seed_d >= v * 3600 / 100.max(1);
        }
        if let Some(v) = metric.uploads {
            has_target = true;
            met &= uploads_d >= v;
        }
        if let Some(v) = metric.subtitles {
            has_target = true;
            met &= subtitles_now >= v;
        }

        let deadline = c.claimed_at + chrono::Duration::days(c.duration_days.max(1) as i64);
        if met && has_target {
            if settle_complete(db, &c).await? {
                done += 1;
            }
        } else if now > deadline || now > task_end(db, c.task_id).await {
            settle_fail(db, &c).await?;
            done += 1;
        }
    }
    Ok(done)
}

async fn task_end(db: &PgPool, task_id: i64) -> chrono::DateTime<chrono::Utc> {
    sqlx::query_scalar("SELECT ends_at FROM tasks WHERE id = $1")
        .bind(task_id)
        .fetch_one(db)
        .await
        .unwrap_or(chrono::DateTime::<chrono::Utc>::MAX_UTC)
}

/// 达标：status=1 + 发奖（幂等键 task_settle:{claim_id}，防 worker 并发双发）
async fn settle_complete(db: &PgPool, c: &OpenClaim) -> anyhow::Result<bool> {
    let mut tx = db.begin().await?;
    let updated = sqlx::query(
        "UPDATE task_claims SET status = 1, settled_at = now(), reward_paid = $2 \
         WHERE id = $1 AND status = 0",
    )
    .bind(c.id)
    .bind(c.reward)
    .execute(&mut *tx)
    .await?;
    if updated.rows_affected() == 0 {
        tx.rollback().await?;
        return Ok(false);
    }
    if c.reward > 0 {
        let idem = format!("task_settle:{}", c.id);
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM spark_ledger WHERE idempotency_key = $1)",
        )
        .bind(&idem)
        .fetch_one(&mut *tx)
        .await?;
        if !exists {
            let balance: i64 = sqlx::query_scalar(
                "SELECT spark_balance FROM users WHERE id = $1 FOR UPDATE",
            )
            .bind(c.user_id)
            .fetch_one(&mut *tx)
            .await?;
            sqlx::query(
                "INSERT INTO spark_ledger (id, user_id, amount, kind, ref_type, ref_id, idempotency_key, balance_after) \
                 VALUES (nextval('spark_ledger_id_seq'), $1, $2, 'task_reward', 'task', $3, $4, $5)",
            )
            .bind(c.user_id)
            .bind(c.reward)
            .bind(c.task_id)
            .bind(&idem)
            .bind(balance + c.reward)
            .execute(&mut *tx)
            .await?;
            sqlx::query("UPDATE users SET spark_balance = spark_balance + $2 WHERE id = $1")
                .bind(c.user_id)
                .bind(c.reward)
                .execute(&mut *tx)
                .await?;
        }
    }
    sqlx::query(
        "INSERT INTO messages (sender_id, receiver_id, subject, body) VALUES (NULL, $1, $2, $3)",
    )
    .bind(c.user_id)
    .bind("任务完成通知")
    .bind(format!(
        "恭喜！您认领的任务已完成，奖励 {} 火花已发放到您的账户。",
        c.reward
    ))
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(true)
}

/// 失败/超时：status=2；配置了罚金则扣（幂等键 task_penalty:{claim_id}）
async fn settle_fail(db: &PgPool, c: &OpenClaim) -> anyhow::Result<()> {
    let mut tx = db.begin().await?;
    let updated = sqlx::query(
        "UPDATE task_claims SET status = 2, settled_at = now() WHERE id = $1 AND status = 0",
    )
    .bind(c.id)
    .execute(&mut *tx)
    .await?;
    if updated.rows_affected() == 0 {
        tx.rollback().await?;
        return Ok(());
    }
    if c.penalty > 0 {
        let idem = format!("task_penalty:{}", c.id);
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM spark_ledger WHERE idempotency_key = $1)",
        )
        .bind(&idem)
        .fetch_one(&mut *tx)
        .await?;
        if !exists {
            let balance: i64 = sqlx::query_scalar(
                "SELECT spark_balance FROM users WHERE id = $1 FOR UPDATE",
            )
            .bind(c.user_id)
            .fetch_one(&mut *tx)
            .await?;
            // 罚金只扣到 0，不制造负余额
            let take = balance.min(c.penalty).max(0);
            if take > 0 {
                sqlx::query(
                    "INSERT INTO spark_ledger (id, user_id, amount, kind, ref_type, ref_id, idempotency_key, balance_after) \
                     VALUES (nextval('spark_ledger_id_seq'), $1, $2, 'task_penalty', 'task', $3, $4, $5)",
                )
                .bind(c.user_id)
                .bind(-take)
                .bind(c.task_id)
                .bind(&idem)
                .bind(balance - take)
                .execute(&mut *tx)
                .await?;
                sqlx::query("UPDATE users SET spark_balance = spark_balance - $2 WHERE id = $1")
                    .bind(c.user_id)
                    .bind(take)
                    .execute(&mut *tx)
                    .await?;
            }
        }
    }
    sqlx::query(
        "INSERT INTO messages (sender_id, receiver_id, subject, body) VALUES (NULL, $1, $2, $3)",
    )
    .bind(c.user_id)
    .bind("任务超时通知")
    .bind(format!(
        "您认领的任务已超出完成时限（{} 天），任务标记为失败{}。",
        c.duration_days,
        if c.penalty > 0 { format!("，扣除罚金 {} 火花", c.penalty) } else { String::new() }
    ))
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}
