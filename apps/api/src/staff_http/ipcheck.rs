//! IP 核查/最大登录/上传量调整/重置密码/删禁用。
//! 从 staff_http.rs 按域拆出。

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;
use actix_web::{get, post, web, HttpRequest, Responder};
use serde::Deserialize;

#[get("/admin/ipcheck")]
pub async fn ipcheck(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::IP_CHECK)
        .await?;
    let rows: Vec<IpCheckRow> = sqlx::query_as(
        "SELECT host(ip) AS ip, \
            count(DISTINCT user_id) AS users, \
            string_agg(DISTINCT u.username, ', ') AS usernames, \
            max(le.created_at) AS last_seen \
         FROM login_events le LEFT JOIN users u ON u.id = le.user_id \
         WHERE ip IS NOT NULL AND user_id > 0 \
         GROUP BY ip HAVING count(DISTINCT user_id) > 1 \
         ORDER BY users DESC LIMIT 100",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

/// 失败登录（maxlogin.php 口径）：最近失败尝试
#[derive(serde::Serialize, sqlx::FromRow)]
struct FailedLoginRow {
    id: i64,
    username: Option<String>,
    ip: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/admin/maxlogin")]
pub async fn maxlogin(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::MAXLOGIN_VIEW,
    )
    .await?;
    let rows: Vec<FailedLoginRow> = sqlx::query_as(
        "SELECT le.id, COALESCE(u.username, '(未知用户)') AS username, host(le.ip) AS ip, le.created_at \
         FROM login_events le LEFT JOIN users u ON u.id = le.user_id \
         WHERE le.ok = false ORDER BY le.id DESC LIMIT 100",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

// ---- staffpanel 第二批运营工具：增加上传/重置密码/删被禁用户/邮箱黑白名单/IP测试/统计/清缓存/做清理/广告/查询页四件 ----

/// 增加上传（amountupload.php 口径）：全部或指定用户加/扣上传量
#[derive(Deserialize)]
struct AmountUploadBody {
    #[serde(default)]
    bytes: i64,
    #[serde(default)]
    user_id: Option<i64>,
    /// GB 口径（0286 P2b）：与 increment-bulk 的 uploaded 单位对齐——
    /// 面板直传 GB 时不再差 1024³ 倍；与 bytes 二选一，同传以 bytes 为准
    #[serde(default)]
    gb: Option<i64>,
    /// 幂等键（0286 P2b）：可选 8~120 字符，防网络重试双发
    #[serde(default)]
    idempotency_key: Option<String>,
}

#[post("/admin/amountupload")]
pub async fn admin_amount_upload(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<AmountUploadBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::USER_AMOUNTUPLOAD,
    )
    .await?;
    // gb → bytes 折算（bytes=0 且给了 gb 时启用；否则保持旧 bytes 语义）
    let bytes = if body.bytes == 0 {
        match body.gb {
            Some(g) => g.saturating_mul(1024 * 1024 * 1024),
            None => 0,
        }
    } else {
        body.bytes
    };
    if bytes == 0 || bytes.abs() > 10 * 1024 * 1024 * 1024 * 1024 {
        return Err(DomainError::Validation(
            "上传量需在 ±10TB 内且非 0（bytes 或 gb 二选一）".into(),
        ));
    }
    // 幂等占位（0286）：traffic_ledger 无独立幂等列，借 reason 字段
    // 埋键（reason 是文本列，查询 LIKE 可检索）；同键已存在 → 拒绝
    let idem_tag = body
        .idempotency_key
        .as_deref()
        .map(str::trim)
        .filter(|k| k.len() >= 8 && k.len() <= 120);
    if let Some(k) = body.idempotency_key.as_deref().map(str::trim) {
        if !k.is_empty() && (k.len() < 8 || k.len() > 120) {
            return Err(DomainError::Validation(
                "idempotency_key 需 8~120 字符".into(),
            ));
        }
    }
    if let Some(k) = idem_tag {
        let seen: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM traffic_ledger \
             WHERE reason = 'amountupload:' || $1)",
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
    // 审计修复（P0 错账）：uploaded 的权威在 traffic_ledger（worker reconcile 与
    // /admin/jobs/run:reconcile 会把 users.uploaded 重算为 sum(ledger)），此前裸
    // UPDATE 不落流水，管理员手工加量在下一次对账时被静默清零。改为同语句内
    // 落差额流水（torrent_id=0 为人工调账标记；扣成负数时按实际截断差额记账）。
    let n = sqlx::query(
        "WITH targets AS ( \
            SELECT id, uploaded FROM users \
            WHERE ($1::bigint IS NULL AND status < 2) OR id = $1 FOR UPDATE \
         ), upd AS ( \
            UPDATE users u SET uploaded = GREATEST(0, u.uploaded + $2) \
            FROM targets t WHERE u.id = t.id \
            RETURNING u.id, GREATEST(0, t.uploaded + $2) - t.uploaded AS delta \
         ) \
         INSERT INTO traffic_ledger (id, user_id, torrent_id, delta_up, delta_down, window_start) \
         SELECT nextval('traffic_ledger_id_seq'), id, 0, delta, 0, now() FROM upd WHERE delta <> 0",
    )
    .bind(body.user_id)
    .bind(bytes)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    // 指定用户不存在时明确报错（0287 P0）：与 amountbonus 同口径
    if body.user_id.is_some() && n == 0 {
        return Err(DomainError::NotFound(body.user_id.unwrap_or(0)));
    }
    if let Some(k) = idem_tag {
        // 幂等锚点行（delta=0 不影响账本）：把键写进 reason 供下次查重
        let _ = sqlx::query(
            "INSERT INTO traffic_ledger (id, user_id, torrent_id, \
             delta_up, delta_down, window_start, reason, operator_id) \
             VALUES (nextval('traffic_ledger_id_seq'), $1, NULL, \
             0, 0, now(), 'amountupload:' || $2, $3)",
        )
        .bind(body.user_id.unwrap_or(0))
        .bind(k)
        .bind(auth.id)
        .execute(&state.repo.db)
        .await;
    }
    state
        .repo
        .audit(Some(auth.id), "amount_upload", body.user_id)
        .await;
    Ok(ok(serde_json::json!({ "affected": n })))
}

/// 重置用户密码（reset.php 口径）：设临时密码 + 强制首登改密
#[derive(Deserialize)]
struct ResetPassBody {
    user_id: i64,
}

#[post("/admin/resetpass")]
pub async fn admin_reset_pass(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ResetPassBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::USER_RESETPASS,
    )
    .await?;
    // 临时密码（仅返回一次）：CSPRNG 16 字节 base64——旧版操作者id+纳秒取模
    // 密码空间小且可预测，配合明文回显存在猜测窗口
    let temp_pass = format!(
        "Tmp@{}",
        data_encoding::BASE64URL_NOPAD.encode(&rand::random::<[u8; 16]>())
    );
    let hash = crate::domain::hash_password(&temp_pass)?;
    let n = sqlx::query(
        "UPDATE users SET pass_hash=$2, \
         must_reset_password=true WHERE id=$1 AND status<3",
    )
    .bind(body.user_id)
    .bind(&hash)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(body.user_id));
    }
    state
        .repo
        .audit(Some(auth.id), "admin_reset_pass", Some(body.user_id))
        .await;
    Ok(ok(
        serde_json::json!({ "user_id": body.user_id, "temp_password": temp_pass }),
    ))
}

