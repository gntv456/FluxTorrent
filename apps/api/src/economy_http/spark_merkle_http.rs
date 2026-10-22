//! 每日火花流水结算凭证（Merkle inclusion proof）。
//!
//! `GET /me/spark/ledger/manifest?date=YYYY-MM-DD`：只回本人当天的叶子
//! 原文、叶哈希与兄弟路径；根只含聚合数字。会员可在浏览器里用
//! WebCrypto 从原文一路算到公开树根，不需要看别人的明细。
//!
//! `GET /system/ledger/roots`：公开的每日树根（匿名可读），供站外
//! 对账/留档。`anchoring=none`：不写链、不发币。
//!
//! 叶子由 worker 封存后不可重算；查询侧只读，绝不在这里补建。

use actix_web::{get, web, HttpRequest, Responder};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;
use flux_shared::merkle::{self, ALGORITHM, ANCHORING, TIMEZONE};

#[derive(Deserialize)]
pub(crate) struct ManifestQuery {
    /// 缺省 = 最近一个已封存日；格式非法回 400。
    #[serde(default)]
    date: Option<String>,
}

#[derive(sqlx::FromRow)]
struct StoredLeaf {
    idx: i64,
    ledger_id: i64,
    amount: i64,
    kind: String,
    created: String,
    leaf: String,
    preimage: String,
}

#[derive(sqlx::FromRow)]
struct DayRow {
    root: String,
    leaf_count: i64,
    user_count: i64,
    built_at: DateTime<Utc>,
}

fn parse_day(raw: &str) -> DomainResult<NaiveDate> {
    NaiveDate::parse_from_str(raw, "%Y-%m-%d")
        .map_err(|_| DomainError::Validation("日期格式应为 YYYY-MM-DD".into()))
}

/// 站点日（UTC+8）的昨天：缺省没指定日期时，今天还没封。
fn default_day() -> NaiveDate {
    (Utc::now() + Duration::hours(8)).date_naive() - Duration::days(1)
}

#[get("/me/spark/ledger/manifest")]
pub(crate) async fn my_ledger_manifest(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: crate::http::Q<ManifestQuery>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let day = match q.date.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => parse_day(s)?,
        None => {
            let latest: Option<NaiveDate> =
                sqlx::query_scalar("SELECT max(day) FROM spark_merkle_days")
                    .fetch_one(&state.repo.db)
                    .await
                    .map_err(|e| DomainError::Internal(e.into()))?;
            latest.unwrap_or_else(default_day)
        }
    };

    let Some(day_row) = sqlx::query_as::<_, DayRow>(
        "SELECT root, leaf_count, user_count, built_at \
         FROM spark_merkle_days WHERE day = $1",
    )
    .bind(day)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    else {
        return Ok(ok(serde_json::json!({
            "built": false,
            "date": day,
            "timezone": TIMEZONE,
            "algorithm": ALGORITHM,
            "anchoring": ANCHORING,
            "rows": [],
        })));
    };

    let all: Vec<String> = sqlx::query_scalar(
        "SELECT leaf FROM spark_merkle_leaves WHERE day = $1 ORDER BY idx",
    )
    .bind(day)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    let mine = sqlx::query_as::<_, StoredLeaf>(
        "SELECT idx, ledger_id, amount, kind, created, leaf, preimage \
         FROM spark_merkle_leaves \
         WHERE day = $1 AND user_id = $2 ORDER BY idx",
    )
    .bind(day)
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    let rows: Vec<serde_json::Value> = mine
        .iter()
        .map(|r| {
            let proof = usize::try_from(r.idx)
                .ok()
                .and_then(|i| merkle::proof(&all, i));
            serde_json::json!({
                "idx": r.idx,
                "ledger_id": r.ledger_id,
                "amount": r.amount,
                "kind": r.kind,
                "created_at": r.created,
                "leaf": r.leaf,
                "preimage": r.preimage,
                "proof": proof,
            })
        })
        .collect();

    Ok(ok(serde_json::json!({
        "built": true,
        "date": day,
        "timezone": TIMEZONE,
        "algorithm": ALGORITHM,
        "anchoring": ANCHORING,
        "root": day_row.root,
        "leaf_count": day_row.leaf_count,
        "user_count": day_row.user_count,
        "built_at": day_row.built_at,
        "included": !rows.is_empty(),
        "rows": rows,
    })))
}

#[derive(Deserialize)]
pub(crate) struct RootsQuery {
    #[serde(default)]
    limit: Option<i64>,
}

#[derive(sqlx::FromRow, serde::Serialize)]
pub(crate) struct RootRow {
    day: NaiveDate,
    root: String,
    leaf_count: i64,
    user_count: i64,
    built_at: DateTime<Utc>,
}

/// 公开树根：匿名可读，只暴露聚合数字，不含任何会员流水。
#[get("/system/ledger/roots")]
pub(crate) async fn ledger_roots(
    state: web::Data<std::sync::Arc<AppState>>,
    q: crate::http::Q<RootsQuery>,
) -> DomainResult<impl Responder> {
    let limit = q.limit.unwrap_or(30).clamp(1, 90);
    let items = sqlx::query_as::<_, RootRow>(
        "SELECT day, root, leaf_count, user_count, built_at \
         FROM spark_merkle_days ORDER BY day DESC LIMIT $1",
    )
    .bind(limit)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "items": items,
        "timezone": TIMEZONE,
        "algorithm": ALGORITHM,
        "anchoring": ANCHORING,
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bad_date_is_validation_not_500() {
        let e = parse_day("2026-13-40").unwrap_err();
        assert!(matches!(e, DomainError::Validation(_)));
        assert_eq!(parse_day("2026-10-20").unwrap().to_string(), "2026-10-20");
    }

    #[test]
    fn default_day_is_site_yesterday() {
        // 不能在测试里冻结时钟；只钉「站点日 - 1」这个关系。
        let site_today = (Utc::now() + Duration::hours(8)).date_naive();
        assert_eq!(default_day(), site_today - Duration::days(1));
    }
}
