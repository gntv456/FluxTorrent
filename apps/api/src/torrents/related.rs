//! 相似推荐（2026-10-08 二轮实证缺口 P1）：详情页"相关种子"。
//!
//! 对标 UNIT3D（TMDB 关联）/ Gazelle（标签投票）。本项目低成本版：
//! 按「同版本组 > 同标签交集 > 标题相似 / 同分类」打分，排除自身与未过审，
//! 取前 N 条。理由：PT 场景同一作品多版本天然同组（0069 torrent_groups），
//! 标签人工标注质量高；trgm 相似作兜底。
//!
//! 不做个性化矩阵（成本高、冷启动差），保守命中即可；后续可叠 user 协同。
//!
//! ## 性能设计（2026-10-08 二轮加固）
//!
//! 候选集用 **UNION ALL 三段**而非裸 OR：
//! - 裸 `A OR B OR C` 会让规划器放弃所有索引（任一段不可索引则整体 SeqScan，
//!   实测确为 `Seq Scan on torrents`）；
//! - 拆开后各段独立可走索引：同组走 `torrents.group_id`、trgm 走
//!   `idx_torrents_name_trgm`（gin_trgm_ops）。
//!
//! trgm 段必须用 **`%` 运算符**粗筛——`similarity(x,y) > 0.25` 这种函数调用
//! **永远用不上 GIN 索引**（索引只能匹配 `%` 这类操作符）。两者叠加：
//! `%` 走索引取候选，`similarity()` 精排。
//!
//! **分类候选上限** `CAT_CAND`：大站某分类可达数万条，不能全量入候选。
//! 同分类只取「做种数最高」的前 200 条参与打分（推荐位本就偏好热门）。
//! 同标签同理限 `TAG_CAND`。

use sqlx::PgPool;

use crate::errors::{DomainError, DomainResult};

/// 同分类候选上限：只取热门（seeders 高）前 N 条参与打分。
const CAT_CAND: i64 = 200;
/// 同标签候选上限。
const TAG_CAND: i64 = 200;
/// trgm 候选上限。
const TRGM_CAND: i64 = 200;

/// 相关种子轻量行（不拖 TorrentRow 全字段：推荐位只需标题/体积/做种/促销）。
#[derive(serde::Serialize, sqlx::FromRow)]
pub struct RelatedRow {
    pub id: i64,
    pub name: String,
    pub small_descr: Option<String>,
    pub category_id: i32,
    pub size: i64,
    pub seeders: i32,
    pub leechers: i32,
    pub promotion: Option<String>,
    pub poster: Option<String>,
    pub times_completed: i32,
}

/// 打分取相关种子。
///
/// 权重（越相关分越高）：
///   同版本组（group_id 相同）  +100
///   同分类                     +3
///   同标签交集                 +10 × 交集数（上限 30）
///   标题 trgm 相似（>0.25）     +round(similarity × 20)
/// 排序按 score DESC, seeders DESC, id DESC。
pub async fn list_related(
    db: &PgPool,
    id: i64,
    limit: i64,
) -> DomainResult<Vec<RelatedRow>> {
    let limit = limit.clamp(1, 20);
    let rows = sqlx::query_as::<_, RelatedRow>(
        r#"
        WITH base AS (
            SELECT t.id, t.name, t.category_id, t.group_id
              FROM torrents t WHERE t.id = $1
        ),
        base_tags AS (
            SELECT tt.tag_id FROM tags tt WHERE tt.torrent_id = $1
        ),
        /* 候选集：三段 UNION ALL，各段独立可走索引 */
        cand AS (
            /* ① 同版本组（最相关） */
            SELECT t.id FROM torrents t, base b
             WHERE b.group_id IS NOT NULL AND t.group_id = b.group_id
               AND t.id <> $1 AND t.approval_status = 1
            UNION
            /* ② 同标签（限热门前 N，避免大站标签爆量） */
            SELECT t.id FROM (
                SELECT t.id FROM torrents t
                 WHERE t.approval_status = 1 AND t.id <> $1
                   AND EXISTS (
                     SELECT 1 FROM tags tg
                      WHERE tg.torrent_id = t.id
                        AND tg.tag_id IN (SELECT tag_id FROM base_tags))
                 ORDER BY t.seeders DESC LIMIT $2
            ) t
            UNION
            /* ③ 标题 trgm：`%` 走 GIN 索引粗筛，similarity 精筛 */
            SELECT t.id FROM (
                SELECT t.id FROM torrents t, base b
                 WHERE t.approval_status = 1 AND t.id <> $1
                   AND t.name % b.name
                   AND similarity(t.name, b.name) > 0.25
                 LIMIT $3
            ) t
            UNION
            /* ④ 同分类：只取热门前 N（大站分类可达数万条） */
            SELECT t.id FROM (
                SELECT t.id FROM torrents t, base b
                 WHERE t.category_id = b.category_id
                   AND t.approval_status = 1 AND t.id <> $1
                 ORDER BY t.seeders DESC LIMIT $4
            ) t
        ),
        scored AS (
            SELECT t.id, t.name, t.small_descr, t.category_id, t.size,
                   t.seeders, t.leechers, t.times_completed,
                   t.media_info->>'poster' AS poster,
                   (SELECT p.kind::text FROM promotions p
                      WHERE p.starts_at <= now() AND p.ends_at > now()
                        AND (p.torrent_id = t.id
                          OR (p.torrent_id IS NULL AND (
                              p.scope = 'global'
                              OR (p.scope = 'official' AND t.official_tag)
                              OR (p.scope = 'non_official'
                                  AND NOT t.official_tag)
                              OR (p.scope = 'category'
                                  AND t.category_id = p.category_id))))
                      ORDER BY p.id DESC LIMIT 1) AS promotion,
                   (
                     CASE WHEN b.group_id IS NOT NULL
                               AND t.group_id = b.group_id
                          THEN 100 ELSE 0 END
                     + CASE WHEN t.category_id = b.category_id
                            THEN 3 ELSE 0 END
                     + LEAST(30, 10 * (
                         SELECT count(*) FROM tags tt
                          WHERE tt.torrent_id = t.id
                            AND tt.tag_id IN (SELECT tag_id FROM base_tags)
                       ))
                     + CASE WHEN similarity(t.name, b.name) > 0.25
                            THEN (similarity(t.name, b.name) * 20)::int
                            ELSE 0 END
                   ) AS score
              FROM torrents t
              JOIN cand c ON c.id = t.id
             CROSS JOIN base b
        )
        SELECT id, name, small_descr, category_id, size, seeders, leechers,
               promotion, poster, times_completed
          FROM scored
         WHERE score > 0
         ORDER BY score DESC, seeders DESC, id DESC
         LIMIT $5
        "#,
    )
    .bind(id)
    .bind(TAG_CAND)
    .bind(TRGM_CAND)
    .bind(CAT_CAND)
    .bind(limit)
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(rows)
}
