//! M15 主题列表（topic_list）。
//! 从 community_http.rs 按域拆出。

use actix_web::{get, web, HttpRequest, Responder};
use serde::Deserialize;

use super::*;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::repo::community::forum_side as repo_side;
use crate::repo::user_prefs;
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
    /// 公告（0396）：版主标记，列表加公告徽标（样图 coral 底徽章）
    #[sqlx(default)]
    notice: bool,
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
    /// 页码（1 起）。此前本出口写死 `LIMIT 50` 且无任何翻页参数：
    /// 超 50 帖的版块第 51 帖起永远打不开，界面也长得像「就这么多了」。
    /// 与 /invites 同口径：page 走信封回显，前端 Pager 据此画页码。
    #[serde(default)]
    page: Option<i64>,
    /// 类型筛选（2026-10-17，对齐样图 § tplForumBoard 的筛选行）：
    /// sticky=只看置顶 / digest=只看精华。白名单常量拼接，非法值回落全量。
    /// 样图还有「只看公告」一档——本站 topics.topic_type 只有
    /// bounty/poll/lottery，没有公告这一类型，故不做（不为一枚装饰徽章
    /// 凭空加字段）。
    #[serde(default)]
    r#type: Option<String>,
}

/// 每页主题数：前端 Pager 与提示语共用这一个数，不许各写一份。
pub(crate) const TOPIC_PAGE_SIZE: i64 = 50;

/// 每页主题数的天花板。上限不自己拍：0039 给 `topics_per_page_default`
/// 登记的元数据就是 `min 1 / max 500`（后台那格控件按它校验），代码夹到
/// 同一个界，管理员填的数与程序给的数才不会两套口径。
/// 用户那一列（`users.topics_per_page`）是自由数字输入、写口不校验区间，
/// `topics_per_page = 100000` 是能被写进库里的 ⇒ 同款夹住，不拿 400
/// 打断设置页（P-275 在种子列表上就是这么处理的）。
const TOPIC_PAGE_MAX: i64 = 500;

