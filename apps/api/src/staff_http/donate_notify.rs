//! 捐赠回调与订单管理。
//! 从 staff_http.rs 按域拆出。

use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

// ============ staffpanel 管理工具（同类站 faqmanage/modrules/catmanage/bans/massmail 口径） ============

/// 支付网关异步回调（GET，易支付口径）：验签 → 幂等入账 → 纯文本 "success"
/// （网关要求响应 success 字面量，不走统一信封）
#[get("/donate/notify")]
pub async fn donate_notify(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: crate::http::Q<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let _ = req;
    // 回执分三种：入账成功与重复回调都 ack（停止重投）；「签名有效但查无此单」
    // 回 fail——钱在网关那侧确实收到了，ack 会把重投掐掉、异常就地蒸发（P-463）。
    use crate::payment::SettleOutcome;
    match crate::payment::settle_notify(&state, &q.into_inner()).await {
        Ok(SettleOutcome::Paid) | Ok(SettleOutcome::Duplicate) => {
            HttpResponse::Ok().body("success")
        }
        Ok(SettleOutcome::NotSuccess) => HttpResponse::Ok().body("success"),
        Ok(SettleOutcome::UnknownOrder) => {
            HttpResponse::Ok().body("fail: unknown order")
        }
        Err(e) => {
            tracing::warn!(?e, "donate notify rejected");
            HttpResponse::Ok().body("fail")
        }
    }
}

/// 旧模拟充值逻辑（FLUX_DEMO=1 专用）：移入 payment.rs 兄弟函数，保留演示站能力
#[allow(dead_code)]
mod removed_legacy_simulated_topup {
    // 历史实现已由 payment::demo_topup 替代（编译期占位，防误回滚参考）
}

#[derive(Deserialize)]
struct OrderBody {
    plan_id: i32,
}

/// 用余额订购套餐（上传量 / 邀请名额 / VIP）
#[post("/donate/order")]
pub async fn donate_order(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<OrderBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let plan: Option<(String, String, Option<String>, f64)> = sqlx::query_as(
        "SELECT plan_type, title, reward, \
         price_usd::float8 FROM donation_plans WHERE id = $1 AND enabled",
    )
    .bind(body.plan_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((plan_type, title, reward, price)) = plan else {
        return Err(DomainError::NotFound(body.plan_id as i64));
    };
    // 发放量取自标题首段数字（upload 计 GB、quota 计枚、vip 计天），上限沿用
    // 批量发放口径（上传 10 TB / 邀请 50 枚 / VIP 36500 天）。必须在扣款之前
    // 判定：解析不出就拒单，收钱不给货是这条链路上唯一不可逆的损失。
    // VIP 旧实现只认 contains("180")/contains("终身")，其余静默按 30 天发：
    // 站长把「180 天」改成「半年」就少发 150 天且照常扣款。
    let grant = match plan_grant(&plan_type, &title) {
        Ok(v) => v,
        Err(msg) => {
            // 校验详情必须整句可查 validation_details.tsv，所以不能用
            // format! 拼（validation_i18n_guard 锁动态串条数）。
            tracing::warn!(
                plan_id = body.plan_id,
                title = %title,
                "donate order rejected: ungrantable plan"
            );
            return Err(DomainError::Validation(msg.into()));
        }
    };
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let balance: Option<f64> = sqlx::query_scalar(
        "UPDATE users SET wallet_usd = wallet_usd - $2 WHERE id = $1 \
         AND wallet_usd >= $2 RETURNING wallet_usd::float8",
    )
    .bind(auth.id)
    .bind(price)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(balance) = balance else {
        return Err(DomainError::Validation(format!(
            "余额不足，还差 {:.2} USD",
            price
        )));
    };
    // 套餐生效
    match plan_type.as_str() {
        "upload" => {
            // 「100 GB 上传量」/「500 GB 上传量」→ grant 已是 GB 数
            // 必须落差额流水（P1）：快照权威在 traffic_ledger，reconcile_snapshots 每 6h
            // 把 users.uploaded 重算为 sum(delta_up)——只 UPDATE 快照不落流水，付费购买的
            // 上传量会在 6h 内被静默抹掉。对照 shop upload_credit 的双写。
            sqlx::query(
                "INSERT INTO traffic_ledger (id, user_id, torrent_id, delta_up, delta_down, window_start) \
                 VALUES (nextval('traffic_ledger_id_seq'), $1, 0, $2, 0, now())",
            )
            .bind(auth.id)
            .bind(grant * 1024 * 1024 * 1024)
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            sqlx::query(
                "UPDATE users SET uploaded = uploaded + $2 WHERE id = $1",
            )
            .bind(auth.id)
            .bind(grant * 1024 * 1024 * 1024)
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }
        "quota" => {
            // 「10 枚邀请名额」→ grant 已是枚数；领码时 quota_extra 优先于周配额消耗
            sqlx::query(
                "UPDATE users SET quota_extra = quota_extra + $2 WHERE id = $1",
            )
            .bind(auth.id)
            .bind(grant as i32)
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }
        "vip" => {
            sqlx::query(
                "UPDATE users SET vip_until = \
                 GREATEST(COALESCE(vip_until, now()), now()) + \
                 make_interval(days => $2::int) WHERE id = $1",
            )
            .bind(auth.id)
            // 参数位是 int4（make_interval(days => $2::int)）：显式转 i32
            // 与 arcade_meta.rs:45 的口径一致，不依赖 PG 对 8 字节绑定的
            // 容忍。上限 36500 已确保不溢出。
            .bind(grant as i32)
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }
        _ => {}
    }
    // 附赠邀请 1（2026-10-10 实测修复：旧实现是空写）：额外邀请名额的
    // 全站唯一杠杆是 `users.quota_extra`——领码时它优先于周配额被消耗
    // （invite_http.rs:183 `if extra.is_none()`），管理发放 Effect::Invite
    // 也是加这一列。旧代码往 invite_quota 插一行 used=0：既不加额度，
    // period 又写 current_date（周口径是 date_trunc('week')，只有周一才
    // 撞同一个键），等于「套餐文案写着送、付完钱什么都没送」。
    // plan_type='quota' 已在上面按 grant 枚数加过，这里跳过防双发。
    if plan_type != "quota" && reward.as_deref().unwrap_or("").contains("邀请")
    {
        sqlx::query(
            "UPDATE users SET quota_extra = quota_extra + 1 WHERE id = $1",
        )
        .bind(auth.id)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }
    sqlx::query(
        "INSERT INTO donation_ledger (user_id, kind, amount_usd, \
         balance_after, plan_id, note) VALUES ($1, 'order', -$2, $3, $4, $5)",
    )
    .bind(auth.id)
    .bind(price)
    .bind(balance)
    .bind(body.plan_id)
    .bind(&title)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "donate_order", Some(body.plan_id as i64))
        .await;
    Ok(ok(
        serde_json::json!({ "plan": title, "wallet_usd": balance }),
    ))
}