/// 删除被禁用户（deletedisabled.php 口径）：status=2 的账号连同业务数据清理
#[post("/admin/deletedisabled")]
pub async fn admin_delete_disabled(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::USER_DELETE_DISABLED,
    )
    .await?;
    // 审计修复：旧版裸 `DELETE FROM users WHERE status = 2` 单条 SQL——任何被
    // NO ACTION 引用（如 invites.inviter_id / audit_log.actor_id）的用户会让整条
    // 语句外键失败 500；部分 CASCADE 则静默丢数据。改为复用权威路径 admin/users/{id}
    // 的口径：强制走 `DELETE /api/v1/admin/users/{id}` 同一实现（逐个、事务化、83 列清理）。
    // 这里直接构造内部请求等价物：调用 admin_http 的清理清单不跨模块，故改为
    // 逐个转发 HTTP 会引入自调用复杂度——最简正确实现：拒绝批量、提示走单删。
    let ids: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM users WHERE status = 2 ORDER BY id LIMIT 500",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let mut deleted: Vec<i64> = Vec::new();
    let mut failed: Vec<(i64, String)> = Vec::new();
    for uid in ids {
        // 与 user_admin_delete 同款清单式单事务删除（简化为直接执行权威清理序列）
        match crate::admin_http::delete_user_cascade(&state.repo.db, uid).await
        {
            Ok(()) => deleted.push(uid),
            Err(e) => failed.push((uid, e.to_string())),
        }
    }
    state
        .repo
        .audit(Some(auth.id), "delete_disabled_users", None)
        .await;
    Ok(ok(
        serde_json::json!({ "deleted": deleted.len(), "ids": deleted, "failed": failed }),
    ))
}

/// 重复 IP 检测（ipcheck.php 口径）：同 IP 登录过的多账号聚合
#[derive(serde::Serialize, sqlx::FromRow)]
pub(super) struct IpCheckRow {
    pub(super) ip: Option<String>,
    pub(super) users: i64,
    pub(super) usernames: Option<String>,
    pub(super) last_seen: Option<chrono::DateTime<chrono::Utc>>,
}
