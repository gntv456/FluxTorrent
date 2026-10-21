//! 泄露复核 + 聊天机器人。
//! 从 community_http.rs 按域拆出。

use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

// ============ 泄露事件复核（0078，U3D Leaker 口径：worker 只报告，staff 复核） ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct LeakRow {
    id: i64,
    kind: String,
    user_id: i64,
    username: Option<String>,
    torrent_id: Option<i64>,
    detail: serde_json::Value,
    score: i16,
    resolved: i16,
    created_at: chrono::DateTime<chrono::Utc>,
}

/// 待复核泄露事件列表（staff）
#[utoipa::path(
    get,
    path = "/api/v1/staff/leaks",
    tag = "社区/站务",
    description = "数据泄露举报列表（需 staff 权）。",
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 403, description = "非 staff（code 2003）")))]
#[get("/staff/leaks")]
pub(crate) async fn leak_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::BANS_MANAGE)
        .await?;
    let rows = sqlx::query_as::<_, LeakRow>(
        "SELECT e.id, e.kind, e.user_id, u.username, e.torrent_id, e.detail, e.score, e.resolved, e.created_at \
         FROM leak_events e LEFT JOIN users u ON u.id = e.user_id \
         WHERE e.resolved = 0 ORDER BY e.score DESC, e.created_at DESC LIMIT 100",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize, utoipa::ToSchema)]
struct LeakResolveReq {
    id: i64,
    /// 1=确认泄露 2=误报
    verdict: i16,
}

/// 泄露事件裁决：确认泄露时通知全部 staff（走 staffmessages 分流），误报仅归档
#[utoipa::path(
    post,
    path = "/api/v1/staff/leaks/resolve",
    tag = "社区/站务",
    description = "处理泄露举报（需 staff 权）。",
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 403, description = "非 staff（code 2003）"),
        (status = 422, description = "参数校验失败（code 1002）")))]
#[post("/staff/leaks/resolve")]
pub(crate) async fn leak_resolve(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<LeakResolveReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::BANS_MANAGE)
        .await?;
    if ![1, 2].contains(&body.verdict) {
        return Err(DomainError::Validation(
            "verdict 需为 1（确认）或 2（误报）".into(),
        ));
    }
    let n = sqlx::query(
        "UPDATE leak_events SET resolved = $2, \
         resolved_by = $3 WHERE id = $1 AND resolved = 0",
    )
    .bind(body.id)
    .bind(body.verdict)
    .bind(auth.id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(body.id));
    }
    if body.verdict == 1 {
        let (uid, kind, detail): (i64, String, serde_json::Value) =
            sqlx::query_as(
                "SELECT user_id, kind, detail FROM leak_events WHERE id = $1",
            )
            .bind(body.id)
            .fetch_one(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        // 审计修复（P1 语义错位）：staffmessages.user_id 是「来信人」——旧版 bind 泄露者
        // 本人，工单列表把被处置对象显示为提交人，且 staff_answer 会把处置意图 PM
        // 提前发给泄露者。改为以复核 staff 名义立项（subject 内带泄露者 id 供追溯）。
        sqlx::query(
            "INSERT INTO staffmessages (user_id, subject, body, permission) \
             VALUES ($1, $2, $3, 'security')",
        )
        .bind(auth.id)
        .bind(format!("泄露事件确认（{}，用户 #{}）", kind, uid))
        .bind(format!(
            "事件 #{} 已由 staff 复核确认为真实泄露（涉及用户 #{uid}）。证据：{}。请按流程处置（重置 passkey / 必要时封号）。",
            body.id, detail
        ))
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }
    state
        .repo
        .audit(Some(auth.id), "leak_resolve", Some(body.id))
        .await;
    Ok(ok(
        serde_json::json!({ "id": body.id, "verdict": body.verdict }),
    ))
}

// ============ 聊天机器人（0078，NerdBot 统计命令系，v3 §27-20） ============
// shoutbox 里发 /命令 即触发系统账号回话（当前会话内直接返回，不落库系统消息——
// 避免机器人刷屏；统计命令只读、零风险）。

#[utoipa::path(
    get,
    path = "/api/v1/shoutbox/bot",
    tag = "社区/喊话",
    description = "喊话机器人命令清单（公开只读，喊话界面输 / 时可发现）。",
    responses((status = 200, description = "ok")))]
#[get("/shoutbox/bot")]
pub(crate) async fn shoutbox_bot_help() -> impl Responder {
    ok(serde_json::json!({
        "commands": [
            { "cmd": "/free",  "desc": "当前生效的免费/双倍促销种子" },
            { "cmd": "/stats", "desc": "站点实时统计（用户/种子/做种）" },
            { "cmd": "/me",    "desc": "我的数据摘要（上传/下载/分享率/魔力）" },
            { "cmd": "/help",  "desc": "命令列表" },
        ]
    }))
}

