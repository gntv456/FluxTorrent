//! 金字幕评选管理端（0148 C5）：候选生成 + 授金。
//! 公开榜与得分公式在 content_http/subtitles_awards.rs。

use actix_web::{get, post, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::guard::staff;

#[derive(sqlx::FromRow, serde::Serialize)]
struct CandidateRow {
    id: i64,
    period: String,
    subtitle_id: i64,
    user_id: i64,
    username: Option<String>,
    title: String,
    lang: Option<String>,
    score: f64,
    rank: i16,
    tier: String,
    rating: Option<f64>,
    downloads: i32,
}

/// 候选列表（period 缺省上上个月——上月的完整月度在次月生成；rank 全档）
#[get("/admin/subtitles/awards/candidates")]
async fn awards_candidates(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: crate::http::Q<std::collections::HashMap<String, String>>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    // P-258：候选池给的是 username + 字幕标题 + score + rating + downloads
    // ，而同文件
    // 的授奖口要 `subtitles.grant`(95) ⇒ 台账与操作同一把键。
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SUBTITLES_GRANT,
    )
    .await?;
    let period = q.get("period").cloned().unwrap_or_else(default_period);
    let rows: Vec<CandidateRow> = sqlx::query_as(
        "SELECT a.id, a.period, a.subtitle_id, a.user_id, u.username, \
         s.title, s.lang, a.score::float8, a.rank, a.tier, \
         (s.rating_sum::float8 / NULLIF(s.rating_count, 0)) AS rating, \
         s.downloads FROM subtitle_awards a JOIN subtitles s ON s.id = \
         a.subtitle_id LEFT JOIN users u ON u.id = a.user_id WHERE \
         a.period = $1 ORDER BY a.tier, a.score DESC",
    )
    .bind(&period)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "period": period, "items": rows,
    })))
}

/// 生成候选（worker 也调同核；此端点供管理手动补跑）
#[post("/admin/subtitles/awards/build")]
async fn awards_build(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: crate::http::Q<std::collections::HashMap<String, String>>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    // P-259：这是**写口**（重建候选池 = 决定谁在池子里、谁能拿奖），过去只
    // 判
    // staff()；与授奖同键。
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SUBTITLES_GRANT,
    )
    .await?;
    let period = q.get("period").cloned().unwrap_or_else(default_period);
    if period.len() != 7
        || !period
            .chars()
            .enumerate()
            .all(|(i, c)| c.is_ascii_digit() || (i == 4 && c == '-'))
    {
        return Err(DomainError::Validation("period 需为 YYYY-MM".into()));
    }
    let n =
        crate::content_http::subtitles_awards_build(&state.repo.db, &period)
            .await
            .map_err(DomainError::Internal)?;
    state
        .repo
        .audit(Some(auth.id), "subawards.build", None)
        .await;
    Ok(ok(serde_json::json!({ "period": period, "added": n })))
}

fn default_period() -> String {
    // 上个自然月（YYYY-MM，UTC 口径）：format 到当月 1 号再回退一天取年月
    let now = chrono::Utc::now();
    let first = format!("{}-01", now.format("%Y-%m"));
    let prev = chrono::NaiveDate::parse_from_str(&first, "%Y-%m-%d")
        .map(|d| d.pred_opt().unwrap_or(d))
        .unwrap_or(now.date_naive());
    prev.format("%Y-%m").to_string()
}

#[derive(serde::Deserialize)]
struct AwardGrantReq {
    period: String,
    /// 授金列表：[{id: subtitle_awards.id, rank: 1|2|3}]
    grants: Vec<AwardGrantItem>,
}

#[derive(serde::Deserialize)]
struct AwardGrantItem {
    id: i64,
    rank: i16,
}

