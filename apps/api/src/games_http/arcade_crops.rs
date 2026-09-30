//! 农场作物表的后台读写（0253 之后这张表才真的可配）。
//!
//! 为什么这一刀要紧：作物表同时撑着**两条**经济口径 ——
//! ① 「产量 = 种子价 × 0.75」这条标定就是 `FARM_BASE_EV = 0.90` 的来源；
//! ② 0252 的彩蛋池按 `min(seed_price)` 定标。
//! 所以改一行作物不是改个名字：它要么改自己的回收率，要么改整池的余量。
//! 两道校验都在这一把请求里做完，不合法整体回滚。
//!
//! 下架（`active=false`）与删除是两件事：收过的作物有 `farm_harvests` 外键挡着
//! 删不掉（那是审计留痕，不该被绕过），而站长要的其实只是「别再让人买这一款」。

use actix_web::{delete, get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;
use serde_json::json;

use crate::authz;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::games;
use crate::http::require_auth;
use crate::state::AppState;

use super::farm_egg::{farm_entries, farm_unit};
use super::pool::dberr;

fn bad(e: sqlx::Error) -> DomainError {
    dberr(e)
}

#[derive(Deserialize)]
pub(super) struct CropSaveReq {
    /// `None` = 新增（id 走序列）
    pub id: Option<i32>,
    pub name: String,
    pub seed_price: i64,
    pub base_yield: i64,
    pub grow_hours: i32,
    #[serde(default = "active_by_default")]
    pub active: bool,
}

fn active_by_default() -> bool {
    true
}

async fn require_staff(
    req: &HttpRequest,
    state: &web::Data<std::sync::Arc<AppState>>,
) -> Result<crate::http::AuthUser, DomainError> {
    let auth = require_auth(req, state).await?;
    authz::require_perm(state, &auth, authz::perm::USER_ADJUST).await?;
    Ok(auth)
}

/// 作物表现值（编辑器读的就是玩法读的这张表，含已下架的）。
/// 每行附带**现算**的回收期望与标定上限：站长一眼看得见自己有没有越线。
#[get("/admin/arcade/crops")]
pub(super) async fn arcade_crops_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    require_staff(&req, &state).await?;
    let rows: Vec<(i32, String, i64, i64, i32, bool)> = sqlx::query_as(
        "SELECT id, name, seed_price::bigint, base_yield::bigint, \
                grow_hours, active FROM farm_crops ORDER BY seed_price, id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(dberr)?;
    let crops: Vec<_> = rows
        .iter()
        .map(|(id, name, seed, y, g, a)| {
            json!({
                "id": id, "name": name, "seed_price": seed,
                "base_yield": y, "grow_hours": g, "active": a,
                "expected_value": games::crop_expected_value(*seed, *y),
            })
        })
        .collect();
    let unit = farm_unit(&state.repo.db).await?;
    Ok(ok(json!({
        "crops": crops,
        "unit": unit,
        "cap": games::FARM_BASE_EV,
        "ev_note": format!(
            "收获期望 = 产量 ÷ 种子价 × 1.2，标定上限 {}",
            games::FARM_BASE_EV
        ),
    })))
}

/// 保存一行作物。**两道闸**：标定（这一行自己），以及彩蛋池余量（跨表回查）。
#[post("/admin/arcade/crop")]
pub(super) async fn arcade_crop_save(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<CropSaveReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_staff(&req, &state).await?;
    let b = body.into_inner();
    let name = b.name.trim();
    if name.is_empty() || name.chars().count() > 40 {
        return Err(DomainError::Validation(
            "作物名不能为空，也不该长过 40 字".into(),
        ));
    }
    games::validate_crop(b.seed_price, b.base_yield, b.grow_hours)
        .map_err(|e| DomainError::Validation(format!("作物档位不合法：{e}")))?;
    let mut tx = state.repo.db.begin().await.map_err(dberr)?;
    let id: i32 = match b.id {
        None => sqlx::query_scalar(
            "INSERT INTO farm_crops \
                 (name, seed_price, base_yield, grow_hours, active) \
             VALUES ($1, $2, $3, $4, $5) RETURNING id",
        )
        .bind(name)
        .bind(b.seed_price as i32)
        .bind(b.base_yield as i32)
        .bind(b.grow_hours)
        .bind(b.active)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| dup_name_err(e, name))?,
        Some(id) => {
            let n = sqlx::query(
                "UPDATE farm_crops SET name = $1, seed_price = $2, \
                        base_yield = $3, grow_hours = $4, active = $5 \
                 WHERE id = $6",
            )
            .bind(name)
            .bind(b.seed_price as i32)
            .bind(b.base_yield as i32)
            .bind(b.grow_hours)
            .bind(b.active)
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| dup_name_err(e, name))?;
            if n.rows_affected() == 0 {
                return Err(DomainError::Validation(
                    "这行作物不存在（id 对不上）".into(),
                ));
            }
            id
        }
    };
    // 跨表回查：新价或新启停都会改掉定标单位，彩蛋池必须按**改完之后**的单位
    // 重新算一遍。不合法就整体回滚 —— 不允许「作物存下来了、池子悄悄变成增发」。
    let unit = farm_unit(&mut *tx).await?;
    let entries = farm_entries(&mut *tx).await?;
    if !entries.is_empty() {
        games::validate_farm(&entries, unit).map_err(|e| {
            DomainError::Validation(format!(
                "作物改动会把农场彩蛋池的定标单位变成 {unit} 魔力，\
                 那一池就不合法了（已回滚）：{e}"
            ))
        })?;
    }
    tx.commit().await.map_err(dberr)?;
    state
        .repo
        .audit(Some(auth.id), "farm.crop.save", Some(id as i64))
        .await;
    Ok(ok(json!({
        "id": id,
        "name": name,
        "active": b.active,
        "expected_value": games::crop_expected_value(
            b.seed_price, b.base_yield
        ),
        "unit": unit,
    })))
}

