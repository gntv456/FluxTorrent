//! 种子互动·感谢/收藏/求补种（2026-11-28 从 interact.rs 拆出守行数门禁）。
//! 评论区三件（create/delete/toggle_like）仍在 interact.rs；
//! bookmark DELETE 对称口在 interact_bookmark.rs。
use actix_web::{post, put, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;
use serde_json::Value;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;
use crate::torrents;

use super::aggregate::invalidate_tdetail_cache;

#[derive(Deserialize, utoipa::ToSchema)]
struct BookmarkReq {
    on: bool,
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct ThankBody {
    /// 魔力答谢数额（同类站通用口径：+1/+10/+100/+500/+1000/+10000；缺省 0 = 免费感谢）
    #[serde(default)]
    amount: Option<i64>,
}

#[utoipa::path(
    post,
    path = "/api/v1/torrents/{id}/thanks",
    tag = "种子",
    description = "感谢（已感谢过返回 code 5002）。",
    params(
        ("id" = i64, description = "种子 id"),
    ),
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 404, description = "种子不存在或不可见（code 1004）"),
        (status = 422, description = "参数校验失败（code 1002）"),
        (status = 403, description = "权限不足（code 2003）"),
    )
)]
#[post("/torrents/{id}/thanks")]
pub(crate) async fn do_thank(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: crate::http::OptJson<ThankBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let tid = path.into_inner();
    // 提取器形状是承重的一环：旧写法 `Option<web::Json<ThankBody>>` 会把
    // **一切解析错误折成 `Ok(None)`**（本仓实测过的 prod-layer 陷阱），于是
    // `deny_unknown_fields` 根本没机会生效——`{"amountt": 100}` 静默降级成
    // 免费感谢还回 200。仓里为这一族已有 `OptJson`（空体/裸 null 仍算缺省，
    // 写坏的载荷回 1002 并走中央中文口径），先例见 loan_repay/pets。
    let amount = body.and_then(|b| b.amount).unwrap_or(0);
    // 白名单判定不带符号：旧写法 `amount > 0 &&` 让负数一路放行，
    // 于是 `{"amount": -100}` 静默演成「免费感谢 + 200」——客户端以为
    // 答谢了 100 魔力，账面一分没转（P-310 同族：缺省分支反做）。
    if amount != 0 && !super::thanks_spark::is_valid_amount(amount) {
        return Err(DomainError::Validation(
            "答谢数额需为 1/10/100/500/1000/10000".into(),
        ));
    }
    // 种主随种一起读出；种不存在直接 404（旧版 owner=None 时静默跳过转账）
    // P-643 同族：入口种先过统一准入门——此前只经 `owner_of` 读种主，任意
    // 会员都能往别人待审/被驳回的种上落一行感谢（注解一直写着 1004）。
    torrents::assert_visible(
        &state.repo.db,
        tid,
        (auth.id, auth.class_id >= 90),
    )
    .await?;
    let owner = super::thanks_spark::owner_of(&state.repo.db, tid).await?;
    if amount > 0 && owner == auth.id {
        return Err(DomainError::Validation("不能向自己的种子答谢魔力".into()));
    }
    // P-360/380：感谢行与转账同事务；键取本次行时间戳（成因见 thanks_spark）
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let row_ts = torrents::thank(&mut *tx, tid, auth.id).await?;
    // 0503 死奖励卡接线（NP 行为奖励表 0034 播种、后台可调、此前零读者）：
    // saythanks=感谢者、receivethanks=发布者——只在新感谢行成立时发
    // （重复感谢 row_ts=None 不发），幂等靠唯一行 + 锚感谢时间戳。
    let mut say_r = 0i64;
    let mut recv_r = 0i64;
    {
        let say: i64 = sqlx::query_scalar(
            "SELECT COALESCE((SELECT value::bigint FROM site_settings \
         WHERE name = 'saythanks'), 0)",
        )
        .fetch_one(&mut *tx)
        .await
        .unwrap_or(0);
        let recv: i64 = sqlx::query_scalar(
            "SELECT COALESCE((SELECT value::bigint FROM site_settings \
         WHERE name = 'receivethanks'), 0)",
        )
        .fetch_one(&mut *tx)
        .await
        .unwrap_or(0);
        if say > 0 {
            let idem = format!("saythanks:{tid}:{}", auth.id);
            say_r = super::thanks_spark::earn_in_tx(
                &mut tx,
                auth.id,
                say,
                "saythanks",
                &idem,
            )
            .await
            .unwrap_or(0);
        }
        if recv > 0 && owner != auth.id {
            let idem = format!("receivethanks:{tid}:{}", auth.id);
            recv_r = super::thanks_spark::earn_in_tx(
                &mut tx,
                owner,
                recv,
                "receivethanks",
                &idem,
            )
            .await
            .unwrap_or(0);
        }
    }
    let given = if amount > 0 {
        super::thanks_spark::transfer(
            &mut tx, auth.id, owner, amount, tid, row_ts,
        )
        .await?
    } else {
        0
    };
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 0423 发布者正反馈：答谢魔力通知种主（详见 notify.rs）
    super::notify::notify_thanks(&state.repo.db, tid, auth.id, amount).await;
    // 聚合缓存：thanks_count 与「最近活动」都在 aggregate 里
    super::aggregate::invalidate_tdetail_cache(&state, tid).await;
    Ok(ok(
        serde_json::json!({ "thanked": true, "spark_given": given }),
    ))
}

/// 请求补种（NP takereseed.php：死种 → PM 全体完成者，900s 限频）
#[utoipa::path(
    post,
    path = "/api/v1/torrents/{id}/reseed",
    tag = "种子",
    description = "求种（挂出求种位；过期天数 <=0 走 30 天兜底，避免悬赏永久冻结）。",
    params(
        ("id" = i64, description = "种子 id"),
    ),
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 404, description = "种子不存在或不可见（code 1004）"),
        (status = 422, description = "参数校验失败（code 1002）"),
        (status = 403, description = "权限不足（code 2003）"),
    )
)]
#[post("/torrents/{id}/reseed")]
pub(crate) async fn request_reseed(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let tid = path.into_inner();
    let username: String =
        sqlx::query_scalar("SELECT username FROM users WHERE id = $1")
            .bind(auth.id)
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or_else(|_| "user".into());
    let n = torrents::request_reseed(&state.repo.db, tid, (auth.id, username))
        .await?;
    let _ = state
        .repo
        .audit(Some(auth.id), "torrent.reseed", Some(tid))
        .await;
    Ok(ok(serde_json::json!({ "notified": n })))
}

#[utoipa::path(
    put,
    path = "/api/v1/torrents/{id}/bookmark",
    tag = "种子",
    description = "收藏切换。",
    params(
        ("id" = i64, description = "种子 id"),
    ),
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 404, description = "种子不存在或不可见（code 1004）"),
        (status = 422, description = "参数校验失败（code 1002）"),
        (status = 403, description = "权限不足（code 2003）"),
    )
)]
#[put("/torrents/{id}/bookmark")]
pub(crate) async fn do_bookmark(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<BookmarkReq>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let tid = path.into_inner();
    torrents::bookmark(&state.repo.db, tid, auth.id, body.on).await?;
    // 聚合缓存：bookmark_count 在 aggregate.detail 里
    super::aggregate::invalidate_tdetail_cache(&state, tid).await;
    Ok(ok(serde_json::json!({ "bookmarked": body.on })))
}
