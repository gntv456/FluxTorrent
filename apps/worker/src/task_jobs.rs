//! 任务系统结算（0051）：领取时记指标基线，达标发奖、超时失败。
//! 口径（对齐参考站 UserTaskRecord 的 base + delta）：
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
    seed_points_delta: Option<i64>, // 做种积分：1 积分 = 1 小时做种（与 class_rules.min_seed_hours 同源）
    #[serde(default)]
    seed_seconds_delta: Option<i64>, // 做种时长增量（秒），无折算歧义；tier 累计口径下为绝对值
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
    /// 0093：任务类型（task/onboard/periodic），用于完成通知区分转正考核文案
    #[sqlx(default)]
    kind: String,
    #[sqlx(default)]
    task_name: String,
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
                t.kind, t.name AS task_name, t.metric, c.claimed_at, \
                c.base_uploaded, c.base_seed_seconds, c.base_uploads \
         FROM task_claims c JOIN tasks t ON t.id = c.task_id \
         WHERE c.status = 0 AND c.exempted_at IS NULL LIMIT 500",
    )
    .fetch_all(db)
    .await?;

    let mut done = 0u64;
    for c in claims {
        let metric: TaskMetric = serde_json::from_value(c.metric.clone()).unwrap_or_default();
        let now = chrono::Utc::now();

        // 用户当前指标
        let cur: Option<(i64, i64)> =
            sqlx::query_as("SELECT uploaded, downloaded FROM users WHERE id = $1")
                .bind(c.user_id)
                .fetch_optional(db)
                .await?;
        let Some((uploaded, downloaded)) = cur else {
            continue;
        }; // 用户已删，跳过

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
        let subtitles_now: i64 =
            sqlx::query_scalar("SELECT count(*) FROM subtitles WHERE user_id = $1")
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
            // 口径厘清（P1-6）：v 的单位是「做种积分」，1 积分 = 1 小时做种时长，
            // 与 class_rules.min_seed_hours 同源；判定 = 累计做种秒 ≥ v×3600。
            // 旧公式 v*3600/100（v×36 秒）无站内依据：站内 seed_points = 在做种数×100
            // （http.rs 口径）与时长无关，旧折算既不对应站内积分也不对应时长，废弃。
            met &= seed_d >= v.saturating_mul(3600);
        }
        if let Some(v) = metric.seed_seconds_delta {
            has_target = true;
            met &= seed_d >= v; // 秒口径，1 小时 = 3600，无折算歧义
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
            let balance: i64 =
                sqlx::query_scalar("SELECT spark_balance FROM users WHERE id = $1 FOR UPDATE")
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
    // 完成通知：转正考核（onboard）用专有文案——「转正」语义不落等级
    // （等级归 class_auto_adjust 管，P1-3 定案：考核通过只做确认 + 发奖，不动 class_id）
    let (subject, body) = if c.kind == "onboard" {
        (
            "转正考核通过",
            format!(
                "恭喜！您已完成新人转正考核「{}」，正式成为本站的一员。奖励 {} 魔力已发放到您的账户。",
                c.task_name, c.reward
            ),
        )
    } else {
        (
            "任务完成通知",
            format!(
                "恭喜！您认领的任务「{}」已完成，奖励 {} 魔力已发放到您的账户。",
                c.task_name, c.reward
            ),
        )
    };
    sqlx::query(
        "INSERT INTO messages (sender_id, receiver_id, subject, body) VALUES (NULL, $1, $2, $3)",
    )
    .bind(c.user_id)
    .bind(subject)
    .bind(body)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(true)
}

/// 失败/超时：status=2；配置了罚金则扣（幂等键 task_penalty:{claim_id}）
async fn settle_fail(db: &PgPool, c: &OpenClaim) -> anyhow::Result<()> {
    let mut tx = db.begin().await?;
    // 罚金说明文案按实扣额生成（见下方审计修复注释）；未配置罚金时为空串
    let mut penalty_note = String::new();
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
            let balance: i64 =
                sqlx::query_scalar("SELECT spark_balance FROM users WHERE id = $1 FOR UPDATE")
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
            // 审计修复（文案失实）：扣款额按实（take = min(余额, 罚金)）告知；
            // 旧文案固定写「扣除罚金 {penalty}」，余额不足被部分扣/零扣时与流水对不上。
            penalty_note = if take < c.penalty {
                format!(
                    "，扣除罚金 {} 魔力（余额不足，本次仅能扣到 0，未扣足 {} 魔力）",
                    take,
                    c.penalty - take
                )
            } else {
                format!("，扣除罚金 {} 魔力", take)
            };
        }
    }
    sqlx::query(
        "INSERT INTO messages (sender_id, receiver_id, subject, body) VALUES (NULL, $1, $2, $3)",
    )
    .bind(c.user_id)
    .bind("任务超时通知")
    .bind(format!(
        "您认领的任务已超出完成时限（{} 天），任务标记为失败{}。",
        c.duration_days, penalty_note
    ))
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

