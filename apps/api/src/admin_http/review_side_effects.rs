//! 过审副作用（0288 从 review.rs 拆出，300 行门禁）。
//!
//!
//! 三条 SQL 都是 best-effort：失败**必须出声**（P-323）。此前写成
//! `let _ = …`，`auto_promo_rules.kind` 是 text、要 cast 成枚举，规则
//! 配错一个值就整条失败而零日志——现网实测：真口转正触发副作用后，
//! promotions 里 0 行、容器日志里 0 行 WARN/ERROR，站长只看到「审核通过」。
//! 写法照抄 community_http/requests.rs 的应求通知（if let Err(e) → warn）。
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
    let side = sqlx::query(
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
    if let Err(e) = side {
        tracing::warn!(?e, torrent_id, "自动促销未挂上（审核已过）");
    }
    // 0075 组级订阅推送：入组种子过审 → 通知组订阅者（每人一信，含通知偏好过滤）
    // 0286：附带 kind/params（P1-9 的第一步——文案不再只以成品中文落库，切语言才翻得动）
    let side = sqlx::query(
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
    if let Err(e) = side {
        tracing::warn!(?e, torrent_id, "组订阅推送未写入（审核已过）");
    }
    // 0332 厂牌订阅 / 0334 字幕组订阅推送：种子带 network / subtitle_group
    // 维度且该实体有人订阅 → 通知订阅者。与组订阅同口径（每人一信 + 通知
    // 偏好过滤）；实体值经 section_dict 反查 content_networks（枚举维度的
    // 标准关联路径）。两种 kind 共用一张 content_networks + 一张订阅表，
    // 这里的 VALUES 列表即「可订阅维度的白名单」——加新可订阅维度只改此处。
    // P-584：实体名键必须叫 entity——叫 name 会被后面的种子名覆盖
    // （jsonb_build_object 同键后者胜），且消息中心无此二 kind 模板。
    let side = sqlx::query(
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
               jsonb_build_object('kind', k.kind, 'entity', cn.name,
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
    if let Err(e) = side {
        tracing::warn!(?e, torrent_id, "厂牌/字幕组订阅推送未写入");
    }
    // 发种奖励（P-4 接线）：`uploadtorrent`（行为奖励卡，缺省 15）此前全仓
    // 零读取方——站长设置页改了数值，发布者一个火花都拿不到。过审/免审
    // 是同一条「发布完成」语义，在此统一发放（待审种的奖励也在这发：
    // 走到本函数即已过审）。
    pay_upload_reward(db, torrent_id).await;
}

/// 发种一次性奖励：读 `uploadtorrent` 站点设定，给种子 owner 入账。
/// 免审发种（upload.rs）与人工过审（review_decide）共用——两侧都是
/// 「种子进入已发布世界」的时刻，只挂一侧会让免审作者永远拿不到
/// 待审作者能拿到的钱（或反过来）。幂等键锚种子（重放安全）；金额 0
/// = 站长关停该奖励，不发流水也不报错。失败只 warn 不阻断主流程
/// （奖励漏发可补，审核/发种不可回滚——与上面三条副作用同口径）。
pub(crate) async fn pay_upload_reward(
    db: &sqlx::PgPool,
    torrent_id: i64,
) {
    let amount: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value::bigint FROM site_settings \
         WHERE name = 'uploadtorrent'), 15)",
    )
    .fetch_one(db)
    .await
    .unwrap_or(15);
    if amount <= 0 {
        return;
    }
    let owner: Option<i64> = sqlx::query_scalar(
        "SELECT owner_id FROM torrents WHERE id = $1",
    )
    .bind(torrent_id)
    .fetch_optional(db)
    .await
    .ok()
    .flatten();
    let Some(owner) = owner else { return };
    let idem = format!("upload-reward:{torrent_id}");
    if let Err(e) = crate::economy_http::earn_spark(
        db,
        owner,
        amount,
        "upload",
        &idem,
    )
    .await
    {
        tracing::warn!(?e, torrent_id, owner, "发种奖励入账失败");
    }
}
