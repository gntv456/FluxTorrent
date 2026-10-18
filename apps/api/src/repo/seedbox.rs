//! seedbox 盒子的 SQL 唯一居所（FRAME §2 H 层不下沉 sqlx）。
//!
//! 消费方：`gaps_http/seedbox_claim.rs`（用户申报/自查）、
//! `admin_http/seedbox_review.rs`（员工审核面）、以及反作弊/H&R 读标志位
//! 的各处。判定与 CIDR 校验在 `domain::seedbox`（纯函数）。
//!
//! 【写入面收敛到本文件的意义】标志位（hnr_exempt / bonus_factor /
//! speed_relax）是反作弊的放宽依据，必须有唯一写入口——多处各写一套
//! UPDATE 会让「谁在什么条件下能拿到放宽」无法审计。

use super::Repo;
use crate::domain::seedbox::Flags;
use crate::errors::{DomainError, DomainResult};

/// 一批用户的盒子标志（列表页批量读，避免 N+1）。
pub type FlagsMap = std::collections::HashMap<i64, Flags>;

/// 申报单的一行。
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct Claim {
    pub id: i64,
    pub user_id: i64,
    pub cidrs: Vec<String>,
    pub status: String,
    pub reviewer_id: Option<i64>,
    pub review_note: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub reviewed_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// 提交申报（同一人可多次提交，保留历史供审计）。
pub async fn submit(
    repo: &Repo,
    user_id: i64,
    cidrs: &[String],
) -> DomainResult<i64> {
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO seedbox_claims (user_id, cidrs) \
         VALUES ($1, $2) RETURNING id",
    )
    .bind(user_id)
    .bind(cidrs)
    .fetch_one(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(id)
}

/// 我提交过的申报（最新在前）。
pub async fn my_claims(repo: &Repo, user_id: i64) -> DomainResult<Vec<Claim>> {
    Ok(sqlx::query_as(
        "SELECT id, user_id, cidrs, status, reviewer_id, review_note, \
                created_at, reviewed_at \
         FROM seedbox_claims WHERE user_id = $1 \
         ORDER BY created_at DESC LIMIT 20",
    )
    .bind(user_id)
    .fetch_all(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?)
}

/// 待审队列（先到先审）。
pub async fn pending_queue(
    repo: &Repo,
    limit: i64,
) -> DomainResult<Vec<Claim>> {
    Ok(sqlx::query_as(
        "SELECT id, user_id, cidrs, status, reviewer_id, review_note, \
                created_at, reviewed_at \
         FROM seedbox_claims WHERE status = 'pending' \
         ORDER BY created_at LIMIT $1",
    )
    .bind(limit)
    .fetch_all(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?)
}

/// 审核落定。`approve=true` 时同时写画像的 approved 位并按需给标志。
///
/// 标志三个值由调用方（员工）显式给出并经 domain::seedbox 校验——
/// 这里不做策略判断，只落「审核员看到的那个结果」。
pub async fn review(
    repo: &Repo,
    claim_id: i64,
    reviewer_id: i64,
    approve: bool,
    note: &str,
    hnr_exempt: bool,
    bonus_factor: f64,
    speed_relax: bool,
) -> DomainResult<Option<i64>> {
    let mut tx = repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let user_id: Option<i64> = sqlx::query_scalar(
        "UPDATE seedbox_claims \
         SET status = $2, reviewer_id = $3, review_note = $4, \
             reviewed_at = now() \
         WHERE id = $1 AND status = 'pending' RETURNING user_id",
    )
    .bind(claim_id)
    .bind(if approve { "approved" } else { "rejected" })
    .bind(reviewer_id)
    .bind(note)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(uid) = user_id else {
        // 已被他人审核（并发）或不存在 → 不做任何副作用
        tx.rollback().await.ok();
        return Ok(None);
    };
    sqlx::query(
        "INSERT INTO seedbox_profiles \
         (user_id, approved, hnr_exempt, bonus_factor, \
          anticheat_speed_relax, updated_at) \
         VALUES ($1, $2, $3, $4, $5, now()) \
         ON CONFLICT (user_id) DO UPDATE \
         SET approved = EXCLUDED.approved, \
             hnr_exempt = EXCLUDED.hnr_exempt, \
             bonus_factor = EXCLUDED.bonus_factor, \
             anticheat_speed_relax = EXCLUDED.anticheat_speed_relax, \
             updated_at = now()",
    )
    .bind(uid)
    .bind(approve)
    .bind(approve && hnr_exempt)
    .bind(bonus_factor)
    .bind(speed_relax)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(Some(uid))
}

/// 读生效标志位（反作弊 / H&R / 保种倍率的唯一读取口）。
pub async fn flags_of(repo: &Repo, user_id: i64) -> DomainResult<Flags> {
    let row: Option<(bool, bool, bool, f64, bool)> = sqlx::query_as(
        "SELECT approved, detected, hnr_exempt, bonus_factor::float8, \
                anticheat_speed_relax \
         FROM seedbox_profiles WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(match row {
        Some((ap, det, hnr, bf, relax)) => {
            Flags::from_profile(ap, det, hnr, bf, relax)
        }
        None => Flags::default(),
    })
}

/// 批量读标志（列表页用：一次取回多人的盒子态，避免 N+1）。
pub async fn flags_for(
    repo: &Repo,
    user_ids: &[i64],
) -> DomainResult<FlagsMap> {
    let mut out = std::collections::HashMap::new();
    if user_ids.is_empty() {
        return Ok(out);
    }
    let rows: Vec<(i64, bool, bool, bool, f64, bool)> = sqlx::query_as(
        "SELECT user_id, approved, detected, hnr_exempt, \
                bonus_factor::float8, anticheat_speed_relax \
         FROM seedbox_profiles \
         WHERE user_id = ANY($1)",
    )
    .bind(user_ids)
    .fetch_all(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    for (uid, ap, det, hnr, bf, relax) in rows {
        out.insert(uid, Flags::from_profile(ap, det, hnr, bf, relax));
    }
    Ok(out)
}

/// 记录一次 ASN 自动识别（detected）。
///
/// 只置 detected 与 asn 列表，**不动** hnr_exempt / bonus_factor——
/// 见 domain::seedbox 模块头的安全边界（强豁免须人工审核）。
pub async fn mark_detected(
    repo: &Repo,
    user_id: i64,
    asn: Option<i64>,
) -> DomainResult<()> {
    sqlx::query(
        "INSERT INTO seedbox_profiles \
         (user_id, detected, detected_asns, updated_at) \
         VALUES ($1, TRUE, $2::jsonb, now()) \
         ON CONFLICT (user_id) DO UPDATE \
         SET detected = TRUE, \
             detected_asns = seedbox_profiles.detected_asns || $2::jsonb, \
             updated_at = now()",
    )
    .bind(user_id)
    .bind(serde_json::json!(asn.map(|a| a).unwrap_or(0)))
    .execute(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(())
}

/// 撤回盒子身份（申诉成立 / 误判纠正）。
///
/// 保留申报历史（seedbox_claims 不动），只清画像——历史是可追溯的证据。
pub async fn revoke(repo: &Repo, user_id: i64) -> DomainResult<()> {
    sqlx::query(
        "UPDATE seedbox_profiles \
         SET approved = FALSE, detected = FALSE, hnr_exempt = FALSE, \
             bonus_factor = 1.0, anticheat_speed_relax = FALSE, \
             updated_at = now() \
         WHERE user_id = $1",
    )
    .bind(user_id)
    .execute(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(())
}
