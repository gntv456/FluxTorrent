//! 种子域仓储与查询（M02/M03）：游标分页 + 覆盖索引（§6.2）。

use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::errors::{DomainError, DomainResult};

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct TorrentRow {
    pub id: i64,
    pub info_hash: String,
    pub name: String,
    pub small_descr: Option<String>,
    pub category_id: i32,
    /// 介质列（0087 起可空，仅为兼容老数据；新数据在 torrent_sections）
    pub medium_id: Option<i32>,
    pub grade_id: Option<i32>,
    pub edition_id: Option<i32>,
    pub size: i64,
    pub seeders: i32,
    pub leechers: i32,
    pub times_completed: i32,
    pub comments: i64,
    #[serde(rename = "official")]
    pub official_tag: bool,
    pub anonymous: bool,
    pub approval_status: i16,
    pub sticky: bool,
    pub owner_name: Option<String>,
    pub promotion: Option<String>,
    /// 进行中促销的截止时刻（列表展示「剩余时间」，参考站口径）
    pub promotion_ends_at: Option<chrono::DateTime<chrono::Utc>>,
    /// 媒体评分（media_info.rating，豆瓣/IMDb 口径由录入方决定；首页海报墙展示）
    pub rating: Option<String>,
    /// 海报图 URL（media_info.poster；缺省时前端用生成式海报兜底）
    pub poster: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// 详情页扩展字段（简介/文件数/感谢数；列表不需要，独立查询）
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TorrentDetailRow {
    pub id: i64,
    pub descr: Option<String>,
    pub numfiles: i32,
    pub thanks_count: i64,
    pub bookmark_count: i64,
    pub last_action: Option<chrono::DateTime<chrono::Utc>>,
    pub views: i64,
    /// 付费下载（0086）：定价（0 = 免费）
    pub price: i64,
    /// 当前用户是否已支付（owner 恒免）
    pub purchased: bool,
    pub is_owner: bool,
    /// 多维属性（第八轮 Section）：kind → { dict_id, name }
    pub sections: serde_json::Value,
}

/// 详情页文件列表（files 表；无记录时前端隐藏该区块）
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct FileRow {
    pub file_index: i32,
    pub path: String,
    pub size: i64,
}

/// 感谢者列表（近 50 人）
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ThankRow {
    pub username: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Default, Serialize)]
