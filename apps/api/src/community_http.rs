//! 社区模块 HTTP 接口（M14 勋章 + M15 论坛 + M16 短讯/好友）。

use actix_web::{delete, get, post, put, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;
use uuid::Uuid;

use crate::dto::ok;
use crate::economy_http::earn_spark;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

pub fn mount_community(scope: actix_web::Scope) -> actix_web::Scope {
    scope
        // M14 勋章
        .service(medal_rarities)
        .service(medal_list)
        .service(medal_buy)
        .service(medal_gift)
        .service(medal_wear)
        .service(my_medals)
        .service(notice_prefs_get)
        .service(notice_prefs_set)
        .service(pool_honor)
        // M15 论坛
        .service(forum_list)
        .service(forum_search)
        .service(topic_create)
        .service(topic_list)
        .service(topic_detail)
        .service(post_reply)
        .service(post_edit)
        .service(post_delete)
        .service(topic_delete)
        .service(topic_manage)
        // M15 论坛互动（0116 点赞/收藏）
        .service(post_like)
        .service(post_unlike)
        .service(topic_favorite)
        .service(topic_unfavorite)
        // M15 论坛关注订阅（0121 用户/版块/主题 + 关注流）
        .service(follow_create)
        .service(follow_delete)
        .service(follow_status)
        .service(follow_mine)
        .service(forum_feed)
        // M15 论坛标签（0123：词表复用 tag_dict，只建 topic_tags 关联）
        .service(forum_tags_dict)
        // M15 论坛悬赏（0124：发帖冻结 → 楼主采纳发放，复用求种悬赏范式）
        .service(bounty_award)
        // M15 论坛投票（0125：发帖定选项 → 一人一票 → 楼主可截止，范式照 fun_polls）
        .service(poll_vote)
        .service(poll_close)
        // M15 论坛抽奖（0126：发帖冻结奖金池 → 付费/免费参与 → 到点或手动开奖，jgg 经济范式）
        .service(lottery_join)
        .service(lottery_draw)
        // M15 论坛打赏（0127：楼层打赏 = spend/earn 同额对冲，不抽税）
        .service(post_tip)
        // M16 短讯与好友
        .service(message_send)
        .service(message_markread)
        .service(message_delete)
        .service(message_move)
        .service(message_boxes)
        .service(message_box_upsert)
        .service(contact_staff)
        .service(staff_messages)
        .service(my_staff_messages)
        .service(my_ticket_confirm)
        .service(staff_answer)
        .service(staff_mark)
        .service(staff_delete)
        .service(shoutbox_list)
        .service(shoutbox_send)
        .service(shoutbox_delete)
        .service(shoutbox_bot_help)
        .service(shoutbox_bot_exec)
        .service(ticket_list)
        .service(ticket_update)
        .service(leak_list)
        .service(leak_resolve)
        .service(message_inbox)
        .service(message_sent)
        .service(message_staff)
        .service(friend_add)
        .service(friend_list)
        .service(friend_action)
}

// ============ M14 勋章 ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct MedalRow {
    id: i64,
    name: String,
    price: Option<i64>,
    rarity: Option<String>,
    limited: bool,
    owned: bool,
    wearing: bool,
    description: Option<String>,
    duration_days: Option<i32>,
    get_type: i16,
    sale_begin_at: Option<chrono::DateTime<chrono::Utc>>,
    sale_end_at: Option<chrono::DateTime<chrono::Utc>>,
    inventory: Option<i32>,
    bonus_addition_factor: f64,
    category_id: i32,
    category_name: Option<String>,
    /// 勋章图片（asset_ref，0001 就有列；此前只在后台接口返回，前台拿不到 → 全站只能画 🏅）
    asset_ref: Option<String>,
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct MedalRarityRow {
    value: String,
    label: String,
    /// 配色档（前端映射到 tailwind token 类）
    tone: String,
    sort: i32,
}

/// 勋章稀有度词表（0143）：前台角标与后台下拉共用；词表主体在后台维护。
/// 免鉴权——纯展示元数据，且 /medals 页面本身就可能以未登录态渲染。
#[get("/medal-rarities")]
async fn medal_rarities(
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let rows: Vec<MedalRarityRow> =
        sqlx::query_as("SELECT value, label, tone, sort FROM medal_rarities ORDER BY sort, value")
            .fetch_all(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[get("/medals")]
async fn medal_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await.ok();
    let uid = auth.map(|a| a.id);
    let rows = sqlx::query_as::<_, MedalRow>(
        "SELECT m.id, m.name, m.price, m.rarity, m.limited, m.description, m.duration_days, m.get_type,             m.sale_begin_at, m.sale_end_at, m.inventory, m.bonus_addition_factor::float8, m.category_id,             c.name AS category_name, m.asset_ref,             ($1::bigint IS NOT NULL AND EXISTS(SELECT 1 FROM user_medals um WHERE um.medal_id = m.id AND um.user_id = $1 AND (um.expires_at IS NULL OR um.expires_at > now()))) AS owned,             ($1::bigint IS NOT NULL AND EXISTS(SELECT 1 FROM user_medals um WHERE um.medal_id = m.id AND um.user_id = $1 AND um.wearing AND (um.expires_at IS NULL OR um.expires_at > now()))) AS wearing          FROM medals m LEFT JOIN medal_categories c ON c.id = m.category_id ORDER BY m.category_id, m.id",
    )
    .bind(uid)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct MedalBuyReq {
    medal_id: i64,
    /// 前端生成的幂等键（同一键重试不双扣）；缺省时回退随机键（兼容旧客户端）
    #[serde(default)]
    idempotency_key: Option<String>,
}

#[post("/medals/buy")]
async fn medal_buy(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<MedalBuyReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let price: Option<i64> = sqlx::query_scalar("SELECT price FROM medals WHERE id = $1")
        .bind(body.medal_id)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .flatten();
    let Some(price) = price else {
        return Err(DomainError::NotFound(body.medal_id)); // 非卖品勋章（如开站勋章）
    };
    // 已拥有直接拒绝（防重复扣款）
    let owned: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM user_medals WHERE user_id = $1 AND medal_id = $2 AND (expires_at IS NULL OR expires_at > now()))",
    )
    .bind(auth.id)
    .bind(body.medal_id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    if owned {
        return Err(DomainError::Validation("已拥有该勋章".into()));
    }
    // 限量与销售期校验（medals.inventory NULL = 不限量；窗口 NULL = 长期在售）
    let (inventory, inv_used, sale_begin, sale_end): (
        Option<i32>,
        i64,
        Option<chrono::DateTime<chrono::Utc>>,
        Option<chrono::DateTime<chrono::Utc>>,
    ) = sqlx::query_as(
        "SELECT m.inventory, (SELECT count(*) FROM user_medals um WHERE um.medal_id = m.id), \
                m.sale_begin_at, m.sale_end_at \
         FROM medals m WHERE m.id = $1",
    )
    .bind(body.medal_id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if let Some(begin) = sale_begin {
        if chrono::Utc::now() < begin {
            return Err(DomainError::Validation("该勋章尚未开售".into()));
        }
    }
    if let Some(end) = sale_end {
        if chrono::Utc::now() > end {
            return Err(DomainError::Validation("该勋章已结束销售".into()));
        }
    }
    if let Some(stock) = inventory {
        if inv_used >= stock as i64 {
            return Err(DomainError::Validation("该勋章已售罄".into()));
        }
    }
    let idem = body
        .idempotency_key
        .clone()
        .filter(|k| !k.trim().is_empty())
        .unwrap_or_else(|| format!("medal-buy:{}:{}:{}", auth.id, body.medal_id, Uuid::new_v4()));
    // 幂等重放闸门（#[must_use] 连审）：重放时 spend 不再扣款，继续执行会绕过
    // 上方的拥有/售期/限量三重检查直接走授予分支
    if !matches!(
        crate::economy_http::spend_spark(
            &state.repo.db,
            auth.id,
            price,
            "shop",
            &idem,
            "medal",
            body.medal_id,
        )
        .await?,
        crate::economy_http::SpendOutcome::Spent
    ) {
        return Err(DomainError::Validation(
            "该笔请求已受理，请勿重复提交".into(),
        ));
    }
    // 限时勋章按 duration_days 写 expires_at（0067；NULL = 永久）
    sqlx::query(
        "INSERT INTO user_medals (user_id, medal_id, source, expires_at) \
         SELECT $1, $2, 'buy', now() + make_interval(days => m.duration_days) \
         FROM medals m WHERE m.id = $2 \
         ON CONFLICT (user_id, medal_id) DO NOTHING",
    )
    .bind(auth.id)
    .bind(body.medal_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(
        serde_json::json!({ "medal_id": body.medal_id, "price": price }),
    ))
}

#[derive(Deserialize)]
struct MedalGiftReq {
    medal_id: i64,
    to_user: String,
    #[serde(default)]
    idempotency_key: Option<String>,
}

#[post("/medals/gift")]
async fn medal_gift(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<MedalGiftReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let to_id: Option<i64> =
        sqlx::query_scalar("SELECT id FROM users WHERE username = $1 AND status < 2")
            .bind(&body.to_user)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .flatten();
    let Some(to_id) = to_id else {
        return Err(DomainError::NotFound(0));
    };
    if to_id == auth.id {
        return Err(DomainError::Validation("不能赠送给自己".into()));
    }
    // 购买并直接入对方账户（赠送弹窗流程：一步完成）
    let price: Option<i64> = sqlx::query_scalar("SELECT price FROM medals WHERE id = $1")
        .bind(body.medal_id)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .flatten();
    let Some(price) = price else {
        return Err(DomainError::NotFound(body.medal_id));
    };
    // 赠送税（0078）：礼物链路抽 gift_tax_bp（缺省 5%）入站免池——
    // 扣款仍按全额（spend_spark price），勋章照常发放；税在「站点收入」侧记账，
    // 即 magic_pool/pool_donations（出资人=送礼人），不另记正向流水（防虚增 minted）。
    let tax_bp: i32 =
        sqlx::query_scalar("SELECT value FROM site_settings WHERE name = 'gift_tax_bp'")
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .and_then(|v: String| v.parse().ok())
            .unwrap_or(500);
    let tax = crate::economy::gift_tax(price, tax_bp);
    let idem = body
        .idempotency_key
        .clone()
        .filter(|k| !k.trim().is_empty())
        .unwrap_or_else(|| {
            format!(
                "medal-gift:{}:{}:{}",
                auth.id,
                body.medal_id,
                Uuid::new_v4()
            )
        });
    // 赠送通道同样受「已拥有/售期/限量」约束（此前 gift 绕过三重检查可超卖限量勋章）
    let receiver_owned: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM user_medals WHERE user_id = $1 AND medal_id = $2 AND (expires_at IS NULL OR expires_at > now()))",
    )
    .bind(to_id)
    .bind(body.medal_id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    if receiver_owned {
        return Err(DomainError::Validation("对方已拥有该勋章".into()));
    }
    let (inventory, inv_used, sale_begin, sale_end): (
        Option<i32>,
        i64,
        Option<chrono::DateTime<chrono::Utc>>,
        Option<chrono::DateTime<chrono::Utc>>,
    ) = sqlx::query_as(
        "SELECT m.inventory, (SELECT count(*) FROM user_medals um WHERE um.medal_id = m.id),                 m.sale_begin_at, m.sale_end_at          FROM medals m WHERE m.id = $1",
    )
    .bind(body.medal_id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if let Some(begin) = sale_begin {
        if chrono::Utc::now() < begin {
            return Err(DomainError::Validation("该勋章尚未开售".into()));
        }
    }
    if let Some(end) = sale_end {
        if chrono::Utc::now() > end {
            return Err(DomainError::Validation("该勋章已结束销售".into()));
        }
    }
    if let Some(stock) = inventory {
        if inv_used >= stock as i64 {
            return Err(DomainError::Validation("该勋章已售罄".into()));
        }
    }
    // 幂等重放闸门（P0）：赠送链路是 spend(赠送人) → earn(受赠人) 对冲，重放时
    // spend 不再扣款而 earn 照发 = 受赠人凭空入账
    if !matches!(
        crate::economy_http::spend_spark(
            &state.repo.db,
            auth.id,
            price,
            "shop",
            &idem,
            "medal_gift",
            body.medal_id,
        )
        .await?,
        crate::economy_http::SpendOutcome::Spent
    ) {
        return Err(DomainError::Validation(
            "该笔赠送已受理，请勿重复提交".into(),
        ));
    }
    if tax > 0 {
        let month = crate::economy::pool_month(chrono::Utc::now());
        sqlx::query(
            "INSERT INTO magic_pool (month, donated_total) VALUES ($1, $2) \
             ON CONFLICT (month) DO UPDATE SET donated_total = magic_pool.donated_total + $2",
        )
        .bind(&month)
        .bind(tax)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        sqlx::query("INSERT INTO pool_donations (user_id, amount, month) VALUES ($1, $2, $3)")
            .bind(auth.id)
            .bind(tax)
            .bind(&month)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    }
    sqlx::query(
        "INSERT INTO user_medals (user_id, medal_id, source, expires_at) \
         SELECT $1, $2, 'gift', now() + make_interval(days => m.duration_days) \
         FROM medals m WHERE m.id = $2 \
         ON CONFLICT (user_id, medal_id) DO NOTHING",
    )
    .bind(to_id)
    .bind(body.medal_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 收件人通知（审计补齐：收礼物却无感知，只能自己去勋章页发现）
    let _ = sqlx::query(
        "INSERT INTO messages (sender_id, receiver_id, subject, body) VALUES ($1, $2, $3, $4)",
    )
    .bind(auth.id)
    .bind(to_id)
    .bind("收到勋章礼物")
    .bind(format!(
        "用户 #{} 向你赠送了勋章「#{}」，快去勋章页看看吧！",
        auth.id, body.medal_id
    ))
    .execute(&state.repo.db)
    .await;
    Ok(ok(
        serde_json::json!({ "to": body.to_user, "medal_id": body.medal_id }),
    ))
}

#[derive(Deserialize)]
struct WearReq {
    medal_id: Option<i64>,
}

#[put("/medals/wear")]
async fn medal_wear(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<WearReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    // 单佩戴位：先全部摘下再佩戴指定勋章。两条语句需原子完成——否则「佩戴未拥有勋章」
    // 报错回滚时会把原本已佩戴的勋章也摘掉（先摘后戴的非事务写已生效，无法随错误回退）。
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query("UPDATE user_medals SET wearing = false WHERE user_id = $1")
        .bind(auth.id)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if let Some(mid) = body.medal_id {
        let updated = sqlx::query(
            "UPDATE user_medals SET wearing = true WHERE user_id = $1 AND medal_id = $2",
        )
        .bind(auth.id)
        .bind(mid)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        if updated.rows_affected() == 0 {
            // 回滚：保留佩戴原状（未拥有该勋章时不动已佩戴勋章）
            tx.rollback()
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
            return Err(DomainError::Validation("未拥有该勋章".into()));
        }
    }
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "wearing": body.medal_id })))
}

