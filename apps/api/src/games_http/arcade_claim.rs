//! 周常 / 赛季里程碑领取：幂等落 arcade_claims → 按 0247 的奖励行发放。
//!
//! 奖励内容（发多少魔力、发哪件物品）在行表里，不在代码里 —— 站长在后台改得动，
//! 这一侧的每一笔又都被发放预算看见（见 arcade_rewards.rs 文件头）。
//!
//! 顺序有讲究：先验奖励行发得出去，再落领取记录，最后发。
//! 反过来会出现「领取记录已写、物品却因停用而报错」—— 那一周/那一季就永远领不到了。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde_json::json;

use super::arcade_rewards::{
    award, load_milestones, load_quests, season_key, Reward,
};
use super::pool::dberr;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[derive(serde::Deserialize)]
pub(super) struct ClaimReq {
    #[serde(default)]
    pub(super) idempotency_key: Option<String>,
}

/// ISO 周键（与 arcade_meta 同口径）
const WEEK_SQL: &str = "SELECT to_char(now(), 'IYYY-\"W\"IW')";

fn find(rows: &[Reward], code: &str) -> DomainResult<Reward> {
    rows.iter()
        .find(|r| r.code == code)
        .cloned()
        .ok_or_else(|| {
            DomainError::Validation(format!("奖励「{code}」不存在或已停用"))
        })
}

/// 引用到的物品必须先能发（或至少报得出价），否则不落领取记录
async fn ref_ok(db: &sqlx::PgPool, r: &Reward) -> DomainResult<()> {
    let Some(key) = &r.item_key else {
        return Ok(());
    };
    let ok: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM arcade_items \
          WHERE key = $1 AND enabled AND (kind = 'cosmetic' OR anchor > 0))",
    )
    .bind(key)
    .fetch_one(db)
    .await
    .map_err(dberr)?;
    if ok {
        return Ok(());
    }
    Err(DomainError::Validation(format!(
        "奖励「{}」引用的物品「{key}」已停用或没有折算价：\
         先在物品目录里修好它，否则这份奖励发不出去",
        r.code
    )))
}

/// 落领取记录；已领过则 Err（幂等主键就是唯一真相，不另存「已领」标记）
async fn mark_claim(
    db: &sqlx::PgPool,
    uid: i64,
    kind: &str,
    code: &str,
    period: &str,
) -> DomainResult<()> {
    let ins = sqlx::query(
        "INSERT INTO arcade_claims (user_id, kind, ref_code, period_key) \
         VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING",
    )
    .bind(uid)
    .bind(kind)
    .bind(code)
    .bind(period)
    .execute(db)
    .await
    .map_err(dberr)?;
    if ins.rows_affected() == 0 {
        return Err(DomainError::Validation("这一期已经领取过了".into()));
    }
    Ok(())
}

/// 客户端幂等键只允许有限长度，其余一律服务端定值（同一期同一人同一奖励）
fn idem(
    prefix: &str,
    code: &str,
    period: &str,
    uid: i64,
    k: &Option<String>,
) -> String {
    match k.as_deref().map(str::trim) {
        Some(x) if !x.is_empty() && x.len() <= 128 => {
            format!("{prefix}:{code}:{period}:{uid}:{x}")
        }
        _ => format!("{prefix}:{code}:{period}:{uid}"),
    }
}

/// 领取周常奖励：校验本周进度 → 幂等落 claim → 按奖励行发放
#[post("/games/arcade/quest/{code}/claim")]
pub(super) async fn claim_quest(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<String>,
    body: web::Json<ClaimReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let uid = auth.id;
    let code = path.into_inner();
    let db = &state.repo.db;
    let r = find(&load_quests(db).await?, &code)?;

    let week: String = sqlx::query_scalar(WEEK_SQL)
        .fetch_one(db)
        .await
        .map_err(dberr)?;
    let done: i64 = sqlx::query_scalar(
        "SELECT count(*)::bigint FROM spark_ledger \
          WHERE user_id = $1 AND kind = 'game' AND amount < 0 \
            AND ($2 = '*' OR ref_type = $2) \
            AND created_at >= date_trunc('week', now())",
    )
    .bind(uid)
    .bind(&r.game_ref)
    .fetch_one(db)
    .await
    .map_err(dberr)?;
    if done < r.target {
        return Err(DomainError::Validation(format!(
            "本周进度未达领取条件：{done}/{}",
            r.target
        )));
    }
    ref_ok(db, &r).await?;
    mark_claim(db, uid, "quest", &code, &week).await?;
    let idem = idem("arcade:quest", &code, &week, uid, &body.idempotency_key);
    let (spark, item, fell) = award(db, uid, &r, &idem).await?;
    state.repo.audit(Some(uid), "arcade.quest.claim", None).await;
    Ok(ok(json!({
        "reward": spark, "item": item, "item_name": r.item_name,
        "item_qty": r.item_qty, "fell_back": fell, "period": week,
    })))
}

/// 领取赛季里程碑奖励：校验票根数 → 幂等落 claim → 按奖励行发放
#[post("/games/arcade/season/{code}/claim")]
pub(super) async fn claim_season(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<String>,
    body: web::Json<ClaimReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let uid = auth.id;
    let code = path.into_inner();
    let db = &state.repo.db;
    let season = season_key(db).await?;
    let r = find(&load_milestones(db, &season).await?, &code)?;

    let owned: i64 = sqlx::query_scalar(
        "SELECT count(*)::bigint FROM user_achievements ua \
         JOIN achievement_defs d ON d.id = ua.def_id \
         WHERE ua.user_id = $1 AND d.family = 'arcade'",
    )
    .bind(uid)
    .fetch_one(db)
    .await
    .map_err(dberr)?;
    if owned < r.target {
        return Err(DomainError::Validation(format!(
            "票根数未达里程碑：{owned}/{}",
            r.target
        )));
    }
    ref_ok(db, &r).await?;
    mark_claim(db, uid, "season", &code, &season).await?;
    let idem =
        idem("arcade:season", &code, &season, uid, &body.idempotency_key);
    let (spark, item, fell) = award(db, uid, &r, &idem).await?;
    state.repo.audit(Some(uid), "arcade.season.claim", None).await;
    Ok(ok(json!({
        "reward": spark, "item": item, "item_name": r.item_name,
        "item_qty": r.item_qty, "fell_back": fell, "period": season,
    })))
}
