//! 申诉提交（0020）：登录/被封两条路径、形状校验、未结上限与落库。
//! 从 appeals.rs 拆出（300 行门禁）：文件内子模块，不碰站方在途的
//! gaps_http/mod.rs；父模块 `use appeals::*` 照常取到路由。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::gaps_http::captcha::captcha_verify;
use crate::http::require_auth;
use crate::state::AppState;

#[derive(Deserialize)]
struct AppealReq {
    kind: String,
    #[serde(default)]
    ref_id: Option<i64>,
    body: String,
    /// 未登录封禁申诉（P0 修复）专用字段：被申诉的用户名 + 验证码
    #[serde(default)]
    username: Option<String>,
    #[serde(default)]
    captcha_id: Option<String>,
    #[serde(default)]
    captcha_answer: Option<i32>,
}

/// 封禁申诉的验证码校验（复用注册同款图形验证码，一次性消费）
async fn captcha_check(
    state: &AppState,
    id: &Option<String>,
    answer: &Option<i32>,
) -> DomainResult<()> {
    let (Some(id), Some(answer)) = (id.as_deref(), *answer) else {
        return Err(DomainError::Validation("请填写图形验证码".into()));
    };
    if !captcha_verify(state, id, answer).await {
        return Err(DomainError::Validation("验证码错误或已过期".into()));
    }
    Ok(())
}

#[post("/appeals")]
pub(crate) async fn appeal_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<AppealReq>,
) -> DomainResult<HttpResponse> {
    // 写口上限（P-466/P-467 同族）：界面 maxLength 2000 只是浏览器提示，
    // 绕开界面直接 POST 可把任意大正文写进 appeals，职员队列与详情原样
    // 渲染。与界面同一枚数；放在 require_auth 与任何查库之前。
    if body.body.trim().chars().count() > 2000 {
        return Err(DomainError::Validation("申诉内容最长 2000 字".into()));
    }
    // 已登录用户：常规提交路径。
    // 审计修复（P0）：封禁申诉的目标人群（status>=2）登录被拒、require_auth 返回 403，
    // 旧版对该人群完全不可用。现在未携带有效凭证时按「被封申诉」专用分支处理：
    // 凭用户名 + 预验证码 + 防刷限流即可提交，不签发任何会话（见下方 guest 分支）。
    let auth = require_auth(&req, &state).await;
    let user_id = match auth {
        Ok(a) => a.id,
        Err(_) => {
            // 未登录 / 被封（403）/ token 过期：仅允许 ban 申诉，且必须通过验证码
            if body.kind != "ban" {
                return Err(DomainError::Unauthorized);
            }
            let username = body
                .username
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| {
                    DomainError::Validation("请填写被封的账户用户名".into())
                })?;
            captcha_check(&state, &body.captcha_id, &body.captcha_answer)
                .await?;
            let uid: Option<i64> = sqlx::query_scalar(
                "SELECT id FROM users WHERE \
                 lower(username::text) = lower($1) AND status >= 2",
            )
            .bind(username)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            let uid = uid.ok_or_else(|| {
                DomainError::Validation("未找到该用户名的封禁记录".into())
            })?;
            // 防刷：每用户名 3 次/小时（未结申诉上限 3 条之外的第二道闸）
            //
            // P-67：旧写法 `incr(...).unwrap_or(0)`——Redis 抖动时这道闸不
            // 存在，可无限灌申诉表。改 fail-close 的 rate_window（1 小时窗，
            // 键名与额度不变）。同分支的 captcha 校验本来就是 fail-close，
            // 这一道不该比它更松。
            crate::http::rate_window(&state, &format!("appeal:{uid}"), 3, 3600)
                .await?;
            uid
        }
    };
    if !["hr", "warn", "ban", "other"].contains(&body.kind.as_str()) {
        return Err(DomainError::Validation(
            "kind 仅支持 hr/warn/ban/other".into(),
        ));
    }
    if body.body.trim().chars().count() < 10 {
        return Err(DomainError::Validation("申诉内容至少 10 字".into()));
    }
    // 未结申诉上限 3 条
    let open: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM appeals WHERE user_id = $1 AND status = 'open'",
    )
    .bind(user_id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if open >= 3 {
        return Err(DomainError::Validation(
            "未结申诉上限 3 条，请等待处理".into(),
        ));
    }
    let ref_id =
        sole_violated_hr(&state, &body.kind, body.ref_id, user_id).await?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO appeals (user_id, kind, ref_id, body) VALUES ($1, \
         $2, $3, $4) RETURNING id",
    )
    .bind(user_id)
    .bind(&body.kind)
    .bind(ref_id)
    .bind(body.body.trim())
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "id": id, "ref_id": ref_id })))
}

/// H&R 申诉的靶子（批七轮 P-?）：「我的 H&R」卡片只把会员递到 /appeals，
/// 界面送出的载荷里**没有条目号**（`appeals/page.tsx` 只拼 kind/body），
/// 于是职员看到的每条 H&R 申诉都不知道指的是哪一条，受理也落不到动作。
/// 只在「该会员恰好有一条已违规记录」时替他把条目号补上；多于一条留空，
/// 宁可让职员去问，也不替会员猜是哪一条。
async fn sole_violated_hr(
    state: &AppState,
    kind: &str,
    given: Option<i64>,
    user_id: i64,
) -> DomainResult<Option<i64>> {
    if kind != "hr" || given.is_some() {
        return Ok(given);
    }
    let rows: Vec<i64> = sqlx::query_scalar(
        "SELECT torrent_id FROM hr_snapshots WHERE user_id = $1 \
         AND status = 'violated' ORDER BY created_at DESC LIMIT 2",
    )
    .bind(user_id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(if rows.len() == 1 {
        rows.into_iter().next()
    } else {
        None
    })
}
