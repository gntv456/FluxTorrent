//! 确定侧奖励写侧（后台）：周常 / 赛季里程碑的 upsert。
//!
//! 这一侧不进 EV 闸，所以写侧必须自己把关：一行奖励**必须发得出东西**、
//! 引用的物品**必须发得出去**、周常归属的玩法**必须是真存在的 ref_type**。
//! 三条都是「配错了不会报错、但玩家永远拿不到」的形状，宁可在保存时就拒。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;
use serde_json::json;

use super::pool::dberr;
use crate::authz;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 周常可以挂的玩法 ref_type（`*` = 任意玩法）。进度是按 `spark_ledger.ref_type`
/// 数的，写一个不存在的 ref 就等于造一条永远完成不了的任务。
const QUEST_REFS: [&str; 7] = [
    "*",
    "scratch",
    "bigsmall",
    "jgg",
    "farm_plant",
    "farm_water",
    "farm_harvest",
];

#[derive(Deserialize)]
pub(super) struct RewardReq {
    /// quest | milestone
    pub kind: String,
    pub code: String,
    /// quest：归属玩法 ref_type；milestone：赛季 key
    pub scope: String,
    /// quest：目标局数；milestone：需集齐票根数
    pub target: i32,
    #[serde(default)]
    pub reward_spark: i64,
    #[serde(default)]
    pub item_key: String,
    #[serde(default = "default_qty")]
    pub item_qty: i32,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub sort: i32,
}

fn default_qty() -> i32 {
    1
}
fn default_true() -> bool {
    true
}

/// 保存一行确定侧奖励
#[post("/admin/arcade/rewards")]
pub(super) async fn arcade_reward_save(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<RewardReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    authz::require_perm(&state, &auth, authz::perm::USER_ADJUST).await?;
    let b = body.into_inner();
    let db = &state.repo.db;
    let code = b.code.trim();
    let scope = b.scope.trim();
    let item_key = b.item_key.trim();

    if code.is_empty() || scope.is_empty() {
        return Err(DomainError::Validation("code / scope 不能为空".into()));
    }
    if b.target <= 0 {
        return Err(DomainError::Validation("目标值必须为正".into()));
    }
    if b.reward_spark < 0 {
        return Err(DomainError::Validation("奖励魔力不能为负".into()));
    }
    if b.item_qty <= 0 {
        return Err(DomainError::Validation("物品件数必须为正".into()));
    }
    if b.reward_spark == 0 && item_key.is_empty() {
        return Err(DomainError::Validation(format!(
            "奖励「{code}」既不发魔力也不发物品：领完什么都没拿到，\
             这种行比报错更难查"
        )));
    }
    if !item_key.is_empty() {
        let ok: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM arcade_items \
              WHERE key = $1 AND enabled \
                AND (kind = 'cosmetic' OR anchor > 0))",
        )
        .bind(item_key)
        .fetch_one(db)
        .await
        .map_err(dberr)?;
        if !ok {
            return Err(DomainError::Validation(format!(
                "物品「{item_key}」不存在、已停用或没有折算价，\
                 不能挂为确定侧奖励"
            )));
        }
    }

    // 两套表只差两个列名，SQL 只写一份。插值进来的列名全部来自下面的闭集
    // （不是用户输入），所以这条 format! 没有注入面。
    let (table, scope_col, target_col) = match b.kind.as_str() {
        "quest" => {
            if !QUEST_REFS.contains(&scope) {
                return Err(DomainError::Validation(format!(
                    "周常归属玩法只能是 {} 之一，收到「{scope}」：\
                     写错 ref_type 会让这条任务永远完成不了",
                    QUEST_REFS.join(" | ")
                )));
            }
            ("arcade_quests", "game_ref", "target")
        }
        "milestone" => ("arcade_milestones", "season_key", "need"),
        other => {
            return Err(DomainError::Validation(format!(
                "kind 只能是 quest | milestone，收到「{other}」"
            )))
        }
    };
    let sql = format!(
        "INSERT INTO {table} \
             (code, {scope_col}, {target_col}, reward_spark, \
              item_key, item_qty, enabled, sort) \
         VALUES ($1, $2, $3, $4, NULLIF($5, ''), $6, $7, $8) \
         ON CONFLICT (code) DO UPDATE SET \
             {scope_col} = EXCLUDED.{scope_col}, \
             {target_col} = EXCLUDED.{target_col}, \
             reward_spark = EXCLUDED.reward_spark, \
             item_key = EXCLUDED.item_key, item_qty = EXCLUDED.item_qty, \
             enabled = EXCLUDED.enabled, sort = EXCLUDED.sort"
    );
    sqlx::query(&sql)
        .bind(code)
        .bind(scope)
        .bind(b.target)
        .bind(b.reward_spark)
        .bind(item_key)
        .bind(b.item_qty)
        .bind(b.enabled)
        .bind(b.sort)
        .execute(db)
        .await
        .map_err(dberr)?;
    state
        .repo
        .audit(Some(auth.id), "arcade.reward.save", None)
        .await;
    Ok(ok(json!({
        "kind": b.kind, "code": code, "scope": scope, "target": b.target,
        "reward_spark": b.reward_spark, "item_key": item_key,
    })))
}
