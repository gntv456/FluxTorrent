//! 站务信/加用户/加成。
//! 从 staff_http.rs 按域拆出。

use actix_web::{post, web, HttpRequest, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

// ============ staffpanel 管理工具（hxpt faqmanage/modrules/catmanage/bans/massmail 口径） ============

#[post("/admin/staffmess")]
pub async fn staffmess_send(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<StaffMessBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::STAFFMESS)
        .await?;
    if body.subject.trim().is_empty() || body.body.trim().is_empty() {
        return Err(DomainError::Validation("主题和正文不能为空".into()));
    }
    let n = sqlx::query(
        "INSERT INTO messages (sender_id, receiver_id, subject, body) \
         SELECT $1, id, $2, $3 FROM users WHERE status < 2 AND ($4::int IS NULL OR class_id >= $4)",
    )
    .bind(auth.id)
    .bind(body.subject.trim())
    .bind(&body.body)
    .bind(body.min_class)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    state
        .repo
        .audit(Some(auth.id), "staffmess_send", None)
        .await;
    Ok(ok(serde_json::json!({ "sent": n })))
}

/// 添加用户（adduser.php 口径）：管理组直接建号（class 0，需首登改密）
#[derive(Deserialize)]
struct AddUserBody {
    username: String,
    email: String,
    password: String,
}

#[post("/admin/adduser")]
pub async fn admin_add_user(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<AddUserBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::USER_CREATE)
        .await?;
    if body.username.trim().len() < 2
        || !body.email.contains('@')
        || body.password.len() < 8
    {
        return Err(DomainError::Validation(
            "用户名≥2字符、邮箱合法、密码≥8位".into(),
        ));
    }
    let pass_hash = crate::domain::hash_password(&body.password)?;
    let uid = state
        .repo
        .create_user(body.username.trim(), body.email.trim(), &pass_hash, None)
        .await
        .map_err(|_| DomainError::Validation("用户名或邮箱已存在".into()))?;
    // 管理组建的号要求首登改密
    let _ = sqlx::query(
        "UPDATE users SET must_reset_password = true WHERE id = $1",
    )
    .bind(uid)
    .execute(&state.repo.db)
    .await;
    state
        .repo
        .audit(Some(auth.id), "admin_add_user", Some(uid))
        .await;
    Ok(ok(serde_json::json!({ "user_id": uid })))
}

/// 增加魔力（amountbonus.php 口径）：全部用户或指定用户
#[derive(Deserialize)]
struct AmountBonusBody {
    amount: i64,
    #[serde(default)]
    user_id: Option<i64>,
    /// 幂等键（0286 P2b）：可选，8~120 字符——网络重试同键直接拒绝，
    /// 防单发双发（此前与 increment-bulk/gacha 的幂等口径不一致）
    #[serde(default)]
    idempotency_key: Option<String>,
}

#[post("/admin/amountbonus")]
pub async fn admin_amount_bonus(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<AmountBonusBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::USER_AMOUNTBONUS,
    )
    .await?;
    if body.amount == 0 || body.amount.abs() > 1_000_000 {
        return Err(DomainError::Validation(
            "数量需在 ±1,000,000 之间且非 0".into(),
        ));
    }
    // 幂等占位（0286）：同键已存在 → 直接拒绝（口径同 gacha grant）
    let idem_prefix = body
        .idempotency_key
        .as_deref()
        .map(str::trim)
        .filter(|k| !k.is_empty());
    if let Some(k) = idem_prefix {
        if k.len() < 8 || k.len() > 120 {
            return Err(DomainError::Validation(
                "idempotency_key 需 8~120 字符".into(),
            ));
        }
        // 落键形态 'amountbonus:{k}:{uid}'（下方循环拼）——LIKE 前缀匹配；
        // 全量发放（user_id=None 多户）时同键在任一户存在即视为已发放
        let seen: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM spark_ledger \
             WHERE idempotency_key LIKE 'amountbonus:' || $1 || ':%')",
        )
        .bind(k)
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or(false);
        if seen {
            return Err(DomainError::Validation(
                "该 idempotency_key 已发放过".into(),
            ));
        }
    }
    // 走统一账务管线：事务 + 逐户流水 + balance_after 快照（原实现裸 UPDATE 绕过
    // spark_ledger，账本 sum(amount) 与余额失配、管理端 spark-logs 查不到这类变动）
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let ids: Vec<(i64, i64)> = sqlx::query_as(
                "SELECT id, \
         spark_balance FROM users WHERE ($1::bigint IS NULL AND status < 2) OR id = $1 FOR UPDATE",
    )
    .bind(body.user_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    for (uid, before) in &ids {
        // 账本权威口径：流水按「实际前后差额」落账（与 increment_bulk 同修复）。
        // 旧版余额 GREATEST(0,...) 截断但流水记原始 amount，负扣被截断的部分
        // 会在 worker 小时级 sum(ledger) 重算时被重新兑现（账本撕裂）。
        let after: i64 = sqlx::query_scalar(
            "UPDATE users SET spark_balance = GREATEST(0, \
             spark_balance + $2) WHERE id = $1 RETURNING spark_balance",
        )
        .bind(uid)
        .bind(body.amount)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        let actual_delta = after - before; // 行已 FOR UPDATE，before 即更新前权威值
        if actual_delta == 0 {
            continue; // 截断后无实际变动（如余额 0 再负扣）：不落流水，保持 sum(ledger)=balance
        }
        sqlx::query(
            "INSERT INTO spark_ledger (id, user_id, amount, kind, \
             ref_type, ref_id, idempotency_key, balance_after) \n VALUES \
             (nextval('spark_ledger_id_seq'), $1, $2, 'admin', 'amountbonus', \
             $3, $4, $5)",
        )
        .bind(uid)
        .bind(actual_delta)
        .bind(auth.id)
        .bind(match idem_prefix {
            Some(k) => format!("amountbonus:{k}:{}", uid),
            None => format!(
                "amountbonus-{}-{}",
                uid,
                uuid::Uuid::new_v4().simple()
            ),
        })
        .bind(after)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let n = ids.len() as i64;
    state
        .repo
        .audit(Some(auth.id), "amount_bonus", body.user_id)
        .await;
    Ok(ok(serde_json::json!({ "affected": n })))
}

/// 批量私信（staffmess.php 口径）：给全部（或某等级以上）用户发站内信
#[derive(Deserialize)]
pub(super) struct StaffMessBody {
    pub(super) subject: String,
    pub(super) body: String,
    #[serde(default)]
    pub(super) min_class: Option<i32>,
}
