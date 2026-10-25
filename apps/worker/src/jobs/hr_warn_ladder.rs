//! H&R 警告阶梯的落地（0388）。
//!
//! [`api::hr_warn`] 是判定矩阵的真身（api/src/domain/hr_warn），本模块用
//! `#[path]` 复用同一份源文件——两 crate 各写一份迟早漂移，而漂移的后果是
//! 「api 算出顺延 30 天、worker 按 14 天解禁」。
//!
//! 【与既有 hr_punish 的关系】`hr.rs::hr_punish` 的「未解决违规数 ≥ 阈值
//! → 禁下」**不改**，本模块在它之前加一层更轻的阶梯：
//!   * 第 1 档：只警告（不动任何权限）；
//!   * 第 2 档起：按阶梯限下 / 禁传 / 禁邀；
//!   * 到期（expires_at）自动恢复。
//! 理由：原来「三次就禁下」对轻度违规过重，且**没有任何恢复承诺**——
//! 本模块给出的 expires_at 才是「什么时候能恢复」这个答案。
//!
//! 【幂等】本模块每轮重算全量：upsert 按 user_id 覆盖，命中「到期且无未解决
//! 违规」的行才清零。所以重复跑同一轮不会叠加惩罚，也不会把已恢复的人再禁一次。

use chrono::{DateTime, Utc};
use sqlx::PgPool;

use super::hr_warn_mail::warn_mail;

// 判定矩阵的真身在 api（api/src/domain/hr_warn），worker 用 #[path] 复用。
#[path = "../../../api/src/domain/hr_warn/mod.rs"]
mod api_domain;

/// 站点参数。
struct WarnCfg {
    enabled: bool,
    base_days: i32,
    max_shift_days: i32,
}

async fn warn_cfg(db: &PgPool) -> WarnCfg {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT name, value FROM site_settings \
             WHERE name IN ('hr_warn_enable', 'hr_warn_base_days', \
                            'hr_warn_max_shift_days')",
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();
    let get =
        |k: &str| rows.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());
    WarnCfg {
        enabled: get("hr_warn_enable")
            .map(|v| v != "0" && v != "no")
            .unwrap_or(true),
        base_days: get("hr_warn_base_days")
            .and_then(|v| v.parse().ok())
            .unwrap_or(7)
            .max(1),
        max_shift_days: get("hr_warn_max_shift_days")
            .and_then(|v| v.parse().ok())
            .unwrap_or(365)
            .max(0),
    }
}

/// 一条待判定的用户警告输入。
struct Candidate {
    user_id: i64,
    open_warnings: i32,
    earliest_expiry: Option<DateTime<Utc>>,
}

/// 取所有「有未解决违规」的用户作为判定对象。
///
/// 口径与 hr_punish 一致（hr_violations.resolved_at IS NULL），但这里
/// 取的是**条数**用于选档，另外取该用户现存警告里最早的一条到期时刻
/// 作为 Gazelle 顺延的起点。
///
/// `open_warnings` 是「本次之前」的条数（domain `WarnInput` 的口径）：
/// 本次违规已经落表，直接拿 count(*) 会把它算进档位，于是
/// `step_for(1)` 成为首次命中——第 1 档「只警告」永远走不到，
/// 一次小违规就直接限下，与 0388 的「第 2 档起才限下」相反。
async fn candidates(db: &PgPool) -> anyhow::Result<Vec<Candidate>> {
    let rows: Vec<(i64, i32, Option<DateTime<Utc>>)> = sqlx::query_as(
        "SELECT v.user_id, count(*)::int, min(w.expires_at) \
         FROM hr_violations v \
         LEFT JOIN hr_warn_states w \
           ON w.user_id = v.user_id AND w.step > 0 \
         WHERE v.resolved_at IS NULL \
         GROUP BY v.user_id",
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(user_id, open_warnings, earliest_expiry)| Candidate {
            user_id,
            open_warnings: (open_warnings - 1).max(0),
            earliest_expiry,
        })
        .collect())
}