/// 名字撞 UNIQUE 要说人话：编辑器一次改一行，报「约束名」等于没报。
fn dup_name_err(e: sqlx::Error, name: &str) -> DomainError {
    if let sqlx::Error::Database(db) = &e {
        if db.code().as_deref() == Some("23505") {
            return DomainError::Validation(format!(
                "已有同名作物「{name}」，改个名再存"
            ));
        }
    }
    bad(e)
}

/// 删除一行作物。有地块或收获史就**拒**并指出该走下架：
/// `farm_harvests` 的外键是审计留痕，不该为了删一款种子而绕过去。
#[delete("/admin/arcade/crop/{id}")]
pub(super) async fn arcade_crop_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    id: web::Path<i32>,
) -> DomainResult<HttpResponse> {
    let auth = require_staff(&req, &state).await?;
    let id = id.into_inner();
    let prev: Option<(String,)> =
        sqlx::query_as("SELECT name FROM farm_crops WHERE id = $1")
            .bind(id)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(dberr)?;
    let Some((name,)) = prev else {
        return Err(DomainError::Validation("这行作物不存在".into()));
    };
    let refs: (i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM farm_plots \
                 WHERE crop_id = $1)::bigint, \
                (SELECT count(*) FROM farm_harvests \
                  WHERE crop_id = $1)::bigint",
    )
    .bind(id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(dberr)?;
    if refs.0 > 0 || refs.1 > 0 {
        return Err(DomainError::Validation(format!(
            "「{name}」还有 {} 块在种的地、{} 条收获史，删不掉；\
             要停止供应请把它「下架」",
            refs.0, refs.1
        )));
    }
    sqlx::query("DELETE FROM farm_crops WHERE id = $1")
        .bind(id)
        .execute(&state.repo.db)
        .await
        .map_err(dberr)?;
    state
        .repo
        .audit(Some(auth.id), "farm.crop.delete", Some(id as i64))
        .await;
    Ok(ok(json!({ "id": id, "name": name, "deleted": true })))
}