/// 每页主题数的三层优先：本人偏好 > 站点默认 > 硬常量（P-449/P-450）。
///
/// · `pref` = `users.topics_per_page`（0024）。此前只有写口与回显读它，
///   面板填 2 照样一次给几十条——「能改能存能回显但没人读」的假开关。
///   0/负数 = 未设置（建表默认就是 0）。
/// · `site` = `site_settings.topics_per_page_default`（0025 播种 20、
///   0039 登记进 `/admin/settings` 的「主题列表默认每页」）。P-450 之前
///   它躺在 `fake_switch_baseline.json` 的存量里无人读：站长在后台改了，
///   版块页照旧 50 条。
/// 正数一律夹到 `TOPIC_PAGE_MAX`；两层都没给到合法正数才回落常量。
/// 前端 Pager 读的是回显的 `per`（`forums/[id]/page.tsx` 的 `data.per`），
/// 所以这里改一个数就够，不在两处各写一份。
fn page_size_of(pref: i32, site: Option<i32>) -> i64 {
    // 本人偏好压过站点默认
    if pref > 0 {
        return i64::from(pref).clamp(1, TOPIC_PAGE_MAX);
    }
    match site {
        Some(v) if v > 0 => i64::from(v).clamp(1, TOPIC_PAGE_MAX),
        _ => TOPIC_PAGE_SIZE,
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/forums/{id}/topics",
    tag = "社区/论坛",
    description = "某版块的主题列表（分页 + 置顶置序）。",
    params(
        ("id" = i64, description = "版块 id"),
        ("page" = Option<i64>, description = "页码，1 起，每页 50 条"),
        ("sort" = Option<String>, description = "hot=热度，缺省 new"),
        ("tag" = Option<String>, description = "标签 tag_dict id"),
    ),
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 403, description = "无该版块阅读权（code 2003）")))]
#[get("/forums/{id}/topics")]
pub(crate) async fn topic_list(
    req: HttpRequest,
    path: web::Path<i64>,
    state: web::Data<std::sync::Arc<AppState>>,
    q: crate::http::Q<TopicListQuery>,
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
        "t.sticky DESC, t.notice DESC, \
         ((SELECT count(*)-1 FROM posts p WHERE p.topic_id = t.id) * 2 \
           + (SELECT count(*) FROM post_likes pl WHERE pl.topic_id = t.id) \
           + t.views * 0.1) \
          / (1 + EXTRACT(EPOCH FROM (now() - COALESCE(t.last_post_at, t.created_at))) / 86400.0) DESC, t.id DESC"
    } else {
        "t.sticky DESC, t.notice DESC, t.id DESC"
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
    // 类型筛选：白名单常量，且**列表与计数共用同一段**（否则 total 会
    // 数出一批空页）。notice=公告（0396）。
    let type_filter = match q.r#type.as_deref() {
        Some("notice") => " AND t.notice",
        Some("sticky") => " AND t.sticky",
        Some("digest") => " AND t.digest",
        _ => "",
    };
    let where_sql = format!("t.forum_id = $1{tag_filter}{type_filter}");
    let sql = format!(
        "SELECT t.id, t.forum_id, t.title, u.username, \
            greatest( \
              (SELECT count(*) FROM posts p WHERE p.topic_id = t.id) - 1, 0 \
            ) AS replies, t.views, t.last_post_at, \
            t.sticky, t.locked, t.digest, t.notice, t.topic_type, \
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
         WHERE {where_sql} ORDER BY {order} \
         LIMIT $3 OFFSET $4"
    );
    // 站点默认那层走 `repo::settings::read`（按 name 读 site_settings 的
    // 唯一居所）；解析不出正整数就当没设，继续回落硬常量。
    let site_default =
        crate::repo::settings::read(&state.repo, "topics_per_page_default")
            .await;
    let per = page_size_of(
        user_prefs::board_per_page(&state.repo, auth.id).await,
        site_default
            .as_deref()
            .and_then(|v| v.trim().parse::<i32>().ok()),
    );
    let page = q.page.unwrap_or(1).clamp(1, 10_000);
    let rows = sqlx::query_as::<_, TopicRow>(&sql)
        .bind(fid)
        .bind(auth.id)
        .bind(per)
        .bind(crate::dto::page_offset(page, per))
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 总数必须与列表同一套 WHERE（含标签与类型谓词），否则前端按 total
    // 画出的页码里会有一批空页。SQL 住在 repo 层。
    let total =
        repo_side::count_board_topics(&state.repo, fid, &where_sql).await?;
    // 版块名与版块规则（0392）随列表一起回：避免前端只为拿标题/规则条
    // 再打一次 /forums。规则为 NULL 时前台整条规则条不渲染。
    let board: Option<(String, Option<String>)> =
        sqlx::query_as("SELECT name, rules FROM forums WHERE id = $1")
            .bind(fid)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let (forum_name, forum_rules) = match board {
        Some((n, r)) => (Some(n), r),
        None => (None, None),
    };
    Ok(ok(serde_json::json!({
        "forum_id": fid,
        "forum_name": forum_name,
        "forum_rules": forum_rules,
        "can_write": perm.can_write,
        "can_create": perm.can_create,
        "can_mod": perm.can_mod,
        "sort": sort,
        "tag": tag_id,
        "page": page,
        "per": per,
        "total": total,
        "topics": rows,
    })))
}

#[cfg(test)]
mod tests {
    use super::{page_size_of, TOPIC_PAGE_MAX, TOPIC_PAGE_SIZE};

    /// 每页条数的三层优先（本人偏好 > 站点默认 > 硬常量）与夹界。
    /// 这条是纯函数层——界面层要等二进制换掉才能验。
    #[test]
    fn per_page_folds_pref() {
        assert_eq!(page_size_of(0, None), TOPIC_PAGE_SIZE, "两层都没给");
        assert_eq!(page_size_of(-7, None), TOPIC_PAGE_SIZE, "负数同缺省");
        assert_eq!(page_size_of(2, Some(20)), 2, "偏好压过站点默认");
        assert_eq!(page_size_of(0, Some(20)), 20, "站点默认生效（P-450）");
        assert_eq!(
            page_size_of(0, Some(0)),
            TOPIC_PAGE_SIZE,
            "站点那格清空=回落常量"
        );
        assert_eq!(
            page_size_of(0, Some(-1)),
            TOPIC_PAGE_SIZE,
            "脏站点值不折成 0 页"
        );
        assert_eq!(
            page_size_of(0, Some(9_999)),
            TOPIC_PAGE_MAX,
            "站点默认夹到元数据上限"
        );
        assert_eq!(
            page_size_of(100_000, None),
            TOPIC_PAGE_MAX,
            "自由数字要夹住"
        );
        assert_eq!(page_size_of(1, None), 1, "下限不夹成 0 页");
    }
}
