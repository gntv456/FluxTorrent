//! 过审副作用（0288 从 review.rs 拆出，300 行门禁）。
//!
//! 单条裁决 `review_decide` 与批量裁决 `review_batch` 共用这一份实现——
//! 两处各写一遍 SQL 迟早漂移（本项目已多次踩「N 个入口各写各的」）。

/// 过审副作用（0077 自动促销 + 0075 组订阅推送）。
/// 单条裁决 `review_decide` 与批量裁决 `review_batch` 共用这一份实现——
/// 两处各写一遍 SQL 迟早漂移（本项目已多次踩「N 个入口各写各的」）。
pub(crate) async fn apply_approval_side_effects(
    db: &sqlx::PgPool,
    torrent_id: i64,
    operator_id: i64,
) {
    // 0077 自动促销（U3D 口径）：过审时按 position 取第一条命中规则挂促销
    let _ = sqlx::query(
        r#"
        INSERT INTO promotions
               (scope, torrent_id, kind, starts_at, ends_at, source, created_by)
        SELECT 'torrent', t.id, r.kind::promotion_kind_enum, now(),
               now() + make_interval(hours => r.hours),
               'task'::promotion_source, $2
        FROM torrents t
        JOIN auto_promo_rules r ON r.enabled
             AND (r.name_regex = '' OR t.name ~* r.name_regex)
             AND (r.min_size = 0 OR t.size >= r.min_size)
             AND (r.max_size = 0 OR t.size < r.max_size)
             AND (r.category_id IS NULL OR r.category_id = t.category_id)
        WHERE t.id = $1
        ORDER BY r.position LIMIT 1
        "#,
    )
    .bind(torrent_id)
    .bind(operator_id)
    .execute(db)
    .await;
    // 0075 组级订阅推送：入组种子过审 → 通知组订阅者（每人一信，含通知偏好过滤）
    // 0286：附带 kind/params（P1-9 的第一步——文案不再只以成品中文落库，切语言才翻得动）
    let _ = sqlx::query(
        r#"
        INSERT INTO messages
               (sender_id, receiver_id, subject, body, kind, params)
        SELECT NULL, gs.user_id, '订阅的聚合组有新版本',
               format('你订阅的资源组「%s」有新种子过审：#%s %s。同类资源聚合页见种子详情。',
                      g.name, t.id, t.name),
               'group_new_version',
               jsonb_build_object('group', g.name, 'id', t.id, 'name', t.name)
        FROM torrents t
        JOIN torrent_groups g ON g.id = t.group_id
        JOIN group_subscriptions gs ON gs.group_id = g.id
        WHERE t.id = $1
          AND (u_notice_enabled(gs.user_id, 'group_new_version'))
        "#,
    )
    .bind(torrent_id)
    .execute(db)
    .await;
    // 0332 厂牌订阅 / 0334 字幕组订阅推送：种子带 network / subtitle_group
    // 维度且该实体有人订阅 → 通知订阅者。与组订阅同口径（每人一信 + 通知
    // 偏好过滤）；实体值经 section_dict 反查 content_networks（枚举维度的
    // 标准关联路径）。两种 kind 共用一张 content_networks + 一张订阅表，
    // 这里的 VALUES 列表即「可订阅维度的白名单」——加新可订阅维度只改此处。
    let _ = sqlx::query(
        r#"
        INSERT INTO messages
               (sender_id, receiver_id, subject, body, kind, params)
        SELECT NULL, ns.user_id,
               CASE k.kind WHEN 'network' THEN '订阅的出品方有新片'
                           ELSE '订阅的字幕组有新作' END,
               format('你订阅的%s「%s」有新种子过审：#%s %s。',
                      CASE k.kind WHEN 'network' THEN '出品方' ELSE '字幕组' END,
                      cn.name, t.id, t.name),
               CASE k.kind WHEN 'network' THEN 'network_new_release'
                           ELSE 'subgroup_new_release' END,
               jsonb_build_object('kind', k.kind, 'name', cn.name,
                                  'id', t.id, 'name', t.name)
        FROM torrents t
        JOIN (VALUES ('network'), ('subtitle_group')) AS k(kind) ON TRUE
        JOIN torrent_sections ts
          ON ts.torrent_id = t.id AND ts.kind = k.kind
        JOIN section_dict sd ON sd.id = ts.dict_id
        JOIN content_networks cn
          ON cn.kind = k.kind AND cn.name = sd.name
        JOIN network_subscriptions ns ON ns.network_id = cn.id
        WHERE t.id = $1
          AND (u_notice_enabled(ns.user_id,
                 CASE k.kind WHEN 'network' THEN 'network_new_release'
                             ELSE 'subgroup_new_release' END))
        "#,
    )
    .bind(torrent_id)
    .execute(db)
    .await;
}
