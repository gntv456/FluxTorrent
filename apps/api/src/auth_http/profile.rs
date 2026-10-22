//! 公开主页（M01）：GET /users/{id} + torrentlist。
//! 从 auth_http.rs 按域拆出。

use actix_web::{get, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;

use super::profile_types::{PublicProfile, RecentUpload};
use crate::state::AppState;

#[get("/users/{id}")]
pub async fn user_public_profile(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    // P2（2026-10-06）：档案页含聚合子查询，可被顺序遍历 id 拉全站——60 次/分
    // per-user。P-61：这个 60 以前只在注释里，实际共用登录档 5 次/分。
    crate::http::throttle_limited(&state, format!("profile:{}", auth.id), 60)
        .await?;
    let uid = path.into_inner();
    // P-339：观看者的「看头像 / 看签名」两档偏好。读口过去从未引用（两个
    // 字段全仓只出现在 me_settings_kv 的写侧），因此用户勾掉后就是个无效开关。
    let (view_avatars, view_signatures): (bool, bool) = sqlx::query_as(
        "SELECT view_avatars, view_signatures FROM users WHERE id = $1",
    )
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or((true, true));
    let profile: Option<PublicProfile> = sqlx::query_as(
        r#"
        -- P-339：头像与签名按**观看者**的两档偏好返回。之前直接
        -- `u.avatar_url`，而两个字段全仓只有写侧（me_settings_kv 的 UPDATE/bind）——
        -- 用户勾掉一树接一树，读路径照旧照常。
        SELECT $1::bigint AS id, u.username, u.title,
               CASE WHEN $2 THEN u.avatar_url ELSE NULL END AS avatar_url,
               CASE WHEN $3 THEN u.signature ELSE NULL END AS signature,
               u.class_id, c.name AS class_name,
               u.uploaded, u.downloaded, u.donor, u.created_at, u.last_seen_at,
               f.css AS avatar_frame_css, f.image_url AS avatar_frame_image,
               (SELECT count(*) FROM snatches s WHERE s.user_id = u.id AND s.seeding) AS seeding,
               (SELECT count(*) FROM snatches s WHERE s.user_id = u.id AND s.leeching) AS leeching,
               (SELECT count(*) FROM torrents t WHERE t.owner_id = u.id AND t.approval_status = 1 AND NOT t.anonymous) AS uploads,
               (SELECT count(*) FROM comments cm WHERE cm.user_id = u.id) AS comment_count,
               (SELECT count(*) FROM user_medals um WHERE um.user_id = u.id AND (um.expires_at IS NULL OR um.expires_at > now())) AS medals
        FROM users u LEFT JOIN user_classes c ON c.id = u.class_id
             LEFT JOIN avatar_frames f ON f.id = u.avatar_frame_id
        WHERE u.id = $1 AND u.status < 2 AND NOT u.archived
        "#,
    )
    .bind(uid)
    .bind(view_avatars)
    .bind(view_signatures)
    .fetch_optional(&state.repo.read_db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(profile) = profile else {
        return Err(DomainError::NotFound(uid));
    };
    // —— 竞品口径补充（NP userdetails / UNIT3D profile 共性板块）——
    // 个人档案列（users 表早已有、此前接口未回）：性别是 INT2 代码（0/1/2 → 保密/男/女），
    // country/isp 是预留的字典 id（尚无关联表，直接回 id 供前端省略展示）。
    // 在线判定用 COALESCE：last_seen_at 可能为 NULL（从未活动），NULL > x = NULL 解码进 bool 会炸。
    let extra: Option<(
        Option<i16>,
        Option<i32>,
        Option<i32>,
        Option<i32>,
        Option<i32>,
        Option<String>,
        Option<String>,
        bool,
    )> = sqlx::query_as(
        // P-339：签名不再从这里二次取原值。之前本语句直接
        // SELECT signature，返回后用它覆盖了 profile 里按开关算出的值 —— 用户
        // 勾掉「不看签名」后顶层仍然泄露签名档（实测定证）。
        "SELECT gender, country, isp, upload_speed, download_speed, info, \
               NULL::text AS signature, \
               COALESCE(last_seen_at > now() - interval '15 minutes', FALSE) AS online \
         FROM users WHERE id = $1",
    )
    .bind(uid)
    .fetch_optional(&state.repo.read_db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let (
        gender_code,
        _country,
        _isp,
        up_speed,
        down_speed,
        info,
        _signature, // P-339: 常量 NULL（见上方注释），顶层改用 profile 里那份
        online,
    ) = extra
        .map(|(g, c, i, u, d, inf, s, o)| (g, c, i, u, d, inf, s, o))
        .unwrap_or((None, None, None, None, None, None, None, false));
    let gender = match gender_code {
        Some(1) => Some("男".to_string()),
        Some(2) => Some("女".to_string()),
        _ => None, // 0/NULL = 保密
    };
    // —— 传输与时间板块（观众站口径：实际流量 + 做种/下载时间 + 比率 + 做种体积）——
    // snatches.uploaded/downloaded = 该用户全站真实流量（含免费/促销不计量部分），口径即 NP「实际」；
    // snatches 无独立「下载时长」列（NP 的 leechtime）——口径退化为 seed 累计/最近活动跨度不可靠，
    // 采用「有 leeching 记录起 last_seen_at 累计」不可得，这里回退用 completed_at 到创建的近似不可行，
    // 故下载时间以 0 展示由 tracker 侧未来补列（见下方 leech_seconds 注释）。
    let traffic: Option<(i64, i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT COALESCE(sum(uploaded), 0)::bigint, COALESCE(sum(downloaded), 0)::bigint, \
                COALESCE(sum(seeded_seconds), 0)::bigint, \
                COALESCE(sum(CASE WHEN leeching THEN 1 ELSE 0 END), 0)::bigint, \
                COALESCE((SELECT u2.seeding_size FROM users u2 WHERE u2.id = $1), 0) \
         FROM snatches WHERE user_id = $1",
    )
    .bind(uid)
    .fetch_optional(&state.repo.read_db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let (real_up, real_down, seed_seconds, _, seeding_size) =
        traffic.unwrap_or((0, 0, 0, 0, 0));
    // H&R：未解决违规数（观众站「H&R 0」+ 站点 hr_violation_limit 上限口径）
    let hr: Option<(i64,)> = sqlx::query_as(
        "SELECT count(*) FROM hr_violations WHERE user_id = $1 AND \
         resolved_at IS NULL",
    )
    .bind(uid)
    .fetch_optional(&state.repo.read_db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let hr_unresolved = hr.map(|(n,)| n).unwrap_or(0);
    let hr_limit: i64 = sqlx::query_scalar(
        "SELECT COALESCE((value)::bigint, 3) FROM site_settings WHERE \
         name = 'hr_violation_limit'",
    )
    .fetch_optional(&state.repo.read_db)
    .await
    .unwrap_or(None)
    .unwrap_or(3);
    // 魔力值余额（观众站「爆米花」位；spark_balance 是流水权威快照）+ 本月做种收益
    let spark: (i64,) =
        sqlx::query_as("SELECT spark_balance FROM users WHERE id = $1")
            .bind(uid)
            .fetch_one(&state.repo.read_db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let month_earn: i64 = sqlx::query_scalar(
        "SELECT sum(amount)::bigint \
        FROM spark_ledger WHERE user_id = $1 AND amount > 0 AND \
        kind = 'seeding_reward' AND created_at >= (date_trunc('month', \
        now() AT TIME ZONE 'Asia/Shanghai') AT TIME ZONE 'Asia/Shanghai')",
    )
    .bind(uid)
    .fetch_one(&state.repo.read_db)
    .await
    .unwrap_or(0);
    // 完成种子数（憨憨「完成种子」口径：completed_at 非空的抓取记录）
    let completed: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM snatches WHERE user_id = $1 AND \
         completed_at IS NOT NULL",
    )
    .bind(uid)
    .fetch_one(&state.repo.read_db)
    .await
    .unwrap_or(0);
    // 邀请：待使用邀请码数（NP「邀请」字段口径）
    let invites_pending: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM invites WHERE inviter_id = $1 AND status = 0",
    )
    .bind(uid)
    .fetch_one(&state.repo.read_db)
    .await
    .unwrap_or(0);
    // 邀请人（脱敏：只回邀请人 id+用户名，不回邮箱）
    let inviter: Option<(i64, String)> = sqlx::query_as(
                "SELECT i.id, \
         i.username FROM users u JOIN users i ON i.id = u.invited_by WHERE u.id = $1",
    )
    .bind(uid)
    .fetch_optional(&state.repo.read_db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 客户端信息（NP 连接信息：最近一次上报的 BT 客户端 Agent；snatches 无记录则空）
    let agent: Option<(String,)> = sqlx::query_as(
        "SELECT agent FROM snatches WHERE user_id = $1 AND agent <> '' \
         ORDER BY last_seen_at DESC LIMIT 1",
    )
    .bind(uid)
    .fetch_optional(&state.repo.read_db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 佩戴中的勋章（展示位：NP 佩戴勋章图 / UNIT3D achievements）
    let worn_medals: Vec<(i64, String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT m.id, m.name, m.description, m.asset_ref FROM user_medals um \
         JOIN medals m ON m.id = um.medal_id \
         WHERE um.user_id = $1 AND um.wearing AND (um.expires_at IS NULL OR um.expires_at > now()) \
         ORDER BY m.id LIMIT 12",
    )
    .bind(uid)
    .fetch_all(&state.repo.read_db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 成就数（user_achievements）
    let achievements: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM user_achievements WHERE user_id = $1",
    )
    .bind(uid)
    .fetch_one(&state.repo.read_db)
    .await
    .unwrap_or(0);
    // 等级进度（对齐 /me/class-progress 口径：当前值 + 距下一级目标，供前端进度条）。
    // EXTRACT 返回 NUMERIC、count 返回 INT8——列类型全部显式对齐 i64，防 sqlx 静默解码失败
    // （此前 .ok() 把解码错误吞成 None，next_class 恒空）。
    let prog: Option<(i64, i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT u.class_id::bigint, u.uploaded, \
                (SELECT count(*) FROM snatches s WHERE s.user_id = u.id AND s.completed_at IS NOT NULL)::bigint, \
                (SELECT COALESCE(sum(s.seeded_seconds), 0) / 3600 FROM snatches s WHERE s.user_id = u.id)::bigint, \
                EXTRACT(DAY FROM now() - u.created_at)::bigint \
         FROM users u WHERE u.id = $1",
    )
    .bind(uid)
    .fetch_one(&state.repo.read_db)
    .await
    .ok();
    let mut next_class: Option<serde_json::Value> = None;
    if let Some((cur_class, uploaded, dl_count, seed_hours, age_days)) = prog {
        let rules: Vec<(i32, String, i64, i32, i32, i32)> = sqlx::query_as(
            "SELECT class_id, name, min_uploaded, min_download_count, min_seed_hours, min_account_age_days \
             FROM class_rules WHERE class_id > $1 ORDER BY class_id LIMIT 1",
        )
        .bind(cur_class)
        .fetch_all(&state.repo.read_db)
        .await
        .unwrap_or_default();
        if let Some((cid, cname, need_up, need_dl, need_sh, need_age)) =
            rules.into_iter().next()
        {
            next_class = Some(serde_json::json!({
                "class_id": cid, "name": cname,
                "uploaded": uploaded, "uploaded_need": need_up,
                "download_count": dl_count, "download_count_need": need_dl,
                "seed_hours": seed_hours, "seed_hours_need": need_sh,
                "account_age_days": age_days, "account_age_days_need": need_age,
            }));
        }
    }
    let recent_posts = super::profile_helpers::recent_posts_of(
        &state.repo.read_db,
        uid,
        auth.class_id,
    )
    .await;
    let uploads: Vec<RecentUpload> = sqlx::query_as(
                "SELECT id, name, small_descr, size, \
         created_at FROM torrents WHERE owner_id = $1 AND approval_status = 1 AND NOT anonymous ORDER BY id DESC LIMIT 10",
    )
    .bind(uid)
    .fetch_all(&state.repo.read_db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let recent_comments = super::profile_helpers::recent_comments_of(
        &state.repo.read_db,
        uid,
        (auth.id, auth.class_id >= 90),
    )
    .await;
    // 字幕作品摘要（0146 P2-2：数量 + 下载总数 + 署名；匿名上传不计入公开归属）
    let sub_stats: Option<(i64, i64)> = sqlx::query_as(
        "SELECT count(*), COALESCE(sum(downloads), 0)::bigint FROM \
         subtitles WHERE user_id = $1 AND deleted_at IS NULL AND status = 1 \
         AND NOT anon",
    )
    .bind(uid)
    .fetch_optional(&state.repo.read_db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let (subtitle_count, subtitle_downloads) = sub_stats.unwrap_or((0, 0));
    // 字幕身份（0149）：gold 优先于 certified（helper 见文件尾）
    let cert_tier =
        super::profile_helpers::subtitle_cert_tier(&state.repo.read_db, uid)
            .await;
    // 用户自定义字段（0186）：public 字段 + 有值才下发
    let user_fields =
        super::user_fields::public_fields_for(&state.repo.read_db, uid).await;
    let mut body = serde_json::json!({
        "user_fields": user_fields,
        "profile": profile,
        "avatar_frame_css": profile.avatar_frame_css,
        "avatar_frame_image": profile.avatar_frame_image,
        "gender": gender, "country": null, "isp": null,
        "upload_speed": up_speed, "download_speed": down_speed,
        // P-339：顶层 signature 取 profile 已过开关的值，不再用 extra 里那份
        // 未经开关的原值（extra 的 signature 已改成常量 NULL）。
        "info": info, "signature": profile.signature.clone(), "online": online,
        "real_uploaded": real_up, "real_downloaded": real_down,
        "seed_seconds": seed_seconds, "hr_unresolved": hr_unresolved,
        "hr_limit": hr_limit, "seeding_size": seeding_size,
        "spark_balance": spark.0, "month_seed_earn": month_earn,
        "completed_snatches": completed,
        "invites_pending": invites_pending,
        "inviter_id": inviter.as_ref().map(|(i, _)| *i),
        "inviter_name": inviter.map(|(_, n)| n),
        "client_agent": agent.map(|(a,)| a),
        "worn_medals": worn_medals
            .into_iter()
            .map(|(id, name, description, asset_ref)| serde_json::json!({
                "id": id, "name": name, "description": description, "asset_ref": asset_ref
            }))
            .collect::<Vec<_>>(),
        "achievements": achievements,
        "next_class": next_class,
        "recent_posts": recent_posts,
        "recent_uploads": uploads,
        "recent_comments": recent_comments,
        "subtitle_count": subtitle_count,
        "subtitle_downloads": subtitle_downloads,
        "subtitle_cert": cert_tier,
    });
    // P-293：隐私等级=「强(个人信息全部隐藏)」时的按键遮蔽，见该模块注释
    super::profile_privacy::mask_personal(&state, &auth, uid, &mut body).await;
    Ok(ok(body))
}