/// 主入口：跑一轮警告阶梯。
pub(crate) async fn run(db: &PgPool) -> anyhow::Result<u64> {
    let cfg = warn_cfg(db).await;
    if !cfg.enabled {
        tracing::debug!("H&R 警告阶梯已由站点设定关闭");
        return Ok(0);
    }
    let now = Utc::now();
    let cands = candidates(db).await?;
    let mut changed = 0u64;

    for c in &cands {
        let (idx, step) = api_domain::step_for(c.open_warnings);
        let lvl = (idx + 1).min(4) as i32;

        // 同档不重算。`decide` 的顺延起点是警告自身的 expires_at，每轮
        // 重算就把到期时刻再加一档：现网 41 行因此被推到 2027 年以后，
        // 同一批违规每分钟写一行 history（3.4 万行）。只有首次警告或
        // 档位上升（真来了新违规）才重新发警告。
        let prior_step: Option<i32> = sqlx::query_scalar(
            "SELECT step FROM hr_warn_states WHERE user_id = $1",
        )
        .bind(c.user_id)
        .fetch_optional(db)
        .await
        .unwrap_or(None);
        if lvl <= prior_step.unwrap_or(0) {
            continue;
        }

        let d = api_domain::decide(&api_domain::WarnInput {
            open_warnings: c.open_warnings,
            earliest_expiry: c.earliest_expiry,
            base_days: cfg.base_days,
            max_shift_days: cfg.max_shift_days,
            now,
        });
        let note = format!(
            "open={} shift={}d{}",
            c.open_warnings,
            d.shifted_days,
            if d.hit_cap { " cap" } else { "" }
        );
        let subject = format!("H&R 警告（第 {lvl} 档）");
        let body = warn_mail(
            c.open_warnings,
            lvl,
            step.restrict_download,
            step.block_upload,
            step.block_invite,
            d.expires_at,
            d.hit_cap,
        );
        record_step(
            db,
            c.user_id,
            lvl,
            cfg.base_days,
            &d,
            step,
            &note,
            &subject,
            &body,
        )
        .await?;
        changed += 1;
        // 只在本档要求限制下载时才真的动 download_enabled；
        // 第 1 档（只警告）不动任何权限。
        if step.restrict_download {
            sqlx::query(
                "UPDATE users SET download_enabled = FALSE, \
                     download_locked_by = 'hr' \
                 WHERE id = $1 AND status < 2 AND download_enabled",
            )
            .bind(c.user_id)
            .execute(db)
            .await?;
        }
    }

    // 到期恢复：判据是 domain 的 should_auto_restore（全部到期才放人）。
    let restored = restore_due(db, now).await?;
    if changed > 0 || restored > 0 {
        tracing::info!(changed, restored, "H&R 警告阶梯推进");
    }
    Ok(changed)
}

/// 到期且已无未解决违规 → 清零警告并解锁下载。
///
/// 「已无未解决违规」是恢复的**必要条件**：只看到期就放人，等于给
/// 「警告期到了但违规还挂着」的人提前解封。
async fn restore_due(db: &PgPool, now: DateTime<Utc>) -> anyhow::Result<u64> {
    let due: Vec<(i64,)> = sqlx::query_as(
        "SELECT w.user_id FROM hr_warn_states w \
         WHERE w.step > 0 AND w.expires_at <= $1 \
           AND NOT EXISTS (SELECT 1 FROM hr_violations v \
                            WHERE v.user_id = w.user_id \
                              AND v.resolved_at IS NULL)",
    )
    .bind(now)
    .fetch_all(db)
    .await?;
    if due.is_empty() {
        return Ok(0);
    }
    let ids: Vec<i64> = due.iter().map(|(u,)| *u).collect();
    sqlx::query("UPDATE hr_warn_states SET step = 0, expires_at = NULL, \
                 shifted_days = 0, hit_cap = false, restrict_download = false, \
                 block_upload = false, block_invite = false, updated_at = now() \
                 WHERE user_id = ANY($1)")
        .bind(&ids)
        .execute(db)
        .await?;
    // 只解 H&R 自己锁的：别把 Ratio Watch 等其它原因锁住的人一并放开。
    sqlx::query(
        "UPDATE users SET download_enabled = TRUE, \
                 download_locked_by = NULL \
                 WHERE id = ANY($1) AND download_locked_by = 'hr'",
    )
    .bind(&ids)
    .execute(db)
    .await?;
    Ok(ids.len() as u64)
}

/// 状态、历史、通知一条语句落库。
///
/// 三者必须同生共死：只写状态不发信，下一轮就因档位相同被跳过，
/// 这封信永远不会补发。
#[allow(clippy::too_many_arguments)]
async fn record_step(
    db: &PgPool,
    user_id: i64,
    lvl: i32,
    base_days: i32,
    d: &api_domain::WarnDecision,
    step: api_domain::WarnStep,
    note: &str,
    subject: &str,
    body: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        WITH st AS (
            INSERT INTO hr_warn_states
                (user_id, step, base_days, expires_at, shifted_days, hit_cap,
                 restrict_download, block_upload, block_invite, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, now())
            ON CONFLICT (user_id) DO UPDATE SET
                step = EXCLUDED.step, base_days = EXCLUDED.base_days,
                expires_at = EXCLUDED.expires_at,
                shifted_days = EXCLUDED.shifted_days,
                hit_cap = EXCLUDED.hit_cap,
                restrict_download = EXCLUDED.restrict_download,
                block_upload = EXCLUDED.block_upload,
                block_invite = EXCLUDED.block_invite,
                updated_at = now()
            RETURNING user_id
        ), hi AS (
            INSERT INTO hr_warn_history
                (user_id, step, base_days, expires_at, shifted_days, hit_cap, note)
            SELECT $1, $2, $3, $4, $5, $6, $10 FROM st
        )
        INSERT INTO messages (sender_id, receiver_id, subject, body, kind)
        SELECT NULL, $1, $11, $12, 'hr_violation'
        FROM st
        WHERE u_notice_enabled($1, 'hr_violation')
        "#,
    )
    .bind(user_id)
    .bind(lvl)
    .bind(base_days)
    .bind(d.expires_at)
    .bind(d.shifted_days)
    .bind(d.hit_cap)
    .bind(step.restrict_download)
    .bind(step.block_upload)
    .bind(step.block_invite)
    .bind(note)
    .bind(subject)
    .bind(body)
    .execute(db)
    .await?;
    Ok(())
}
