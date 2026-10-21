//! 我的成就（GET /me/achievements）。
//! 从 v4_http.rs 按域拆出：补齐未达成行的实时指标值。
//!
//! `user_achievements.metric_value` 只在达标那一刻落库（worker 的
//! achievement_grant / 票根册的 grant 同口径）。只读此列时，所有未达成
//! 行都是 NULL ⇒ 成就墙的「进行中」恒 0、进度条全空。这里对未达成行
//! 现算实时值：五个 PT 指标与 worker 逐键同源，街机指标与票根册同源。

use actix_web::{get, web, HttpRequest};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[derive(sqlx::FromRow, serde::Serialize)]
struct AchievementRow {
    code: String,
    family: String,
    name: String,
    descr: String,
    threshold: i64,
    reward_sparks: i64,
    earned: bool,
    granted_at: Option<chrono::DateTime<chrono::Utc>>,
    /// 已达成为达成时的快照；未达成是实时值（进度展示用）
    metric_value: Option<i64>,
}

#[get("/me/achievements")]
pub(crate) async fn my_achievements(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl actix_web::Responder> {
    let auth = require_auth(&req, &state).await?;
    let rows = sqlx::query_as::<_, AchievementRow>(SQL)
        .bind(auth.id)
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

/// 五个 PT 指标与 `worker/src/jobs/achv_dormant.rs` 一致；街机的
/// ref_type 口径与 `games_http/arcade_stubs.rs` 一致（票根数按已得算）。
const SQL: &str = r#"
WITH me AS (
    SELECT COALESCE(u.seeding_size, 0)::bigint AS seeding_bytes,
           (SELECT count(*) FROM resurrections r
             WHERE r.user_id = u.id AND r.status = 'done') AS rescue_count,
           (SELECT count(*) FROM torrents t
             WHERE t.owner_id = u.id AND t.approval_status = 1) AS upload_count,
           (SELECT count(*) FROM posts p
             WHERE p.user_id = u.id) AS post_count,
           (SELECT count(*) FROM subtitles sb
             WHERE sb.user_id = u.id AND sb.deleted_at IS NULL
               AND sb.status = 1 AND NOT sb.anon) AS subtitle_count
    FROM users u WHERE u.id = $1
), gl AS (
    SELECT count(*)::bigint AS game_plays,
           count(*) FILTER (WHERE ref_type = 'scratch')::bigint
             AS scratch_plays,
           count(*) FILTER (WHERE ref_type = 'jgg')::bigint AS jgg_plays,
           count(*) FILTER (WHERE ref_type = 'bigsmall')::bigint
             AS bs_plays,
           count(*) FILTER (WHERE ref_type = 'farm_water')::bigint
             AS farm_water,
           count(*) FILTER (WHERE ref_type = 'farm_plant')::bigint
             AS farm_plant,
           count(*) FILTER (WHERE ref_type = 'capsule')::bigint
             AS capsule_plays,
           count(*) FILTER (WHERE ref_type = 'wheel')::bigint
             AS wheel_plays,
           count(*) FILTER (WHERE ref_type = 'fishing')::bigint
             AS fishing_plays,
           count(*) FILTER (WHERE ref_type = 'pet_feed')::bigint
             AS pet_feeds
    FROM spark_ledger
    WHERE user_id = $1 AND kind = 'game' AND amount < 0
), arc AS (
    SELECT count(*)::bigint AS owned FROM user_achievements ua
    JOIN achievement_defs d2 ON d2.id = ua.def_id
    WHERE ua.user_id = $1 AND d2.family = 'arcade'
)
SELECT d.code, d.family, d.name, d.descr, d.threshold, d.reward_sparks,
       (ua.user_id IS NOT NULL) AS earned, ua.granted_at,
       COALESCE(ua.metric_value, CASE d.metric
         WHEN 'seeding_bytes' THEN me.seeding_bytes
         WHEN 'rescue_count' THEN me.rescue_count
         WHEN 'upload_count' THEN me.upload_count
         WHEN 'post_count' THEN me.post_count
         WHEN 'subtitle_count' THEN me.subtitle_count
         WHEN 'game_plays' THEN gl.game_plays
         WHEN 'scratch_plays' THEN gl.scratch_plays
         WHEN 'jgg_plays' THEN gl.jgg_plays
         WHEN 'bs_plays' THEN gl.bs_plays
         WHEN 'farm_water' THEN gl.farm_water
         WHEN 'farm_plant' THEN gl.farm_plant
         WHEN 'capsule_plays' THEN gl.capsule_plays
         WHEN 'wheel_plays' THEN gl.wheel_plays
         WHEN 'fishing_plays' THEN gl.fishing_plays
         WHEN 'pet_feeds' THEN gl.pet_feeds
         WHEN 'stub_count' THEN (SELECT owned FROM arc)
       END) AS metric_value
FROM achievement_defs d CROSS JOIN me CROSS JOIN gl
LEFT JOIN user_achievements ua
  ON ua.def_id = d.id AND ua.user_id = $1
ORDER BY d.family, d.position
"#;