#[get("/me/medals")]
async fn my_medals(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let rows: Vec<(i64, String, bool)> = sqlx::query_as(
        "SELECT m.id, m.name, um.wearing FROM user_medals um JOIN medals m ON m.id = um.medal_id WHERE um.user_id = $1 AND (um.expires_at IS NULL OR um.expires_at > now()) ORDER BY m.id",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

// ============ M15 论坛（forums.php 三层权限口径） ============
//
// 三层判定叠加：
//   1) 版块三档门槛：minclassread 看 / minclasswrite 回 / minclasscreate 发主题（逐版块独立）
//   2) 版主：forum_mods（forumid+userid，无需等级，仅本版块有效）
//   3) 全局：postmanage = class≥90 全版块管帖；forummanage（≥93，见 admin_http）管版块
// 反直觉点照搬：作者不能删自己的帖（删帖只对版主/postmanage 开放）；
// 删主题收回发帖 +2 火花；管理员编辑他人帖自动 PM 通知并留 edited_by；
// forumpost=FALSE 账户级禁言；发帖 10 秒防刷（postmanage 豁免）；
// 受保护版块 2 楼起正文替换为提示（class≥90/发帖人本人/楼主/本版版主放行）。

/// 三层权限判定结果
#[derive(serde::Serialize, sqlx::FromRow, Clone, Copy)]
struct ForumPerm {
    can_read: bool,
    can_write: bool,
    can_create: bool,
    can_mod: bool,
}

async fn forum_access(
    db: &sqlx::PgPool,
    user_id: i64,
    class_id: i32,
    forum_id: i64,
) -> DomainResult<ForumPerm> {
    let none = ForumPerm {
        can_read: false,
        can_write: false,
        can_create: false,
        can_mod: false,
    };
    let row: Option<(i32, i32, i32)> = sqlx::query_as(
        "SELECT minclassread, minclasswrite, minclasscreate FROM forums WHERE id = $1",
    )
    .bind(forum_id)
    .fetch_optional(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((min_read, min_write, min_create)) = row else {
        return Ok(none);
    };
    // 全局 postmanage：全版块放行
    if class_id >= 90 {
        return Ok(ForumPerm {
            can_read: true,
            can_write: true,
            can_create: true,
            can_mod: true,
        });
    }
    // 版主：本版块全放行（任命不需要等级）
    let is_mod: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM forum_mods WHERE forum_id = $1 AND user_id = $2)",
    )
    .bind(forum_id)
    .bind(user_id)
    .fetch_one(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 账户级禁言（forumpost='no' 口径）：与等级、版块门槛无关，一律不能发帖回帖
    let can_post: bool =
        sqlx::query_scalar("SELECT COALESCE(forumpost, TRUE) FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_one(db)
            .await
            .unwrap_or(true);
    let can_read = is_mod || class_id >= min_read;
    let can_write = can_read && can_post && (is_mod || class_id >= min_write);
    let can_create = can_write && (is_mod || class_id >= min_create);
    Ok(ForumPerm {
        can_read,
        can_write,
        can_create,
        can_mod: is_mod,
    })
}

/// 发帖 10 秒防刷（postmanage/版主豁免）
async fn forum_flood_check(db: &sqlx::PgPool, user_id: i64, class_id: i32) -> DomainResult<()> {
    if class_id >= 90 {
        return Ok(());
    }
    let last: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT last_sent_at FROM forum_flood WHERE user_id = $1")
            .bind(user_id)
            .fetch_optional(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .flatten();
    if let Some(t) = last {
        if (chrono::Utc::now() - t).num_seconds() < 10 {
            return Err(DomainError::Validation(
                "发送过于频繁，请 10 秒后再试".into(),
            ));
        }
    }
    Ok(())
}

async fn forum_flood_mark(db: &sqlx::PgPool, user_id: i64) {
    let _ = sqlx::query(
        "INSERT INTO forum_flood (user_id, last_sent_at) VALUES ($1, now()) \
         ON CONFLICT (user_id) DO UPDATE SET last_sent_at = now()",
    )
    .bind(user_id)
    .execute(db)
    .await;
}

/// 把 Markdown 正文压成纯文本摘要（写入 posts.body_text，供搜索/列表预览）。
/// 不追求完美解析，够用即可：去代码围栏/标题/引用/列表符号 + 行内强调与链接语法。
fn strip_markdown(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut in_fence = false;
    for line in src.lines() {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue; // 代码块内容不进摘要，避免噪音
        }
        let mut l = t.trim_start_matches('#').trim_start();
        while let Some(rest) = l.strip_prefix('>') {
            l = rest.trim_start();
        }
        if let Some(rest) = l
            .strip_prefix("- ")
            .or_else(|| l.strip_prefix("* "))
            .or_else(|| l.strip_prefix("+ "))
        {
            l = rest;
        } else {
            let digits = l.chars().take_while(|c| c.is_ascii_digit()).count();
            if digits > 0 {
                let rest = &l[digits..];
                if let Some(r2) = rest.strip_prefix(". ").or_else(|| rest.strip_prefix(") ")) {
                    l = r2;
                }
            }
        }
        let l = l.trim();
        if l.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(l);
    }
    // 行内：![alt](url)/[text](url) 保留文字去 URL
    let mut s = strip_inline_links(&out);
    for pat in ["**", "__", "`", "~~", "*", "_"] {
        s = s.replace(pat, "");
    }
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn strip_inline_links(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0usize;
    while i < b.len() {
        if b[i] == b'!' && b.get(i + 1) == Some(&b'[') {
            i += 1; // 跳过 ! 保留 [ 交给下面分支
            continue;
        }
        if b[i] == b'[' {
            if let Some(close) = s[i + 1..].find(']') {
                let text = &s[i + 1..i + 1 + close];
                let after = i + 1 + close + 1;
                if b.get(after) == Some(&b'(') {
                    if let Some(pc) = s[after + 1..].find(')') {
                        out.push_str(text);
                        i = after + 1 + pc + 1;
                        continue;
                    }
                }
                out.push_str(text);
                i = after;
                continue;
            }
        }
        let ch = s[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct ForumRow {
    id: i64,
    name: String,
    descr: Option<String>,
    topics: i64,
    posts: i64,
    #[sqlx(default)]
    latest_topic: Option<String>,
    #[sqlx(default)]
    latest_author: Option<String>,
    #[sqlx(default)]
    latest_at: Option<chrono::DateTime<chrono::Utc>>,
    /// 分区/节点（0115）：首页按分类分组
    #[sqlx(default)]
    category_id: Option<i64>,
    #[sqlx(default)]
    category_name: Option<String>,
    can_write: bool,
    can_create: bool,
    can_mod: bool,
}

#[derive(Deserialize)]
struct ForumSearchQuery {
    q: String,
}

/// 论坛搜索（NP 顶栏搜索搜帖口径，Phase3 增强）：标题 OR 正文纯文本（body_text，0115 落列）
/// 命中正文时带回摘要片段（关键词前后各 ~30 字符，前端高亮用）。
/// 权限：与列表同套谓词（minclassread / 版主），无权版块的内容搜不到。
#[get("/forums/search")]
async fn forum_search(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<ForumSearchQuery>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let kw = q.q.trim();
    if kw.len() < 2 {
        return Err(DomainError::Validation("关键字至少 2 个字符".into()));
    }
    // 转义 LIKE 通配符（与种子搜索同口径）
    let esc_kw = kw
        .replace("\\", "\\\\")
        .replace("%", "\\%")
        .replace("_", "\\_");
    let pattern = format!("%{esc_kw}%");
    // 每主题取「标题命中优先，否则正文最早命中」的楼层做片段；t.id 去重保一主题一行
    let rows: Vec<(i64, String, i64, String, Option<String>, chrono::DateTime<chrono::Utc>, i64, bool, Option<String>)> =
        sqlx::query_as(
            "SELECT t.id, t.title, f.id, f.name, u.username, t.created_at, \
                    (SELECT count(*) FROM posts p WHERE p.topic_id = t.id), t.locked, \
                    (SELECT left(p2.body_text, 90) FROM posts p2 WHERE p2.topic_id = t.id \
                      AND p2.body_text ILIKE $1 ORDER BY p2.id LIMIT 1) AS snippet \
             FROM topics t \
             JOIN forums f ON f.id = t.forum_id \
             LEFT JOIN users u ON u.id = t.user_id \
             WHERE (t.title ILIKE $1 OR EXISTS (SELECT 1 FROM posts p3 WHERE p3.topic_id = t.id AND p3.body_text ILIKE $1)) \
               AND (f.minclassread <= $2 OR EXISTS (SELECT 1 FROM forum_mods fm WHERE fm.forum_id = f.id AND fm.user_id = $3)) \
             ORDER BY t.id DESC LIMIT 30",
        )
        .bind(&pattern)
        .bind(auth.class_id)
        .bind(auth.id)
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let items: Vec<serde_json::Value> = rows
        .iter()
        .map(
            |(tid, title, fid, fname, author, at, replies, locked, snippet)| {
                serde_json::json!({
                    "topic_id": tid, "title": title, "forum_id": fid, "forum_name": fname,
                    "author": author, "created_at": at.to_rfc3339(),
                    "replies": replies, "locked": locked,
                    "snippet": snippet, "keyword": kw,
                })
            },
        )
        .collect();
    Ok(ok(items))
}

/// 论坛敏感词（Phase3）：词表存 site_settings.forum_banned_words（换行分隔，无新表），
/// 发主题/回帖/编辑时命中即 422 拒发。管理面走 settings 通用编辑（staff 已有权限模型）。
async fn forum_banned_words(db: &sqlx::PgPool) -> Vec<String> {
    let raw: Option<String> =
        sqlx::query_scalar("SELECT value FROM site_settings WHERE name = 'forum_banned_words'")
            .fetch_optional(db)
            .await
            .unwrap_or(None)
            .flatten();
    raw.unwrap_or_default()
        .split(['\n', '\r'])
        .map(str::trim)
        .filter(|s| s.len() >= 2)
        .map(str::to_string)
        .collect()
}

/// 命中检测：大小写不敏感（中文无大小写，天然不受影响）；返回第一个命中的词。
fn hit_banned_word<'a>(text: &str, words: &'a [String]) -> Option<&'a str> {
    let lower = text.to_lowercase();
    words
        .iter()
        .find(|w| lower.contains(&w.to_lowercase()))
        .map(String::as_str)
}

/// 发主题 / 回帖 / 编辑共用的敏感词闸门（Phase3 治理：先挡再落库，不做「发后删」）
async fn check_banned_words(db: &sqlx::PgPool, text: &str) -> DomainResult<()> {
    let words = forum_banned_words(db).await;
    if words.is_empty() {
        return Ok(());
    }
    if let Some(w) = hit_banned_word(text, &words) {
        return Err(DomainError::Validation(format!(
            "内容包含敏感词「{w}」，请修改后重发"
        )));
    }
    Ok(())
}

#[get("/forums")]
async fn forum_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    // 只列本版块门槛允许（minclassread）+ 版主兼任的版块；按分类 sort 分组排序（0115）
    let rows = sqlx::query_as::<_, (i64, String, Option<String>, i64, i64, Option<i64>, Option<String>)>(
        "SELECT f.id, f.name, f.descr, \
            (SELECT count(*) FROM topics t WHERE t.forum_id = f.id) AS topics, \
            (SELECT count(*) FROM posts p JOIN topics t ON t.id = p.topic_id WHERE t.forum_id = f.id) AS posts, \
            f.category_id, c.name AS category_name \
         FROM forums f LEFT JOIN forum_categories c ON c.id = f.category_id \
         WHERE f.minclassread <= $1 OR EXISTS (SELECT 1 FROM forum_mods fm WHERE fm.forum_id = f.id AND fm.user_id = $2) \
         ORDER BY c.sort NULLS LAST, f.id",
    )
    .bind(auth.class_id)
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    let mut latest: std::collections::HashMap<
        i64,
        (
            Option<String>,
            Option<String>,
            Option<chrono::DateTime<chrono::Utc>>,
        ),
    > = std::collections::HashMap::new();
    let latest_rows: Vec<(
        i64,
        Option<String>,
        Option<String>,
        Option<chrono::DateTime<chrono::Utc>>,
    )> = sqlx::query_as(
        "SELECT DISTINCT ON (t.forum_id) t.forum_id, t.title, u.username, t.created_at \
         FROM topics t LEFT JOIN users u ON u.id = t.user_id \
         WHERE t.forum_id = ANY(SELECT id FROM forums WHERE minclassread <= $1) \
         ORDER BY t.forum_id, t.id DESC",
    )
    .bind(auth.class_id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    for (fid, title, author, at) in latest_rows {
        latest.insert(fid, (title, author, at));
    }

    let mut out = Vec::with_capacity(rows.len());
    for (id, name, descr, topics, posts, category_id, category_name) in rows {
        let perm = forum_access(&state.repo.db, auth.id, auth.class_id, id).await?;
        let (latest_topic, latest_author, latest_at) =
            latest.get(&id).cloned().unwrap_or((None, None, None));
        out.push(ForumRow {
            id,
            name,
            descr,
            topics,
            posts,
            latest_topic,
            latest_author,
            latest_at,
            category_id,
            category_name,
            can_write: perm.can_write,
            can_create: perm.can_create,
            can_mod: perm.can_mod,
        });
    }
    Ok(ok(out))
}

#[derive(Deserialize)]
struct TopicCreateReq {
    forum_id: i64,
    title: String,
    body: String,
    /// 帖子类型（0115）：normal|bounty|poll|lottery，缺省 normal；本阶段仅落库，形态逻辑后续迁移
    #[serde(default)]
    topic_type: Option<String>,
    /// 论坛标签（0123）：tag_dict id 数组，最多 5 个，超出截断；禁用/不存在 id 静默丢弃
    #[serde(default)]
    tags: Vec<i32>,
    /// 悬赏金额（0124）：topic_type=bounty 时生效，>0 冻结；其余类型忽略（防借普通帖试探字段）
    #[serde(default)]
    bounty_spark: Option<i64>,
    /// 投票选项（0125）：topic_type=poll 时生效，2~10 项非空文本；其余类型忽略
    #[serde(default)]
    poll_options: Vec<String>,
    /// 抽奖参数（0126）：topic_type=lottery 时生效——winners 名额 × prize 魔力 + 票价 + 开奖时限（小时）
    #[serde(default)]
    lottery_winners: Option<i32>,
    #[serde(default)]
    lottery_prize: Option<i64>,
    #[serde(default)]
    lottery_ticket: Option<i64>,
    #[serde(default)]
    lottery_hours: Option<i32>,
}

/// 合法的帖子类型白名单（防止前端塞任意值）
fn normalize_topic_type(raw: Option<&str>) -> &'static str {
    match raw.map(|s| s.trim().to_ascii_lowercase()).as_deref() {
        Some("bounty") => "bounty",
        Some("poll") => "poll",
        Some("lottery") => "lottery",
        _ => "normal",
    }
}

// ---- 论坛通知（Phase1）：复用既有私信 `messages`，不新建通知表 ----
//
// 设计取舍：FluxTorrent 已有 `messages`（收件箱 location=1 / 自建文件夹 pmboxes /
// 未读筛选 / 顶栏 `unread_messages` 红点，见 user-box.tsx）与「勋章礼物 → 插私信」
// 的既有范式。论坛通知再建一张表 + 一套页面 + 一个红点，只会重复这套设施，
// 因此统一落 `messages`：`sender_id IS NULL` 即系统通知（区别于真人私信）。
// 好处：顶栏红点、收件箱、未读筛选、文件夹归类全部零改动生效。

/// 给用户发一条系统通知（不阻塞主流程：失败只丢通知，不能让回帖/点赞 500）。
async fn notify_user(db: &sqlx::PgPool, to: i64, subject: &str, body: &str) {
    if to <= 0 {
        return;
    }
    let _ = sqlx::query(
        "INSERT INTO messages (sender_id, receiver_id, subject, body) VALUES (NULL, $1, $2, $3)",
    )
    .bind(to)
    .bind(subject)
    .bind(body)
    .execute(db)
    .await;
}

/// 从 Markdown 正文里抽出 @提及 的用户名（与 forum-markdown.tsx 的 @ 正则同口径：
/// 字母/数字/下划线/连字符/中日韩汉字，2..=30 字符）。返回去重后的名字，最多 10 个。
fn extract_mentions(src: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0usize;
    while i < chars.len() {
        if chars[i] != '@' {
            i += 1;
            continue;
        }
        // @ 前一个字符若是字母/数字/汉字，说明是邮箱等场景，跳过
        if i > 0 {
            let prev = chars[i - 1];
            if prev.is_alphanumeric() || prev == '_' || prev == '-' {
                i += 1;
                continue;
            }
        }
        let mut j = i + 1;
        while j < chars.len() {
            let c = chars[j];
            if c.is_alphanumeric() || c == '_' || c == '-' {
                j += 1;
            } else {
                break;
            }
        }
        let name: String = chars[i + 1..j].iter().collect();
        let n = name.chars().count();
        if (2..=30).contains(&n) && !out.iter().any(|x| x.eq_ignore_ascii_case(&name)) {
            out.push(name);
        }
        i = j.max(i + 1);
    }
    out.truncate(10);
    out
}

/// 给「关注了 target_type/target_id」的所有人发系统通知，`exclude` 用于排除自己/已单独通知过的人。
/// 关注者可能很多，故批量一次查出来再逐条插；单条插入失败不影响其余（notify_user 本身 best-effort）。
async fn notify_followers(
    db: &sqlx::PgPool,
    target_type: &str,
    target_id: i64,
    subject: &str,
    body: &str,
    exclude: &[i64],
) {
    let rows: Vec<i64> = sqlx::query_scalar(
        "SELECT user_id FROM follows WHERE target_type = $1 AND target_id = $2 LIMIT 500",
    )
    .bind(target_type)
    .bind(target_id)
    .fetch_all(db)
    .await
    .unwrap_or_default();
    for uid in rows {
        if exclude.contains(&uid) {
            continue;
        }
        notify_user(db, uid, subject, body).await;
    }
}

/// 解析 @提及 到真实用户（精确匹配用户名，避免「张小」误伤「张小三」）。
/// `exclude` 用于排除自己（自己 @ 自己不发通知）。
async fn notify_mentions(
    db: &sqlx::PgPool,
    text: &str,
    from: i64,
    topic_id: i64,
    topic_title: &str,
    where_txt: &str,
    exclude: &[i64],
) {
    let names = extract_mentions(text);
    if names.is_empty() {
        return;
    }
    let rows: Vec<(i64, String)> =
        sqlx::query_as("SELECT id, username FROM users WHERE username = ANY($1) AND status < 2")
            .bind(&names)
            .fetch_all(db)
            .await
            .unwrap_or_default();
    for (uid, _uname) in rows {
        if uid == from || exclude.contains(&uid) {
            continue;
        }
        // 幂等：同一人对同一主题的重复提及只提醒一次。
        // 判重锚定在链接尾部的 `)`，避免 topic/12 误匹配 topic/123；
        // 用私信存在性判重，避免为通知单独加表/加列。
        let dup: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM messages WHERE receiver_id = $1 AND sender_id IS NULL \
             AND subject = '论坛提及' AND body LIKE $2)",
        )
        .bind(uid)
        .bind(format!("%/forums/topic/{})%", topic_id))
        .fetch_one(db)
        .await
        .unwrap_or(false);
        if dup {
            continue;
        }
        notify_user(
            db,
            uid,
            "论坛提及",
            &format!(
                "用户 #{} 在{}提到了你：[{}](/forums/topic/{})",
                from, where_txt, topic_title, topic_id
            ),
        )
        .await;
    }
}

// ---- 论坛标签（0123）：词表复用 tag_dict（带样式的通用标签字典），关联表 topic_tags ----

#[derive(sqlx::FromRow, serde::Serialize)]
struct TagChipRow {
    id: i32,
    name: String,
    /// 官方标签（仅 kind=official；论坛打标签不区分权限，仅样式语义）
    kind: String,
    /// 0138：论坛域标签（scope=forum），种子域标签不再进论坛
    #[sqlx(default)]
    scope: String,
    /// 以下为 0063 的样式列，前端 TagChip 直接吃
    #[sqlx(default)]
    bg_color: String,
    #[sqlx(default)]
    color: String,
    #[sqlx(default)]
    font_size: String,
    #[sqlx(default)]
    margin: String,
    #[sqlx(default)]
    padding: String,
    #[sqlx(default)]
    border_radius: String,
}

/// 论坛标签字典（发帖选择器 + TagChip 渲染样式源）：只出启用中的论坛域标签（0138 scope=forum）。
/// 公开读——种子页/版块页 SSR 要在登录前渲染标签筛选，与 /tags-dict（种子口径）同级的公开词表。
#[get("/forums/tags")]
async fn forum_tags_dict(
    _req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let rows: Vec<TagChipRow> = sqlx::query_as(
        "SELECT id, name, kind, scope, bg_color, color, font_size, margin, padding, border_radius \
         FROM tag_dict WHERE COALESCE(enabled, TRUE) AND scope = 'forum' ORDER BY sort, id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::to_value(rows).unwrap_or_default()))
}

/// 发帖选的标签写入 topic_tags：只收启用中的论坛域字典 id（0138 scope=forum；
/// 禁用/已删/种子域 id 不写，防私插），去重后逐条插入（PK 幂等，量级 ≤5 无需批量）。
async fn attach_topic_tags(
    tx: &mut sqlx::PgTransaction<'_>,
    topic_id: i64,
    tag_ids: &[i32],
) -> Result<(), sqlx::Error> {
    let mut seen = std::collections::HashSet::new();
    for tid in tag_ids.iter().take(5) {
        if !seen.insert(*tid) {
            continue;
        }
        let ok: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM tag_dict \
              WHERE id = $1 AND COALESCE(enabled, TRUE) AND scope = 'forum')",
        )
        .bind(*tid)
        .fetch_one(&mut **tx)
        .await?;
        if !ok {
            continue;
        }
        sqlx::query(
            "INSERT INTO topic_tags (topic_id, tag_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
        )
        .bind(topic_id)
        .bind(*tid)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

#[post("/forums/topics")]
async fn topic_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<TopicCreateReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if body.title.trim().is_empty() || body.body.trim().is_empty() {
        return Err(DomainError::Validation("标题与正文不能为空".into()));
    }
    // 敏感词（Phase3）：标题与正文一起过闸
    check_banned_words(&state.repo.db, &format!("{}\n{}", body.title, body.body)).await?;
    let perm = forum_access(&state.repo.db, auth.id, auth.class_id, body.forum_id).await?;
    if !perm.can_create {
        return Err(DomainError::Forbidden);
    }
    forum_flood_check(&state.repo.db, auth.id, auth.class_id).await?;
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let ttype = normalize_topic_type(body.topic_type.as_deref());
    // 悬赏（0124）：只有 bounty 类型认金额；1~1,000,000 钳位（0 = 发普通 bounty 帖不冻结，也合法）
    let bounty = if ttype == "bounty" {
        match body.bounty_spark {
            Some(b) if b < 0 => return Err(DomainError::Validation("悬赏金额不能为负".into())),
            Some(b) => b.min(1_000_000),
            None => 0,
        }
    } else {
        0
    };
    // 投票（0125）：poll 类型必须带 2~10 项非空选项；去重（同文本选项没意义）后定死
    let poll_options: Vec<String> = if ttype == "poll" {
        let mut seen = std::collections::HashSet::new();
        let mut opts: Vec<String> = body
            .poll_options
            .iter()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .filter(|s| seen.insert(s.clone()))
            .collect();
        opts.truncate(10);
        if opts.len() < 2 {
            return Err(DomainError::Validation(
                "投票帖至少需要 2 个非空选项".into(),
            ));
        }
        opts
    } else {
        Vec::new()
    };
    // 抽奖（0126）：lottery 类型参数钳位；奖金池 = winners × prize（发帖时整池冻结）
    let (lot_winners, lot_prize, lot_ticket, lot_hours) = if ttype == "lottery" {
        let w = body.lottery_winners.unwrap_or(1).clamp(1, 100);
        let p = body.lottery_prize.unwrap_or(0).clamp(0, 100_000);
        let t = body.lottery_ticket.unwrap_or(0).clamp(0, 10_000);
        let h = body.lottery_hours.unwrap_or(24).clamp(1, 720);
        if p <= 0 {
            return Err(DomainError::Validation(
                "抽奖帖必须设置每名中奖人的魔力数".into(),
            ));
        }
        (w, p, t, h)
    } else {
        (1, 0, 0, 24)
    };
    let topic_id: i64 = sqlx::query_scalar(
        "INSERT INTO topics (forum_id, user_id, title, topic_type, bounty_spark) VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(body.forum_id)
    .bind(auth.id)
    .bind(&body.title)
    .bind(ttype)
    .bind(bounty)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let body_text = strip_markdown(&body.body);
    sqlx::query("INSERT INTO posts (id, topic_id, user_id, body, body_text) VALUES (nextval('posts_id_seq'), $1, $2, $3, $4)")
        .bind(topic_id)
        .bind(auth.id)
        .bind(&body.body)
        .bind(&body_text)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query("UPDATE topics SET last_post_at = now() WHERE id = $1")
        .bind(topic_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 论坛标签（0123）：发帖时选的 tag_dict id（校验启用后才写，非法 id 静默丢弃不报错——
    // 发帖主流程不该被一个过期标签 id 卡死）
    if !body.tags.is_empty() {
        attach_topic_tags(&mut tx, topic_id, &body.tags)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    }
    // 悬赏冻结（0124）+ 抽奖奖金池冻结（0126）：与建主题同事务（对齐求种 req-bounty 范式）
    let frozen = bounty + lot_winners as i64 * lot_prize;
    if frozen > 0 {
        let idem = format!("forum-bounty:{}:{}", auth.id, topic_id);
        // 幂等重放闸门（#[must_use] 连审）：冻结与建主题同事务，重放即整体拒绝
        if !matches!(
            crate::economy_http::spend_spark_tx(
                &mut tx,
                auth.id,
                frozen,
                "forum_bounty",
                &idem,
                "forum_bounty",
                topic_id,
            )
            .await?,
            crate::economy_http::SpendOutcome::Spent
        ) {
            return Err(DomainError::Validation(
                "该笔请求已受理，请勿重复提交".into(),
            ));
        }
    }
    // 投票选项（0125）：与主题同事务落一行（选项发帖时定死，之后不可增删）
    if !poll_options.is_empty() {
        sqlx::query("INSERT INTO topic_polls (topic_id, options) VALUES ($1, $2)")
            .bind(topic_id)
            .bind(serde_json::json!(poll_options))
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    }
    // 抽奖参数（0126）：同事务落一行；draw_at 到点由 worker 扫描开奖（或楼主提前手动开）
    if ttype == "lottery" {
        sqlx::query(
            "INSERT INTO topic_lotteries (topic_id, winners, prize_per_winner, ticket_spark, draw_at) \
             VALUES ($1, $2, $3, $4, now() + make_interval(hours => $5))",
        )
        .bind(topic_id)
        .bind(lot_winners)
        .bind(lot_prize)
        .bind(lot_ticket)
        .bind(lot_hours)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    forum_flood_mark(&state.repo.db, auth.id).await;
    // 发主题 +2 火花（M11 论坛行为奖励）
    let idem = format!("forum-topic:{}:{}", auth.id, topic_id);
    earn_spark(&state.repo.db, auth.id, 2, "forum", &idem).await?;
    // 楼主的首帖里 @ 了谁就通知谁
    notify_mentions(
        &state.repo.db,
        &body.body,
        auth.id,
        topic_id,
        &body.title,
        "主题中",
        &[],
    )
    .await;
    // 关注了作者的人：他发新主题时收到通知（关注版块只进关注流，不发通知，见 0121 注释）
    notify_followers(
        &state.repo.db,
        "user",
        auth.id,
        "关注的人发了新主题",
        &format!(
            "[{}](/forums/topic/{}) · 来自用户 #{}",
            body.title, topic_id, auth.id
        ),
        &[],
    )
    .await;
    Ok(ok(
        serde_json::json!({ "topic_id": topic_id, "bounty_frozen": bounty }),
    ))
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct TopicRow {
    id: i64,
    forum_id: i64,
    title: String,
    username: Option<String>,
    replies: i64,
    views: i32,
    last_post_at: Option<chrono::DateTime<chrono::Utc>>,
    sticky: bool,
    locked: bool,
    /// 精华帖（0099，NP digest 口径）
    #[sqlx(default)]
    digest: bool,
    /// 帖子类型（0115）：normal|bounty|poll|lottery
    #[sqlx(default)]
    topic_type: String,
    /// 已读到的楼层 post_id（0078 已读跟踪；NULL=从未读过）
    read_last_post_id: Option<i64>,
    /// 是否有未读新回复（last_post 晚于已读位置）
    #[serde(default)]
    has_unread: bool,
    /// 标签（0123）：json 数组 [{id,name,kind,样式列}]，tag_dict 全量样式随行回
    #[sqlx(default)]
    tags: serde_json::Value,
}

#[derive(Deserialize)]
struct TopicListQuery {
    /// 排序：hot=热度（回复*2 + 点赞 + 浏览*0.1，按距最后回复的小时数衰减）/ 缺省 new
    #[serde(default)]
    sort: Option<String>,
    /// 按标签筛选（0123）：tag_dict id，只保留带该标签的主题。0138：非论坛域
    /// 标签 id（种子域/不存在）直接视为无此筛选——回显空但不算非法，回落全量。
    /// actix Query 对 i32 的反序列化失败会直接 400，故先收字符串再自行解析，
    /// 非数字（爬虫乱造的 URL）静默回落全量，不拿 400 打断正常浏览。
    #[serde(default)]
    tag: Option<String>,
}

#[get("/forums/{id}/topics")]
async fn topic_list(
    req: HttpRequest,
    path: web::Path<i64>,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<TopicListQuery>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let fid = path.into_inner();
    let perm = forum_access(&state.repo.db, auth.id, auth.class_id, fid).await?;
    if !perm.can_read {
        return Err(DomainError::Forbidden);
    }
    // ORDER BY 片段是白名单常量（不含用户输入），不可注入；
    // 回显值同样取白名单结果，不把原始输入透传给前端。
    let sort = if q.sort.as_deref() == Some("hot") {
        "hot"
    } else {
        "new"
    };
    let order = if sort == "hot" {
        "t.sticky DESC, \
         ((SELECT count(*)-1 FROM posts p WHERE p.topic_id = t.id) * 2 \
           + (SELECT count(*) FROM post_likes pl WHERE pl.topic_id = t.id) \
           + t.views * 0.1) \
          / (1 + EXTRACT(EPOCH FROM (now() - COALESCE(t.last_post_at, t.created_at))) / 86400.0) DESC, t.id DESC"
    } else {
        "t.sticky DESC, t.id DESC"
    };
    // 标签筛选（0123）：EXISTS 谓词拼接（tag 解析成 i32 后格式化，无注入面），参数位次随谓词平移
    let tag_id = q
        .tag
        .as_deref()
        .and_then(|s| s.trim().parse::<i32>().ok())
        .filter(|v| *v > 0);
    let tag_filter = tag_id
        .map(|tg| format!(" AND EXISTS(SELECT 1 FROM topic_tags tt WHERE tt.topic_id = t.id AND tt.tag_id = {tg})"))
        .unwrap_or_default();
    let sql = format!(
        "SELECT t.id, t.forum_id, t.title, u.username, \
            (SELECT count(*)-1 FROM posts p WHERE p.topic_id = t.id) AS replies, t.views, t.last_post_at, \
            t.sticky, t.locked, t.digest, t.topic_type, \
            (SELECT r.last_post_id FROM topic_reads r WHERE r.user_id = $2 AND r.topic_id = t.id) AS read_last_post_id, \
            EXISTS(SELECT 1 FROM posts p2 WHERE p2.topic_id = t.id \
                   AND p2.id > COALESCE((SELECT r2.last_post_id FROM topic_reads r2 \
                                         WHERE r2.user_id = $2 AND r2.topic_id = t.id), 0)) AS has_unread, \
            COALESCE( \
              (SELECT json_agg(json_build_object('id', d.id, 'name', d.name, 'kind', d.kind, \
                     'bg_color', d.bg_color, 'color', d.color, 'font_size', d.font_size, \
                     'margin', d.margin, 'padding', d.padding, 'border_radius', d.border_radius)) \
                 FROM (SELECT td.* FROM tag_dict td JOIN topic_tags tt2 ON tt2.tag_id = td.id \
                        WHERE tt2.topic_id = t.id AND td.scope = 'forum' ORDER BY td.sort, td.id) d), \
              '[]'::json) AS tags \
         FROM topics t LEFT JOIN users u ON u.id = t.user_id \
         WHERE t.forum_id = $1{tag_filter} ORDER BY {order} LIMIT 50"
    );
    let rows = sqlx::query_as::<_, TopicRow>(&sql)
        .bind(fid)
        .bind(auth.id)
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 版块名随列表一起回（避免前端再打一次 /forums 只为拿标题）
    let forum_name: Option<String> = sqlx::query_scalar("SELECT name FROM forums WHERE id = $1")
        .bind(fid)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "forum_id": fid,
        "forum_name": forum_name,
        "can_write": perm.can_write,
        "can_create": perm.can_create,
        "can_mod": perm.can_mod,
        "sort": sort,
        "tag": tag_id,
        "topics": rows,
    })))
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct PostRow {
    id: i64,
    username: Option<String>,
    user_id: Option<i64>,
    body: String,
    created_at: chrono::DateTime<chrono::Utc>,
    edited_at: Option<chrono::DateTime<chrono::Utc>>,
    edited_by: Option<i64>,
    /// 点赞数（0116）
    #[sqlx(default)]
    likes: i64,
    /// 当前用户是否已点赞（0116）
    #[sqlx(default)]
    liked_by_me: bool,
    /// 打赏总额（0127）：该楼收到的魔力总和（次数用 tips 单独计数）
    #[sqlx(default)]
    tips: i64,
    /// 打赏次数（0127）
    #[sqlx(default)]
    tip_count: i64,
    #[serde(skip)]
    hidden: bool,
}

#[derive(Deserialize)]
struct TopicDetailQuery {
    /// 加载更早楼层：返回 id < before 的最后 200 楼（长帖游标翻页，NP 分页口径）
    #[serde(default)]
    before: Option<i64>,
}

#[get("/forums/topics/{id}")]
async fn topic_detail(
    req: HttpRequest,
    path: web::Path<i64>,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<TopicDetailQuery>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let tid = path.into_inner();
    sqlx::query("UPDATE topics SET views = views + 1 WHERE id = $1")
        .bind(tid)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // NexusPHP 帖子页头部：主题标题 + 所属版块（找不到主题时 404）
    let meta: Option<(
        String,
        i64,
        Option<String>,
        Option<i64>,
        bool,
        bool,
        bool,
        String,
        i64,
        String,
        Option<i64>,
    )> = sqlx::query_as(
        "SELECT t.title, f.id, f.name, t.user_id, t.sticky, t.locked, t.digest, t.topic_type, \
                t.bounty_spark, t.bounty_status, t.bounty_post_id \
         FROM topics t LEFT JOIN forums f ON f.id = t.forum_id WHERE t.id = $1",
    )
    .bind(tid)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((
        title,
        fid,
        forum_name,
        op_id,
        sticky,
        locked,
        digest,
        topic_type,
        bounty_spark,
        bounty_status,
        bounty_post_id,
    )) = meta
    else {
        return Err(DomainError::NotFound(tid));
    };
    let perm = forum_access(&state.repo.db, auth.id, auth.class_id, fid).await?;
    if !perm.can_read {
        return Err(DomainError::Forbidden);
    }
    let mut posts = sqlx::query_as::<_, PostRow>(
        "SELECT p.id, u.username, p.user_id, p.body, p.created_at, p.edited_at, p.edited_by, FALSE AS hidden, \
            (SELECT count(*) FROM post_likes pl WHERE pl.post_id = p.id) AS likes, \
            EXISTS(SELECT 1 FROM post_likes pl2 WHERE pl2.post_id = p.id AND pl2.user_id = $3) AS liked_by_me, \
            COALESCE((SELECT sum(t.spark) FROM post_tips t WHERE t.post_id = p.id), 0)::bigint AS tips, \
            (SELECT count(*) FROM post_tips t2 WHERE t2.post_id = p.id) AS tip_count \
         FROM posts p LEFT JOIN users u ON u.id = p.user_id \
         WHERE p.topic_id = $1 AND ($2::bigint IS NULL OR p.id < $2)          ORDER BY p.id DESC LIMIT 200",
    )
    .bind(tid)
    .bind(q.before)
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 游标取的是「最新 200 楼倒序」，回正为升序展示（无游标时同样无副作用）
    posts.reverse();
    // 受保护版块：2 楼起正文替换为提示（class≥90 / 发帖人本人 / 楼主 / 本版版主放行）
    let protected: bool = sqlx::query_scalar("SELECT protected FROM forums WHERE id = $1")
        .bind(fid)
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or(false);
    if protected {
        let is_op = op_id == Some(auth.id);
        for (i, p) in posts.iter_mut().enumerate() {
            let is_self = p.user_id == Some(auth.id);
            if i > 0 && !is_self && !is_op && !perm.can_mod {
                p.hidden = true;
                p.body = "……".to_string();
            }
        }
    }
    // 论坛已读（0078，NP readposts 口径）：读到哪楼记哪楼（最大已见 post_id），
    // 列表页据此算「有新回复」角标。read_at 顺带刷新，供排序。
    if let Some(last_pid) = posts.last().map(|p| p.id) {
        sqlx::query(
            "INSERT INTO topic_reads (user_id, topic_id, last_post_id, read_at) \
             VALUES ($1, $2, $3, now()) \
             ON CONFLICT (user_id, topic_id) DO UPDATE \
             SET last_post_id = GREATEST(topic_reads.last_post_id, EXCLUDED.last_post_id), \
                 read_at = now()",
        )
        .bind(auth.id)
        .bind(tid)
        .bind(last_pid)
        .execute(&state.repo.db)
        .await
        .ok();
    }
    // 收藏态（0116）：主题收藏总数 + 当前用户是否已收藏
    let favorites: i64 =
        sqlx::query_scalar("SELECT count(*) FROM topic_favorites WHERE topic_id = $1")
            .bind(tid)
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or(0);
    let faved: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM topic_favorites WHERE topic_id = $1 AND user_id = $2)",
    )
    .bind(tid)
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    // 标签（0123）：详情页头部 TagChip 渲染（样式列与列表同源 tag_dict）
    let tags: serde_json::Value = sqlx::query_scalar(
        "SELECT COALESCE( \
            (SELECT json_agg(json_build_object('id', d.id, 'name', d.name, 'kind', d.kind, \
                   'bg_color', d.bg_color, 'color', d.color, 'font_size', d.font_size, \
                   'margin', d.margin, 'padding', d.padding, 'border_radius', d.border_radius)) \
               FROM (SELECT td.* FROM tag_dict td JOIN topic_tags tt ON tt.tag_id = td.id \
                      WHERE tt.topic_id = $1 AND td.scope = 'forum' ORDER BY td.sort, td.id) d), \
            '[]'::json)",
    )
    .bind(tid)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(serde_json::json!([]));
    // 投票（0125）：选项 + 截止态 + 我的票 + 每项计数（未投也回计数——论坛投票结果公开是常态）
    let poll: serde_json::Value = sqlx::query_scalar(
        "SELECT json_build_object('options', tp.options, 'closed', tp.closed, \
                'my_vote', (SELECT v.option_index FROM poll_votes v \
                             WHERE v.topic_id = tp.topic_id AND v.user_id = $2), \
                'total', (SELECT count(*) FROM poll_votes v WHERE v.topic_id = tp.topic_id), \
                'counts', (SELECT COALESCE(json_agg(json_build_object('index', c.idx, 'votes', c.n)), '[]'::json) \
                             FROM (SELECT option_index AS idx, count(*) AS n FROM poll_votes \
                                    WHERE topic_id = tp.topic_id GROUP BY option_index) c)) \
         FROM topic_polls tp WHERE tp.topic_id = $1",
    )
    .bind(tid)
    .bind(auth.id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .unwrap_or(serde_json::Value::Null);
    // 抽奖（0126）：详情回 lottery{winners,prize,ticket,status,draw_at,entries,won_count,joined,my_won,winners_list}
    let lottery: serde_json::Value = sqlx::query_scalar(
        "SELECT json_build_object('winners', tl.winners, 'prize', tl.prize_per_winner, \
                'ticket', tl.ticket_spark, 'status', tl.status, 'draw_at', tl.draw_at, \
                'entries', (SELECT count(*) FROM lottery_entries e WHERE e.topic_id = tl.topic_id), \
                'joined', EXISTS(SELECT 1 FROM lottery_entries e2 WHERE e2.topic_id = tl.topic_id AND e2.user_id = $2), \
                'my_won', COALESCE((SELECT e3.won FROM lottery_entries e3 WHERE e3.topic_id = tl.topic_id AND e3.user_id = $2), FALSE), \
                'winner_ids', (SELECT COALESCE(json_agg(json_build_object('id', w.user_id, 'name', u.username)), '[]'::json) \
                                 FROM (SELECT e4.user_id FROM lottery_entries e4 \
                                        WHERE e4.topic_id = tl.topic_id AND e4.won ORDER BY e4.user_id) w \
                                 LEFT JOIN users u ON u.id = w.user_id)) \
         FROM topic_lotteries tl WHERE tl.topic_id = $1",
    )
    .bind(tid)
    .bind(auth.id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .unwrap_or(serde_json::Value::Null);
    Ok(ok(serde_json::json!({
        "topic_id": tid,
        "title": title,
        "forum_id": fid,
        "forum_name": forum_name,
        "sticky": sticky,
        "locked": locked,
        "digest": digest,
        "topic_type": topic_type,
        "bounty_spark": bounty_spark,
        "bounty_status": bounty_status,
        "bounty_post_id": bounty_post_id,
        "tags": tags,
        "poll": poll,
        "lottery": lottery,
        "favorites": favorites,
        "faved": faved,
        "is_op": op_id == Some(auth.id),
        "current_user_id": auth.id,
        "can_write": perm.can_write,
        "can_mod": perm.can_mod,
        "posts": posts,
        // 长帖游标（NP 分页口径）：本窗口之外还有更早楼层时前端显示「加载更早的回复」
        "has_more": posts.first().map(|p| p.id > 1).unwrap_or(false)
            && sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(SELECT 1 FROM posts WHERE topic_id = $1 AND id < $2)",
            )
            .bind(tid)
            .bind(posts.first().map(|p| p.id).unwrap_or(0))
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or(false),
    })))
}

#[derive(Deserialize)]
struct ReplyReq {
    body: String,
}

#[post("/forums/topics/{id}/reply")]
async fn post_reply(
    req: HttpRequest,
    path: web::Path<i64>,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ReplyReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let tid = path.into_inner();
    if body.body.trim().is_empty() {
        return Err(DomainError::Validation("回复不能为空".into()));
    }
    // 敏感词（Phase3）
    check_banned_words(&state.repo.db, &body.body).await?;
    let row: Option<(i64, bool, i64, String)> = sqlx::query_as(
        "SELECT forum_id, locked, COALESCE(user_id, 0), title FROM topics WHERE id = $1",
    )
    .bind(tid)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((fid, locked, author, title)) = row else {
        return Err(DomainError::NotFound(tid));
    };
    let perm = forum_access(&state.repo.db, auth.id, auth.class_id, fid).await?;
    if !perm.can_write {
        return Err(DomainError::Forbidden);
    }
    if locked && !perm.can_mod {
        return Err(DomainError::Validation("主题已锁定".into()));
    }
    forum_flood_check(&state.repo.db, auth.id, auth.class_id).await?;
    let body_text = strip_markdown(&body.body);
    let post_id: i64 = sqlx::query_scalar(
        "INSERT INTO posts (id, topic_id, user_id, body, body_text) VALUES (nextval('posts_id_seq'), $1, $2, $3, $4) RETURNING id",
    )
    .bind(tid)
    .bind(auth.id)
    .bind(&body.body)
    .bind(&body_text)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query("UPDATE topics SET last_post_at = now() WHERE id = $1")
        .bind(tid)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    forum_flood_mark(&state.repo.db, auth.id).await;
    // 回帖 +1 火花
    let idem = format!("forum-reply:{}:{}", auth.id, post_id);
    earn_spark(&state.repo.db, auth.id, 1, "forum", &idem).await?;
    // 通知楼主有新回复（自己回自己不通知）；@提及 排除楼主，避免同一件事两条通知
    if author != auth.id && author != 0 {
        notify_user(
            &state.repo.db,
            author,
            "主题收到回复",
            &format!(
                "用户 #{} 回复了你的主题：[{}](/forums/topic/{})",
                auth.id, title, tid
            ),
        )
        .await;
    }
    notify_mentions(
        &state.repo.db,
        &body.body,
        auth.id,
        tid,
        &title,
        "回复中",
        &[author],
    )
    .await;
    // 关注了本主题的人：有新回复时通知（楼主已由上面「主题收到回复」通知过，回帖者自己更不必通知）
    notify_followers(
        &state.repo.db,
        "topic",
        tid,
        "关注的主题有新回复",
        &format!("[{}](/forums/topic/{}) · 来自用户 #{}", title, tid, auth.id),
        &[author, auth.id],
    )
    .await;
    Ok(ok(serde_json::json!({ "post_id": post_id })))
}

// ---- 论坛互动：点赞 / 收藏（0116） ----

/// 点赞/取消点赞共用的落库逻辑。
/// 幂等：post_likes 主键 (user_id, post_id) + ON CONFLICT DO NOTHING。
/// 火花：仅「非本人作者 + 本次真发生变化 + 点赞方向」时给作者 +1（幂等键 forum-like:{pid}:{fan}），
/// 取消点赞不回收 —— 因为幂等键去重已堵住 like/unlike 循环刷分，回收反而会算错。
async fn set_post_like(
    state: &web::Data<std::sync::Arc<AppState>>,
    auth: &crate::http::AuthUser,
    pid: i64,
    on: bool,
) -> DomainResult<HttpResponse> {
    let row: Option<(i64, i64, i64, String)> = sqlx::query_as(
        "SELECT p.topic_id, t.forum_id, COALESCE(p.user_id, 0), t.title \
         FROM posts p JOIN topics t ON t.id = p.topic_id WHERE p.id = $1",
    )
    .bind(pid)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((tid, fid, author, title)) = row else {
        return Err(DomainError::NotFound(pid));
    };
    let perm = forum_access(&state.repo.db, auth.id, auth.class_id, fid).await?;
    if !perm.can_read {
        return Err(DomainError::Forbidden);
    }
    let changed = if on {
        sqlx::query(
            "INSERT INTO post_likes (user_id, post_id, topic_id) VALUES ($1, $2, $3) \
             ON CONFLICT (user_id, post_id) DO NOTHING",
        )
        .bind(auth.id)
        .bind(pid)
        .bind(tid)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected()
            > 0
    } else {
        sqlx::query("DELETE FROM post_likes WHERE user_id = $1 AND post_id = $2")
            .bind(auth.id)
            .bind(pid)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .rows_affected()
            > 0
    };
    if on && changed && author != auth.id && author != 0 {
        let idem = format!("forum-like:{}:{}", pid, auth.id);
        // 通知与火花共用同一幂等键：`changed=true` 只保证「本次真的写入了点赞行」，
        // 但「取消后再点赞」同样会 changed=true。若不按幂等键判重，
        // like/unlike 循环就能反复给作者刷通知（火花侧已有幂等键，通知侧漏了）。
        let already: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM spark_ledger WHERE idempotency_key = $1)",
        )
        .bind(&idem)
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or(false);
        let _ = earn_spark(&state.repo.db, author, 1, "forum-like", &idem).await;
        if !already {
            notify_user(
                &state.repo.db,
                author,
                "帖子被点赞",
                &format!(
                    "用户 #{} 赞了你在主题「{}」里的回复：[查看](/forums/topic/{})",
                    auth.id, title, tid
                ),
            )
            .await;
        }
    }
    let likes: i64 = sqlx::query_scalar("SELECT count(*) FROM post_likes WHERE post_id = $1")
        .bind(pid)
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or(0);
    Ok(ok(serde_json::json!({
        "post_id": pid,
        "liked": on,
        "likes": likes,
        "changed": changed,
    })))
}

