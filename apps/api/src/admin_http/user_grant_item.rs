//! 单发道具（详情页「授予道具」口径）。0291 从 `user_detail.rs` 拆出，
//! 一并修掉三条实测缺陷：
//! 1. **发出去是空的**：生效值只读 `config.amount`，而站里那张魔力卡配的是
//!    `{"spark":1000}`（商店购买路径读的就是 `spark`）⇒ 接口 200、库存照扣、
//!    用户零入账。改为与购买路径共用一把尺（`economy_http::spark_amount` 等）。
//! 2. **越权发钱**：整条链只卡 `staff.panel`（90+ 即可），而魔力/上传量在
//!    批量侧要 `user.amountbonus/amountupload`、单号调账要 `user.adjust`（93+）。
//!    现在即时经济类走 `user.adjust`，其余走 `prop.manage`。
//! 3. **配额与发放不同事务**：先 `stock_used+1` 再发放，发放失败靠手工 `-1`
//!    兜（进程崩在中间就永久少卖一格）。现在占位与发放同事务，失败整体回滚。

use actix_web::{post, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::economy_http::{
    credit_gb, earn_spark_tx, spark_amount, voucher_kind,
};
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::guard::{ensure_outranks, staff};

/// 一次发放要落的效果。取不到正数一律拒发——「200 但什么都没发生」
/// 是发放面最贵的一类缺陷（站长以为发了）。
enum Effect {
    Spark(i64),
    UploadBytes(i64),
    Invite,
    Voucher(String),
    Backpack,
}

fn parse_effect(
    kind: &str,
    config: &serde_json::Value,
) -> DomainResult<Effect> {
    Ok(match kind {
        "gift_spark" => match spark_amount(config) {
            Some(v) => Effect::Spark(v),
            None => {
                return Err(DomainError::Validation(
                    "该道具配置里没有可用的魔力数额（spark），已拒绝发放".into(),
                ))
            }
        },
        "upload_credit" => match credit_gb(config) {
            Some(gb) => Effect::UploadBytes(gb * 1024 * 1024 * 1024),
            None => {
                return Err(DomainError::Validation(
                    "该道具配置里没有可用的 GB 数（gb），已拒绝发放".into(),
                ))
            }
        },
        "invite" | "temp_invite" => Effect::Invite,
        "voucher_free" | "voucher_neutral" => {
            Effect::Voucher(voucher_kind(config, kind))
        }
        _ => Effect::Backpack,
    })
}

/// 库存配额占位（同事务）。返回剩余配额；无限额时 None。
async fn stock_take(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    item_id: i64,
) -> DomainResult<Option<i64>> {
    let row: Option<(bool, i64)> = sqlx::query_as(
        "UPDATE shop_items SET stock_used = stock_used + 1 \
         WHERE id = $1 AND stock_quota IS NOT NULL \
         RETURNING (stock_used <= stock_quota) AS within, \
                   (stock_quota - stock_used) AS left",
    )
    .bind(item_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if let Some((within, left)) = row {
        if !within {
            return Err(DomainError::Validation(
                "该道具库存已发罄（stock_quota）".into(),
            ));
        }
        return Ok(Some(left));
    }
    Ok(None)
}

#[post("/admin/users/{id}/grant-item/{item_id}")]
pub async fn user_grant_item(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<(i64, i64)>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    let (uid, item_id) = path.into_inner();
    let item: Option<(String, String, serde_json::Value)> = sqlx::query_as(
        "SELECT name, kind, config FROM shop_items \
         WHERE id = $1 AND active = true",
    )
    .bind(item_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((name, kind, config)) = item else {
        return Err(DomainError::NotFound(item_id));
    };
    let effect = parse_effect(&kind, &config)?;
    // 经济即时类 = 调账级；其余（装饰/背包/券）= 道具管理级。
    // 判定只看 kind，不看 id，所以改 SKU 配置不会把权限位带跑。
    let perm = match effect {
        Effect::Spark(_) | Effect::UploadBytes(_) => {
            crate::authz::perm::USER_ADJUST
        }
        _ => crate::authz::perm::PROP_MANAGE,
    };
    crate::authz::require_perm(&state, &auth, perm).await?;
    ensure_outranks(&state.repo.db, auth.class_id, uid).await?;

    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let stock_left = stock_take(&mut tx, item_id).await?;
    let mut granted = serde_json::Map::new();
    match &effect {
        Effect::Spark(amount) => {
            let idem = format!("admin-grant-item:{uid}:{item_id}:{}",
                uuid::Uuid::new_v4().simple());
            crate::economy_http::expect_spent(
                earn_spark_tx(&mut tx, uid, *amount, "admin_grant_item", &idem)
                    .await?,
            )?;
            granted.insert("spark".into(), serde_json::json!(*amount));
        }
        Effect::UploadBytes(bytes) => {
            sqlx::query(
                "INSERT INTO traffic_ledger (id, user_id, torrent_id, \
                 delta_up, delta_down, window_start, reason, operator_id) \
                 VALUES (nextval('traffic_ledger_id_seq'), $1, NULL, $2, 0, \
                         now(), $3, $4)",
            )
            .bind(uid)
            .bind(bytes)
            .bind(format!("#{} grant_item#{} {}", auth.id, item_id, name))
            .bind(auth.id)
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            sqlx::query(
                "UPDATE users SET uploaded = uploaded + $2 WHERE id = $1",
            )
            .bind(uid)
            .bind(bytes)
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            granted.insert("uploaded_bytes".into(), serde_json::json!(bytes));
            granted.insert(
                "uploaded_gb".into(),
                serde_json::json!(bytes / 1073741824),
            );
        }
        Effect::Invite => {
            sqlx::query(
                "UPDATE users SET quota_extra = quota_extra + 1 WHERE id = $1",
            )
            .bind(uid)
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            granted.insert("invite_quota".into(), serde_json::json!(1));
        }
        Effect::Voucher(vkind) => {
            sqlx::query(
                "INSERT INTO user_vouchers (user_id, kind, source) \
                 VALUES ($1, $2, 'admin')",
            )
            .bind(uid)
            .bind(vkind)
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            granted.insert("voucher".into(), serde_json::json!(vkind));
        }
        Effect::Backpack => {
            let idem = format!(
                "admin-grant-item:{}:{}:{}",
                uid,
                item_id,
                uuid::Uuid::new_v4().simple()
            );
            let order_id: i64 = sqlx::query_scalar(
                "INSERT INTO shop_orders (user_id, item_id, price, \
                 idempotency_key, config_snapshot) VALUES ($1, $2, 0, $3, $4) \
                 RETURNING id",
            )
            .bind(uid)
            .bind(item_id)
            .bind(&idem)
            .bind(&config)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            granted.insert("order_id".into(), serde_json::json!(order_id));
        }
    }
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

    // 台账（0291）：单发也进 grant_batches，站长在一处核对全部发放。
    // 台账写失败不能把已成功的发放报成失败——那会让人重发一遍。
    let batch_id = uuid::Uuid::new_v4().simple().to_string();
    let spec = crate::admin_p3_http::BatchSpec {
        batch_id: &batch_id,
        idem: None,
        actor_id: auth.id,
        kind: "item",
        amount: 1,
        item_id: Some(item_id),
        medal_id: None,
        days: None,
        selector: serde_json::json!({ "mode": "single" }),
        targets: &[uid],
        subject: None,
    };
    let mut ledger = "ok";
    if crate::admin_p3_http::batch_open(&state.repo.db, &spec)
        .await
        .is_err()
    {
        ledger = "write_failed";
    } else {
        let _ = crate::admin_p3_http::batch_finish(
            &state.repo.db,
            &batch_id,
            1,
            "done",
            None,
        )
        .await;
    }

    state
        .repo
        .audit_detail(
            Some(auth.id),
            "user.grant_item",
            Some(uid),
            None,
            Some(serde_json::json!({
                "item_id": item_id, "item_name": name, "kind": kind,
                "granted": granted, "stock_left": stock_left,
                "batch_id": batch_id,
            })),
        )
        .await;
    // 附送 PM：只在真发出去之后发（前面已 commit，此处失败不影响结果）。
    let _ = sqlx::query(
        "INSERT INTO messages (sender_id, receiver_id, subject, body) \
         VALUES ($1, $2, $3, $4)",
    )
    .bind(auth.id)
    .bind(uid)
    .bind(format!("管理员向你发放了道具：{name}"))
    .bind("你收到了管理员发放的道具，请到个人中心查看使用。")
    .execute(&state.repo.db)
    .await;
    Ok(ok(serde_json::json!({
        "user_id": uid, "item_id": item_id, "name": name, "kind": kind,
        "granted": granted, "stock_left": stock_left,
        "batch_id": batch_id, "ledger": ledger,
    })))
}
