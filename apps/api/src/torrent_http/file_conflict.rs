//! 文件树冲突检测（G13，Gazelle file conflict 口径）。
//!
//! 同一聚合组（= 同一资源的多个版本）内，两个过审种不该出现**相同的文件路径**
//! ——那意味着重复打包（Gazelle 把「劣质即重复」作为 trumping 的判据之一）。
//! 本端点只做**读侧检测**：给定一颗种，列出它与同组其他过审种重合的路径与
//! 对应的另一颗种，供审核台/发布者判断是否重复。
//!
//! 不阻断发种：口径上「重复」要人来裁决（谁更优），自动拒绝会误杀
//! 「同片不同版」的正当并列。

use actix_web::{get, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 该种与同组其他过审种的文件路径冲突清单。
#[get("/torrents/{id}/file-conflicts")]
pub async fn file_conflicts(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let _auth = require_auth(&req, &state).await?;
    let id = path.into_inner();
    // torrents.group_id 可空：fetch_optional 得 Option<Option<i64>>
    let gid: Option<Option<i64>> =
        sqlx::query_scalar("SELECT group_id FROM torrents WHERE id = $1")
            .bind(id)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(gid) = gid else {
        return Err(DomainError::NotFound(id));
    };
    let Some(gid) = gid else {
        // 未入组 ⇒ 无同组可比
        return Ok(ok(serde_json::json!({
            "group_id": null, "total": 0, "conflicts": [],
        })));
    };
    let rows: Vec<(String, i64, String)> = sqlx::query_as(
        "SELECT f.path, t2.id, t2.name FROM files f \
         JOIN torrents t2 ON t2.id = f.torrent_id \
         WHERE t2.group_id = $1 AND t2.approval_status = 1 \
           AND t2.id <> $2 \
           AND f.path IN (SELECT path FROM files WHERE torrent_id = $2) \
         ORDER BY f.path LIMIT 200",
    )
    .bind(gid)
    .bind(id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let total = rows.len();
    let conflicts: Vec<_> = rows
        .into_iter()
        .map(|(p, tid, tname)| {
            serde_json::json!({
                "path": p, "other_id": tid, "other_name": tname,
            })
        })
        .collect();
    Ok(ok(serde_json::json!({
        "group_id": gid, "total": total, "conflicts": conflicts,
    })))
}
