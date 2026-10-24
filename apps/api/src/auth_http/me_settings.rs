//! me 总览与设置（M01）：overview + settings get/put。
//! 从 auth_http.rs 按域拆出。

use actix_web::{get, web, HttpRequest, Responder};
use sqlx::Row;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[get("/me/overview")]
pub async fn me_overview(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let uid = auth.id;
    // 主档 + 统计（一次查询，口径与旧站 my_data_stats 一致）
    // P-550：收藏夹数接 /me 的同一把尺（visibility_sql 带本人与职员档），
    // 此前这一格在 payload 里被硬写成 0。
    let vis =
        crate::torrents::visibility_sql(Some((uid, auth.class_id >= 90)));
    let row = sqlx::query(&format!(
        r#"
        SELECT u.username, u.email, u.uploaded, u.downloaded, u.created_at, u.avatar_url,
               u.parked, u.privacy, u.totp_enabled, u.passkey,
               u.spark_balance, f.css AS avatar_frame_css, f.image_url AS avatar_frame_image,
               (SELECT count(*) FROM snatches s WHERE s.user_id = u.id AND s.seeding) AS seeding,
               (SELECT count(*) FROM snatches s WHERE s.user_id = u.id AND s.leeching) AS leeching,
               (SELECT count(*) FROM torrents t WHERE t.owner_id = u.id AND t.approval_status = 1) AS uploads,
               (SELECT count(*) FROM comments c WHERE c.user_id = u.id) AS comments,
               (SELECT count(*) FROM invites i WHERE i.inviter_id = u.id AND i.status = 0) AS invites_pending,
               (SELECT count(*) FROM invites i WHERE i.inviter_id = u.id AND i.status = 1) AS invites_used,
               (SELECT count(*) FROM user_medals m WHERE m.user_id = u.id AND (m.expires_at IS NULL OR m.expires_at > now())) AS medals,
               (SELECT count(*) FROM bookmarks b
                JOIN torrents t ON t.id = b.torrent_id
                WHERE b.user_id = u.id AND {vis}) AS bookmarks,
               c.name AS class_name, u.class_id::bigint AS cid
        FROM users u LEFT JOIN user_classes c ON c.id = u.class_id
             LEFT JOIN avatar_frames f ON f.id = u.avatar_frame_id
        WHERE u.id = $1
        "#,
    ))
    .bind(uid)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .ok_or(DomainError::Unauthorized)?;
    // 佩戴勋章（userbar 同口径，控制面板资料卡用户名角标；
    // 上限 0204 起跟随 medals_max_worn，缺省 3）
    let max_worn =
        crate::community_http::medal::medals_max_worn(&state.repo.db).await;
    let worn_medals: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT m.name, m.asset_ref FROM user_medals um JOIN medals m ON m.id = um.medal_id \
         WHERE um.user_id = $1 AND um.wearing AND (um.expires_at IS NULL OR um.expires_at > now()) \
         ORDER BY m.id LIMIT $2",
    )
    .bind(uid)
    .bind(max_worn)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let r = &row;
    // P-554：下面三个闭包把「解码失败」咽成 0/false/""，那正是 P-515
    // （等级名恒空）与 P-517（cid 恒 0）的机制。为可用性仍然回默认值，
    // 但必须留声——以后新增列一旦类型对不上，日志里要有字段名，
    // 而不是界面安静显示 0、所有静态门全绿。
    let decode_warn = |col: &str, e: &sqlx::Error| {
        tracing::warn!(
            field = col,
            error = %e,
            "/me/overview 解码失败，回默认值"
        );
    };
    let get_opt_str = |col: &str| -> Option<String> {
        r.try_get::<Option<String>, _>(col).ok().flatten()
    };
    let get_i64 = |col: &str| -> i64 {
        match r.try_get::<i64, _>(col) {
            Ok(v) => v,
            Err(e) => {
                decode_warn(col, &e);
                0
            }
        }
    };
    let get_bool = |col: &str| -> bool {
        match r.try_get::<bool, _>(col) {
            Ok(v) => v,
            Err(e) => {
                decode_warn(col, &e);
                false
            }
        }
    };
    let get_str = |col: &str| -> String {
        match r.try_get::<String, _>(col) {
            Ok(v) => v,
            Err(e) => {
                decode_warn(col, &e);
                String::new()
            }
        }
    };
    let get_ts = |col: &str| -> Option<String> {
        r.try_get::<chrono::DateTime<chrono::Utc>, _>(col)
            .ok()
            .map(|t| t.to_rfc3339())
    };

    let downloaded = get_i64("downloaded");
    let uploaded = get_i64("uploaded");
    let ratio = if downloaded == 0 {
        None
    } else {
        Some((uploaded as f64 / downloaded as f64 * 100.0).round() / 100.0)
    };
    // 最近 30 天登录趋势（对齐旧站 usercp 首页活跃图）。日界按**站点日**：
    // 旧写法是 `created_at::date`（本库会话 UTC）配 `now() - interval '30 days'`
    // （滚动 720 小时）⇒ 北京 0–8 点的登录全记到「昨天」那一格，今日那格空一截，
    // 而界面下面那句「30 天内登录 N 天」数的也是这些 UTC 格（P-474，与后台
    // 大盘 P-473、首页 P-465 同族）。窗口取「今天(北京)往前数 30 个站点日」。
    let trend: Vec<(chrono::NaiveDate, i64)> = sqlx::query_as(
        "SELECT ((created_at AT TIME ZONE 'UTC') + interval '8 hours')::date \
         AS d, count(*) AS n FROM login_events \
         WHERE user_id = $1 AND created_at >= date_trunc('day', \
             now() AT TIME ZONE 'Asia/Shanghai') \
             AT TIME ZONE 'Asia/Shanghai' - interval '29 days' \
         GROUP BY d ORDER BY d",
    )
    .bind(uid)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .into_iter()
    .map(|(d, n)| (d, n))
    .collect::<Vec<_>>();
    // 0417 F11：稀疏行补零成连续 30 格（SQL 只回有登录的日）。此前账号注册
    // 两天就只回 1 个数据点——图表孤点、宽度塌陷，「30 天趋势」名不副实。
    // 日界与上面 SQL 同一口径：北京时间今天往前数 30 天。
    let sparse: std::collections::HashMap<chrono::NaiveDate, i64> =
        trend.into_iter().collect();
    let today_cn = (chrono::Utc::now() + chrono::Duration::hours(8))
        .date_naive();
    let trend: Vec<(chrono::NaiveDate, i64)> = (0..30)
        .map(|i| {
            let d = today_cn - chrono::Duration::days(29 - i);
            (d, sparse.get(&d).copied().unwrap_or(0))
        })
        .collect();
    let trend_json: Vec<serde_json::Value> = trend
        .iter()
        .map(|(d, n)| serde_json::json!({ "date": d.format("%Y-%m-%d").to_string(), "count": n }))
        .collect();
    let login_total_30d: i64 = trend.iter().map(|(_, n)| n).sum();
    // 最近一次**成功**登录的 IP + 城市（样图 § ucp 概览「最后登录 … · 地点」）。
    // 写法照 admin_http/user_detail.rs:76：host() 把 inet 收成文本。取不到就
    // 回 null 由界面说「未记录」——旧版这里写死「—」，前端再用「有没有 passkey」
    // 猜一个 127.0.0.1 画上去，两层都在编数据。
    let (last_login, last_ip): (Option<String>, Option<String>) =
        sqlx::query_as(
            "SELECT max(created_at)::text, \
             host((SELECT l.ip FROM login_events l \
             WHERE l.user_id = $1 AND l.ok \
             ORDER BY l.id DESC LIMIT 1)) \
             FROM login_events WHERE user_id = $1",
        )
        .bind(uid)
        .fetch_one(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let last_city = last_ip.as_deref().and_then(|ip| crate::geo::lookup(ip).2);

    // 等级进度：按 worker `class_adj.rs` 升降级权威谓词的四项条件预览
    // （上传量 / 完成数 / 做种小时 / 账龄），现值写法与 `auth_http/profile.rs`
    // 公开主页同一份尺。旧版分子是「在做种数×100」的假积分、分母却是
    // min_uploaded（字节），量纲不同 ⇒ 进度条恒 0%、「还差」永远等于整个
    // 门槛（P-516）。无下一档规则行（职员档）出 null，界面显示已达最高级。
    // cid 必须是 bigint：`get_i64` 把解码失败吞成 0，而 `user_classes.id` 是
    // integer（sqlx 不把 int4 解成 i64）⇒ 旧写法对每个会员都读成 0，下一
    // 等级恒按 LV2 门槛预览，LV5 与职员档也一样（P-517：现网 root
    // class_id=99 仍回 next_class=LV2/53687091200）。
    let class_id = get_i64("cid") as i32;
    let seeding = get_i64("seeding");
    type NextRow = (String, i64, i64, i64, i64, i64, i64, i64, i64);
    let next: Option<NextRow> = sqlx::query_as(
        "WITH s AS (SELECT u.uploaded, \
                    COALESCE((SELECT count(*) FROM snatches sn \
                              WHERE sn.user_id = u.id \
                                AND sn.completed_at IS NOT NULL), \
                             0)::bigint AS dl, \
                    (COALESCE((SELECT sum(sn.seeded_seconds) \
                               FROM snatches sn \
                               WHERE sn.user_id = u.id), 0) \
                     / 3600)::bigint AS sh, \
                    EXTRACT(DAY FROM now() - u.created_at)::bigint AS age \
             FROM users u WHERE u.id = $1) \
         SELECT r.name, s.uploaded, r.min_uploaded, s.dl, \
                r.min_download_count::bigint, s.sh, \
                r.min_seed_hours::bigint, s.age, \
                r.min_account_age_days::bigint \
         FROM s JOIN class_rules r \
           ON r.class_id > $2 AND r.class_id < 90 \
              AND r.min_uploaded > 0 \
         ORDER BY r.class_id LIMIT 1",
    )
    .bind(uid)
    .bind(class_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let next_class = next.map(
        |(name, up, up_need, dl, dl_need, sh, sh_need, age, age_need)| {
            serde_json::json!({
                "name": name,
                "uploaded": up,
                "uploaded_need": up_need,
                "download_count": dl,
                "download_count_need": dl_need,
                "seed_hours": sh,
                "seed_hours_need": sh_need,
                "account_age_days": age,
                "account_age_days_need": age_need,
            })
        },
    );
    // 通知中心「未读私信」卡片读的就是这个字段（批二十 P-29：/me/overview
    // 原本没有它，页面 `ov?.unread_messages ?? 0` 于是对谁都恒显示 0——
    // 实测 root 收件箱 56 封未读，卡片仍写 0）。谓词与 `/messages?unread=true`
    // 那条一致（unread + location=1 + 主收件箱）：卡片上的数字必须等于点进去
    // 看到的条数，否则数字与它自己的链接打架。
    let unread_messages: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM messages WHERE receiver_id = $1 AND unread \
         AND location = 1 AND folder IS NULL",
    )
    .bind(uid)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    // 最近读过的主题（样图 § ucp「最近阅读主题」那张表）。SQL 住在
    // repo/community/forum_mine.rs——本文件 sqlx 计数压在基线上，只许转调。
    // 取不到不整页 500：这一块是回顾性信息，缺了不影响任何操作。
    let recent_topics = crate::repo::community::forum_mine::recent_read_topics(
        &state.repo,
        uid,
        class_id,
        5,
    )
    .await
    .unwrap_or_default();
    let recent_topics: Vec<serde_json::Value> = recent_topics
        .into_iter()
        .map(|(id, title, forum_id, forum_name, replies, read_at)| {
            serde_json::json!({
                "topic_id": id,
                "title": title,
                "forum_id": forum_id,
                "forum_name": forum_name,
                "replies": replies,
                "read_at": read_at.to_rfc3339(),
            })
        })
        .collect();

    Ok(ok(serde_json::json!({
        "id": uid,
        "username": get_str("username"),
        "email": get_str("email"),
        // P-515：旧写法按 serde_json::Value 读 text 列（无该解码），失败被
        // `unwrap_or(Null)` 吞掉 ⇒ 等级名对每个会员恒空
        "class_name": get_opt_str("class_name"),
        "unread_messages": unread_messages,
        "recent_topics": recent_topics,
        "avatar_url": get_opt_str("avatar_url"),
        "avatar_frame_css": get_opt_str("avatar_frame_css"),
        "avatar_frame_image": get_opt_str("avatar_frame_image"),
        "worn_medals": worn_medals
            .into_iter()
            .map(|(name, asset_ref)| serde_json::json!({ "name": name, "asset_ref": asset_ref }))
            .collect::<Vec<_>>(),
        "created_at": get_ts("created_at"),
        "uploaded": uploaded,
        "downloaded": downloaded,
        "ratio": ratio,
        "seeding": seeding,
        "leeching": get_i64("leeching"),
        "uploads": get_i64("uploads"),
        "comments": get_i64("comments"),
        "bookmarks": get_i64("bookmarks"),
        "spark_balance": get_i64("spark_balance"),
        "invites_pending": get_i64("invites_pending"),
        "invites_used": get_i64("invites_used"),
        "medals": get_i64("medals"),
        "parked": get_bool("parked"),
        "privacy": get_str("privacy"),
        "totp_enabled": get_bool("totp_enabled"),
        "passkey": get_str("passkey"),
        "last_ip": last_ip,
        "last_city": last_city,
        "login_trend_30d": trend_json,
        "login_days_30d": trend.iter().filter(|(_, n)| *n > 0).count(),
        "login_total_30d": login_total_30d,
        "last_login": last_login,
        "next_class": next_class,
    })))
}