// ---- 批量邮件（massmail）----

/// 套餐发货量：upload 计 GB / quota 计枚 / vip 计天。
/// 标题首段必须给出 1..=上限 的数字（VIP 的「终身」按上限发）。
/// 解析不出或类型未知一律 Err(整句文案)：调用方必须在**扣款前**拒单，
/// 收钱不给货是这条链路上唯一不可逆的损失。
fn plan_grant(
    plan_type: &str,
    title: &str,
) -> Result<i64, &'static str> {
    let cap = match plan_type {
        "upload" => 10_000,
        "quota" => 50,
        "vip" => 36_500,
        _ => return Err("套餐类型无效，已拒单未扣款"),
    };
    if plan_type == "vip" && title.contains("终身") {
        return Ok(cap);
    }
    let n: i64 = title
        .split_whitespace()
        .next()
        .and_then(|w| w.parse().ok())
        .unwrap_or(0);
    if (1..=cap).contains(&n) {
        Ok(n)
    } else {
        Err("套餐标题首段数量无效，已拒单未扣款")
    }
}

#[cfg(test)]
mod tests {
    use super::plan_grant;

    #[test]
    fn plan_grant_reads_live_titles() {
        // 现网 6 个套餐的标题（donation_plans）必须全部解析成功
        assert_eq!(plan_grant("quota", "10 枚邀请名额"), Ok(10));
        assert_eq!(plan_grant("upload", "100 GB 上传量"), Ok(100));
        assert_eq!(plan_grant("upload", "500 GB 上传量"), Ok(500));
        assert_eq!(plan_grant("vip", "30 天"), Ok(30));
        assert_eq!(plan_grant("vip", "180 天"), Ok(180));
        assert_eq!(plan_grant("vip", "终身用户"), Ok(36_500));
    }

    #[test]
    fn plan_grant_rejects_ungrantable() {
        // 文案改成「半年」必须拒单，不能静默按 30 天发货
        assert!(plan_grant("vip", "半年").is_err());
        assert!(plan_grant("vip", "180天").is_err());
        assert!(plan_grant("upload", "大量上传量").is_err());
        assert!(plan_grant("quota", "0 枚邀请名额").is_err());
        assert!(plan_grant("spark", "100 火花").is_err());
    }
}
