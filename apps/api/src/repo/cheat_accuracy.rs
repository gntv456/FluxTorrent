//! 反作弊规则准确率（0372 M5）的 SQL 唯一居所（FRAME §2 下沉）：
//! 消费方 admin_http/cheat_accuracy.rs。口径：false_positive 只在申诉
//! overturned 时置位——accuracy 是下界估计，分母用已处置证据数。

use super::Repo;

/// 统计窗口天数（site_settings.cheat_accuracy_days；缺省/坏值 90）。
pub async fn accuracy_days(repo: &Repo) -> i64 {
    sqlx::query_scalar(
        "SELECT COALESCE((SELECT value::bigint FROM site_settings \
         WHERE name = 'cheat_accuracy_days'), 90)",
    )
    .fetch_one(&repo.db)
    .await
    .unwrap_or(90)
}

/// 家族级原始统计行（accuracy 派生在 H 层做，保持 repo 纯查询）。
#[derive(sqlx::FromRow)]
pub struct RawFamily {
    pub family: String,
    pub events: i64,
    pub resolved: i64,
    pub false_positive: i64,
    pub hits: i64,
}

/// 家族 = agent 的冒号前缀（`ghost:`→ghost；无前缀的自成一家族）。
/// group by 1 用别名在 PG 里可用（输出列名）。
pub async fn family_accuracy(
    repo: &Repo,
    days: i64,
) -> Result<Vec<RawFamily>, sqlx::Error> {
    sqlx::query_as::<_, RawFamily>(
        "SELECT \
           CASE WHEN position(':' in agent) > 0 \
                THEN split_part(agent, ':', 1) ELSE agent END AS family, \
           count(*)::bigint AS events, \
           count(*) FILTER (WHERE resolved_at IS NOT NULL)::bigint \
             AS resolved, \
           count(*) FILTER (WHERE false_positive IS TRUE)::bigint \
             AS false_positive, \
           COALESCE(sum(hits), 0)::bigint AS hits \
         FROM cheat_events \
         WHERE last_seen > now() - ($1::bigint * interval '1 day') \
         GROUP BY 1 ORDER BY false_positive DESC, events DESC",
    )
    .bind(days)
    .fetch_all(&repo.db)
    .await
}

/// 处置结论分布行（M3 状态机；sanction NULL = 仅结案未处置）。
#[derive(sqlx::FromRow, serde::Serialize)]
pub struct SanctionRow {
    pub kind: String,
    pub total: i64,
    pub applied: i64,
    pub reversed: i64,
    pub expired: i64,
    pub pending: i64,
}

pub async fn sanction_distribution(
    repo: &Repo,
    days: i64,
) -> Result<Vec<SanctionRow>, sqlx::Error> {
    sqlx::query_as::<_, SanctionRow>(
        "SELECT COALESCE(sanction->>'kind', 'none') AS kind, \
           count(*)::bigint AS total, \
           count(*) FILTER (WHERE sanction->>'state' = 'applied')::bigint \
             AS applied, \
           count(*) FILTER (WHERE sanction->>'state' = 'reversed')::bigint \
             AS reversed, \
           count(*) FILTER (WHERE sanction->>'state' = 'expired')::bigint \
             AS expired, \
           count(*) FILTER (WHERE sanction->>'state' = 'pending')::bigint \
             AS pending \
         FROM reports \
         WHERE ref_type = 'cheat' \
           AND created_at > now() - ($1::bigint * interval '1 day') \
         GROUP BY 1 ORDER BY total DESC",
    )
    .bind(days)
    .fetch_all(&repo.db)
    .await
}
