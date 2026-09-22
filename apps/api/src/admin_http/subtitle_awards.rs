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
    q: web::Query<std::collections::HashMap<String, String>>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
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
    q: web::Query<std::collections::HashMap<String, String>>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
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
    if body.grants.is_empty() {
        return Err(DomainError::Validation("授金列表不能为空".into()));
    }
    let mut results = Vec::new();
    for g in &body.grants {
        if !(1..=3).contains(&g.rank) {
            return Err(DomainError::Validation("rank 需为 1/2/3".into()));
        }
        let row: Option<(i64, i64, String)> = sqlx::query_as(
            "UPDATE subtitle_awards SET rank = $2 WHERE id = $1 AND period \
             = $3 AND rank = 0 RETURNING subtitle_id, user_id, tier",
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
    // 揭晓公告（shoutbox；系统口径挂在发起管理名下——user_id NOT NULL）
    let _ =
        sqlx::query("INSERT INTO shoutbox (user_id, message) VALUES ($1, $2)")
            .bind(auth.id)
            .bind(format!(
                "「{}」期金字字幕评选已揭晓！详见字幕区评选榜。",
                body.period
            ))
            .execute(&state.repo.db)
            .await;
    state
        .repo
        .audit(Some(auth.id), "subawards.grant", None)
        .await;
    Ok(ok(serde_json::json!({ "granted": results })))
}
