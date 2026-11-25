//! 固定抽签·候选集查询（worker 镜像）。与 api/repo/fixed_lottery 同口径。
//! 提供普通池与开奖事务内两个版本，后者必须在 CAS 持锁之后调用。

use sqlx::{PgPool, Postgres, Transaction};

fn eligibility_parts(
    elig: &serde_json::Value,
) -> (Option<i64>, Option<i64>, Option<i64>, Vec<i64>) {
    let min = elig.get("min_class").and_then(|v| v.as_i64());
    let max = elig.get("max_class").and_then(|v| v.as_i64());
    let reg = elig.get("registered_days_min").and_then(|v| v.as_i64());
    let excl: Vec<i64> = elig
        .get("exclude_uids")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_i64()).collect())
        .unwrap_or_default();
    (min, max, reg, excl)
}

pub(crate) async fn eligible_uids(
    db: &PgPool,
    cid: i64,
) -> anyhow::Result<Vec<i64>> {
    let elig: serde_json::Value = sqlx::query_scalar(
        "SELECT eligibility FROM fixed_lottery_campaigns WHERE id = $1",
    )
    .bind(cid)
    .fetch_optional(db)
    .await?
    .flatten()
    .unwrap_or(serde_json::Value::Null);
    let (min, max, reg, excl) = eligibility_parts(&elig);
    Ok(sqlx::query_scalar(
        "SELECT p.user_id FROM fixed_lottery_participants p \
         JOIN users u ON u.id = p.user_id \
         WHERE p.campaign_id = $1 AND u.status = 0 \
           AND ($2::bigint IS NULL OR u.class_id >= $2::bigint) \
           AND ($3::bigint IS NULL OR u.class_id <= $3::bigint) \
           AND ($4::bigint IS NULL \
                OR now() - u.created_at >= make_interval(days => $4::int)) \
           AND NOT (u.id = ANY($5::bigint[]))",
    )
    .bind(cid)
    .bind(min)
    .bind(max)
    .bind(reg)
    .bind(&excl)
    .fetch_all(db)
    .await?)
}

/// CAS 之后在同一事务内冻结候选集：报名口对活动行持 FOR SHARE，
/// 与这里 CAS 的 FOR UPDATE 互斥，故不会再有报名挤进快照之后。
pub(crate) async fn eligible_uids_tx(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
) -> anyhow::Result<Vec<i64>> {
    let elig: serde_json::Value = sqlx::query_scalar(
        "SELECT eligibility FROM fixed_lottery_campaigns WHERE id = $1",
    )
    .bind(cid)
    .fetch_one(&mut **tx)
    .await?;
    let (min, max, reg, excl) = eligibility_parts(&elig);
    Ok(sqlx::query_scalar(
        "SELECT p.user_id FROM fixed_lottery_participants p \
         JOIN users u ON u.id = p.user_id \
         WHERE p.campaign_id = $1 AND u.status = 0 \
           AND ($2::bigint IS NULL OR u.class_id >= $2::bigint) \
           AND ($3::bigint IS NULL OR u.class_id <= $3::bigint) \
           AND ($4::bigint IS NULL \
                OR now() - u.created_at >= make_interval(days => $4::int)) \
           AND NOT (u.id = ANY($5::bigint[]))",
    )
    .bind(cid)
    .bind(min)
    .bind(max)
    .bind(reg)
    .bind(&excl)
    .fetch_all(&mut **tx)
    .await?)
}
