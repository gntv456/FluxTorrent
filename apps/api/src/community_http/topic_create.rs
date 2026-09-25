//! M15 发主题（topic_create）。
//! 从 community_http.rs 按域拆出。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use super::*;
use crate::dto::ok;
use crate::economy_http::earn_spark;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[derive(Deserialize)]
pub(super) struct TopicCreateReq {
    pub(super) forum_id: i64,
    pub(super) title: String,
    pub(super) body: String,
    /// 帖子类型（0115）：normal|bounty|poll|lottery，缺省 normal；本阶段仅落库，形态逻辑后续迁移
    #[serde(default)]
    pub(super) topic_type: Option<String>,
    /// 论坛标签（0123）：tag_dict id 数组，最多 5 个，超出截断；禁用/不存在 id 静默丢弃
    #[serde(default)]
    pub(super) tags: Vec<i32>,
    /// 悬赏金额（0124）：topic_type=bounty 时生效，>0 冻结；其余类型忽略（防借普通帖试探字段）
    #[serde(default)]
    bounty_spark: Option<i64>,
    /// 投票选项（0125）：topic_type=poll 时生效，2~10 项非空文本；其余类型忽略
    #[serde(default)]
    pub(super) poll_options: Vec<String>,
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
pub(super) fn normalize_topic_type(raw: Option<&str>) -> &'static str {
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
    // 视频内嵌（0190）：每帖视频块上限（!video 出现次数，0=不限）
    check_video_count(&state.repo.db, &body.body).await?;
    // 敏感词（Phase3）：标题与正文一起过闸
    check_banned_words(
        &state.repo.db,
        &format!("{}\n{}", body.title, body.body),
    )
    .await?;
    let perm =
        forum_access(&state.repo.db, auth.id, auth.class_id, body.forum_id)
            .await?;
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
            Some(b) if b < 0 => {
                return Err(DomainError::Validation("悬赏金额不能为负".into()))
            }
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
    let (lot_winners, lot_prize, lot_ticket, lot_hours) = if ttype == "lottery"
    {
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
        "INSERT INTO topics (forum_id, user_id, title, topic_type, \
         bounty_spark) VALUES ($1, $2, $3, $4, $5) RETURNING id",
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
    sqlx::query(
        "INSERT INTO posts (id, topic_id, user_id, body, \
     body_text) VALUES (nextval('posts_id_seq'), $1, $2, $3, $4)",
    )
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
        sqlx::query(
            "INSERT INTO topic_polls (topic_id, options) VALUES ($1, $2)",
        )
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