pub struct TorrentFilter {
    /// 分类筛选（0088 起支持多选：数组传 ANY 命中；空数组 = 不过滤）
    pub category_id: Option<Vec<i32>>,
    pub medium_id: Option<i32>,
    pub grade_id: Option<i32>,
    pub edition_id: Option<i32>,
    pub official: Option<bool>,
    pub include_dead: bool,
    /// 查看未过审（待审/被拒）种子——需 torrent.see_banned 权限，端点侧校验
    pub include_unapproved: bool,
    pub search: Option<String>,
    /// 列表排序（旧站 torrents.php 口径）：created（默认）/ seeders / size / completed
    pub sort: Option<String>,
    /// 标签筛选（T-04）：tag_dict.id，命中 tags 关联
    pub tag_id: Option<i32>,
    /// 第八轮 Section 多维筛选：kind → dict_id（kind 走白名单，dict_id 为整数，拼接安全）
    #[serde(default)]
    pub sections: Vec<(String, i64)>,
    /// 搜索范围（旧站口径）：0=标题(默认) 1=副标题/简介 3=发布者 4=IMDb
    #[serde(default)]
    pub search_area: Option<i32>,
    /// 匹配模式：0=AND 模糊(默认) 2=精确等值
    #[serde(default)]
    pub search_mode: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TorrentPage {
    pub items: Vec<TorrentRow>,
    pub next_cursor: Option<String>,
    pub total_estimate: i64,
}

const MAX_LIMIT: i64 = 50;

pub async fn list_torrents(
    db: &PgPool,
    filter: &TorrentFilter,
    cursor: Option<i64>,
    limit: i64,
) -> DomainResult<TorrentPage> {
    let limit = limit.clamp(1, MAX_LIMIT);
    list_torrents_noclamp(db, filter, cursor, limit).await
}

/// 不做 50 上限钳制的列表查询：仅供 Torznab 等需要 offset+limit>50 深翻页的外部端点，
/// 调用方必须自行 clamp 防滥用（见 economy_http torznab_search）。
pub async fn list_torrents_noclamp(
    db: &PgPool,
    filter: &TorrentFilter,
    cursor: Option<i64>,
    limit: i64,
) -> DomainResult<TorrentPage> {
    let limit = limit.max(1);
    // 匹配模式（旧站 torrents.php 口径）：0/缺省 = AND 模糊；2 = 精确等值（不带通配）
    let exact = filter.search_mode == Some(2);
    let pattern = filter.search.as_deref().map(|s| {
        if exact {
            s.to_string()
        } else {
            format!(
                "%{}%",
                s.replace('\\', "").replace('%', "\\%").replace('_', "\\_")
            )
        }
    });

    // 排序白名单（防注入）；非 id 排序时退化为 OFFSET 无关的「前 N 截断」：
    // 排序键 + id 组成稳定排序，游标仍按 id 翻页（与默认排序一致，简单可靠）。
    // 置顶口径（0063 起；0089 扩展二级置顶）：pos_state 1=一级 2=二级，pos_state_until 到期自动回落，
    // 旧列 sticky 仍被官种联动使用——「任一生效即置顶」，一级 > 二级 > 普通置顶。
    let sticky_expr = "(GREATEST(t.sticky::int, CASE WHEN t.pos_state IN (1, 2) AND (t.pos_state_until IS NULL OR t.pos_state_until > now()) THEN CASE t.pos_state WHEN 1 THEN 2 WHEN 2 THEN 1 ELSE 0 END ELSE 0 END)) DESC";
    let order = match filter.sort.as_deref() {
        Some("seeders") => format!("{sticky_expr}, t.seeders DESC, t.id DESC"),
        Some("size") => format!("{sticky_expr}, t.size DESC, t.id DESC"),
        Some("completed") => format!("{sticky_expr}, t.times_completed DESC, t.id DESC"),
        _ => format!("{sticky_expr}, t.id DESC"),
    };
    // 第八轮 Section 多维筛选：每个维度一个子查询谓词（kind 以 section_kinds 存在性校验 + i64 内插，无注入面）
    let mut sec_sql = String::new();
    for (kind, dict_id) in &filter.sections {
        if !crate::admin_p3_http::is_custom_kind(db, kind).await {
            continue;
        }
        sec_sql.push_str(&format!(
            " AND t.id IN (SELECT torrent_id FROM torrent_sections WHERE kind = '{kind}' AND dict_id = {dict_id})"
        ));
    }
    // 搜索范围分流（旧站口径）：0=标题+全字段(默认) 1=副标题/简介 3=发布者 4=IMDb
    let esc = if exact { "" } else { " ESCAPE chr(92)" };
    let search_pred = match filter.search_area.unwrap_or(0) {
        1 => format!("AND ($7::text IS NULL OR t.small_descr ILIKE $7{esc} OR t.descr ILIKE $7{esc})"),
        3 => format!("AND ($7::text IS NULL OR u.username ILIKE $7{esc})"),
        4 => format!("AND ($7::text IS NULL OR t.media_info->>'imdb' ILIKE $7{esc} OR t.descr ILIKE $7{esc})"),
        _ => format!("AND ($7::text IS NULL OR t.name ILIKE $7{esc}                OR t.small_descr ILIKE $7{esc}                OR t.descr ILIKE $7{esc}                OR t.id IN (SELECT torrent_id FROM files WHERE path ILIKE $7{esc}))"),
    };

    let sql = format!(
        r#"
        SELECT t.id, t.info_hash, t.name, t.small_descr, t.category_id, t.medium_id,
               t.grade_id, t.edition_id, t.size, t.seeders, t.leechers, t.times_completed,
               (SELECT count(*) FROM comments c WHERE c.torrent_id = t.id) AS comments,
               t.official_tag, t.anonymous, t.approval_status, t.sticky,
               CASE WHEN t.anonymous THEN NULL ELSE u.username END AS owner_name,
               (SELECT p.kind::text FROM promotions p
                  WHERE p.starts_at <= now() AND p.ends_at > now() AND (
                    p.torrent_id = t.id
                    OR (p.torrent_id IS NULL AND (
                        p.scope = 'global'
                        OR (p.scope = 'official' AND t.official_tag)
                        OR (p.scope = 'non_official' AND NOT t.official_tag)
                        OR (p.scope = 'category' AND t.category_id = p.category_id))))
                  ORDER BY CASE p.kind::text WHEN 'x2free' THEN 6 WHEN 'x2half' THEN 5 WHEN 'x2' THEN 4 WHEN 'free' THEN 3 WHEN 'half' THEN 2 WHEN 'p30' THEN 1 ELSE 0 END DESC, p.id DESC LIMIT 1) AS promotion,
               (SELECT p.ends_at FROM promotions p
                  WHERE p.starts_at <= now() AND p.ends_at > now() AND (
                    p.torrent_id = t.id
                    OR (p.torrent_id IS NULL AND (
                        p.scope = 'global'
                        OR (p.scope = 'official' AND t.official_tag)
                        OR (p.scope = 'non_official' AND NOT t.official_tag)
                        OR (p.scope = 'category' AND t.category_id = p.category_id))))
                  ORDER BY CASE p.kind::text WHEN 'x2free' THEN 6 WHEN 'x2half' THEN 5 WHEN 'x2' THEN 4 WHEN 'free' THEN 3 WHEN 'half' THEN 2 WHEN 'p30' THEN 1 ELSE 0 END DESC, p.id DESC LIMIT 1) AS promotion_ends_at,
               t.media_info->>'rating' AS rating,
               t.media_info->>'poster' AS poster,
               t.created_at
        FROM torrents t
        LEFT JOIN users u ON u.id = t.owner_id
        WHERE (t.approval_status = 1 OR $11::bool)
          AND ($1::int[] IS NULL OR t.category_id = ANY($1))
          AND ($2::int IS NULL OR t.medium_id = $2)
          AND ($3::int IS NULL OR t.grade_id = $3)
          AND ($4::int IS NULL OR t.edition_id = $4)
          AND ($5::bool IS NULL OR t.official_tag = $5)
          AND ($6::bool OR t.seeders > 0)
          {search_pred}
          AND ($8::bigint IS NULL OR t.id < $8)
          AND ($10::int IS NULL OR t.id IN (SELECT torrent_id FROM tags WHERE tag_id = $10))
          {sec_sql}
        ORDER BY {order}
        LIMIT $9
        "#
    );
    let rows = sqlx::query_as::<_, TorrentRow>(&sql)
        .bind(filter.category_id.clone())
        .bind(filter.medium_id)
        .bind(filter.grade_id)
        .bind(filter.edition_id)
        .bind(filter.official)
        .bind(filter.include_dead)
        .bind(&pattern)
        .bind(cursor)
        .bind(limit + 1)
        .bind(filter.tag_id)
        .bind(filter.include_unapproved)
        .fetch_all(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM torrents t WHERE (t.approval_status = 1 OR $8::bool) \
         AND ($1::int[] IS NULL OR t.category_id = ANY($1)) AND ($2::int IS NULL OR t.medium_id = $2) \
         AND ($3::int IS NULL OR t.grade_id = $3) AND ($4::int IS NULL OR t.edition_id = $4) \
         AND ($5::bool IS NULL OR t.official_tag = $5) AND ($6::bool OR t.seeders > 0) \
         AND ($7::text IS NULL OR t.name ILIKE $7 ESCAPE chr(92) OR t.small_descr ILIKE $7 ESCAPE chr(92) \
          OR t.descr ILIKE $7 ESCAPE chr(92) \
          OR t.id IN (SELECT torrent_id FROM files WHERE path ILIKE $7 ESCAPE chr(92)))",
    )
    .bind(filter.category_id.clone())
    .bind(filter.medium_id)
    .bind(filter.grade_id)
    .bind(filter.edition_id)
    .bind(filter.official)
    .bind(filter.include_dead)
    .bind(&pattern)
    .bind(filter.include_unapproved)
    .fetch_one(db)
    .await
    .unwrap_or(0);

    let has_more = rows.len() as i64 > limit;
    let items = rows.into_iter().take(limit as usize).collect::<Vec<_>>();
    let next_cursor = has_more.then(|| items.last().map(|r| r.id.to_string()).unwrap_or_default());
    Ok(TorrentPage {
        items,
        next_cursor,
        total_estimate: total,
    })
}

pub async fn get_torrent(
    db: &PgPool,
    id: i64,
    reveal_owner: bool,
    viewer: Option<(i64, bool)>, // (user_id, is_staff)：G7 暂缓种仅本人+staff 可见
) -> DomainResult<TorrentRow> {
    // torrent.view_anonymous：持权者可见匿名种子的真实发布者
    let owner_expr = if reveal_owner {
        "u.username AS owner_name"
    } else {
        "CASE WHEN t.anonymous THEN NULL ELSE u.username END AS owner_name"
    };
    // G7 POSTPONED：status=4 的种子只对发布者本人与 staff 开放；
    // staff（viewer.is_staff）或本人（owner_id = viewer.0）时放宽到 approval_status IN (1,4)
    // 审计修复（P1）：本人对 status=0 待审种子也应可见（上传后马上看详情不再 404）
    let vis = match viewer {
        Some((uid, is_staff)) => format!(
            "(t.approval_status = 1 OR ((t.approval_status = 4 OR t.approval_status = 0) AND ({is_staff} OR t.owner_id = {uid})))"
        ),
        None => "t.approval_status = 1".to_string(),
    };
    let page = sqlx::query_as::<_, TorrentRow>(
        &format!(r#"
        SELECT t.id, t.info_hash, t.name, t.small_descr, t.category_id, t.medium_id,
               t.grade_id, t.edition_id, t.size, t.seeders, t.leechers, t.times_completed,
               (SELECT count(*) FROM comments c WHERE c.torrent_id = t.id) AS comments,
               t.official_tag, t.anonymous, t.approval_status, t.sticky,
               {owner_expr},
               (SELECT p.kind::text FROM promotions p
                  WHERE p.starts_at <= now() AND p.ends_at > now() AND (
                    p.torrent_id = t.id
                    OR (p.torrent_id IS NULL AND (
                        p.scope = 'global'
                        OR (p.scope = 'official' AND t.official_tag)
                        OR (p.scope = 'non_official' AND NOT t.official_tag)
                        OR (p.scope = 'category' AND t.category_id = p.category_id))))
                  ORDER BY CASE p.kind::text WHEN 'x2free' THEN 6 WHEN 'x2half' THEN 5 WHEN 'x2' THEN 4 WHEN 'free' THEN 3 WHEN 'half' THEN 2 WHEN 'p30' THEN 1 ELSE 0 END DESC, p.id DESC LIMIT 1) AS promotion,
               (SELECT p.ends_at FROM promotions p
                  WHERE p.starts_at <= now() AND p.ends_at > now() AND (
                    p.torrent_id = t.id
                    OR (p.torrent_id IS NULL AND (
                        p.scope = 'global'
                        OR (p.scope = 'official' AND t.official_tag)
                        OR (p.scope = 'non_official' AND NOT t.official_tag)
                        OR (p.scope = 'category' AND t.category_id = p.category_id))))
                  ORDER BY CASE p.kind::text WHEN 'x2free' THEN 6 WHEN 'x2half' THEN 5 WHEN 'x2' THEN 4 WHEN 'free' THEN 3 WHEN 'half' THEN 2 WHEN 'p30' THEN 1 ELSE 0 END DESC, p.id DESC LIMIT 1) AS promotion_ends_at,
               t.media_info->>'rating' AS rating,
               t.media_info->>'poster' AS poster,
               t.created_at
        FROM torrents t LEFT JOIN users u ON u.id = t.owner_id
        WHERE t.id = $1 AND {vis}
        "#)
    )
    .bind(id)
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    items_or_not_found(page, id)
}

fn items_or_not_found(mut v: Vec<TorrentRow>, id: i64) -> DomainResult<TorrentRow> {
    if v.is_empty() {
        Err(DomainError::NotFound(id))
    } else {
        Ok(v.remove(0))
    }
}

pub async fn get_torrent_detail(db: &PgPool, id: i64, viewer: i64) -> DomainResult<TorrentDetailRow> {
    let row = sqlx::query_as::<_, TorrentDetailRow>(
        r#"
        SELECT t.id, t.descr, t.numfiles, t.price,
               (t.owner_id = $2) AS is_owner,
               EXISTS(SELECT 1 FROM torrent_purchases p WHERE p.torrent_id = t.id AND p.user_id = $2) AS purchased,
               (SELECT count(*) FROM thanks th WHERE th.torrent_id = t.id) AS thanks_count,
               (SELECT count(*) FROM bookmarks b WHERE b.torrent_id = t.id) AS bookmark_count,
               GREATEST(
                   t.created_at,
                   COALESCE((SELECT max(s.completed_at) FROM snatches s WHERE s.torrent_id = t.id), t.created_at)
               ) AS last_action,
               (t.times_completed * 2 + 1)::bigint AS views
        FROM torrents t
        WHERE t.id = $1 AND t.approval_status = 1
        "#,
    )
    .bind(id)
    .bind(viewer)
    .fetch_optional(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let mut row = row.ok_or(DomainError::NotFound(id))?;
    // 0087：sections 附带维度显示名与排序（section_kinds.label），前端直接渲染
    let secs: Vec<(String, i64, String, String, i32)> = sqlx::query_as(
        "SELECT ts.kind, ts.dict_id, d.name, COALESCE(k.label, ts.kind) AS label, COALESCE(k.sort, 999) AS sort \
         FROM torrent_sections ts \
         JOIN section_dict d ON d.id = ts.dict_id \
         LEFT JOIN section_kinds k ON k.kind = ts.kind \
         WHERE ts.torrent_id = $1 ORDER BY sort, ts.kind",
    )
    .bind(id)
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let mut m = serde_json::Map::new();
    for (kind, dict_id, name, label, sort) in secs {
        m.insert(
            kind,
            serde_json::json!({ "dict_id": dict_id, "name": name, "label": label, "sort": sort }),
        );
    }
    row.sections = serde_json::Value::Object(m);
    Ok(row)
}

pub async fn list_files(db: &PgPool, torrent_id: i64) -> DomainResult<Vec<FileRow>> {
    sqlx::query_as::<_, FileRow>(
        "SELECT file_index, path, size FROM files WHERE torrent_id = $1 ORDER BY file_index LIMIT 500",
    )
    .bind(torrent_id)
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))
}

pub async fn list_thanks(db: &PgPool, torrent_id: i64) -> DomainResult<Vec<ThankRow>> {
    sqlx::query_as::<_, ThankRow>(
        r#"
        SELECT u.username, th.created_at
        FROM thanks th LEFT JOIN users u ON u.id = th.user_id
        WHERE th.torrent_id = $1
        ORDER BY th.created_at DESC LIMIT 50
        "#,
    )
    .bind(torrent_id)
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct CommentRow {
    pub id: i64,
    pub torrent_id: i64,
    pub username: Option<String>,
    pub body: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn list_comments(
    db: &PgPool,
    torrent_id: i64,
    limit: i64,
) -> DomainResult<Vec<CommentRow>> {
    sqlx::query_as::<_, CommentRow>(
        "SELECT c.id, c.torrent_id, u.username, c.body, c.created_at \
         FROM comments c LEFT JOIN users u ON u.id = c.user_id \
         WHERE c.torrent_id = $1 ORDER BY c.id DESC LIMIT $2",
    )
    .bind(torrent_id)
    .bind(limit.clamp(1, MAX_LIMIT))
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))
}

pub async fn add_comment(
    db: &PgPool,
    torrent_id: i64,
    user_id: i64,
    body: &str,
) -> DomainResult<i64> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM torrents WHERE id = $1 AND approval_status = 1)",
    )
    .bind(torrent_id)
    .fetch_one(db)
    .await
    .unwrap_or(false);
    if !exists {
        return Err(DomainError::NotFound(torrent_id));
    }
    if body.trim().is_empty() {
        return Err(DomainError::Validation("评论不能为空".into()));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO comments (torrent_id, user_id, body) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(torrent_id)
    .bind(user_id)
    .bind(body)
    .fetch_one(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(id)
}

pub async fn thank(db: &PgPool, torrent_id: i64, user_id: i64) -> DomainResult<()> {
    let res = sqlx::query(
        "INSERT INTO thanks (torrent_id, user_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(torrent_id)
    .bind(user_id)
    .execute(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if res.rows_affected() == 0 {
        return Err(DomainError::AlreadyThanked);
    }
    Ok(())
}

pub async fn bookmark(db: &PgPool, torrent_id: i64, user_id: i64, on: bool) -> DomainResult<()> {
    if on {
        sqlx::query(
            "INSERT INTO bookmarks (user_id, torrent_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
        )
        .bind(user_id)
        .bind(torrent_id)
        .execute(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    } else {
        sqlx::query("DELETE FROM bookmarks WHERE user_id = $1 AND torrent_id = $2")
            .bind(user_id)
            .bind(torrent_id)
            .execute(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    }
    Ok(())
}

/// 种子编辑（NP takeedit.php 作者口径）：name/small_descr/descr/anonymous/category/medium/grade/edition
/// 修改后回退到待审核（approval_status=0），走审核流重新过审。
pub struct TorrentEdit<'a> {
    pub name: Option<&'a str>,
    pub small_descr: Option<&'a str>,
    pub descr: Option<&'a str>,
    pub anonymous: Option<bool>,
    pub category_id: Option<i32>,
    pub medium_id: Option<i32>,
    pub grade_id: Option<i32>,
    pub edition_id: Option<i32>,
}

pub async fn edit_torrent(
    db: &PgPool,
    torrent_id: i64,
    editor: (i64, i16), // (user_id, class_id)：作者本人或 staff（>=90）
    e: &TorrentEdit<'_>,
) -> DomainResult<()> {
    let owner: Option<i64> = sqlx::query_scalar("SELECT owner_id FROM torrents WHERE id = $1")
        .bind(torrent_id)
        .fetch_optional(db)
        .await
        .map_err(|err| DomainError::Internal(err.into()))?
        .flatten();
    let Some(owner_id) = owner else {
        return Err(DomainError::NotFound(torrent_id));
    };
    if editor.1 < 90 && owner_id != editor.0 {
        return Err(DomainError::Forbidden);
    }
    let n = sqlx::query(
        r#"
        UPDATE torrents SET
            name = COALESCE($2, name),
            small_descr = COALESCE($3, small_descr),
            descr = COALESCE($4, descr),
            anonymous = COALESCE($5, anonymous),
            category_id = COALESCE($6, category_id),
            medium_id = COALESCE($7, medium_id),
            grade_id = COALESCE($8, grade_id),
            edition_id = COALESCE($9, edition_id),
            approval_status = 0,
            mtime = now()
        WHERE id = $1
        "#,
    )
    .bind(torrent_id)
    .bind(e.name)
    .bind(e.small_descr)
    .bind(e.descr)
    .bind(e.anonymous)
    .bind(e.category_id)
    .bind(e.medium_id)
    .bind(e.grade_id)
    .bind(e.edition_id)
    .execute(db)
    .await
    .map_err(|err| DomainError::Internal(err.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(torrent_id));
    }
    Ok(())
}

/// 种子软删除（NP delete.php 口径）：staff 或作者本人（未过审的可直接删；已过审的作者删除需 staff）
pub async fn delete_torrent(db: &PgPool, torrent_id: i64, actor: (i64, i16)) -> DomainResult<()> {
    let row: Option<(Option<i64>, i16)> =
        sqlx::query_as("SELECT owner_id, approval_status FROM torrents WHERE id = $1")
            .bind(torrent_id)
            .fetch_optional(db)
            .await
            .map_err(|err| DomainError::Internal(err.into()))?;
    let Some((owner_id, approval)) = row else {
        return Err(DomainError::NotFound(torrent_id));
    };
    let is_staff = actor.1 >= 90;
    let is_owner = owner_id == Some(actor.0);
    // staff 任意删；作者只能删自己未过审（pending/rejected）的种子
    if !is_staff && !(is_owner && approval != 1) {
        return Err(DomainError::Forbidden);
    }
    // 软删 + 清理关联促销（审计修复：促销残留会被计费/H&R 豁免回查误命中）
    let mut tx = db.begin().await.map_err(|err| DomainError::Internal(err.into()))?;
    sqlx::query("UPDATE torrents SET approval_status = 3, mtime = now() WHERE id = $1")
        .bind(torrent_id)
        .execute(&mut *tx)
        .await
        .map_err(|err| DomainError::Internal(err.into()))?;
    sqlx::query("DELETE FROM promotions WHERE torrent_id = $1")
        .bind(torrent_id)
        .execute(&mut *tx)
        .await
        .map_err(|err| DomainError::Internal(err.into()))?;
    tx.commit().await.map_err(|err| DomainError::Internal(err.into()))?;
    Ok(())
}

/// 恢复软删种子（approval_status 3 → 0 待审）：此前误删后只能直连数据库手工修数。
pub async fn restore_torrent(db: &PgPool, torrent_id: i64) -> DomainResult<()> {
    let n = sqlx::query(
        "UPDATE torrents SET approval_status = 0, mtime = now() WHERE id = $1 AND approval_status = 3",
    )
    .bind(torrent_id)
    .execute(db)
    .await
    .map_err(|err| DomainError::Internal(err.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(torrent_id));
    }
    Ok(())
}

/// 下载/做种记录（NP viewsnatches.php 口径）：snatches 联 users，活跃状态由 seeding/leeching 标记
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct SnatchRow {
    pub user_id: i64,
    pub username: String,
    pub uploaded: i64,
    pub downloaded: i64,
    pub seeded_seconds: i32,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub seeding: bool,
    pub leeching: bool,
}

pub async fn list_snatches(db: &PgPool, torrent_id: i64) -> DomainResult<Vec<SnatchRow>> {
    sqlx::query_as::<_, SnatchRow>(
        "SELECT s.user_id, u.username, s.uploaded, s.downloaded, s.seeded_seconds, \
                s.completed_at, s.seeding, s.leeching \
         FROM snatches s JOIN users u ON u.id = s.user_id \
         WHERE s.torrent_id = $1 \
         ORDER BY s.completed_at DESC NULLS LAST LIMIT 100",
    )
    .bind(torrent_id)
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))
}

/// NFO（NP viewnfo.php 口径）：纯文本返回。
/// 种子存在但 nfo 为 NULL 时返回 Ok(None)（无 NFO），而非 404——
/// 修复前 fetch_optional 展平后把「行存在列空」也当 NotFound。
pub async fn get_nfo(db: &PgPool, torrent_id: i64) -> DomainResult<Option<String>> {
    let row: Option<Option<String>> =
        sqlx::query_scalar("SELECT nfo FROM torrents WHERE id = $1 AND approval_status = 1")
            .bind(torrent_id)
            .fetch_optional(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    row.ok_or(DomainError::NotFound(torrent_id))
}

/// 请求补种（NP takereseed.php 口径）：
/// 仅限 seeders=0 的"死种"；向所有完成过下载的用户群发 PM；900 秒限频。
/// 返回通知人数。
pub async fn request_reseed(
    db: &PgPool,
    torrent_id: i64,
    requester: (i64, String),
) -> DomainResult<usize> {
    let row: Option<(i32, Option<chrono::DateTime<chrono::Utc>>, String)> = sqlx::query_as(
        "SELECT seeders, last_reseed, name FROM torrents WHERE id = $1 AND approval_status = 1",
    )
    .bind(torrent_id)
    .fetch_optional(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((seeders, last_reseed, name)) = row else {
        return Err(DomainError::NotFound(torrent_id));
    };
    if seeders > 0 {
        return Err(DomainError::Validation("该种子仍有人做种，无需补种".into()));
    }
    if let Some(lr) = last_reseed {
        if chrono::Utc::now() - lr < chrono::Duration::seconds(900) {
            return Err(DomainError::Validation(
                "15 分钟内已发起过补种请求，请稍候".into(),
            ));
        }
    }
    // 完成过下载的用户（含发布者）
    let receivers: Vec<i64> = sqlx::query_scalar(
        "SELECT DISTINCT user_id FROM snatches WHERE torrent_id = $1 AND completed_at IS NOT NULL \
         UNION SELECT owner_id FROM torrents WHERE id = $1 AND owner_id IS NOT NULL",
    )
    .bind(torrent_id)
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let subject = "【补种请求】".to_string();
    let body = format!(
        "用户 {} 请求补种：{}（/torrent/{}）。如果你保留了文件，欢迎重新做种，谢谢！",
        requester.1, name, torrent_id
    );
    for uid in &receivers {
        sqlx::query(
            "INSERT INTO messages (sender_id, receiver_id, subject, body) VALUES ($1, $2, $3, $4)",
        )
        .bind(requester.0)
        .bind(uid)
        .bind(&subject)
        .bind(&body)
        .execute(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }
    sqlx::query("UPDATE torrents SET last_reseed = now() WHERE id = $1")
        .bind(torrent_id)
        .execute(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(receivers.len())
}

/// 种子标签（T-04）：列出字典 + 该种子已打的标签
pub async fn list_tags(db: &PgPool, torrent_id: i64) -> DomainResult<serde_json::Value> {
    let dict: Vec<(i32, String, String)> =
        sqlx::query_as("SELECT id, name, kind FROM tag_dict ORDER BY id")
            .fetch_all(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let mine: Vec<i32> =
        sqlx::query_scalar("SELECT tag_id FROM tags WHERE torrent_id = $1 ORDER BY tag_id")
            .bind(torrent_id)
            .fetch_all(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(serde_json::json!({ "dict": dict, "mine": mine }))
}

/// 打/去标签（作者或 staff；官种标签仅 staff 可打）
pub async fn tag_torrent(
    db: &PgPool,
    torrent_id: i64,
    actor: (i64, i16),
    tag_id: i32,
    on: bool,
) -> DomainResult<()> {
    let row: Option<(Option<i64>, String)> = sqlx::query_as(
        "SELECT owner_id, kind FROM torrents t JOIN tag_dict d ON d.id = $2 WHERE t.id = $1",
    )
    .bind(torrent_id)
    .bind(tag_id)
    .fetch_optional(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((owner_id, kind)) = row else {
        return Err(DomainError::NotFound(torrent_id));
    };
    if actor.1 < 90 && owner_id != Some(actor.0) {
        return Err(DomainError::Forbidden);
    }
    if kind == "official" && actor.1 < 90 {
        return Err(DomainError::Forbidden); // 官种/官方标签仅 staff
    }
    if on {
        sqlx::query("INSERT INTO tags (torrent_id, tag_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
            .bind(torrent_id)
            .bind(tag_id)
            .execute(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    } else {
        sqlx::query("DELETE FROM tags WHERE torrent_id = $1 AND tag_id = $2")
            .bind(torrent_id)
            .bind(tag_id)
            .execute(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    }
    Ok(())
}

/// 付费下载扣费（0086，NP 价格 口径）：下载前调用。
/// 免费/发布者本人/已购 → 直接放行；否则原子扣款 → 发布者得 (100-税)% → 税入当月魔法池。
/// 全程单事务，余额不足返回校验错误。
pub async fn charge_for_download(db: &PgPool, user_id: i64, torrent_id: i64) -> DomainResult<()> {
    let row: Option<(i64, Option<i64>)> =
        sqlx::query_as("SELECT price, owner_id FROM torrents WHERE id = $1 AND approval_status = 1")
            .bind(torrent_id)
            .fetch_optional(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((price, owner_id)) = row else {
        return Err(DomainError::NotFound(torrent_id));
    };
    if price <= 0 || owner_id == Some(user_id) {
        return Ok(());
    }
    let purchased: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM torrent_purchases WHERE user_id = $1 AND torrent_id = $2)")
            .bind(user_id)
            .bind(torrent_id)
            .fetch_one(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    if purchased {
        return Ok(());
    }
    let tax: i32 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value::int FROM site_settings WHERE name = 'upload_price_tax'), 30)",
    )
    .fetch_one(db)
    .await
    .unwrap_or(30)
    .clamp(0, 90);
    let net = price * (100 - tax as i64) / 100;
    let tax_amount = price - net;
    let month = chrono::Utc::now().format("%Y-%m").to_string();

    let mut tx = db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 原子扣款：余额不足时 UPDATE 命中 0 行
    let paid: Option<i64> =
        sqlx::query_scalar("UPDATE users SET spark_balance = spark_balance - $2 WHERE id = $1 AND spark_balance >= $2 RETURNING spark_balance")
            .bind(user_id)
            .bind(price)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    if paid.is_none() {
        return Err(DomainError::Validation(format!(
            "魔力不足：该种子为付费种子（{price} 魔力），请先充值或签到攒魔力"
        )));
    }
    sqlx::query("UPDATE users SET spark_balance = spark_balance + $2 WHERE id = $1")
        .bind(owner_id)
        .bind(net)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if tax_amount > 0 {
        sqlx::query(
            "INSERT INTO magic_pool (month, donated_total) VALUES ($1, $2) \
             ON CONFLICT (month) DO UPDATE SET donated_total = magic_pool.donated_total + $2",
        )
        .bind(&month)
        .bind(tax_amount)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }
    sqlx::query(
        "INSERT INTO torrent_purchases (user_id, torrent_id, price) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
    )
    .bind(user_id)
    .bind(torrent_id)
    .bind(price)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(())
}

/// 站点统计（M10 概览，旧站首页口径）
pub async fn site_stats(db: &PgPool) -> DomainResult<serde_json::Value> {
    let users: i64 = sqlx::query_scalar("SELECT count(*) FROM users WHERE status < 2")
        .fetch_one(db)
        .await
        .unwrap_or(0);
    let torrents: i64 =
        sqlx::query_scalar("SELECT count(*) FROM torrents WHERE approval_status = 1")
            .fetch_one(db)
            .await
            .unwrap_or(0);
    let dead: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM torrents WHERE approval_status = 1 AND seeders = 0",
    )
    .fetch_one(db)
    .await
    .unwrap_or(0);
    let seed_size: i64 = sqlx::query_scalar(
        "SELECT COALESCE(sum(t.size), 0) FROM torrents t \
         JOIN (SELECT DISTINCT torrent_id FROM snatches WHERE seeding) s ON s.torrent_id = t.id",
    )
    .fetch_one(db)
    .await
    .unwrap_or(0);
    Ok(serde_json::json!({
        "users": users, "torrents": torrents, "dead": dead, "seed_size": seed_size
    }))
}
