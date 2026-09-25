//! 种子详情与文件（M03）：get/detail/files/thanks。
//! 从 torrents.rs 按域拆出。

use sqlx::PgPool;

use crate::errors::{DomainError, DomainResult};

use super::types::{FileRow, ThankRow, TorrentDetailRow, TorrentRow};

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
               t.imdb_id,
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

fn items_or_not_found(
    mut v: Vec<TorrentRow>,
    id: i64,
) -> DomainResult<TorrentRow> {
    if v.is_empty() {
        Err(DomainError::NotFound(id))
    } else {
        Ok(v.remove(0))
    }
}

pub async fn get_torrent_detail(
    db: &PgPool,
    id: i64,
    viewer: i64,
) -> DomainResult<TorrentDetailRow> {
    // 可见性与 get_torrent 同口径（2026-09-22 对齐修复）：过审 1 全员可见；
    // 待审 0/暂缓 4 仅 owner 与 staff——否则「编辑打回待审后 staff 的 aggregate
    // 立刻 404（主行可见、扩展块不可见的口径劈叉）」。
    // viewer 解析 owner（is_owner）但不能判定 staff，这里放宽到 owner；staff 态
    // 由 SQL 内 EXISTS 子查询判定（user_status 权威行 class_id >= 90）。
    let row = sqlx::query_as::<_, TorrentDetailRow>(
        r#"
        SELECT t.id, t.descr, t.numfiles, t.price, t.media_info->>'mediainfo' AS mediainfo,
               (t.owner_id = $2) AS is_owner,
               EXISTS(SELECT 1 FROM torrent_purchases p WHERE p.torrent_id = t.id AND p.user_id = $2) AS purchased,
               (SELECT count(*) FROM thanks th WHERE th.torrent_id = t.id) AS thanks_count,
               (SELECT count(*) FROM bookmarks b WHERE b.torrent_id = t.id) AS bookmark_count,
               GREATEST(
                   t.created_at,
                   COALESCE((SELECT max(s.completed_at) FROM snatches s WHERE s.torrent_id = t.id), t.created_at)
               ) AS last_action,
               (t.times_completed * 2 + 1)::bigint AS views,
               t.pos_state, t.pos_state_until, t.pick_type,
               '{}'::jsonb AS sections
        FROM torrents t
        WHERE t.id = $1 AND (
            t.approval_status = 1
            OR ((t.approval_status = 0 OR t.approval_status = 4)
                AND (t.owner_id = $2
                     OR EXISTS(SELECT 1 FROM users su
                         WHERE su.id = $2 AND su.class_id >= 90)))
        )
        "#,
    )
    .bind(id)
    .bind(viewer)
    .fetch_optional(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let mut row = row.ok_or(DomainError::NotFound(id))?;
    // 0087：sections 附带维度显示名与排序（section_kinds.label），前端直接渲染。
    // B2（2026-09-25）：`section_dict` 改 **LEFT JOIN** —— 六类型字段系统下自由值行
    // （dict_id IS NULL）原来会被内连接直接丢掉；同时按 `ordinal` 支持多值。
    let secs: Vec<(String, Option<i64>, Option<serde_json::Value>, i32, Option<String>,
                   String, i32, Option<String>)> = sqlx::query_as(
        "SELECT ts.kind, ts.dict_id, ts.value, ts.ordinal, \
                d.name, COALESCE(k.label, ts.kind) AS label, \
                COALESCE(k.sort, 999) AS sort, k.field_type \
         FROM torrent_sections ts \
         LEFT JOIN section_dict d ON d.id = ts.dict_id \
         LEFT JOIN section_kinds k ON k.kind = ts.kind \
         WHERE ts.torrent_id = $1 ORDER BY sort, ts.kind, ts.ordinal",
    )
    .bind(id)
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 按 kind 聚合（多值维度：name 取首个以兼容旧消费点，values 给全量有序数组）。
    // 单趟收集 → 按首次出现顺序输出，避免多次遍历。
    struct Agg {
        dict_id: Option<i64>,
        label: String,
        sort: i32,
        field_type: String,
        values: Vec<String>,
    }
    let mut aggs: Vec<(String, Agg)> = Vec::new();
    for (kind, dict_id, value, _ord, name, label, sort, field_type) in secs {
        let display = match (&name, &value) {
            (Some(n), _) => n.clone(),
            (None, Some(v)) => free_value_display(v),
            _ => String::new(),
        };
        match aggs.iter_mut().find(|(k, _)| *k == kind) {
            Some((_, a)) => a.values.push(display),
            None => aggs.push((
                kind,
                Agg {
                    dict_id,
                    label,
                    sort,
                    field_type: field_type
                        .unwrap_or_else(|| "select".to_string()),
                    values: vec![display],
                },
            )),
        }
    }
    let mut m = serde_json::Map::new();
    for (kind, a) in aggs {
        let first = a.values.first().cloned().unwrap_or_default();
        m.insert(
            kind,
            serde_json::json!({
                "dict_id": a.dict_id,
                "name": first,
                "label": a.label,
                "sort": a.sort,
                "field_type": a.field_type,
                "values": a.values,
            }),
        );
    }
    row.sections = serde_json::Value::Object(m);
    Ok(row)
}

/// 自由值的展示串（text 直出；number/bool/date 转字符串）。
fn free_value_display(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}

pub async fn list_files(
    db: &PgPool,
    torrent_id: i64,
) -> DomainResult<Vec<FileRow>> {
    sqlx::query_as::<_, FileRow>(
        "SELECT file_index, path, \
         size FROM files WHERE torrent_id = $1 ORDER BY file_index LIMIT 500",
    )
    .bind(torrent_id)
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))
}

pub async fn list_thanks(
    db: &PgPool,
    torrent_id: i64,
) -> DomainResult<Vec<ThankRow>> {
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

pub async fn thank(
    db: &PgPool,
    torrent_id: i64,
    user_id: i64,
) -> DomainResult<()> {
    let res = sqlx::query(
        "INSERT INTO thanks (torrent_id, user_id) VALUES ($1, $2) ON \
         CONFLICT DO NOTHING",
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

pub async fn bookmark(
    db: &PgPool,
    torrent_id: i64,
    user_id: i64,
    on: bool,
) -> DomainResult<()> {
    if on {
        sqlx::query(
            "INSERT INTO bookmarks (user_id, torrent_id) VALUES \
             ($1, $2) ON CONFLICT DO NOTHING",
        )
        .bind(user_id)
        .bind(torrent_id)
        .execute(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    } else {
        sqlx::query(
            "DELETE FROM bookmarks WHERE user_id = $1 AND torrent_id = $2",
        )
        .bind(user_id)
        .bind(torrent_id)
        .execute(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }
    Ok(())
}
