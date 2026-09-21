//! 金字幕评选（0148 C5）：公开榜 + 月度统计核心。
//! 候选/授金的管理端点在 admin 侧（staff_http）；worker 周期调 build_candidates。

use actix_web::{get, web};
use sqlx::PgPool;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

#[derive(sqlx::FromRow, serde::Serialize)]
struct AwardRow {
    id: i64,
    period: String,
    subtitle_id: i64,
    user_id: i64,
    username: Option<String>,
    title: String,
    lang: Option<String>,
    machine_translated: bool,
    score: f64,
    rank: i16,
    tier: String,
    granted_at: chrono::DateTime<chrono::Utc>,
}

/// 评选榜（公开只读；period 缺省最新一期；rank>0 = 获奖，0 = 入围）
#[get("/subtitles/awards")]
pub(super) async fn subtitle_awards_board(
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<std::collections::HashMap<String, String>>,
) -> DomainResult<impl actix_web::Responder> {
    let period = q.get("period").cloned();
    let rows: Vec<AwardRow> = sqlx::query_as(
        "SELECT a.id, a.period, a.subtitle_id, a.user_id, u.username, \
         s.title, s.lang, s.machine_translated, a.score::float8, a.rank, \
         a.tier, a.granted_at FROM subtitle_awards a JOIN subtitles s ON \
         s.id = a.subtitle_id LEFT JOIN users u ON u.id = a.user_id WHERE \
         ($1::text IS NULL OR a.period = $1) ORDER BY a.period DESC, \
         a.tier, a.rank, a.score DESC",
    )
    .bind(period.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

/// 月度候选计算（C5-1，worker 与管理端共用）：
/// 得分 = 评分均分 × ln(1+评分人数) + min(下载,500)×0.05 + verified+3。
/// 双赛道：human（machine_translated=false）与 ai（true 且无人工校对标记）。
/// AI+人工校对（machine_translated + proofreader 非空）归 human 赛道（C6 口径）。
/// 每赛道取 Top 10 入候选（rank=0）；同字幕同期间幂等（UNIQUE 覆盖）。
pub(crate) async fn build_candidates(
    db: &PgPool,
    period: &str,
) -> anyhow::Result<usize> {
    let ai_track_on: bool = sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = \
         'subtitle_award_ai_track'",
    )
    .fetch_optional(db)
    .await?
    .map(|v| v != "0")
    .unwrap_or(true);
    let mut n = 0usize;
    for tier in ["human", "ai"] {
        if tier == "ai" && !ai_track_on {
            continue;
        }
        let rows = sqlx::query(
            r#"
            INSERT INTO subtitle_awards
                (period, subtitle_id, user_id, score, rank, tier)
            SELECT $1, s.id, s.user_id,
                   (s.rating_sum::float8 / NULLIF(s.rating_count, 0)
                    * ln(1 + s.rating_count)
                    + least(s.downloads, 500) * 0.05
                    + CASE WHEN s.verified THEN 3 ELSE 0 END)::numeric(10,2),
                   0, $2
            FROM subtitles s
            WHERE s.deleted_at IS NULL AND s.status = 1
              AND s.rating_count > 0
              AND s.created_at >= ($1 || '-01')::date
              AND s.created_at <  ($1 || '-01')::date + interval '1 month'
              AND ($3::text = 'human'
                   AND (NOT s.machine_translated
                        OR COALESCE(s.proofreader, '') <> '')
                   OR $3::text = 'ai'
                   AND s.machine_translated
                   AND COALESCE(s.proofreader, '') = '')
            ORDER BY 3 DESC LIMIT 10
            ON CONFLICT (period, subtitle_id) DO NOTHING
            "#,
        )
        .bind(period)
        .bind(tier)
        .bind(tier)
        .execute(db)
        .await?;
        n += rows.rows_affected() as usize;
    }
    Ok(n)
}
