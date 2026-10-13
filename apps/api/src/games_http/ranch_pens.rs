//! 牧场栏位侧（POST /farm/ranch/{buy,feed,collect}）：从 ranch.rs 按域拆出
//! （只读面板与加工坊留在原文件）。
//!
//! 拆的同一刀把两处「注释写了、代码没做」补上：
//! · P-183 collect 的 `FOR UPDATE OF p` 原本跑在连接池的自动提交上 —— 行锁在
//!   SELECT 返回那一刻就释放，「并收只放一发」并不成立：并发两发都把
//!   `collected_spark` 记一份，而魔力受幂等键只入一次 ⇒ 终身额度被白吃、
//!   台账与余额不再相等。
//! · P-184 buy 的扣费与落栏分两次写，只有唯一键冲突那一支有退款兜底；
//!   其余失败（外键、DB 抖）= 钱扣了没牲畜。同域 `fishing.rs` 的注释早就把
//!   这条纪律写死：「扣鱼饵与落下这一竿必须同一事务」。

use actix_web::{post, web, HttpRequest, HttpResponse};

use super::ranch::AnimalReq;
use crate::dto::ok;
use crate::economy_http::{earn_spark_tx, spend_spark_tx, SpendOutcome};
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

type Db = web::Data<std::sync::Arc<AppState>>;

fn dberr(e: sqlx::Error) -> DomainError {
    DomainError::Internal(e.into())
}

/// 买入牲畜（POST /farm/ranch/buy）：每用户同种一只（产物是固定回收环，
/// 多只等于把回收上限无限放大）。扣费成功才落栏位。
#[post("/farm/ranch/buy")]
pub(super) async fn ranch_buy(
    req: HttpRequest,
    state: Db,
    body: web::Json<AnimalReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let db = &state.repo.db;
    let price: Option<i64> = sqlx::query_scalar(
        "SELECT price FROM arcade_ranch_animals WHERE key = $1 AND enabled",
    )
    .bind(&body.animal)
    .fetch_optional(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(price) = price else {
        return Err(DomainError::Validation("没有这种牲畜".into()));
    };
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM arcade_ranch_pens \
          WHERE user_id = $1 AND animal = $2)",
    )
    .bind(auth.id)
    .bind(&body.animal)
    .fetch_one(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if exists {
        return Err(DomainError::Validation("这只已经养在栏里了".into()));
    }
    let idem = format!("ranch-buy:{}:{}", auth.id, body.animal);
    // 扣费与落栏同一事务（P-184）：任何一步失败整笔回滚，钱不会先走
    let mut tx = db.begin().await.map_err(dberr)?;
    match spend_spark_tx(&mut tx, auth.id, price, "game", &idem,
                         "ranch_buy", 0)
        .await?
    {
        SpendOutcome::Replayed => {
            let _ = tx.rollback().await;
            return Err(DomainError::Validation("请勿重复提交".into()))
        }
        SpendOutcome::Spent => {}
    }
    // 首个周期从购入时起算
    let inserted = sqlx::query(
        "INSERT INTO arcade_ranch_pens (user_id, animal) VALUES ($1, $2) \
         ON CONFLICT (user_id, animal) DO NOTHING",
    )
    .bind(auth.id)
    .bind(&body.animal)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if inserted.rows_affected() == 0 {
        // 同一事务里读到冲突，说明并发那发已提交：回滚本次扣费与写入，
        // 净效果为零（原先靠退款兜底，现在不再需要第二笔流水）
        let _ = tx.rollback().await;
        return Err(DomainError::Validation("这只已经养在栏里了".into()));
    }
    tx.commit().await.map_err(dberr)?;
    Ok(ok(
        serde_json::json!({ "bought": body.animal, "cost": price }),
    ))
}