/// 授金（C5-2）：rank 1/2/3 → 勋章 + 火花（5000/2000/1000，幂等键
/// sub-award:{period}:{uid}:{rank}）+ 👑 生效（rank>0 即显示）。
/// 勋章按 code 自动找（sub_gold/sub_silver/sub_bronze，后台 medals 预建，
/// 缺失时跳过发章只发火花——不阻断）。
#[post("/admin/subtitles/awards/grant")]
async fn awards_grant(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<AwardGrantReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    // P-236：授金一次可发出 5000/2000/1000 火花并挂勋章，过去只判 staff()
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SUBTITLES_GRANT,
    )
    .await?;
    if body.grants.is_empty() {
        return Err(DomainError::Validation("授金列表不能为空".into()));
    }
    let mut results = Vec::new();
    for g in &body.grants {
        if !(1..=3).contains(&g.rank) {
            return Err(DomainError::Validation("rank 需为 1/2/3".into()));
        }
        // 三轮遗留（2026-10-07）：CAS 加「未发过奖」卫语句——旧版置位后
        // 崩溃，重跑被 rank=0 条件跳过，火花永不发。幂等流水已存在时
        // 视为已发过，跳过置位（skipped），未发条目可完整补发。
        let row: Option<(i64, i64, String)> = sqlx::query_as(
            "UPDATE subtitle_awards SET rank = $2 WHERE id = $1 AND period              = $3 AND rank = 0 AND NOT EXISTS (                SELECT 1 FROM spark_ledger WHERE kind = 'subtitle_award'                  AND idempotency_key = format('sub-award:{}:{}:{}', $3,                    (SELECT user_id FROM subtitle_awards WHERE id = $1), $2))              RETURNING subtitle_id, user_id, tier",
        )
        .bind(g.id)
        .bind(g.rank)
        .bind(&body.period)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        let Some((sid, uid, tier)) = row else {
            results.push(serde_json::json!({
                "id": g.id, "skipped": true,
            }));
            continue;
        };
        let reward = match g.rank {
            1 => 5000,
            2 => 2000,
            _ => 1000,
        };
        let idem = format!("sub-award:{}:{}:{}", body.period, uid, g.rank);
        crate::economy_http::earn_spark(
            &state.repo.db,
            uid,
            reward,
            "subtitle_award",
            &idem,
        )
        .await?;
        // 勋章（后台预建名称「金字幕/银字幕/铜字幕」；缺失跳过）
        let medal_name = match g.rank {
            1 => "金字幕",
            2 => "银字幕",
            _ => "铜字幕",
        };
        let medal: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM medals WHERE name = $1 ORDER BY id LIMIT 1",
        )
        .bind(medal_name)
        .fetch_optional(&state.repo.db)
        .await
        .unwrap_or(None);
        if let Some(mid) = medal {
            let _ = sqlx::query(
                "INSERT INTO user_medals (user_id, medal_id, source) VALUES \
                 ($1, $2, 'award') ON CONFLICT (user_id, medal_id) DO \
                 NOTHING",
            )
            .bind(uid)
            .bind(mid)
            .execute(&state.repo.db)
            .await;
        }
        // 通知
        let _ = sqlx::query(
            "INSERT INTO messages (sender_id, receiver_id, subject, body) \
             VALUES (NULL, $1, '金字幕评选揭晓', $2)",
        )
        .bind(uid)
        .bind(format!(
            "你的字幕（#{sid}）在 {tier} 赛道获得第 {} 名，奖励 {reward} 火花！",
            g.rank
        ))
        .execute(&state.repo.db)
        .await;
        results.push(serde_json::json!({
            "id": g.id, "rank": g.rank, "user_id": uid,
            "reward": reward, "medal": medal_name,
        }));
    }
    // 揭晓公告（0405 双路口径）：缺省写首页公告区（系统身份+「评选」徽章），
    // 站长开 `shoutbox_announce_awards` 后才同时广播聊天室。
    // P-239 纪律保留：两路任一发不出去都要报出来（不能让站长以为已通知）。
    let announce_text =
        format!("「{}」期金字字幕评选已揭晓！详见字幕区评选榜。", body.period);
    if let Err(e) = sqlx::query(
        "INSERT INTO announcements (title, body, badge, author_id, sort) \
         VALUES ($1, $2, '评选', $3, \
                 (SELECT COALESCE(max(sort),0)+10 FROM announcements))",
    )
    .bind(format!("金字字幕评选揭晓（{}）", body.period))
    .bind(announce_text.clone())
    .bind(auth.id)
    .execute(&state.repo.db)
    .await
    {
        tracing::warn!(%e, period = %body.period, "揭晓公告写入公告区失败");
    }
    crate::http::home::invalidate_home_cache(&state).await;
    let broadcast: Option<String> = sqlx::query_scalar(
        "SELECT value FROM site_settings \
         WHERE name = 'shoutbox_announce_awards'",
    )
    .bind(0)
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten();
    if broadcast.as_deref() == Some("1") {
        if let Err(e) = sqlx::query(
            "INSERT INTO shoutbox (user_id, message) VALUES ($1, $2)",
        )
        .bind(auth.id)
        .bind(announce_text)
        .execute(&state.repo.db)
        .await
        {
            tracing::warn!(%e, period = %body.period, "揭晓公告写入聊天室失败");
        }
    }
    state
        .repo
        .audit_detail(
            Some(auth.id),
            "subawards.grant",
            None,
            None,
            Some(serde_json::json!({
                "period": body.period, "granted": results.clone(),
            })),
        )
        .await;
    Ok(ok(serde_json::json!({ "granted": results })))
}