#[post("/forums/posts/{id}/like")]
async fn post_like(
    req: HttpRequest,
    path: web::Path<i64>,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let pid = path.into_inner();
    set_post_like(&state, &auth, pid, true).await
}

#[delete("/forums/posts/{id}/like")]
async fn post_unlike(
    req: HttpRequest,
    path: web::Path<i64>,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let pid = path.into_inner();
    set_post_like(&state, &auth, pid, false).await
}

/// 收藏/取消收藏主题共用的落库逻辑（幂等，主键 (user_id, topic_id)）
async fn set_topic_favorite(
    state: &web::Data<std::sync::Arc<AppState>>,
    auth: &crate::http::AuthUser,
    tid: i64,
    on: bool,
) -> DomainResult<HttpResponse> {
    let fid: Option<i64> = sqlx::query_scalar("SELECT forum_id FROM topics WHERE id = $1")
        .bind(tid)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(fid) = fid else {
        return Err(DomainError::NotFound(tid));
    };
    let perm = forum_access(&state.repo.db, auth.id, auth.class_id, fid).await?;
    if !perm.can_read {
        return Err(DomainError::Forbidden);
    }
    if on {
        sqlx::query(
            "INSERT INTO topic_favorites (user_id, topic_id) VALUES ($1, $2) \
             ON CONFLICT (user_id, topic_id) DO NOTHING",
        )
        .bind(auth.id)
        .bind(tid)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    } else {
        sqlx::query("DELETE FROM topic_favorites WHERE user_id = $1 AND topic_id = $2")
            .bind(auth.id)
            .bind(tid)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    }
    let favorites: i64 =
        sqlx::query_scalar("SELECT count(*) FROM topic_favorites WHERE topic_id = $1")
            .bind(tid)
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or(0);
    Ok(ok(serde_json::json!({
        "topic_id": tid,
        "faved": on,
        "favorites": favorites,
    })))
}