/// 投喂（POST /farm/ranch/feed）：把产出周期重新计时（没到点也能喂，
/// 喂了就把 ready_at 往前挪——相当于提前续期；产物只在收集时结一次）。
#[post("/farm/ranch/feed")]
pub(super) async fn ranch_feed(
    req: HttpRequest,
    state: Db,
    body: web::Json<AnimalReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let db = &state.repo.db;
    let cycle: Option<i32> = sqlx::query_scalar(
        "SELECT cycle_mins FROM arcade_ranch_animals WHERE key = $1",
    )
    .bind(&body.animal)
    .fetch_optional(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(cycle) = cycle else {
        return Err(DomainError::Validation("没有这种牲畜".into()));
    };
    let updated = sqlx::query(
        "UPDATE arcade_ranch_pens \
            SET ready_at = LEAST(ready_at, \
                now() + make_interval(mins => $3::int)) \
          WHERE user_id = $1 AND animal = $2",
    )
    .bind(auth.id)
    .bind(&body.animal)
    .bind(cycle)
    .execute(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if updated.rows_affected() == 0 {
        return Err(DomainError::Validation("栏里没有这只牲畜".into()));
    }
    Ok(ok(serde_json::json!({ "fed": body.animal })))
}

/// 收集产物（POST /farm/ranch/collect）：到点后领一次；幂等键带周期
/// 时间戳，收完把 ready_at 推到下一周期。
#[post("/farm/ranch/collect")]
pub(super) async fn ranch_collect(
    req: HttpRequest,
    state: Db,
    body: web::Json<AnimalReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let db = &state.repo.db;
    // 整段一笔事务（P-183）：`FOR UPDATE OF p` 的行锁只在事务内有效
    let mut tx = db.begin().await.map_err(dberr)?;
    let row: Option<(chrono::DateTime<chrono::Utc>, i64, i32, i64, i64)> =
        sqlx::query_as(
            "SELECT p.ready_at, a.yield_spark, a.cycle_mins, \
                    p.collected_spark, a.price \
           FROM arcade_ranch_pens p \
           JOIN arcade_ranch_animals a ON a.key = p.animal \
          WHERE p.user_id = $1 AND p.animal = $2 AND p.ready_at <= now() \
          FOR UPDATE OF p",
        )
        .bind(auth.id)
        .bind(&body.animal)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((ready_at, yld, cycle, collected, price)) = row else {
        let _ = tx.rollback().await;
        return Err(DomainError::Validation("还没到产出时间".into()));
    };
    // ZT2（2026-10-03）：终身产出封顶。原实现只推移 ready_at、无总次数/寿命上限
    // ——「一次买入 → 永久产出」回本后是纯增发（实测约 +2,680 魔力/日/账号）。
    // 上限 = 购入价 ×95%（回收口：终身返还 < 100%），达上限即退休、需重新领养。
    let cap = price * 95 / 100;
    let pay = yld.min((cap - collected).max(0));
    if pay <= 0 {
        sqlx::query(
            "DELETE FROM arcade_ranch_pens WHERE user_id = $1 AND animal = $2",
        )
        .bind(auth.id)
        .bind(&body.animal)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        // 退休是一步真实的账变更：先提交这笔删除，再告诉玩家结果
        tx.commit().await.map_err(dberr)?;
        return Err(DomainError::Validation(
            "这只已产出满额（回本 95%），已退休；想继续请重新领养".into(),
        ));
    }
    let next = ready_at + chrono::Duration::minutes(i64::from(cycle));
    sqlx::query(
        "UPDATE arcade_ranch_pens SET ready_at = $3, \
                collected_spark = collected_spark + $4 \
          WHERE user_id = $1 AND animal = $2",
    )
    .bind(auth.id)
    .bind(&body.animal)
    .bind(next)
    .bind(pay)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 入账幂等键带本次周期的 ready_at（唯一）；与推进同事务（P-183）
    let got = earn_spark_tx(
        &mut tx,
        auth.id,
        pay,
        "arcade",
        &format!("ranch-collect:{}:{}:{}", auth.id, body.animal, ready_at),
    )
    .await?;
    if matches!(got, SpendOutcome::Replayed) {
        // 这一轮已被结过（极端并发下的兜底）：整笔回滚，台账不涨、不重复入账
        let _ = tx.rollback().await;
        return Err(DomainError::Validation(
            "这一轮产出已结算，请勿重复提交".into(),
        ));
    }
    tx.commit().await.map_err(dberr)?;
    Ok(ok(
        serde_json::json!({ "collected": body.animal, "earned": pay }),
    ))
}
