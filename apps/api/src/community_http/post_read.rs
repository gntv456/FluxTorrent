//! M15 帖子读取（楼层行/主题详情）。
//! 从 community_http.rs 按域拆出。

use actix_web::{get, web, HttpRequest, Responder};
use serde::Deserialize;

use super::post_group::{group_by_root, PostRow};
use super::*;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

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
    let perm =
        forum_access(&state.repo.db, auth.id, auth.class_id, fid).await?;
    if !perm.can_read {
        return Err(DomainError::Forbidden);
    }
    let mut posts = sqlx::query_as::<_, PostRow>(
        "SELECT p.id, u.username, p.user_id, p.body, p.created_at, p.edited_at, p.edited_by, FALSE AS hidden, \
            u.avatar_url, c.name AS class_name, \
            u.created_at AS author_joined_at, \
            COALESCE(wm.j, '[]'::jsonb) AS worn_medals, \
            p.root_id, p.parent_id, rt.name AS reply_to_name, \
            (SELECT count(*) FROM post_likes pl WHERE pl.post_id = p.id) AS likes, \
            EXISTS(SELECT 1 FROM post_likes pl2 WHERE pl2.post_id = p.id AND pl2.user_id = $3) AS liked_by_me, \
            COALESCE((SELECT sum(t.spark) FROM post_tips t WHERE t.post_id = p.id), 0)::bigint AS tips, \
            (SELECT count(*) FROM post_tips t2 WHERE t2.post_id = p.id) AS tip_count \
         FROM posts p \
         LEFT JOIN users u ON u.id = p.user_id \
         LEFT JOIN user_classes c ON c.id = u.class_id \
         LEFT JOIN LATERAL ( \
             SELECT u2.username AS name FROM posts pp \
             LEFT JOIN users u2 ON u2.id = pp.user_id WHERE pp.id = p.parent_id \
         ) rt ON TRUE \
         LEFT JOIN LATERAL ( \
             SELECT json_agg(json_build_object( \
                 'name', m.name, 'asset_ref', m.asset_ref, \
                 'rarity', m.rarity))::jsonb AS j \
             FROM user_medals um JOIN medals m ON m.id = um.medal_id \
             WHERE um.user_id = p.user_id AND um.wearing \
               AND (um.expires_at IS NULL OR um.expires_at > now()) \
         ) wm ON TRUE \
         WHERE p.topic_id = $1 \
           AND (p.root_id IS NOT NULL OR ($2::bigint IS NULL OR p.id < $2)) \
           AND (p.root_id IS NULL OR EXISTS(SELECT 1 FROM posts t0 \
                WHERE t0.id = p.root_id \
                  AND ($2::bigint IS NULL OR t0.id < $2))) \
         ORDER BY p.id DESC LIMIT 200",
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
    let protected: bool =
        sqlx::query_scalar("SELECT protected FROM forums WHERE id = $1")
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
    if let Some(last_pid) = posts.iter().map(|p| p.id).max() {
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
    let favorites: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM topic_favorites WHERE topic_id = $1",
    )
    .bind(tid)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    let faved: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM topic_favorites WHERE topic_id = \
         $1 AND user_id = $2)",
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
    // 长帖游标（NP 分页口径）：先算好再消费 posts（楼中楼分组会 move）
    let has_more = posts.first().map(|p| p.id > 1).unwrap_or(false)
        && sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM posts WHERE \
             topic_id = $1 AND root_id IS NULL AND id < $2)",
        )
        .bind(tid)
        .bind(posts.first().map(|p| p.id).unwrap_or(0))
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or(false);
    let posts_with_replies = group_by_root(posts);
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
        // 楼中楼（0163）：顶层楼线性排列，楼中楼挂在所属顶层楼的 replies 里
        "posts": posts_with_replies,
        "has_more": has_more,
    })))
}