#[post("/forums/topics/{id}/favorite")]
async fn topic_favorite(
    req: HttpRequest,
    path: web::Path<i64>,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let tid = path.into_inner();
    set_topic_favorite(&state, &auth, tid, true).await
}

#[delete("/forums/topics/{id}/favorite")]
async fn topic_unfavorite(
    req: HttpRequest,
    path: web::Path<i64>,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let tid = path.into_inner();
    set_topic_favorite(&state, &auth, tid, false).await
}

// ---- 论坛关注订阅（0121）：用户 / 版块 / 主题 ----
//
// 语义分工（刻意的）：
//   · 关注**用户** → 他发新主题时给关注者发一条通知（关注某人是明确意图，通知不算打扰）；
//   · 关注**版块** → 只进「关注流」，不发通知（版块流量大，逐个通知必成刷屏）；
//   · 关注**主题** → 有人回帖时给关注者发通知（等价于「订阅该主题」，这才是关注主题的意义）。
// 通知复用既有 `messages`（sender_id IS NULL = 系统通知），不新建通知表。

/// 关注对象白名单，防前端塞任意 type
fn normalize_follow_type(raw: &str) -> Option<&'static str> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "user" => Some("user"),
        "forum" => Some("forum"),
        "topic" => Some("topic"),
        _ => None,
    }
}

#[derive(Deserialize)]
struct FollowReq {
    target_type: String,
    target_id: i64,
}

/// 校验关注目标存在 + 当前用户对其有可见权限；返回规范化后的 type。
/// 自关注被拒（否则自己的新主题会通知自己）。
async fn validate_follow_target(
    db: &sqlx::PgPool,
    auth: &crate::http::AuthUser,
    ttype: &str,
    tid: i64,
) -> DomainResult<()> {
    match ttype {
        "user" => {
            if tid == auth.id {
                return Err(DomainError::Validation("不能关注自己".into()));
            }
            let ok: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM users WHERE id = $1 AND status < 2)",
            )
            .bind(tid)
            .fetch_one(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            if !ok {
                return Err(DomainError::NotFound(tid));
            }
        }
        "forum" => {
            let perm = forum_access(db, auth.id, auth.class_id, tid).await?;
            if !perm.can_read {
                return Err(DomainError::NotFound(tid));
            }
        }
        "topic" => {
            let fid: Option<i64> = sqlx::query_scalar("SELECT forum_id FROM topics WHERE id = $1")
                .bind(tid)
                .fetch_optional(db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
            let Some(fid) = fid else {
                return Err(DomainError::NotFound(tid));
            };
            let perm = forum_access(db, auth.id, auth.class_id, fid).await?;
            if !perm.can_read {
                return Err(DomainError::NotFound(tid));
            }
        }
        _ => return Err(DomainError::Validation("非法的关注对象".into())),
    }
    Ok(())
}

#[post("/follows")]
async fn follow_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<FollowReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let ttype = normalize_follow_type(&body.target_type)
        .ok_or_else(|| DomainError::Validation("非法的关注对象".into()))?;
    validate_follow_target(&state.repo.db, &auth, ttype, body.target_id).await?;
    sqlx::query(
        "INSERT INTO follows (user_id, target_type, target_id) VALUES ($1, $2, $3) \
         ON CONFLICT (user_id, target_type, target_id) DO NOTHING",
    )
    .bind(auth.id)
    .bind(ttype)
    .bind(body.target_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let followers: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM follows WHERE target_type = $1 AND target_id = $2",
    )
    .bind(ttype)
    .bind(body.target_id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    Ok(ok(serde_json::json!({
        "target_type": ttype,
        "target_id": body.target_id,
        "following": true,
        "followers": followers,
    })))
}

#[delete("/follows/{ttype}/{tid}")]
async fn follow_delete(
    req: HttpRequest,
    path: web::Path<(String, i64)>,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let (raw_type, tid) = path.into_inner();
    let ttype = normalize_follow_type(&raw_type)
        .ok_or_else(|| DomainError::Validation("非法的关注对象".into()))?;
    sqlx::query("DELETE FROM follows WHERE user_id = $1 AND target_type = $2 AND target_id = $3")
        .bind(auth.id)
        .bind(ttype)
        .bind(tid)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let followers: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM follows WHERE target_type = $1 AND target_id = $2",
    )
    .bind(ttype)
    .bind(tid)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    Ok(ok(serde_json::json!({
        "target_type": ttype,
        "target_id": tid,
        "following": false,
        "followers": followers,
    })))
}

#[derive(Deserialize)]
struct FollowStatusQuery {
    target_type: String,
    target_id: i64,
}

/// 关注状态。用户页无当前登录者信息，故 `is_self` 也在这里回给前端，由组件决定隐藏按钮。
#[get("/follows/status")]
async fn follow_status(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<FollowStatusQuery>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let Some(ttype) = normalize_follow_type(&q.target_type) else {
        return Err(DomainError::Validation("非法的关注对象".into()));
    };
    let following: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM follows WHERE user_id = $1 AND target_type = $2 AND target_id = $3)",
    )
    .bind(auth.id)
    .bind(ttype)
    .bind(q.target_id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    let followers: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM follows WHERE target_type = $1 AND target_id = $2",
    )
    .bind(ttype)
    .bind(q.target_id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    Ok(ok(serde_json::json!({
        "following": following,
        "followers": followers,
        "is_self": ttype == "user" && q.target_id == auth.id,
    })))
}

#[derive(Deserialize)]
struct FollowMineQuery {
    #[serde(default)]
    target_type: Option<String>,
}

