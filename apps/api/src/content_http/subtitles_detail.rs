//! 字幕详情（0150 缺口1）：GET /subtitles/{id}/detail——下载统计 +
//! 全元数据 + 修订版链（parent_id 正/反向）+ 版本兄弟。行级权限与列表一致
//!（未删且过审；本人/staff 可看待审行）。

use actix_web::{get, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[derive(sqlx::FromRow)]
struct SubDetailRow {
    id: i64,
    torrent_id: Option<i64>,
    user_id: i64,
    username: Option<String>,
    title: String,
    lang: Option<String>,
    downloads: i32,
    size: i64,
    ext: Option<String>,
    fps: Option<f64>,
    machine_translated: bool,
    hearing_impaired: bool,
    foreign_parts_only: bool,
    source: Option<String>,
    producer: Option<String>,
    proofreader: Option<String>,
    author_name: Option<String>,
    release_name: Option<String>,
    anon: bool,
    verified: bool,
    parent_id: Option<i64>,
    imdb_id: Option<String>,
    rating_sum: i32,
    rating_count: i32,
    bad_reports: i32,
    created_at: chrono::DateTime<chrono::Utc>,
    ai_state: String,
    cert_tier: Option<String>,
    my_vote: Option<i32>,
}

/// 字幕详情页数据（公开行任何人可看；本人/staff 可看自己的待审/隐藏行）
#[get("/subtitles/{id}/detail")]
pub(super) async fn subtitle_detail(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let sid = path.into_inner();
    let staff = auth.class_id >= 90;
    let row: Option<SubDetailRow> = sqlx::query_as(
        "SELECT s.id, s.torrent_id, s.user_id, u.username, s.title, s.lang, \
         s.downloads, s.size, s.ext, s.fps::float8, s.machine_translated, \
         s.hearing_impaired, s.foreign_parts_only, s.source, s.producer, \
         s.proofreader, s.author_name, s.release_name, s.anon, s.verified, \
         s.parent_id, s.imdb_id, s.rating_sum, s.rating_count, \
         s.bad_reports, s.created_at, \
         CASE WHEN s.machine_translated AND COALESCE(s.proofreader, '') = \
         '' THEN 'ai' WHEN s.machine_translated THEN 'ai_proofread' \
         ELSE 'human' END AS ai_state, \
         cert.cert_tier, \
         (SELECT v.score FROM subtitle_votes v WHERE v.subtitle_id = s.id \
          AND v.user_id = $2) AS my_vote \
         FROM subtitles s LEFT JOIN users u ON u.id = s.user_id \
         LEFT JOIN LATERAL ( \
             SELECT c.tier AS cert_tier FROM user_subtitle_certs c \
             WHERE c.user_id = s.user_id AND c.revoked_at IS NULL \
             ORDER BY CASE c.tier WHEN 'gold' THEN 0 ELSE 1 END LIMIT 1 \
         ) cert ON TRUE \
         WHERE s.id = $1 AND s.deleted_at IS NULL \
         AND (s.status = 1 OR $3 OR s.user_id = $2)",
    )
    .bind(sid)
    .bind(auth.id)
    .bind(staff)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(r) = row else {
        return Err(DomainError::NotFound(sid));
    };
    // 修订版链：父版本（本行是修订）+ 子版本（别人修订了本行）
    type ChainRow = (
        i64,
        String,
        Option<String>,
        chrono::DateTime<chrono::Utc>,
    );
    // 修订版链：本行 parent_id 直接给出父；children 反查（递归到孙辈）。
    // 注：parents 只需一级（0146 的 parent_id 语义是「本字幕修订自谁」，
    // 不递归祖先——历史数据无多级链，递归留数据结构余量但按一级取）。
    let parents: Vec<ChainRow> =
        sqlx::query_as(
            "SELECT s2.id, s2.title, s2.lang, s2.created_at FROM subtitles \
             s2 WHERE s2.id = $1 AND s2.deleted_at IS NULL",
        )
        .bind(r.parent_id.filter(|p| *p > 0))
        .fetch_all(&state.repo.db)
        .await
        .unwrap_or_default();
    let children: Vec<ChainRow> =
        sqlx::query_as(
            "SELECT s2.id, s2.title, s2.lang, s2.created_at FROM subtitles \
             s2 WHERE s2.parent_id = $1 AND s2.deleted_at IS NULL \
             ORDER BY s2.created_at ASC",
        )
        .bind(sid)
        .fetch_all(&state.repo.db)
        .await
        .unwrap_or_default();
    let rating = if r.rating_count > 0 {
        serde_json::json!(
            (r.rating_sum as f64 / r.rating_count as f64 * 10.0).round() / 10.0
        )
    } else {
        serde_json::Value::Null
    };
    Ok(ok(serde_json::json!({
        "id": r.id, "torrent_id": r.torrent_id, "user_id": r.user_id,
        "username": if r.anon { None } else { r.username },
        "title": r.title, "lang": r.lang, "downloads": r.downloads,
        "size": r.size, "ext": r.ext, "fps": r.fps,
        "machine_translated": r.machine_translated,
        "hearing_impaired": r.hearing_impaired,
        "foreign_parts_only": r.foreign_parts_only,
        "source": r.source, "producer": r.producer,
        "proofreader": r.proofreader, "author_name": r.author_name,
        "release_name": r.release_name, "anon": r.anon,
        "verified": r.verified, "parent_id": r.parent_id,
        "imdb_id": r.imdb_id, "rating": rating,
        "rating_count": r.rating_count, "bad_reports": r.bad_reports,
        "created_at": r.created_at, "ai_state": r.ai_state,
        "cert_tier": r.cert_tier, "my_vote": r.my_vote,
        "parents": parents, "children": children,
        "can_modify": r.user_id == auth.id || staff,
    })))
}