/// /free 回话的种子卡片数据（0420 播报带种卡）：与论坛 !torrent 引用卡
/// 同族，随回话直接下发（零新端点）；promotion 取 promotions.kind。
#[derive(sqlx::FromRow, serde::Serialize, Default)]
struct FreeCard {
    id: i64,
    name: String,
    small_descr: Option<String>,
    size: i64,
    seeders: i32,
    leechers: i32,
    poster: Option<String>,
    promotion: String,
}

/// /free 纯文本回话；种卡由 cards[] 并行下发，文本保留供纯文本消费方。
fn free_reply(cards: &[FreeCard]) -> String {
    if cards.is_empty() {
        "当前没有免费促销的种子。".to_string()
    } else {
        format!(
            "当前免费：{}",
            cards
                .iter()
                .map(|c| format!("#{} {}", c.id, c.name))
                .collect::<Vec<_>>()
                .join("；")
        )
    }
}

/// 命令分发（GET 供前端在发送 /命令 时调用并展示回话）
#[utoipa::path(
    get,
    path = "/api/v1/shoutbox/bot/exec",
    tag = "社区/喊话",
    description = "执行喊话机器人指令：三条只读回话，任何登录用户可用。 \
     /free 另附免费种子卡片数据 cards[]（喊话盒「播报带种卡」消费）。",
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）")))]
#[get("/shoutbox/bot/exec")]
pub(crate) async fn shoutbox_bot_exec(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: crate::http::Q<std::collections::HashMap<String, String>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let cmd = q.get("cmd").map(|s| s.trim()).unwrap_or("");
    // cards 仅 /free 填充（0420 播报带种卡）；其余命令回空数组，前端不渲染
    let mut cards: Vec<FreeCard> = Vec::new();
    let reply = match cmd {
        "/free" => {
            cards = sqlx::query_as(
                "SELECT t.id, t.name, t.small_descr, t.size, t.seeders, \
                 t.leechers, t.media_info->>'poster' AS poster, \
                 p.kind AS promotion \
                 FROM promotions p JOIN torrents t ON t.id = p.torrent_id \
                 WHERE p.starts_at <= now() AND p.ends_at > now() \
                   AND p.kind IN ('free','x2free') \
                 ORDER BY p.ends_at LIMIT 5",
            )
            .fetch_all(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            free_reply(&cards)
        }
        "/stats" => {
            let (users, torrents, seeding): (i64, i64, i64) = sqlx::query_as(
                "SELECT (SELECT count(*) FROM users WHERE status < 2), \
                        (SELECT count(*) FROM torrents), \
                        (SELECT count(*) FROM snatches WHERE seeding)",
            )
            .fetch_one(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            format!(
                "站点现状：{} 位用户 / {} 个种子 / {} 个做种连接。",
                users, torrents, seeding
            )
        }
        "/me" => {
            let (up, down, spark): (i64, i64, i64) = sqlx::query_as(
                "SELECT uploaded, downloaded, \
                 spark_balance FROM users WHERE id = $1",
            )
            .bind(auth.id)
            .fetch_one(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            format!(
                "你的数据：上传 {:.1} GB / 下载 {:.1} GB / 分享率 {:.2} / 魔力 {}。",
                up as f64 / 1073741824.0,
                down as f64 / 1073741824.0,
                if down > 0 {
                    up as f64 / down as f64
                } else {
                    0.0
                },
                spark
            )
        }
        _ => "可用命令：/free /stats /me /help".to_string(),
    };
    let body =
        serde_json::json!({ "cmd": cmd, "reply": reply, "cards": cards });
    Ok(ok(body))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(id: i64, name: &str) -> FreeCard {
        FreeCard {
            id,
            name: name.to_string(),
            promotion: "free".to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn free_reply_text() {
        // 无促销一句话；有种列「#id 名称」分号相连
        assert_eq!(free_reply(&[]), "当前没有免费促销的种子。");
        let cards = vec![card(7, "甲"), card(9, "乙")];
        assert_eq!(free_reply(&cards), "当前免费：#7 甲；#9 乙");
    }

    /// cards 序列化含前端紧凑卡所需字段（缺字段 = 静默少渲染一角）
    #[test]
    fn free_cards_serialize() {
        let v = serde_json::to_value(&card(7, "甲")).unwrap();
        let s = v.to_string();
        for k in ["id", "name", "size", "seeders", "promotion", "poster"] {
            assert!(s.contains(&format!("\"{k}\":")), "cards 缺字段 {k}");
        }
        assert_eq!(v["promotion"], "free");
    }
}