/// 我的关注列表。只关注用户时返回 username，关注版块返回 name，关注主题返回 title——
/// 一次 JOIN 三张表反而更绕，改成按 type 分别取再合并（数量级很小）。
#[get("/follows/mine")]
async fn follow_mine(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<FollowMineQuery>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let want = match q.target_type.as_deref() {
        Some(s) => Some(
            normalize_follow_type(s)
                .ok_or_else(|| DomainError::Validation("非法的关注对象".into()))?,
        ),
        None => None,
    };
    let mut users: Vec<serde_json::Value> = Vec::new();
    let mut forums: Vec<serde_json::Value> = Vec::new();
    let mut topics: Vec<serde_json::Value> = Vec::new();

    if want.is_none() || want == Some("user") {
        let rows: Vec<(i64, String)> = sqlx::query_as(
            "SELECT u.id, u.username FROM follows f JOIN users u ON u.id = f.target_id \
             WHERE f.user_id = $1 AND f.target_type = 'user' ORDER BY f.created_at DESC LIMIT 200",
        )
        .bind(auth.id)
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        users = rows
            .into_iter()
            .map(|(id, username)| serde_json::json!({ "id": id, "name": username }))
            .collect();
    }
    if want.is_none() || want == Some("forum") {
        let rows: Vec<(i64, String)> = sqlx::query_as(
            "SELECT fc.id, fc.name FROM follows f JOIN forums fc ON fc.id = f.target_id \
             WHERE f.user_id = $1 AND f.target_type = 'forum' ORDER BY f.created_at DESC LIMIT 200",
        )
        .bind(auth.id)
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        forums = rows
            .into_iter()
            .map(|(id, name)| serde_json::json!({ "id": id, "name": name }))
            .collect();
    }
    if want.is_none() || want == Some("topic") {
        let rows: Vec<(i64, String, i64)> = sqlx::query_as(
            "SELECT t.id, t.title, t.forum_id FROM follows f JOIN topics t ON t.id = f.target_id \
             WHERE f.user_id = $1 AND f.target_type = 'topic' ORDER BY f.created_at DESC LIMIT 200",
        )
        .bind(auth.id)
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        topics = rows
            .into_iter()
            .map(|(id, title, forum_id)| serde_json::json!({ "id": id, "name": title, "forum_id": forum_id }))
            .collect();
    }
    Ok(ok(serde_json::json!({
        "users": users, "forums": forums, "topics": topics,
    })))
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct FeedRow {
    topic_id: i64,
    title: String,
    forum_id: i64,
    forum_name: Option<String>,
    username: Option<String>,
    #[sqlx(default)]
    topic_type: String,
    last_post_at: Option<chrono::DateTime<chrono::Utc>>,
    created_at: chrono::DateTime<chrono::Utc>,
    #[sqlx(default)]
    sticky: bool,
    #[sqlx(default)]
    locked: bool,
    replies: i64,
    /// 命中的关注来源：forum=关注版块 / user=关注作者 / topic=关注主题
    via: String,
}

#[derive(Deserialize)]
struct FeedQuery {
    #[serde(default)]
    limit: Option<i64>,
    /// 游标分页（0123）：只取 activity < before 的行；activity = COALESCE(last_post_at, created_at)
    #[serde(default)]
    before: Option<chrono::DateTime<chrono::Utc>>,
}

/// 关注流：我关注的版块/用户的新主题 + 我关注主题的最新回复，按最后活动时间倒序。
/// 权限：逐行复刻 forum_access 的 can_read（`class_id>=90` 放行 / 版主放行 / `class_id>=minclassread`），
/// 否则用户降级后会在流里看到已无权限版块的标题（越权泄露）。
#[get("/forums/feed")]
async fn forum_feed(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<FeedQuery>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let limit = q.limit.unwrap_or(50).clamp(1, 100);
    // 多取一些（UNION 可能同一主题命中多路），去重后再截到 limit
    let fetch = (limit * 2).min(200);
    let rows: Vec<FeedRow> = sqlx::query_as(
        "SELECT * FROM ( \
           SELECT t.id AS topic_id, t.title, t.forum_id, fc.name AS forum_name, u.username, \
                  t.topic_type, t.last_post_at, t.created_at, t.sticky, t.locked, \
                  (SELECT count(*)-1 FROM posts p WHERE p.topic_id = t.id) AS replies, \
                  'forum' AS via \
             FROM follows fl \
             JOIN topics t ON t.forum_id = fl.target_id \
             JOIN forums fc ON fc.id = t.forum_id \
             LEFT JOIN users u ON u.id = t.user_id \
            WHERE fl.user_id = $1 AND fl.target_type = 'forum' \
           UNION ALL \
           SELECT t.id, t.title, t.forum_id, fc.name, u.username, \
                  t.topic_type, t.last_post_at, t.created_at, t.sticky, t.locked, \
                  (SELECT count(*)-1 FROM posts p WHERE p.topic_id = t.id), \
                  'user' \
             FROM follows fl \
             JOIN topics t ON t.user_id = fl.target_id \
             JOIN forums fc ON fc.id = t.forum_id \
             LEFT JOIN users u ON u.id = t.user_id \
            WHERE fl.user_id = $1 AND fl.target_type = 'user' \
           UNION ALL \
           SELECT t.id, t.title, t.forum_id, fc.name, u.username, \
                  t.topic_type, t.last_post_at, t.created_at, t.sticky, t.locked, \
                  (SELECT count(*)-1 FROM posts p WHERE p.topic_id = t.id), \
                  'topic' \
             FROM follows fl \
             JOIN topics t ON t.id = fl.target_id \
             JOIN forums fc ON fc.id = t.forum_id \
             LEFT JOIN users u ON u.id = t.user_id \
            WHERE fl.user_id = $1 AND fl.target_type = 'topic' \
         ) x \
         WHERE ($4::timestamptz IS NULL OR COALESCE(x.last_post_at, x.created_at) < $4) \
           AND ($2 >= 90 OR x.forum_id IN ( \
                 SELECT fc2.id FROM forums fc2 WHERE fc2.minclassread <= $2 \
             ) OR EXISTS(SELECT 1 FROM forum_mods fm WHERE fm.forum_id = x.forum_id AND fm.user_id = $1)) \
         ORDER BY x.last_post_at DESC NULLS LAST, x.topic_id DESC LIMIT $3",
    )
    .bind(auth.id)
    .bind(auth.class_id)
    .bind(fetch)
    .bind(q.before)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 同一主题可能同时因「关注版块」和「关注作者」命中，按 topic_id 去重（保留首次=最后活动最新的那条）
    let mut seen = std::collections::HashSet::new();
    let items: Vec<&FeedRow> = rows
        .iter()
        .filter(|r| seen.insert(r.topic_id))
        .take(limit as usize)
        .collect();
    // 下一页游标：末行的最后活动时间（取到 limit 整页才可能有下一页；与主题页 before=post_id 同范式）
    let next_before = if items.len() as i64 == limit {
        items
            .last()
            .and_then(|r| r.last_post_at.or(Some(r.created_at)))
    } else {
        None
    };
    Ok(ok(
        serde_json::json!({ "items": items, "next_before": next_before }),
    ))
}

// ---- 论坛悬赏（0124）：发帖冻结（topic_create 内 spend_spark_tx）→ 楼主采纳发放 ----

#[derive(Deserialize)]
struct BountyAwardReq {
    topic_id: i64,
    post_id: i64,
}

/// 楼主采纳回复：悬赏发放给答主。约束：
/// · 仅 topic_type=bounty 且 bounty_status='open'（CAS 防并发双采）；
/// · 仅楼主本人（版主代采会引发「谁的钱谁做主」纠纷，不做）；
/// · 不能采楼主首帖（自己给自己发钱）；
/// · 发放幂等键锚定 topic：`forum-bounty-pay:{topic_id}`（一个悬赏只发一次）。
#[post("/forums/bounty/award")]
async fn bounty_award(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<BountyAwardReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let t: Option<(i64, i64, i64, String, String)> = sqlx::query_as(
        "SELECT id, user_id, bounty_spark, bounty_status, topic_type FROM topics WHERE id = $1",
    )
    .bind(body.topic_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((tid, op, spark, bstatus, ttype)) = t else {
        return Err(DomainError::NotFound(body.topic_id));
    };
    if ttype != "bounty" {
        return Err(DomainError::Validation("该主题不是悬赏帖".into()));
    }
    if auth.id != op {
        return Err(DomainError::Forbidden);
    }
    if bstatus != "open" {
        return Err(DomainError::Validation("悬赏已处理".into()));
    }
    // 目标楼必须属于本主题、非楼主首帖
    let p: Option<i64> =
        sqlx::query_scalar("SELECT user_id FROM posts WHERE id = $1 AND topic_id = $2")
            .bind(body.post_id)
            .bind(tid)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(answerer) = p else {
        return Err(DomainError::Validation("目标回复不存在".into()));
    };
    if answerer == op {
        return Err(DomainError::Validation("不能采纳自己的首帖".into()));
    }
    // CAS：open → awarded（并发双采只成功一个）
    let n = sqlx::query(
        "UPDATE topics SET bounty_status = 'awarded', bounty_post_id = $2 \
         WHERE id = $1 AND bounty_status = 'open'",
    )
    .bind(tid)
    .bind(body.post_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::LedgerConflict);
    }
    // 审计流水（钱以 spark_ledger 为准，此表只留「谁采了谁」）
    let _ = sqlx::query(
        "INSERT INTO topic_bounty_awards (topic_id, post_id, answerer_id, awarded_by, spark) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(tid)
    .bind(body.post_id)
    .bind(answerer)
    .bind(auth.id)
    .bind(spark)
    .execute(&state.repo.db)
    .await;
    if spark > 0 {
        let idem = format!("forum-bounty-pay:{}", tid);
        earn_spark(&state.repo.db, answerer, spark, "forum_bounty", &idem).await?;
    }
    // 双向通知：答主拿钱（后端拿不到 site_settings，货币名落默认口径「魔力」，与 economy_http 同约定）
    notify_user(
        &state.repo.db,
        answerer,
        "悬赏已发放",
        &format!(
            "您的回复被采纳，获得 {} 魔力悬赏：[/forums/topic/{}]",
            spark, tid
        ),
    )
    .await;
    state
        .repo
        .audit(Some(auth.id), "forum.bounty_award", Some(tid))
        .await;
    Ok(ok(
        serde_json::json!({ "topic_id": tid, "post_id": body.post_id, "spark": spark }),
    ))
}

// ---- 论坛投票（0125）：范式照 fun_polls（0016）——选项 JSONB、一人一票 UNIQUE、ON CONFLICT 幂等 ----

#[derive(Deserialize)]
struct PollVoteReq {
    topic_id: i64,
    option_index: i32,
}

/// 投一票：登录即可投（版块 can_read 再验一次 forum_access）。
/// 免费（趣味盒扣 1 魔力是游戏口径，论坛投票是表达渠道）。校验全部前置再落占位（对齐 fun_vote 的竞态修复）。
#[post("/forums/poll/vote")]
async fn poll_vote(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<PollVoteReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let p: Option<(serde_json::Value, bool, i64)> = sqlx::query_as(
        "SELECT tp.options, tp.closed, t.forum_id FROM topic_polls tp \
         JOIN topics t ON t.id = tp.topic_id WHERE tp.topic_id = $1",
    )
    .bind(body.topic_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((options, closed, fid)) = p else {
        return Err(DomainError::NotFound(body.topic_id));
    };
    let perm = forum_access(&state.repo.db, auth.id, auth.class_id, fid).await?;
    if !perm.can_read {
        return Err(DomainError::Forbidden);
    }
    if closed {
        return Err(DomainError::Validation("投票已截止".into()));
    }
    let n = options.as_array().map(|a| a.len()).unwrap_or(0);
    if body.option_index < 0 || body.option_index as usize >= n {
        return Err(DomainError::Validation("选项无效".into()));
    }
    let voted = sqlx::query(
        "INSERT INTO poll_votes (topic_id, user_id, option_index) VALUES ($1, $2, $3) \
         ON CONFLICT (topic_id, user_id) DO NOTHING",
    )
    .bind(body.topic_id)
    .bind(auth.id)
    .bind(body.option_index)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if voted == 0 {
        return Err(DomainError::Validation("已经投过啦，一人一票".into()));
    }
    state
        .repo
        .audit(Some(auth.id), "forum.poll_vote", Some(body.topic_id))
        .await;
    Ok(ok(
        serde_json::json!({ "topic_id": body.topic_id, "option_index": body.option_index }),
    ))
}

#[derive(Deserialize)]
struct PollCloseReq {
    topic_id: i64,
}

/// 楼主提前截止投票（截止后只读结果；与 fun_polls.closed 同语义）。版主亦可截止（治理口径）。
#[post("/forums/poll/close")]
async fn poll_close(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<PollCloseReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let p: Option<(i64, i64)> = sqlx::query_as(
        "SELECT t.user_id, t.forum_id FROM topic_polls tp JOIN topics t ON t.id = tp.topic_id \
         WHERE tp.topic_id = $1",
    )
    .bind(body.topic_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((op, fid)) = p else {
        return Err(DomainError::NotFound(body.topic_id));
    };
    let perm = forum_access(&state.repo.db, auth.id, auth.class_id, fid).await?;
    if auth.id != op && !perm.can_mod {
        return Err(DomainError::Forbidden);
    }
    let n = sqlx::query("UPDATE topic_polls SET closed = TRUE WHERE topic_id = $1 AND NOT closed")
        .bind(body.topic_id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("投票已截止".into()));
    }
    state
        .repo
        .audit(Some(auth.id), "forum.poll_close", Some(body.topic_id))
        .await;
    Ok(ok(
        serde_json::json!({ "topic_id": body.topic_id, "closed": true }),
    ))
}

// ---- 论坛抽奖（0126）：发帖冻结奖金池（topic_create）→ 参与 → 开奖 ----

#[derive(Deserialize)]
struct LotteryJoinReq {
    topic_id: i64,
}

/// 参与抽奖：付票价（0=免费）换一个名额。约束：
/// · 仅 open 且未到 draw_at（到点等 worker 开奖，不接受「补票」）；
/// · 楼主不能参与自己的抽奖（既当庄又下注必起纠纷）；
/// · 一人一次（PK 幂等）；票价与占位原子（对齐 fun_vote/jgg 的 spend-then-insert 回滚纪律，此处反过来 insert-then-spend 失败删占位）。
#[post("/forums/lottery/join")]
async fn lottery_join(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<LotteryJoinReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let l: Option<(i64, i64, String, chrono::DateTime<chrono::Utc>, i64)> = sqlx::query_as(
        "SELECT tl.ticket_spark::bigint, t.user_id, tl.status, tl.draw_at, t.forum_id \
         FROM topic_lotteries tl JOIN topics t ON t.id = tl.topic_id WHERE tl.topic_id = $1",
    )
    .bind(body.topic_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((ticket, op, status, draw_at, fid)) = l else {
        return Err(DomainError::NotFound(body.topic_id));
    };
    let perm = forum_access(&state.repo.db, auth.id, auth.class_id, fid).await?;
    if !perm.can_read {
        return Err(DomainError::Forbidden);
    }
    if status != "open" {
        return Err(DomainError::Validation("抽奖不在进行中".into()));
    }
    if chrono::Utc::now() >= draw_at {
        return Err(DomainError::Validation("已到开奖时间，等待开奖".into()));
    }
    if op == auth.id {
        return Err(DomainError::Validation("楼主不能参与自己的抽奖".into()));
    }
    let entered = sqlx::query(
        "INSERT INTO lottery_entries (topic_id, user_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(body.topic_id)
    .bind(auth.id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if entered == 0 {
        return Err(DomainError::Validation("已经参与过了".into()));
    }
    // 票价（免费则跳过）：扣费失败回滚占位（对齐 fun_vote 纪律）
    if ticket > 0 {
        let idem = format!("forum-lottery-ticket:{}:{}", auth.id, body.topic_id);
        if let Err(e) = crate::economy_http::spend_spark(
            &state.repo.db,
            auth.id,
            ticket,
            "forum_lottery",
            &idem,
            "forum_lottery",
            body.topic_id,
        )
        .await
        {
            let _ = sqlx::query("DELETE FROM lottery_entries WHERE topic_id = $1 AND user_id = $2")
                .bind(body.topic_id)
                .bind(auth.id)
                .execute(&state.repo.db)
                .await;
            return Err(e);
        }
    }
    state
        .repo
        .audit(Some(auth.id), "forum.lottery_join", Some(body.topic_id))
        .await;
    Ok(ok(
        serde_json::json!({ "topic_id": body.topic_id, "ticket": ticket }),
    ))
}

/// 开奖核心（手动入口与 worker 共用）：CAS open→drawn 后随机抽 winners 名，
/// 每人发 prize_per_winner（幂等键 `forum-lottery-win:{tid}:{uid}`）。
/// 参与人数不足名额时全中奖（钱不留在池里）；零参与则奖金池退回楼主。
pub async fn lottery_draw_core(
    db: &sqlx::PgPool,
    topic_id: i64,
) -> DomainResult<serde_json::Value> {
    let l: Option<(i32, i64, i64, String)> = sqlx::query_as(
        "SELECT winners, prize_per_winner, ticket_spark::bigint, status FROM topic_lotteries WHERE topic_id = $1",
    )
    .bind(topic_id)
    .fetch_optional(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((winners, prize, _ticket, status)) = l else {
        return Err(DomainError::NotFound(topic_id));
    };
    if status != "open" {
        return Err(DomainError::Validation("抽奖不在进行中".into()));
    }
    let n = sqlx::query(
        "UPDATE topic_lotteries SET status = 'drawn' WHERE topic_id = $1 AND status = 'open'",
    )
    .bind(topic_id)
    .execute(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::LedgerConflict);
    }
    // 参与者全表捞出来在应用层抽（数量级 ≤ 数百，RANDOM() 洗牌即可）
    let mut entries: Vec<i64> =
        sqlx::query_scalar("SELECT user_id FROM lottery_entries WHERE topic_id = $1")
            .bind(topic_id)
            .fetch_all(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    if entries.is_empty() {
        // 无人参与：奖金池退回楼主
        let op: i64 = sqlx::query_scalar("SELECT user_id FROM topics WHERE id = $1")
            .bind(topic_id)
            .fetch_one(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        let refund = winners as i64 * prize;
        if refund > 0 {
            let _ = earn_spark(
                db,
                op,
                refund,
                "forum_lottery_refund",
                &format!("forum-lottery-refund:{topic_id}"),
            )
            .await;
        }
        return Ok(serde_json::json!({ "topic_id": topic_id, "winners": [], "refunded": refund }));
    }
    use rand::seq::SliceRandom;
    entries.shuffle(&mut rand::thread_rng());
    let take = (winners as usize).min(entries.len());
    let picked: Vec<i64> = entries.into_iter().take(take).collect();
    for uid in &picked {
        let _ = sqlx::query(
            "UPDATE lottery_entries SET won = TRUE WHERE topic_id = $1 AND user_id = $2",
        )
        .bind(topic_id)
        .bind(uid)
        .execute(db)
        .await;
        if prize > 0 {
            let _ = earn_spark(
                db,
                *uid,
                prize,
                "forum_lottery",
                &format!("forum-lottery-win:{topic_id}:{uid}"),
            )
            .await;
            notify_user(
                db,
                *uid,
                "抽奖中奖",
                &format!("您在 [/forums/topic/{topic_id}] 的抽奖中中奖，获得 {prize} 魔力！"),
            )
            .await;
        }
    }
    Ok(serde_json::json!({ "topic_id": topic_id, "winners": picked, "prize": prize }))
}

#[derive(Deserialize)]
struct LotteryDrawReq {
    topic_id: i64,
}

/// 楼主手动开奖（提前开或到点 worker 没来得及时的兜底）。版主亦可（治理口径，同 poll_close）。
#[post("/forums/lottery/draw")]
async fn lottery_draw(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<LotteryDrawReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let l: Option<(i64, i64)> = sqlx::query_as(
        "SELECT t.user_id, t.forum_id FROM topic_lotteries tl JOIN topics t ON t.id = tl.topic_id \
         WHERE tl.topic_id = $1",
    )
    .bind(body.topic_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((op, fid)) = l else {
        return Err(DomainError::NotFound(body.topic_id));
    };
    let perm = forum_access(&state.repo.db, auth.id, auth.class_id, fid).await?;
    if auth.id != op && !perm.can_mod {
        return Err(DomainError::Forbidden);
    }
    let out = lottery_draw_core(&state.repo.db, body.topic_id).await?;
    state
        .repo
        .audit(Some(auth.id), "forum.lottery_draw", Some(body.topic_id))
        .await;
    Ok(ok(out))
}

// ---- 论坛打赏（0127）：楼层打赏 = spend(打赏人) + earn(作者) 同额对冲，不抽税 ----

#[derive(Deserialize)]
struct PostTipReq {
    post_id: i64,
    spark: i64,
    #[serde(default)]
    note: Option<String>,
}

/// 打赏某楼：任何登录用户可打赏任何非匿名楼的作者。约束：
/// · 金额 1~100,000 钳位外拒绝；
/// · 不能打赏自己的楼（自己转自己只是流水噪音）；
/// · 打赏人余额不足自然被 spend_spark 拒（409）；
/// · 幂等键 `forum-tip:{from}:{post}:{uuid}`——打赏是主动行为可重复（同一个人可以多次打赏同一楼），
///   幂等只防网络重试，不防故意多次。
#[post("/forums/tip")]
async fn post_tip(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<PostTipReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if body.spark <= 0 || body.spark > 100_000 {
        return Err(DomainError::Validation("打赏金额需在 1~100000 之间".into()));
    }
    let note: String = body
        .note
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .chars()
        .take(50)
        .collect();
    // 目标楼必须存在且属于某主题（匿名楼的 user_id 为 NULL 不可打赏）
    let p: Option<(i64, Option<i64>, i64)> = sqlx::query_as(
        "SELECT p.topic_id, p.user_id, t.forum_id FROM posts p JOIN topics t ON t.id = p.topic_id \
         WHERE p.id = $1",
    )
    .bind(body.post_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((topic_id, author, fid)) = p else {
        return Err(DomainError::NotFound(body.post_id));
    };
    let Some(author_id) = author else {
        return Err(DomainError::Validation("匿名帖不可打赏".into()));
    };
    if author_id == auth.id {
        return Err(DomainError::Validation("不能打赏自己".into()));
    }
    let perm = forum_access(&state.repo.db, auth.id, auth.class_id, fid).await?;
    if !perm.can_read {
        return Err(DomainError::Forbidden);
    }
    let idem = format!("forum-tip:{}:{}:{}", auth.id, body.post_id, Uuid::new_v4());
    // spend(打赏人) → earn(作者) 对冲；三写（spend/earn/落账）单事务——旧版 earn 失败
    // 时打赏已扣、作者未入账且 post_tips 无痕。earn 幂等键锚定 spend 的 uuid。
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 幂等重放闸门（#[must_use] 连审）：重放时 spend 不再扣款而 earn 照发 = 凭空入账
    if !matches!(
        crate::economy_http::spend_spark_tx(
            &mut tx,
            auth.id,
            body.spark,
            "forum_tip",
            &idem,
            "forum_tip",
            body.post_id,
        )
        .await?,
        crate::economy_http::SpendOutcome::Spent
    ) {
        return Err(DomainError::Validation(
            "该笔打赏已受理，请勿重复提交".into(),
        ));
    }
    // spend 已闸 Spent，earn 幂等键锚定本次新 spend id，必为 Spent；
    // 显式丢弃以满足 must_use 契约
    let earn_outcome = crate::economy_http::earn_spark_tx(
        &mut tx,
        author_id,
        body.spark,
        "forum_tip",
        &format!("{idem}:to"),
    )
    .await?;
    let _ = earn_outcome;
    sqlx::query(
        "INSERT INTO post_tips (post_id, topic_id, from_user, to_user, spark, note) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(body.post_id)
    .bind(topic_id)
    .bind(auth.id)
    .bind(author_id)
    .bind(body.spark)
    .bind(&note)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    notify_user(
        &state.repo.db,
        author_id,
        "收到打赏",
        &format!(
            "用户 #{} 打赏了您在 [/forums/topic/{}] 的回复：{} 魔力{}",
            auth.id,
            topic_id,
            body.spark,
            if note.is_empty() {
                String::new()
            } else {
                format!("（{note}）")
            }
        ),
    )
    .await;
    state
        .repo
        .audit(Some(auth.id), "forum.post_tip", Some(body.post_id))
        .await;
    Ok(ok(
        serde_json::json!({ "post_id": body.post_id, "spark": body.spark, "to": author_id }),
    ))
}

/// 单帖上下文：(topic_id, forum_id, author_id)
async fn post_context(db: &sqlx::PgPool, post_id: i64) -> DomainResult<Option<(i64, i64, i64)>> {
    let row: Option<(i64, i64, i64)> = sqlx::query_as(
        "SELECT p.topic_id, t.forum_id, p.user_id FROM posts p \
         JOIN topics t ON t.id = p.topic_id \
         WHERE p.id = $1",
    )
    .bind(post_id)
    .fetch_optional(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(row)
}

/// 编辑帖子：作者本人随时可改；版主/postmanage 可改他人帖 —— 后者自动 PM 通知作者并留 edited_by
#[derive(Deserialize)]
struct PostEditReq {
    body: String,
}

#[put("/forums/posts/{id}")]
async fn post_edit(
    req: HttpRequest,
    path: web::Path<i64>,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<PostEditReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if body.body.trim().is_empty() {
        return Err(DomainError::Validation("正文不能为空".into()));
    }
    // 敏感词（Phase3）：编辑同样过闸（防止先发合规后改敏感词绕过）
    check_banned_words(&state.repo.db, &body.body).await?;
    let pid = path.into_inner();
    let Some((_tid, fid, author_id)) = post_context(&state.repo.db, pid).await? else {
        return Err(DomainError::NotFound(pid));
    };
    let perm = forum_access(&state.repo.db, auth.id, auth.class_id, fid).await?;
    if !perm.can_mod && author_id != auth.id {
        return Err(DomainError::Forbidden);
    }
    let n =
        sqlx::query("UPDATE posts SET body = $1, body_text = $4, edited_at = now(), edited_by = $2 WHERE id = $3")
            .bind(body.body.trim())
            .bind(auth.id)
            .bind(pid)
            .bind(strip_markdown(body.body.trim()))
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(pid));
    }
    // 管理员/版主编辑他人帖：自动 PM 通知作者（forums.php:426 口径）
    if author_id != auth.id {
        let _ = sqlx::query(
            "INSERT INTO messages (sender_id, receiver_id, subject, body) VALUES ($1, $2, $3, $4)",
        )
        .bind(auth.id)
        .bind(author_id)
        .bind("您的帖子被编辑")
        .bind(format!("您的帖子已被管理组成员编辑，请查看最新内容。"))
        .execute(&state.repo.db)
        .await;
    }
    Ok(ok(serde_json::json!({ "edited": pid })))
}

/// 删帖：仅版主/postmanage —— 普通用户（含作者本人）删不掉自己的帖，想删找版主
#[delete("/forums/posts/{id}")]
async fn post_delete(
    req: HttpRequest,
    path: web::Path<i64>,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let pid = path.into_inner();
    let Some((tid, fid, author_id)) = post_context(&state.repo.db, pid).await? else {
        return Err(DomainError::NotFound(pid));
    };
    let perm = forum_access(&state.repo.db, auth.id, auth.class_id, fid).await?;
    if !perm.can_mod {
        return Err(DomainError::Forbidden);
    }
    // 删单帖收回该帖的回帖 +1（与删主题收回发帖 +2 对称，堵住「发帖赚火花再被删」的获利通道）
    {
        let _ = earn_spark(
            &state.repo.db,
            author_id,
            -1,
            "forum-post-del",
            &format!("forum-post-del:{pid}"),
        )
        .await;
    }
    // 审计修复（P2）：被删帖若是该主题最新回复，last_post_at 残留已删时间——
    // 版块「最后回复」排序/展示失真。先取被删帖时间，删后回填剩余最新回复
    // （无回复则回落主题创建时间）。
    let deleted_at: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT created_at FROM posts WHERE id = $1 AND topic_id = $2")
            .bind(pid)
            .bind(tid)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .flatten();
    sqlx::query("DELETE FROM posts WHERE id = $1 AND topic_id = $2")
        .bind(pid)
        .bind(tid)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 点赞行无 FK（posts 为分区表，无法对 post_id 建外键）：单帖删除时手动清理，避免孤儿点赞
    let _ = sqlx::query("DELETE FROM post_likes WHERE post_id = $1")
        .bind(pid)
        .execute(&state.repo.db)
        .await;
    if let Some(d) = deleted_at {
        let _ = sqlx::query(
            "UPDATE topics t SET last_post_at = COALESCE( \
                (SELECT max(created_at) FROM posts p WHERE p.topic_id = t.id), t.created_at) \
             WHERE t.id = $1 AND t.last_post_at = $2",
        )
        .bind(tid)
        .bind(d)
        .execute(&state.repo.db)
        .await;
    }
    state
        .repo
        .audit(Some(auth.id), "forum.post_delete", Some(pid))
        .await;
    Ok(ok(serde_json::json!({ "deleted": pid })))
}

/// 删主题：仅版主/postmanage；收回发帖 +2 火花（KPS("-", starttopic_bonus) 口径）
#[delete("/forums/topics/{id}")]
async fn topic_delete(
    req: HttpRequest,
    path: web::Path<i64>,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let tid = path.into_inner();
    let fid: Option<i64> = sqlx::query_scalar("SELECT forum_id FROM topics WHERE id = $1")
        .bind(tid)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(fid) = fid else {
        return Err(DomainError::NotFound(tid));
    };
    let perm = forum_access(&state.repo.db, auth.id, auth.class_id, fid).await?;
    if !perm.can_mod {
        return Err(DomainError::Forbidden);
    }
    let op: Option<i64> = sqlx::query_scalar("SELECT user_id FROM topics WHERE id = $1")
        .bind(tid)
        .fetch_one(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 悬赏未决退回（0124）：open 状态的悬赏在删主题时退还楼主（已 awarded 的不动——钱已归答主）。
    // 幂等键与发放错开：`-refund` 后缀，退回与发放都各只发生一次。
    if let Some(op_id) = op {
        let bounty_open: Option<i64> = sqlx::query_scalar(
            "SELECT bounty_spark FROM topics WHERE id = $1 AND topic_type = 'bounty' \
             AND bounty_status = 'open' AND bounty_spark > 0",
        )
        .bind(tid)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        if let Some(refund) = bounty_open {
            let _ = earn_spark(
                &state.repo.db,
                op_id,
                refund,
                "forum_bounty_refund",
                &format!("forum-bounty-refund:{tid}"),
            )
            .await;
            let _ = sqlx::query(
                "UPDATE topics SET bounty_status = 'refunded' WHERE id = $1 AND bounty_status = 'open'",
            )
            .bind(tid)
            .execute(&state.repo.db)
            .await;
        }
        // 抽奖未决退回（0126）：open 状态的抽奖在删主题时奖金池退还楼主（drawn 的不动——钱已归中奖人）。
        // 票价不退（参与者享受了参与过程；与删悬赏帖不追回已发赏金同一不对称口径）。
        let lot_open: Option<i64> = sqlx::query_scalar(
            "SELECT winners::bigint * prize_per_winner FROM topic_lotteries \
             WHERE topic_id = $1 AND status = 'open' AND prize_per_winner > 0",
        )
        .bind(tid)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        if let Some(refund) = lot_open {
            let _ = earn_spark(
                &state.repo.db,
                op_id,
                refund,
                "forum_lottery_refund",
                &format!("forum-lottery-refund:{tid}"),
            )
            .await;
            let _ = sqlx::query(
                "UPDATE topic_lotteries SET status = 'cancelled' WHERE topic_id = $1 AND status = 'open'",
            )
            .bind(tid)
            .execute(&state.repo.db)
            .await;
        }
    }
    if let Some(op_id) = op {
        let _ = earn_spark(
            &state.repo.db,
            op_id,
            -2,
            "forum-topic-del",
            &format!("forum-topic-del:{tid}"),
        )
        .await;
    }
    // 级联回收回帖 +1：主题删除会把全部回帖 CASCADE 掉，逐笔回收回帖奖励保持对称。
    // 审计修复（P1 双重扣分）：楼主首帖也存于 posts（topic_create 落 posts 行），
    // 旧版把它计为「回帖」再 -1，楼主实扣 -3（发帖 -2 + 首帖按回帖 -1）。
    // 排除楼主首帖，楼主只按发帖口径 -2。
    let replies: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT DISTINCT user_id, count(*) OVER (PARTITION BY user_id) FROM posts \
         WHERE topic_id = $1 AND NOT (user_id = $2 AND id = (SELECT min(id) FROM posts WHERE topic_id = $1))",
    )
    .bind(tid)
    .bind(op.unwrap_or(0))
    .fetch_all(&state.repo.db)
    .await
    .unwrap_or_default();
    for (uid, n) in replies {
        let _ = earn_spark(
            &state.repo.db,
            uid,
            -(n as i64),
            "forum-topic-del-reply",
            &format!("forum-topic-del-reply:{tid}:{uid}"),
        )
        .await;
    }
    sqlx::query("DELETE FROM topics WHERE id = $1")
        .bind(tid)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 关注本主题的行：target_id 无外键（指向 3 张表），必须应用层清理，否则留下悬空关注
    let _ = sqlx::query("DELETE FROM follows WHERE target_type = 'topic' AND target_id = $1")
        .bind(tid)
        .execute(&state.repo.db)
        .await;
    state
        .repo
        .audit(Some(auth.id), "forum.topic_delete", Some(tid))
        .await;
    Ok(ok(serde_json::json!({ "deleted": tid })))
}

/// 主题管理：置顶/锁定/移动版块（版主限本版块）
#[derive(Deserialize)]
struct TopicManageReq {
    #[serde(default)]
    sticky: Option<bool>,
    #[serde(default)]
    locked: Option<bool>,
    /// 精华帖（NP digest 口径）：版主标记，列表/详情加精徽标
    #[serde(default)]
    digest: Option<bool>,
    #[serde(default)]
    move_to_forum_id: Option<i64>,
}

#[post("/forums/topics/{id}/manage")]
async fn topic_manage(
    req: HttpRequest,
    path: web::Path<i64>,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<TopicManageReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let tid = path.into_inner();
    let fid: Option<i64> = sqlx::query_scalar("SELECT forum_id FROM topics WHERE id = $1")
        .bind(tid)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(fid) = fid else {
        return Err(DomainError::NotFound(tid));
    };
    let perm = forum_access(&state.repo.db, auth.id, auth.class_id, fid).await?;
    if !perm.can_mod {
        return Err(DomainError::Forbidden);
    }
    let mut moved = false;
    if let Some(target) = body.move_to_forum_id {
        if target != fid {
            let exists: Option<i64> = sqlx::query_scalar("SELECT id FROM forums WHERE id = $1")
                .bind(target)
                .fetch_optional(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
            if exists.is_none() {
                return Err(DomainError::Validation("目标版块不存在".into()));
            }
            sqlx::query("UPDATE topics SET forum_id = $1 WHERE id = $2")
                .bind(target)
                .bind(tid)
                .execute(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
            moved = true;
        }
    }
    if let Some(sticky) = body.sticky {
        sqlx::query("UPDATE topics SET sticky = $1 WHERE id = $2")
            .bind(sticky)
            .bind(tid)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    }
    if let Some(locked) = body.locked {
        sqlx::query("UPDATE topics SET locked = $1 WHERE id = $2")
            .bind(locked)
            .bind(tid)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    }
    if let Some(digest) = body.digest {
        sqlx::query("UPDATE topics SET digest = $1 WHERE id = $2")
            .bind(digest)
            .bind(tid)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    }
    state
        .repo
        .audit(Some(auth.id), "forum.topic_manage", Some(tid))
        .await;
    Ok(ok(serde_json::json!({ "managed": tid, "moved": moved })))
}
// ============ M16 短讯与好友 ============

// （我的 H&R /me/hr 由 gaps_http::my_hr_status 提供——hr_snapshots 口径；
//  旧 snatches 口径实现已删除，曾与前者重复注册同一路由）

// ============ 聊天盒（shoutbox.php 口径：最近消息 + 发言） ============

#[derive(serde::Serialize, sqlx::FromRow)]
struct ShoutRow {
    id: i64,
    username: Option<String>,
    message: String,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/shoutbox")]
async fn shoutbox_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    // 审计修复（P1）：与全站鉴权口径对齐——聊天记录含用户名与发言内容，不应对匿名开放
    let _auth = require_auth(&req, &state).await?;
    let rows: Vec<ShoutRow> = sqlx::query_as(
        "SELECT sb.id, u.username, sb.message, sb.created_at          FROM shoutbox sb LEFT JOIN users u ON u.id = sb.user_id          ORDER BY sb.id DESC LIMIT 50",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct ShoutReq {
    message: String,
}

#[post("/shoutbox")]
async fn shoutbox_send(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ShoutReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if body.message.trim().is_empty() || body.message.len() > 300 {
        return Err(DomainError::Validation("发言需 1-300 字".into()));
    }
    // 禁言位（NP chatpost 口径）：被禁言用户不能在聊天室继续刷屏
    let can_chat: bool =
        sqlx::query_scalar("SELECT COALESCE(forumpost, TRUE) FROM users WHERE id = $1")
            .bind(auth.id)
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or(false);
    if !can_chat {
        return Err(DomainError::Validation("你已被禁言".into()));
    }
    // 防刷：每用户 10 秒冷却（Redis 计数，首条设 TTL；故障静默放行不影响可用性）
    {
        let mut c = state.redis.clone();
        let key = format!("rl:shout:{}", auth.id);
        let n: i64 = redis::AsyncCommands::incr(&mut c, &key, 1)
            .await
            .unwrap_or(0);
        if n == 1 {
            let _: () = redis::AsyncCommands::expire(&mut c, &key, 10)
                .await
                .unwrap_or(());
        }
        if n > 1 {
            return Err(DomainError::RateLimited);
        }
    }
    let id: i64 =
        sqlx::query_scalar("INSERT INTO shoutbox (user_id, message) VALUES ($1, $2) RETURNING id")
            .bind(auth.id)
            .bind(body.message.trim())
            .fetch_one(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "id": id })))
}

/// 删除聊天发言（staff：审计修复 P1——此前全后端无 DELETE FROM shoutbox，
/// 违规发言只能等禁言、无法清除已发内容）。发言本人 2 分钟内也可撤回。
#[derive(Deserialize)]
struct ShoutDeleteReq {
    id: i64,
}

#[post("/shoutbox/delete")]
async fn shoutbox_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ShoutDeleteReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let row: Option<(i64, chrono::DateTime<chrono::Utc>)> =
        sqlx::query_as("SELECT user_id, created_at FROM shoutbox WHERE id = $1")
            .bind(body.id)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((uid, at)) = row else {
        return Err(DomainError::NotFound(body.id));
    };
    let is_staff = auth.class_id >= 90;
    let own_recent = uid == auth.id && (chrono::Utc::now() - at).num_seconds() <= 120;
    if !is_staff && !own_recent {
        return Err(DomainError::Forbidden);
    }
    sqlx::query("DELETE FROM shoutbox WHERE id = $1")
        .bind(body.id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if is_staff && uid != auth.id {
        state
            .repo
            .audit(Some(auth.id), "shoutbox_delete", Some(body.id))
            .await;
    }
    Ok(ok(serde_json::json!({ "deleted": body.id })))
}

#[derive(Deserialize)]
struct SendMsgReq {
    to: String,
    subject: String,
    body: String,
    /// 回复/转发的原信 id（回复自动加 Re: 前缀+引用原文；转发原样带文）
    #[serde(default)]
    reply_to: Option<i64>,
    #[serde(default)]
    forward_of: Option<i64>,
}

/// 发信（sendmessage.php 口径 + takemessage 的管理组豁免）：
/// - 接收限制：普通用户须过对方 accept_pm（yes/friends/no）+ 非黑名单；管理组（staff≥90）一律放行
/// - 防刷：普通用户 60s 一条（message_flood 表）；staff 不限
/// - 回复：subject 加 Re:/Re(n): 并引用原文；转发：原封带文给第三人
#[post("/messages")]
async fn message_send(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<SendMsgReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let staff = auth.class_id >= 90;

    let target: Option<(i64, String, i32)> = sqlx::query_as(
        "SELECT id, accept_pm, class_id FROM users WHERE username = $1 AND status < 2",
    )
    .bind(&body.to)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((to_id, accept, _to_class)) = target else {
        return Err(DomainError::NotFound(0));
    };
    // 审计修复（P1）：不能给自己发私信（对照 medal_gift 的同款护栏）
    if to_id == auth.id {
        return Err(DomainError::Validation("不能给自己发私信".into()));
    }
    if body.subject.trim().is_empty() {
        return Err(DomainError::Validation("主题不能为空".into()));
    }
    // 接收限制（管理组豁免 —— takemessage.php staffmem 口径）
    if !staff {
        // 审计修复（P0）：黑名单拦截此前从未实现（注释声称校验却无查询），
        // 被拉黑者可继续私信目标用户。
        let blocked: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM friendships f \
             WHERE f.user_id = $2 AND f.friend_id = $1 AND f.list = 'black')",
        )
        .bind(auth.id)
        .bind(to_id)
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or(false);
        if blocked {
            return Err(DomainError::Validation("对方不接受你的私信".into()));
        }
        match accept.as_str() {
            "no" => return Err(DomainError::Validation("对方仅接收管理组私信".into())),
            "friends" => {
                let is_friend: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM friendships f WHERE (f.user_id=$1 AND f.friend_id=$2) OR (f.user_id=$2 AND f.friend_id=$1))",
                )
                .bind(auth.id)
                .bind(to_id)
                .fetch_one(&state.repo.db)
                .await
                .unwrap_or(false);
                if !is_friend {
                    return Err(DomainError::Validation("对方仅接收好友私信".into()));
                }
            }
            _ => {}
        }
        // 防刷：60s 一条
        let last: Option<chrono::DateTime<chrono::Utc>> =
            sqlx::query_scalar("SELECT last_sent_at FROM message_flood WHERE user_id = $1")
                .bind(auth.id)
                .fetch_optional(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?
                .flatten();
        if let Some(t) = last {
            if (chrono::Utc::now() - t).num_seconds() < 60 {
                return Err(DomainError::Validation(
                    "发送过于频繁，请 1 分钟后再试".into(),
                ));
            }
        }
    }

    // 回复：Re:/Re(n): 前缀 + 引用原文；转发：原样带文
    let mut subject = body.subject.trim().to_string();
    let mut text = body.body.clone();
    if let Some(rid) = body.reply_to {
        let orig: Option<(String, String)> = sqlx::query_as(
            "SELECT subject, body FROM messages WHERE id = $1 AND (receiver_id = $2 OR sender_id = $2)",
        )
        .bind(rid)
        .bind(auth.id)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        if let Some((osub, obody)) = orig {
            let n = osub.match_indices("Re(").count();
            subject = if n == 0 && osub.starts_with("Re:") {
                format!("Re: {}", osub.trim_start_matches("Re: "))
            } else if n > 0 {
                format!("Re({}): {}", n + 1, osub)
            } else {
                format!("Re: {}", osub)
            };
            text = format!("{}\n\n———— 原信 ————\n{}", body.body, obody);
        }
    }
    if let Some(fid) = body.forward_of {
        let orig: Option<(String, String)> = sqlx::query_as(
            "SELECT subject, body FROM messages WHERE id = $1 AND (receiver_id = $2 OR sender_id = $2)",
        )
        .bind(fid)
        .bind(auth.id)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        if let Some((osub, obody)) = orig {
            subject = format!("Fw: {}", osub);
            text = obody;
        }
    }

    let id: i64 = sqlx::query_scalar(
        "INSERT INTO messages (sender_id, receiver_id, subject, body, location, saved, unread) \
         VALUES ($1, $2, $3, $4, 1, 1, true) RETURNING id",
    )
    .bind(auth.id)
    .bind(to_id)
    .bind(&subject)
    .bind(&text)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "INSERT INTO message_flood (user_id, last_sent_at) VALUES ($1, now()) \
                 ON CONFLICT (user_id) DO UPDATE SET last_sent_at = now()",
    )
    .bind(auth.id)
    .execute(&state.repo.db)
    .await
    .ok();
    Ok(ok(serde_json::json!({ "id": id })))
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct MessageRow {
    id: i64,
    counterpart: Option<String>,
    subject: String,
    body: String,
    read_at: Option<chrono::DateTime<chrono::Utc>>,
    created_at: chrono::DateTime<chrono::Utc>,
    #[sqlx(default)]
    unread: Option<bool>,
    #[sqlx(default)]
    folder: Option<i32>,
    /// 系统通知（sender_id IS NULL）：前端据此做视觉区分（通知带 🔔 徽标、不可回复）
    #[sqlx(default)]
    is_system: bool,
}

/// 收件箱（messages.php location=1 口径）：支持 box=folderid、关键词搜索（主题/正文/两者）与未读筛选
#[get("/messages/inbox")]
async fn message_inbox(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<InboxQuery>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    // 关键词经 like_pattern 转义（%/_/\）后与原始串绑定：NULL 跳过、非 NULL 模糊匹配
    let pattern = q.search.as_deref().map(crate::http::like_pattern);
    let rows = sqlx::query_as::<_, MessageRow>(
        "SELECT m.id, u.username AS counterpart, m.subject, m.body, m.read_at, m.created_at, m.unread, m.folder, \
                (m.sender_id IS NULL) AS is_system \
         FROM messages m LEFT JOIN users u ON u.id = m.sender_id \
         WHERE m.receiver_id = $1 AND m.location = 1 \
           AND ($2::int IS NULL OR m.folder = $2) \
           AND ($3::text IS NULL OR m.subject ILIKE $3 OR m.body ILIKE $3) \
           AND ($4::bool IS NULL OR m.unread = $4) \
         ORDER BY m.id DESC LIMIT 100",
    )
    .bind(auth.id)
    .bind(q.box_id)
    .bind(pattern.as_deref())
    .bind(q.unread)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct InboxQuery {
    /// 自建文件夹 id（缺省 = 主收件箱 folder IS NULL）
    box_id: Option<i32>,
    search: Option<String>,
    unread: Option<bool>,
}

/// 发件箱（saved=1 口径）
#[get("/messages/sent")]
async fn message_sent(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let rows = sqlx::query_as::<_, MessageRow>(
        "SELECT m.id, u.username AS counterpart, m.subject, m.body, m.read_at, m.created_at, m.unread, m.folder \
         FROM messages m LEFT JOIN users u ON u.id = m.receiver_id \
         WHERE m.sender_id = $1 AND m.saved = 1 ORDER BY m.id DESC LIMIT 100",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

/// 管理组信箱（staffbox.php 口径）：与管理组成员（class_id >= 90）互发的短讯
#[get("/messages/staff")]
async fn message_staff(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let rows = sqlx::query_as::<_, MessageRow>(
        "SELECT m.id, u.username AS counterpart, m.subject, m.body, m.read_at, m.created_at \
         FROM messages m \
         LEFT JOIN users u ON u.id = CASE WHEN m.sender_id = $1 THEN m.receiver_id ELSE m.sender_id END \
         WHERE (m.sender_id = $1 AND EXISTS (SELECT 1 FROM users su WHERE su.id = m.receiver_id AND su.class_id >= 90)) \
            OR (m.receiver_id = $1 AND EXISTS (SELECT 1 FROM users su WHERE su.id = m.sender_id AND su.class_id >= 90)) \
         ORDER BY m.id DESC LIMIT 50",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

// ============ 信箱操作（messages.php 口径：已读/逻辑删/移动/文件夹） ============

#[derive(Deserialize)]
struct MarkReadReq {
    ids: Vec<i64>,
}

/// 标记已读（单条打开或列表批量 markread）
#[post("/messages/markread")]
async fn message_markread(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<MarkReadReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if body.ids.is_empty() {
        return Err(DomainError::Validation("未选择信件".into()));
    }
    let n = sqlx::query(
        "UPDATE messages SET unread = false, read_at = COALESCE(read_at, now()) \
         WHERE receiver_id = $1 AND id = ANY($2)",
    )
    .bind(auth.id)
    .bind(&body.ids)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    Ok(ok(serde_json::json!({ "updated": n })))
}

#[derive(Deserialize)]
struct DeleteReq {
    ids: Vec<i64>,
}

/// 逻辑删除（NexusPHP 双删语义）：收件方删 → location=0；发件方删 → saved=0；
/// 两边都删才物理 DELETE。
#[post("/messages/delete")]
async fn message_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<DeleteReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if body.ids.is_empty() {
        return Err(DomainError::Validation("未选择信件".into()));
    }
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "UPDATE messages SET location = 0 WHERE receiver_id = $1 AND id = ANY($2) AND location = 1",
    )
    .bind(auth.id)
    .bind(&body.ids)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "UPDATE messages SET saved = 0 WHERE sender_id = $1 AND id = ANY($2) AND saved = 1",
    )
    .bind(auth.id)
    .bind(&body.ids)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query("DELETE FROM messages WHERE id = ANY($1) AND location = 0 AND saved = 0")
        .bind(&body.ids)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "deleted": body.ids.len() })))
}

#[derive(Deserialize)]
struct MoveReq {
    ids: Vec<i64>,
    folder: Option<i32>,
}

/// 移动到自建文件夹（folder=NULL 即移回主收件箱）
#[post("/messages/move")]
async fn message_move(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<MoveReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if body.ids.is_empty() {
        return Err(DomainError::Validation("未选择信件".into()));
    }
    if let Some(f) = body.folder {
        let owned: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM pmboxes WHERE id = $1 AND user_id = $2)",
        )
        .bind(f)
        .bind(auth.id)
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or(false);
        if !owned {
            return Err(DomainError::Validation("目标文件夹不存在".into()));
        }
    }
    let n = sqlx::query(
        "UPDATE messages SET folder = $2 WHERE receiver_id = $1 AND id = ANY($3) AND location = 1",
    )
    .bind(auth.id)
    .bind(body.folder)
    .bind(&body.ids)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    Ok(ok(serde_json::json!({ "moved": n })))
}

/// 文件夹列表 + 新建/改名（editmailboxes 口径：一人最多 3 个、名 ≤14 字；清空名=删除并连带清信）
#[derive(sqlx::FromRow, serde::Serialize)]
struct PmBoxRow {
    id: i64,
    name: String,
    count: i64,
}

#[get("/messages/boxes")]
async fn message_boxes(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let rows = sqlx::query_as::<_, PmBoxRow>(
        "SELECT b.id, b.name, (SELECT count(*) FROM messages m WHERE m.folder = b.id AND m.receiver_id = $1 AND m.location = 1) AS count \
         FROM pmboxes b WHERE b.user_id = $1 ORDER BY b.id",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct BoxReq {
    /// 缺省 = 新建；有值 = 改名/删除该 id
    id: Option<i64>,
    name: String,
}

#[post("/messages/boxes")]
async fn message_box_upsert(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<BoxReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let name = body.name.trim();
    // 清空名 = 删除文件夹（连带清掉夹内信件，NexusPHP 口径）
    if name.is_empty() {
        if let Some(bid) = body.id {
            sqlx::query("DELETE FROM messages WHERE folder = $2 AND receiver_id = $1")
                .bind(auth.id)
                .bind(bid)
                .execute(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
            sqlx::query("DELETE FROM pmboxes WHERE id = $2 AND user_id = $1")
                .bind(auth.id)
                .bind(bid)
                .execute(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        }
        return Ok(ok(serde_json::json!({ "deleted": true })));
    }
    if name.chars().count() > 14 {
        return Err(DomainError::Validation("文件夹名最多 14 字".into()));
    }
    match body.id {
        None => {
            let n: i64 = sqlx::query_scalar("SELECT count(*) FROM pmboxes WHERE user_id = $1")
                .bind(auth.id)
                .fetch_one(&state.repo.db)
                .await
                .unwrap_or(0);
            if n >= 3 {
                return Err(DomainError::Validation("最多 3 个文件夹".into()));
            }
            let id: i64 = sqlx::query_scalar(
                "INSERT INTO pmboxes (user_id, name) VALUES ($1, $2) \
                 ON CONFLICT (user_id, name) DO UPDATE SET name = EXCLUDED.name RETURNING id",
            )
            .bind(auth.id)
            .bind(name)
            .fetch_one(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            Ok(ok(serde_json::json!({ "id": id })))
        }
        Some(bid) => {
            let n = sqlx::query("UPDATE pmboxes SET name = $3 WHERE id = $2 AND user_id = $1")
                .bind(auth.id)
                .bind(bid)
                .bind(name)
                .execute(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?
                .rows_affected();
            if n == 0 {
                return Err(DomainError::NotFound(0));
            }
            Ok(ok(serde_json::json!({ "renamed": true })))
        }
    }
}

// ============ 咨询工作台（contactstaff → staffbox.php 口径） ============

#[derive(Deserialize)]
struct ContactReq {
    subject: String,
    body: String,
}

/// 用户提交咨询（takecontact.php 口径：写 staffmessages；普通用户 60s 防刷）
#[post("/contactstaff")]
async fn contact_staff(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ContactReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if auth.class_id < 90 {
        let last: Option<chrono::DateTime<chrono::Utc>> =
            sqlx::query_scalar("SELECT last_sent_at FROM message_flood WHERE user_id = $1")
                .bind(auth.id)
                .fetch_optional(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?
                .flatten();
        if let Some(t) = last {
            if (chrono::Utc::now() - t).num_seconds() < 60 {
                return Err(DomainError::Validation(
                    "发送过于频繁，请 1 分钟后再试".into(),
                ));
            }
        }
    }
    if body.subject.trim().is_empty() || body.body.trim().is_empty() {
        return Err(DomainError::Validation("主题与正文不能为空".into()));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO staffmessages (user_id, subject, body) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(auth.id)
    .bind(body.subject.trim())
    .bind(body.body.trim())
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "INSERT INTO message_flood (user_id, last_sent_at) VALUES ($1, now()) \
                 ON CONFLICT (user_id) DO UPDATE SET last_sent_at = now()",
    )
    .bind(auth.id)
    .execute(&state.repo.db)
    .await
    .ok();
    Ok(ok(serde_json::json!({ "id": id })))
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct StaffMessageRow {
    id: i64,
    username: Option<String>,
    subject: String,
    body: String,
    answered: i32,
    answered_by: Option<String>,
    answer: Option<String>,
    answered_at: Option<chrono::DateTime<chrono::Utc>>,
    permission: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

/// 咨询列表（staffbox.php）：全量给 staff（≥90）；permission 标签按标签名分流——
/// 与 NP 的权限位映射不同，这里简化为「全员可见普通咨询 + 带 permission 标签的定向分流
/// 也全员可见」，保留字段供未来按权限位过滤。
#[get("/staffmessages")]
async fn staff_messages(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<StaffMsgQuery>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::STAFF_MESSAGE).await?;
    // answered 解析：0/1/true/false → Option<i32>（非法值按未过滤处理，不再裸 400）
    let answered_i: Option<i32> = q.answered.as_deref().and_then(|v| match v {
        "0" | "false" => Some(0),
        "1" | "true" => Some(1),
        _ => v.parse::<i32>().ok().filter(|n| (0..=1).contains(n)),
    });
    let rows = sqlx::query_as::<_, StaffMessageRow>(
        "SELECT s.id, u.username, s.subject, s.body, s.answered, a.username AS answered_by, \
                s.answer, s.answered_at, s.permission, s.created_at \
         FROM staffmessages s \
         LEFT JOIN users u ON u.id = s.user_id \
         LEFT JOIN users a ON a.id = s.answered_by \
         WHERE ($1::int IS NULL OR s.answered = $1) \
         ORDER BY s.answered ASC, s.id DESC LIMIT 100",
    )
    .bind(answered_i)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct StaffMsgQuery {
    /// 审计修复（P2 信封）：serde 对 ?answered=false 直接反序列化报裸文本 400（击穿
    /// JSON 信封）。改为 String 自行解析，支持 0/1/true/false 四种形态。
    answered: Option<String>,
}

/// 我的工单（用户侧，审计修复 P1）：普通用户此前看不到自己提交的咨询进度——
/// contactstaff 提交后只能等 PM，状态 2（已答复待确认）也无法自行确认关闭。
/// 仅返回本人提交的记录（staff 视角走 /stafftickets）。
#[derive(sqlx::FromRow, serde::Serialize)]
struct MyTicketRow {
    id: i64,
    subject: String,
    body: String,
    ticket_status: i16,
    priority: i16,
    answer: Option<String>,
    answered_at: Option<chrono::DateTime<chrono::Utc>>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/me/staffmessages")]
async fn my_staff_messages(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let rows: Vec<MyTicketRow> = sqlx::query_as(
        "SELECT id, subject, body, ticket_status, priority, answer, answered_at, created_at \
         FROM staffmessages WHERE user_id = $1 ORDER BY id DESC LIMIT 50",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

/// 用户确认关闭工单（2 已答复待确认 → 3 关闭，闭环补全）：仅来信人本人可关。
#[derive(Deserialize)]
struct TicketConfirmReq {
    id: i64,
}

#[post("/me/staffmessages/confirm")]
async fn my_ticket_confirm(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<TicketConfirmReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let n = sqlx::query(
        "UPDATE staffmessages SET ticket_status = 3 \
         WHERE id = $1 AND user_id = $2 AND ticket_status = 2",
    )
    .bind(body.id)
    .bind(auth.id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation(
            "工单不存在、非本人或尚未答复（仅已答复待确认的工单可确认关闭）".into(),
        ));
    }
    Ok(ok(serde_json::json!({ "id": body.id, "ticket_status": 3 })))
}

#[derive(Deserialize)]
struct AnswerReq {
    id: i64,
    answer: String,
}

/// 答复（takeanswer 口径）：给来信人发一条私信 + 答复原文回写 staffmessages + 置已答复
#[post("/staffmessages/answer")]
async fn staff_answer(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<AnswerReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::STAFF_MESSAGE).await?;
    if body.answer.trim().is_empty() {
        return Err(DomainError::Validation("答复内容不能为空".into()));
    }
    let orig: Option<(i64, String)> =
        sqlx::query_as("SELECT user_id, subject FROM staffmessages WHERE id = $1 AND answered = 0")
            .bind(body.id)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((uid, subject)) = orig else {
        return Err(DomainError::Validation("来信不存在或已答复".into()));
    };
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // ① 给用户发私信（staff 直发，不走接收限制/防刷）
    sqlx::query(
        "INSERT INTO messages (sender_id, receiver_id, subject, body, location, saved, unread) \
         VALUES ($1, $2, $3, $4, 1, 1, true)",
    )
    .bind(auth.id)
    .bind(uid)
    .bind(format!("Re: {}", subject))
    .bind(body.answer.trim())
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // ② 回写答复原文 + 状态。
    // 审计修复（P1）：旧实现只写 answered=1 不动 ticket_status，"已答复"的单永远停在
    // "新"并被列表 ORDER BY ticket_status ASC 置顶。答复即推进到 2=已答复待确认。
    sqlx::query(
        "UPDATE staffmessages SET answered = 1, answered_by = $2, answer = $3, answered_at = now(), \
             ticket_status = CASE WHEN ticket_status < 2 THEN 2 ELSE ticket_status END \
         WHERE id = $1",
    )
    .bind(body.id)
    .bind(auth.id)
    .bind(body.answer.trim())
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "staffmsg_answer", Some(body.id))
        .await;
    Ok(ok(serde_json::json!({ "answered": true })))
}

#[derive(Deserialize)]
struct StaffMsgActionReq {
    ids: Vec<i64>,
}

/// 批量标记已答复
#[post("/staffmessages/mark")]
async fn staff_mark(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<StaffMsgActionReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::STAFF_MESSAGE).await?;
    let n = sqlx::query(
        "UPDATE staffmessages SET answered = 1, answered_by = COALESCE(answered_by, $2), answered_at = COALESCE(answered_at, now()), ticket_status = CASE WHEN ticket_status < 2 THEN 2 ELSE ticket_status END\
         WHERE id = ANY($1) AND answered = 0",
    )
    .bind(&body.ids)
    .bind(auth.id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    Ok(ok(serde_json::json!({ "marked": n })))
}

/// 删除来信（单条/批量）
#[post("/staffmessages/delete")]
async fn staff_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<StaffMsgActionReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::STAFF_MESSAGE).await?;
    let n = sqlx::query("DELETE FROM staffmessages WHERE id = ANY($1)")
        .bind(&body.ids)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    Ok(ok(serde_json::json!({ "deleted": n })))
}

// ============ 好友（M16） ============

#[derive(Deserialize)]
struct FriendReq {
    username: String,
}

#[post("/friends")]
async fn friend_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<FriendReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let fid: Option<i64> =
        sqlx::query_scalar("SELECT id FROM users WHERE username = $1 AND status < 2")
            .bind(&body.username)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .flatten();
    let Some(fid) = fid else {
        return Err(DomainError::NotFound(0));
    };
    if fid == auth.id {
        return Err(DomainError::Validation("不能添加自己".into()));
    }
    // 审计修复（P1 隐私）：旧版单方 INSERT 即成好友，可绕过 accept_pm='friends' 屏障。
    // 新流程：对方拉黑则拒绝；对方已申请我 → 双向转正为好友（接受）；否则写 pending 申请并通知。
    let blacklisted: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM friendships WHERE user_id = $2 AND friend_id = $1 AND list = 'black')",
    )
    .bind(auth.id)
    .bind(fid)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    if blacklisted {
        return Err(DomainError::Validation("对方拒绝了你的好友申请".into()));
    }
    let they_pending: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM friendships WHERE user_id = $2 AND friend_id = $1 AND list = 'pending')",
    )
    .bind(auth.id)
    .bind(fid)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    if they_pending {
        // 对方先申请过我：双向转正
        sqlx::query("UPDATE friendships SET list = 'friend' WHERE (user_id = $1 AND friend_id = $2) OR (user_id = $2 AND friend_id = $1)")
            .bind(auth.id)
            .bind(fid)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        let _ = sqlx::query(
            "INSERT INTO messages (sender_id, receiver_id, subject, body) VALUES (NULL, $1, $2, $3)",
        )
        .bind(fid)
        .bind("好友申请已通过")
        .bind(format!("用户 #{} 接受了你的好友申请，你们现在是好友了。", auth.id))
        .execute(&state.repo.db)
        .await;
        return Ok(ok(
            serde_json::json!({ "friend": body.username, "state": "friend" }),
        ));
    }
    sqlx::query("INSERT INTO friendships (user_id, friend_id, list) VALUES ($1, $2, 'pending') ON CONFLICT DO NOTHING")
        .bind(auth.id)
        .bind(fid)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let _ = sqlx::query(
        "INSERT INTO messages (sender_id, receiver_id, subject, body) VALUES (NULL, $1, $2, $3)",
    )
    .bind(fid)
    .bind("收到好友申请")
    .bind(format!(
        "用户 #{} 向你发送了好友申请。添加对方为好友即可接受。",
        auth.id
    ))
    .execute(&state.repo.db)
    .await;
    Ok(ok(
        serde_json::json!({ "friend": body.username, "state": "pending" }),
    ))
}

#[get("/friends")]
async fn friend_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    // 双向口径：我发出的行（user_id=我，含 pending/black）+ 他人发给我的 pending 申请（list='incoming'）。
    // 旧版只查 user_id=$1，被申请方永远看不到收到的申请 → pending 流程发得出、收不到。
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT u.username, f.list FROM friendships f JOIN users u ON u.id = f.friend_id WHERE f.user_id = $1 \
         UNION ALL \
         SELECT u.username, 'incoming' FROM friendships f JOIN users u ON u.id = f.user_id \
         WHERE f.friend_id = $1 AND f.list = 'pending' \
         ORDER BY 1 LIMIT 400",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct FriendActionReq {
    username: String,
    /// black = 拉黑；unblack = 取消拉黑
    action: String,
}

/// 好友操作：拉黑/取消拉黑入口（此前黑名单只有消费端没有生产端，PM 拦截成死代码）。
/// 拉黑语义：upsert 我为 user_id 的行 list='black'；并撤回我对对方的 pending 申请。
#[post("/friends/action")]
async fn friend_action(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<FriendActionReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let fid: Option<i64> =
        sqlx::query_scalar("SELECT id FROM users WHERE username = $1 AND status < 2")
            .bind(&body.username)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .flatten();
    let Some(fid) = fid else {
        return Err(DomainError::NotFound(0));
    };
    if fid == auth.id {
        return Err(DomainError::Validation("不能对自己操作".into()));
    }
    match body.action.as_str() {
        "black" => {
            sqlx::query(
                "INSERT INTO friendships (user_id, friend_id, list) VALUES ($1, $2, 'black') \
                 ON CONFLICT (user_id, friend_id) DO UPDATE SET list = 'black'",
            )
            .bind(auth.id)
            .bind(fid)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            sqlx::query(
                "DELETE FROM friendships WHERE user_id = $1 AND friend_id = $2 AND list = 'pending'",
            )
            .bind(auth.id)
            .bind(fid)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            Ok(ok(
                serde_json::json!({ "user": body.username, "state": "black" }),
            ))
        }
        "unblack" => {
            let n = sqlx::query(
                "DELETE FROM friendships WHERE user_id = $1 AND friend_id = $2 AND list = 'black'",
            )
            .bind(auth.id)
            .bind(fid)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .rows_affected();
            Ok(ok(
                serde_json::json!({ "user": body.username, "removed": n }),
            ))
        }
        _ => Err(DomainError::Validation(
            "action 需为 black / unblack".into(),
        )),
    }
}

/// 主配置：主 scope + 安装向导 + 经济路由 + 社区路由（单一 /api/v1 scope）
pub fn configure(cfg: &mut web::ServiceConfig) {
    let scope =
        crate::setup_http::mount_setup(crate::economy_http::mount_economy(crate::http::v1_scope()));
    let scope = mount_community(scope);
    let scope = crate::games_http::mount_games(crate::ops_http::mount_ops(
        crate::content_http::mount_content(scope),
    ));
    let scope = crate::admin_p3_http::mount_p3_tools(crate::admin_p2_http::mount_p2_tools(
        crate::admin_http::mount_admin(scope),
    ));
    let scope = crate::settings_http::mount_settings(scope);
    let scope = crate::push_http::mount_push(scope);
    let scope = crate::gaps_http::mount_gaps(scope);
    let scope = crate::rss_http::mount_rss(scope);
    let scope = crate::twofa_http::mount_twofa(scope);
    let scope = crate::compat_http::mount_compat(scope);
    let scope = crate::v4_http::mount_v4(scope);
    cfg.service(crate::openapi_http::mount_openapi(scope));
}

// ============ 通知偏好 + 站免池荣誉榜（0075） ============

/// 我的通知偏好（缺省键 = 开）。已知事件类见 0075 迁移注释。
#[get("/me/notice-prefs")]
async fn notice_prefs_get(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let prefs: serde_json::Value =
        sqlx::query_scalar("SELECT notice_prefs FROM users WHERE id = $1")
            .bind(auth.id)
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or(serde_json::json!({}));
    Ok(ok(prefs))
}

#[derive(Deserialize)]
struct NoticePrefsSetReq {
    key: String,
    enabled: bool,
}

/// 设置单项通知偏好（白名单键，防塞任意 JSON）
#[post("/me/notice-prefs")]
async fn notice_prefs_set(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<NoticePrefsSetReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    const KEYS: [&str; 9] = [
        "hr_prewarn",
        "hr_violation",
        "wishlist",
        "group_new_version",
        "resurrection",
        "class_promo",
        "gift",
        "comment_reply",
        "system",
    ];
    if !KEYS.contains(&body.key.as_str()) {
        return Err(DomainError::Validation(format!(
            "未知通知类 {}（可选：{}）",
            body.key,
            KEYS.join("/")
        )));
    }
    sqlx::query(
        "UPDATE users SET notice_prefs = jsonb_set(notice_prefs, ARRAY[$2], to_jsonb($3::boolean)) WHERE id = $1",
    )
    .bind(auth.id)
    .bind(&body.key)
    .bind(body.enabled)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(
        serde_json::json!({ "key": body.key, "enabled": body.enabled }),
    ))
}

/// 站免池贡献荣誉榜（0075，AB 池页口径：本月 + 累计，公开可见）
#[get("/pool/honor")]
async fn pool_honor(state: web::Data<std::sync::Arc<AppState>>) -> DomainResult<impl Responder> {
    let rows: Vec<(i64, String, bool, i64, i64)> = sqlx::query_as(
        "SELECT id, username, donor, this_month, total FROM v_pool_honor          ORDER BY total DESC LIMIT 50",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

// ============ 工单体系（0078，U3D Ticket 口径，v3 §27-16） ============
// staffmessages 工单化：优先级/指派/四态流转，复用 STAFF_MESSAGE 权限与答复链路。

#[derive(sqlx::FromRow, serde::Serialize)]
struct TicketRow {
    id: i64,
    username: Option<String>,
    subject: String,
    priority: i16,
    assigned_to: Option<String>,
    ticket_status: i16,
    created_at: chrono::DateTime<chrono::Utc>,
    answered_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// 工单列表（staff）：按状态过滤，优先级降序 + 新单在前
#[get("/stafftickets")]
async fn ticket_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<std::collections::HashMap<String, String>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::STAFF_MESSAGE).await?;
    let status: Option<i16> = q
        .get("status")
        .and_then(|s| s.parse::<i16>().ok())
        .filter(|s| (0..=3).contains(s));
    let rows = sqlx::query_as::<_, TicketRow>(
        "SELECT s.id, u.username, s.subject, s.priority, a.username AS assigned_to, \
                s.ticket_status, s.created_at, s.answered_at \
         FROM staffmessages s \
         LEFT JOIN users u ON u.id = s.user_id \
         LEFT JOIN users a ON a.id = s.assigned_to \
         WHERE ($1::smallint IS NULL OR s.ticket_status = $1) \
         ORDER BY s.ticket_status ASC, s.priority DESC, s.id DESC LIMIT 100",
    )
    .bind(status)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct TicketUpdateReq {
    id: i64,
    /// 0低 1中 2高 3紧急
    #[serde(default)]
    priority: Option<i16>,
    /// 0=新 1=处理中 2=已答复待确认 3=关闭
    #[serde(default)]
    ticket_status: Option<i16>,
    /// 指派给（用户名；空串=取消指派）
    #[serde(default)]
    assign: Option<String>,
}

/// 工单流转：改优先级/状态/指派（任意子集）。答复仍走既有 staff_answer（其顺带置 3）。
#[post("/stafftickets/update")]
async fn ticket_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<TicketUpdateReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::STAFF_MESSAGE).await?;
    if let Some(p) = body.priority {
        if !(0..=3).contains(&p) {
            return Err(DomainError::Validation("priority 需在 0-3".into()));
        }
    }
    if let Some(s) = body.ticket_status {
        if !(0..=3).contains(&s) {
            return Err(DomainError::Validation("ticket_status 需在 0-3".into()));
        }
    }
    // 审计修复（P1）：工单状态机此前无任何流转约束——已答复/关闭的单可随意回 0，
    // "已答复"单永远停在"新"被置顶。现在：
    //   0新 → 1处理中 → 2已答复待确认 → 3关闭 单向推进；
    //   3关闭 仅允许显式重开回 1处理中（不允许回 0，保留处理轨迹）。
    if let Some(new_s) = body.ticket_status {
        let cur: Option<i16> =
            sqlx::query_scalar("SELECT ticket_status FROM staffmessages WHERE id = $1")
                .bind(body.id)
                .fetch_optional(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?
                .flatten();
        let Some(cur_s) = cur else {
            return Err(DomainError::Validation("工单不存在".into()));
        };
        let legal = new_s > cur_s || (cur_s == 3 && new_s == 1);
        if !legal {
            return Err(DomainError::Validation(format!(
                "非法状态流转：{cur_s} → {new_s}（工单状态只能单向推进；关闭单仅可重开为处理中）"
            )));
        }
    }
    let assignee: Option<i64> = match &body.assign {
        Some(name) if !name.trim().is_empty() => {
            let uid: Option<i64> =
                sqlx::query_scalar("SELECT id FROM users WHERE username = $1 AND class_id >= 50")
                    .bind(name.trim())
                    .fetch_optional(&state.repo.db)
                    .await
                    .map_err(|e| DomainError::Internal(e.into()))?
                    .flatten();
            let Some(uid) = uid else {
                return Err(DomainError::Validation(
                    "指派对象不存在或不是工作人员（class≥50）".into(),
                ));
            };
            Some(uid)
        }
        Some(_) => None, // 空串 = 清指派（置 NULL，见下方 clear_assign）
        None => {
            // 未传 assign = 不动：取当前值回写（COALESCE 不更新语义）
            let cur: Option<i64> =
                sqlx::query_scalar("SELECT assigned_to FROM staffmessages WHERE id = $1")
                    .bind(body.id)
                    .fetch_optional(&state.repo.db)
                    .await
                    .map_err(|e| DomainError::Internal(e.into()))?
                    .flatten();
            cur
        }
    };
    // 审计修复（P1）：assign="" 走 COALESCE($4, assigned_to) 会被 NULL 吞回当前值，
    // 注释宣称的「空串=取消指派」从未生效。显式空串时改用 SET assigned_to = NULL。
    let clear_assign = matches!(body.assign.as_deref(), Some(s) if s.trim().is_empty());
    let n = if clear_assign {
        sqlx::query(
            "UPDATE staffmessages SET \
                priority = COALESCE($2, priority), \
                ticket_status = COALESCE($3, ticket_status), \
                assigned_to = NULL \
             WHERE id = $1",
        )
        .bind(body.id)
        .bind(body.priority)
        .bind(body.ticket_status)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected()
    } else {
        sqlx::query(
            "UPDATE staffmessages SET \
                priority = COALESCE($2, priority), \
                ticket_status = COALESCE($3, ticket_status), \
                assigned_to = COALESCE($4, assigned_to) \
             WHERE id = $1",
        )
        .bind(body.id)
        .bind(body.priority)
        .bind(body.ticket_status)
        .bind(assignee)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected()
    };
    if n == 0 {
        return Err(DomainError::NotFound(body.id));
    }
    state
        .repo
        .audit(Some(auth.id), "ticket_update", Some(body.id))
        .await;
    Ok(ok(serde_json::json!({ "id": body.id, "updated": n })))
}

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
#[get("/staff/leaks")]
async fn leak_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::BANS_MANAGE).await?;
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

#[derive(Deserialize)]
struct LeakResolveReq {
    id: i64,
    /// 1=确认泄露 2=误报
    verdict: i16,
}

/// 泄露事件裁决：确认泄露时通知全部 staff（走 staffmessages 分流），误报仅归档
#[post("/staff/leaks/resolve")]
async fn leak_resolve(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<LeakResolveReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::BANS_MANAGE).await?;
    if ![1, 2].contains(&body.verdict) {
        return Err(DomainError::Validation(
            "verdict 需为 1（确认）或 2（误报）".into(),
        ));
    }
    let n = sqlx::query(
        "UPDATE leak_events SET resolved = $2, resolved_by = $3 WHERE id = $1 AND resolved = 0",
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
            sqlx::query_as("SELECT user_id, kind, detail FROM leak_events WHERE id = $1")
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

#[get("/shoutbox/bot")]
async fn shoutbox_bot_help() -> impl Responder {
    ok(serde_json::json!({
        "commands": [
            { "cmd": "/free",  "desc": "当前生效的免费/双倍促销种子" },
            { "cmd": "/stats", "desc": "站点实时统计（用户/种子/做种）" },
            { "cmd": "/me",    "desc": "我的数据摘要（上传/下载/分享率/魔力）" },
            { "cmd": "/help",  "desc": "命令列表" },
        ]
    }))
}

/// 命令分发（GET 供前端在发送 /命令 时调用并展示回话）
#[get("/shoutbox/bot/exec")]
async fn shoutbox_bot_exec(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<std::collections::HashMap<String, String>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let cmd = q.get("cmd").map(|s| s.trim()).unwrap_or("");
    let reply = match cmd {
        "/free" => {
            let rows: Vec<(i64, String)> = sqlx::query_as(
                "SELECT t.id, t.name FROM promotions p JOIN torrents t ON t.id = p.torrent_id \
                 WHERE p.starts_at <= now() AND p.ends_at > now() \
                   AND p.kind IN ('free','x2free') ORDER BY p.ends_at LIMIT 5",
            )
            .fetch_all(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            if rows.is_empty() {
                "当前没有免费促销的种子。".to_string()
            } else {
                format!(
                    "当前免费：{}",
                    rows.iter()
                        .map(|(id, name)| format!("#{} {}", id, name))
                        .collect::<Vec<_>>()
                        .join("；")
                )
            }
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
                "SELECT uploaded, downloaded, spark_balance FROM users WHERE id = $1",
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
    Ok(ok(serde_json::json!({ "cmd": cmd, "reply": reply })))
}
