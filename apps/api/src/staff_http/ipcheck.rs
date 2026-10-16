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
    // 幂等键校验（0286）：8~120 字符，防网络重试双发
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
    // 目标须是操作者的严格下级（与 /admin/users/adjust 同判据）。这一口直接
    // 写 `uploaded`，而它是分享率的分子与 H&R 义务的基线；同族改账口都有
    // 护栏、只有它没有 ⇒ 批四十三 P-168（默认权限表下要 class≥99 才够得着，
    // 属纵深缺口：`role_permissions` 可被后台改，键一旦下放就没人挡）。
    if let Some(uid) = body.user_id {
        crate::admin_http::guard::ensure_outranks(
            &state.repo.db,
            auth.class_id,
            uid,
        )
        .await?;
    }
    // 幂等键改走 grant_batches 那把专职槽（同 `/admin/users/increment-bulk`）。
    // 0286 的老写法是「借 traffic_ledger.reason 埋键 + 先 SELECT EXISTS 再
    // INSERT」——两个同键并发请求能双双通过 EXISTS，实测同一把键加出两份量
    // （1MiB ×2 都 200、锚点行 2 条）；UNIQUE(idempotency_key) 让数据库来判重。
    let batch_id = uuid::Uuid::new_v4().simple().to_string();
    if idem_tag.is_some() {
        crate::admin_p3_http::batch_open(
            &state.repo.db,
            &crate::admin_p3_http::BatchSpec {
                batch_id: &batch_id,
                idem: idem_tag,
                actor_id: auth.id,
                kind: "amount_upload",
                amount: bytes,
                item_id: None,
                medal_id: None,
                days: None,
                selector: serde_json::json!({
                    "scope": if body.user_id.is_some() { "one" } else { "all" }
                }),
                targets: &body.user_id.into_iter().collect::<Vec<_>>(),
                subject: None,
            },
        )
        .await?;
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
    if idem_tag.is_some() {
        // 回写批次结果：grant_batches.status 是站长排查「重复提交有没有被挡」的入口
        let _ = crate::admin_p3_http::batch_finish(
            &state.repo.db,
            &batch_id,
            n as i64,
            if n > 0 { "done" } else { "failed" },
            None,
        )
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
    // 审计修复：旧版裸 `DELETE ... WHERE status = 2` 里凡被 NO ACTION 引用（invites.
    // inviter_id / audit_log.actor_id）的账号都把整句顶成 500，部分 CASCADE 又静默丢
    // 数据 ⇒ 逐个走 admin_http::delete_user_cascade（同单删口）。`count(*) OVER()` 在
    // LIMIT 前求值：一次查询同时给出本批与存量，界面才报得出「还剩几个」。
    let ids: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT id, count(*) OVER() AS total FROM users          WHERE status = 2 ORDER BY id LIMIT 500",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let total = ids.first().map(|r| r.1).unwrap_or(0);
    let mut deleted: Vec<i64> = Vec::new();
    let mut failed: Vec<(i64, String)> = Vec::new();
    for (uid, _) in ids {
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
        serde_json::json!({ "deleted": deleted.len(), "ids": deleted,
            "failed": failed, "remaining": total - deleted.len() as i64 }),
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
