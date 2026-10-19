//! 保种区认领（`POST /preserve/claim`）。从 `preserve.rs` 拆出——那边是
//! 列表/移出宽限/插件/复活的合集，认领这一支的口径自成一段，拆开后两边都
//! 在 300 行门禁内（照 `torrents/reseed.rs` 的先例）。
//!
//! 认领记两样基线：做种时长与上传量（NP `claims.seed_time_begin` /
//! `uploaded_begin` 口径），结算 worker 之后按「现值 − 基线」算增量。
//!
//! **与保种区列表必须同一把尺**：本域列表的谓词是 `t.approval_status = 1`
//! （见 `preserve.rs` 的 preserve_list），后台 claims 的 active/unclaimed
//! 又以 `sp.exited_at IS NULL` 为界。旧写法只判 `sp.claimed_by IS NULL` ⇒
//! 一条被下架/驳回却仍留着保种区行的种子，列表里根本看不见，却能被人直接
//! 打接口认领：认领人凭空背上保种义务，结算也按这条记 credit（P-317 同族
//! ——「审核态」与「用户动作」两把尺各量各的）。现网实测：在区 69 行里
//! 未过审 0 行 ⇒ 加这道闸对合法认领是零行为变化。
//!
//! 拒绝原因也分开报：不存在 / 被别人抢先 / 根本不在可认领范围。旧写法三样
//! 挤在一句「该种子已被认领或不在保种区」里，会员只会对着同一个按钮反复点
//! （同族见 P-373 收藏、P-374 求种写口）。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[derive(Deserialize)]
struct PreserveClaimReq {
    torrent_id: i64,
}

#[post("/preserve/claim")]
pub(crate) async fn preserve_claim(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<PreserveClaimReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let updated = sqlx::query(
        "UPDATE seed_preserve sp SET \
            claimed_by = $2, claimed_at = now(), last_settle_at = now(), \
            seed_time_begin = COALESCE(( \
                SELECT s.seeded_seconds FROM snatches s \
                WHERE s.torrent_id = sp.torrent_id AND s.user_id = $2), 0), \
            uploaded_begin = COALESCE(( \
                SELECT s.uploaded FROM snatches s \
                WHERE s.torrent_id = sp.torrent_id AND s.user_id = $2), 0) \
         WHERE sp.torrent_id = $1 AND sp.claimed_by IS NULL \
            AND sp.exited_at IS NULL \
            AND EXISTS(SELECT 1 FROM torrents t \
                       WHERE t.id = sp.torrent_id AND t.approval_status = 1)",
    )
    .bind(body.torrent_id)
    .bind(auth.id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if updated.rows_affected() == 0 {
        // 写失败只代表三件事之一，逐条报出来（读一次不影响原子性：判定
        // 仍然由上面那条 UPDATE 的谓词承担）
        let why: Option<(bool, bool)> = sqlx::query_as(
            "SELECT sp.exited_at IS NULL, t.approval_status = 1 FROM \
             seed_preserve sp JOIN torrents t ON t.id = sp.torrent_id \
             WHERE sp.torrent_id = $1",
        )
        .bind(body.torrent_id)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        let msg = match why {
            None => return Err(DomainError::NotFound(body.torrent_id)),
            Some((true, true)) => "该种子已被别人认领",
            Some(_) => "该种子不在可认领范围（未过审或已退出保种区）",
        };
        return Err(DomainError::Validation(msg.into()));
    }
    state
        .repo
        .audit(Some(auth.id), "preserve_claim", Some(body.torrent_id))
        .await;
    Ok(ok(serde_json::json!({ "claimed": body.torrent_id })))
}
