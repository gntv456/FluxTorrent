//! M15 主题列表（topic_list）。
//! 从 community_http.rs 按域拆出。

use actix_web::{get, web, HttpRequest, Responder};
use serde::Deserialize;

use super::*;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

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
    let perm =
        forum_access(&state.repo.db, auth.id, auth.class_id, fid).await?;
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
    let forum_name: Option<String> =
        sqlx::query_scalar("SELECT name FROM forums WHERE id = $1")
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
