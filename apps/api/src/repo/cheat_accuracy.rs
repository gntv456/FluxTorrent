//! 反作弊读口的 SQL 居所（FRAME §2 下沉）：
//!   · 规则准确率（0372 M5）：消费方 admin_http/cheat_accuracy.rs。
//!     口径：false_positive 只在申诉 overturned 时置位——accuracy 是
//!     下界估计，分母用已处置证据数。
//!   · 会员本人风险（账户健康页）：消费方 gaps_http/my_risk.rs。
//!     只读**本人**的分数、存活证据与进行中案件；peer_ip 一律不选。

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

// ---- 会员本人风险（gaps_http/my_risk.rs 消费） ---------------------------

/// 本人分数行；未算过（干净号）时 None，由调用方回落 0/none。
#[derive(sqlx::FromRow, serde::Serialize)]
pub struct MemberRisk {
    pub score: i32,
    pub band: String,
    pub ruleset_ver: i32,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

pub async fn member_risk(
    repo: &Repo,
    uid: i64,
) -> Result<Option<MemberRisk>, sqlx::Error> {
    sqlx::query_as::<_, MemberRisk>(
        "SELECT score, band, ruleset_ver, updated_at \
         FROM user_risk_scores WHERE user_id = $1",
    )
    .bind(uid)
    .fetch_optional(&repo.db)
    .await
}

/// 存活证据（resolved_at IS NULL）。`reason` 是给用户看的说明，
/// `peer_ip` 属他人可关联信息，不选。
#[derive(sqlx::FromRow)]
pub struct MemberEvidence {
    pub agent: String,
    pub reason: String,
    pub hits: i64,
    pub first_seen: chrono::DateTime<chrono::Utc>,
    pub last_seen: chrono::DateTime<chrono::Utc>,
}

pub async fn member_evidence(
    repo: &Repo,
    uid: i64,
) -> Result<Vec<MemberEvidence>, sqlx::Error> {
    sqlx::query_as::<_, MemberEvidence>(
        "SELECT agent, reason, hits, first_seen, last_seen \
         FROM cheat_events WHERE user_id = $1 AND resolved_at IS NULL \
         ORDER BY hits DESC, last_seen DESC LIMIT 200",
    )
    .bind(uid)
    .fetch_all(&repo.db)
    .await
}

/// 进行中案件（0=open / 2=claimed）。`risk_score` 是开案时固化的快照。
#[derive(sqlx::FromRow, serde::Serialize)]
pub struct MemberCase {
    pub id: i64,
    pub status: i16,
    pub reason: String,
    pub priority: i16,
    pub risk_score: Option<i32>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn member_open_case(
    repo: &Repo,
    uid: i64,
) -> Result<Option<MemberCase>, sqlx::Error> {
    sqlx::query_as::<_, MemberCase>(
        "SELECT id, status, reason, priority, risk_score, created_at \
         FROM reports \
         WHERE ref_type = 'cheat' AND ref_id = $1 AND status IN (0, 2) \
         ORDER BY id DESC LIMIT 1",
    )
    .bind(uid)
    .fetch_optional(&repo.db)
    .await
}

/// 最近一条申诉摘要（不含正文）：账户健康页只展示状态与结论。
#[derive(sqlx::FromRow, serde::Serialize)]
pub struct AppealSummary {
    pub id: i64,
    pub case_id: Option<i64>,
    pub status: String,
    pub outcome: Option<String>,
    pub result_note: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn latest_appeal(
    repo: &Repo,
    uid: i64,
) -> Result<Option<AppealSummary>, sqlx::Error> {
    sqlx::query_as::<_, AppealSummary>(
        "SELECT id, case_id, status, outcome, result_note, created_at \
         FROM appeals WHERE user_id = $1 AND kind = 'cheat' \
         ORDER BY id DESC LIMIT 1",
    )
    .bind(uid)
    .fetch_optional(&repo.db)
    .await
}

/// (未结申诉总数, 该案是否已有未结申诉)；case_id 为 None 时后者必 false。
/// 与 `cheat_appeals::MAX_OPEN` 同口径，供账户健康页决定是否放出表单。
pub async fn open_flags(
    repo: &Repo,
    uid: i64,
    case_id: Option<i64>,
) -> Result<(i64, bool), sqlx::Error> {
    sqlx::query_as(
        "SELECT count(*) FILTER (WHERE status = 'open')::bigint, \
                EXISTS(SELECT 1 FROM appeals p \
                       WHERE p.user_id = $1 AND p.kind = 'cheat' \
                         AND p.status = 'open' AND p.case_id = $2) \
         FROM appeals WHERE user_id = $1 AND kind = 'cheat'",
    )
    .bind(uid)
    .bind(case_id)
    .fetch_one(&repo.db)
    .await
}