// ============ 考核引擎（0093，tasks 单表方案） ============

/// 自动派发考核（worker 分钟级调用，幂等：task_claims UNIQUE(task_id,user_id) 挡重复）。
/// 派发口径：
///   onboard  —— 注册 ≤ exam_onboard_days（默认 30）天、等级 ≥ target_class 的用户
///   periodic —— 等级 ≥ target_class 的全体用户（period 重派：上一个周期认领已结算才再插）
/// 派发即发 PM 通知（P1-4：静默派发用户不知情）。复用 claim 的基线快照列。
pub async fn exam_assign(db: &PgPool) -> anyhow::Result<u64> {
    let onboard_days: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value::bigint FROM site_settings WHERE name='exam_onboard_days'), 30)",
    )
    .fetch_one(db)
    .await
    .unwrap_or(30);

    let mut tx = db.begin().await?;
    let mut total = 0usize;

    // onboard：新人窗口内 + 未删号 + 未领过（含已结算——转正考核一次性）
    let onboard = sqlx::query(
        r#"
        INSERT INTO task_claims (task_id, user_id, base_uploaded, base_seed_seconds, base_uploads)
        SELECT t.id, u.id, u.uploaded,
               COALESCE((SELECT sum(s.seeded_seconds) FROM snatches s WHERE s.user_id = u.id), 0),
               (SELECT count(*) FROM torrents tr WHERE tr.owner_id = u.id AND tr.approval_status = 1)
        FROM tasks t
        JOIN users u ON u.class_id >= t.target_class
             AND u.created_at > now() - ($1 || ' days')::interval
        WHERE t.auto_assign AND t.kind = 'onboard'
          AND now() BETWEEN t.starts_at AND t.ends_at
          AND NOT EXISTS (SELECT 1 FROM task_claims tc WHERE tc.task_id = t.id AND tc.user_id = u.id)
        "#,
    )
    .bind(onboard_days.to_string())
    .execute(&mut *tx)
    .await?;
    total += onboard.rows_affected() as usize;

    // periodic：等级达标全体；period='once' 仅未领过；monthly/quarterly 上轮已结算可重派
    let periodic = sqlx::query(
        r#"
        INSERT INTO task_claims (task_id, user_id, base_uploaded, base_seed_seconds, base_uploads)
        SELECT t.id, u.id, u.uploaded,
               COALESCE((SELECT sum(s.seeded_seconds) FROM snatches s WHERE s.user_id = u.id), 0),
               (SELECT count(*) FROM torrents tr WHERE tr.owner_id = u.id AND tr.approval_status = 1)
        FROM tasks t
        JOIN users u ON u.class_id >= t.target_class
        WHERE t.auto_assign AND t.kind = 'periodic'
          AND now() BETWEEN t.starts_at AND t.ends_at
          AND NOT EXISTS (
                SELECT 1 FROM task_claims tc WHERE tc.task_id = t.id AND tc.user_id = u.id
                  AND (t.period = 'once' OR tc.status = 0))
        "#,
    )
    .execute(&mut *tx)
    .await?;
    total += periodic.rows_affected() as usize;

    // 派发 PM：本轮新插的考核认领（以 claim 无消息为界，避免重跑重复轰炸）
    sqlx::query(
        r#"
        INSERT INTO messages (sender_id, receiver_id, subject, body)
        SELECT NULL, c.user_id, '考核已派发',
               '您已被派发考核「' || t.name || '」，达成目标即可获得奖励，详情请在「考核&任务」页面查看。'
        FROM task_claims c
        JOIN tasks t ON t.id = c.task_id
        WHERE t.kind IN ('onboard', 'periodic')
          AND c.pm_sent = FALSE
        "#,
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE task_claims c SET pm_sent = TRUE \
                 FROM tasks t WHERE t.id = c.task_id AND t.kind IN ('onboard','periodic') AND c.pm_sent = FALSE")
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    if total > 0 {
        tracing::info!(total, "exam assigned");
    }
    Ok(total as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// seed_points 口径（P1-6 厘清）：1 积分 = 1 小时做种 = 3600 秒，
    /// 与 class_rules.min_seed_hours 同源。旧公式 v*3600/100（v×36 秒）无站内依据，已废弃。
    #[test]
    fn seed_points_one_point_per_hour() {
        let m: TaskMetric =
            serde_json::from_value(serde_json::json!({ "seed_points_delta": 10 })).unwrap();
        assert_eq!(m.seed_points_delta, Some(10));
        // v=10 积分 → 门槛 10×3600=36000 秒（10 小时）
        assert_eq!(m.seed_points_delta.unwrap().saturating_mul(3600), 36000);
    }
}
