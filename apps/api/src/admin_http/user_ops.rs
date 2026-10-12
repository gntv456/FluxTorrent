use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::guard::{ensure_outranks, staff};

/// 「修改上传量等」（参考站用户详情按钮口径）：delta 语义，正加负减，下限 0
#[derive(Deserialize)]
struct UserAdjustReq {
    user_id: i64,
    #[serde(default)]
    uploaded_delta: Option<i64>,
    #[serde(default)]
    downloaded_delta: Option<i64>,
    #[serde(default)]
    spark_delta: Option<i64>,
    /// 增发邀请码数量（正数为增发，直接发放有效邀请）
    #[serde(default)]
    invite_grant: Option<i32>,
    #[serde(default)]
    note: Option<String>,
    /// 幂等键（0287 P1）：可选 8~120 字符——同键重试直接拒绝，
    /// 防管理员手抖双击造成双份调整（与 amountbonus 口径一致）
    #[serde(default)]
    idempotency_key: Option<String>,
}

#[post("/admin/users/adjust")]
async fn user_adjust(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<UserAdjustReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    if body.uploaded_delta.is_none()
        && body.downloaded_delta.is_none()
        && body.spark_delta.is_none()
        && body.invite_grant.is_none()
    {
        return Err(DomainError::Validation("至少提供一项调整".into()));
    }
    // 留痕长度上限（批四十一 P-156/157 同族）：note 原样进 traffic_ledger.reason
    // 与 user_modify_logs.content，实测 2000 字的 note 落成 2003 字符；同仓
    // `audit_detail` 的 reason 是按 200 字截断的。管理侧的理由一律拒超长而非
    // 静默截——截掉的正是纠纷仲裁要的那半句。
    if let Some(n) = body.note.as_deref() {
        if n.chars().count() > 200 {
            return Err(DomainError::Validation("note 需 ≤200 字符".into()));
        }
    }
    // 单次数值上限（P-154，实测红线）：旧实现只 `max(0)` 再 `as i64`，
    // `uploaded_delta = i64::MAX` 直接 200 落库 ⇒ uploaded 变 9.2E18、
    // traffic_ledger 照记这条天量流水。而余额权威是
    // `balance_baseline + SUM(traffic_ledger)`，脏账不会自愈，还会污染
    // 分享率 / H&R / 排行榜 / 促销资格。上限取 1 TiB（流量）与 10 亿（火花）。
    const MAX_TRAFFIC: i64 = 1_i64 << 40;
    const MAX_SPARK: i64 = 1_000_000_000;
    for (k, v, lim) in [
        ("uploaded_delta", body.uploaded_delta.unwrap_or(0), MAX_TRAFFIC),
        ("downloaded_delta", body.downloaded_delta.unwrap_or(0), MAX_TRAFFIC),
        ("spark_delta", body.spark_delta.unwrap_or(0), MAX_SPARK),
    ] {
        if v.abs() > lim {
            return Err(DomainError::Validation(format!(
                "{k} 超单次上限（流量 ±1TiB、火花 ±10 亿）"
            )));
        }
    }
    // 邀请上限要在动任何账之前拒掉。旧版是 `grant.min(50)` 静默截断，
    // 再把**请求值**回显给站长（实测：要 100 拿到 50，响应写 100），
    // 于是报表与库对不上，而且没人知道被截过。
    if let Some(g) = body.invite_grant {
        if g.abs() > 50 {
            return Err(DomainError::Validation("邀请单次 ±50".into()));
        }
    }
    // 数值调整仅 sysop/管理员（等级 93+）
    crate::authz::require_perm(&state, &auth, crate::authz::perm::USER_ADJUST)
        .await?;
    // 幂等占位（0287）：spark 侧落键 admin-adjust:{k}:{uid} 前缀查重；
    // traffic 侧 reason 埋 admin-adjust:{k} 锚点
    let idem = body
        .idempotency_key
        .as_deref()
        .map(str::trim)
        .filter(|k| !k.is_empty());
    if let Some(k) = idem {
        if k.len() < 8 || k.len() > 120 {
            return Err(DomainError::Validation(
                "idempotency_key 需 8~120 字符".into(),
            ));
        }
        let seen: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM spark_ledger \
             WHERE idempotency_key LIKE 'admin-adjust:' || $1 || ':%') \
             OR EXISTS(SELECT 1 FROM traffic_ledger \
             WHERE reason = 'admin-adjust:' || $1)",
        )
        .bind(k)
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or(false);
        if seen {
            return Err(DomainError::Validation(
                "该 idempotency_key 已使用过".into(),
            ));
        }
    }
    // 数值调整同属伤害性操作，须严格高于目标等级
    ensure_outranks(&state.repo.db, auth.class_id, body.user_id).await?;
    // 键的归属：只发邀请的请求不写任何流水，旧实现于是「同键连发两次各 5
    // 张码 = 库里 10 张」（P-155 红线实拍）。复用本仓给「无流水的管理动作」
    // 留的幂等位 `grant_batches`（idempotency_key 有唯一约束，撞键即 400）。
    if let Some(k) = idem {
        let batch_id = uuid::Uuid::new_v4().simple().to_string();
        crate::admin_p3_http::batch_open(
            &state.repo.db,
            &crate::admin_p3_http::BatchSpec {
                batch_id: &batch_id,
                idem: Some(k),
                actor_id: auth.id,
                kind: "user_adjust",
                amount: body.invite_grant.unwrap_or(0) as i64,
                item_id: None,
                medal_id: None,
                days: None,
                selector: serde_json::json!({"note": body.note}),
                targets: &[body.user_id],
                subject: body.note.as_deref(),
            },
        )
        .await?;
    }
    let row: Option<(i64, i64, i64)> = sqlx::query_as(
        "SELECT uploaded, downloaded, spark_balance FROM users WHERE id = $1",
    )
    .bind(body.user_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let (up0, down0, spark0) =
        row.ok_or(DomainError::NotFound(body.user_id))?;
    // clamp 到 [0, i64::MAX]：上限校验已挡住常规越界，这里兜住「库里已有
    // 天量脏值」的存量情形，绝不再用 `as i64` 把溢出截成乱数。
    let add = |base: i64, delta: i64| -> i64 {
        (base as i128 + delta as i128).clamp(0, i64::MAX as i128) as i64
    };
    let up = add(up0, body.uploaded_delta.unwrap_or(0));
    let down = add(down0, body.downloaded_delta.unwrap_or(0));
    let spark = add(spark0, body.spark_delta.unwrap_or(0));
    // 流量调整必须走流水（P0-2，0285）：users.uploaded/downloaded 只是
    // 「balance_baseline + SUM(traffic_ledger)」的快照，worker 每次消费 announce 都会重算覆盖。
    // 旧实现只 UPDATE users ⇒ 实测站长补的 1GB 在用户下一次 announce 后归零，
    // 而接口全程 200、无任何告警（补偿/捐赠/商店发放同病）。
    let actual_up = up - up0;
    let actual_down = down - down0;
    if actual_up != 0 || actual_down != 0 {
        let reason = match idem {
            Some(k) => format!("admin-adjust:{k}"),
            None => body
                .note
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|s| format!("#{}/{}", auth.id, s))
                .unwrap_or_else(|| format!("#{} admin_adjust", auth.id)),
        };
        sqlx::query(
            "INSERT INTO traffic_ledger (id, user_id, torrent_id, \
             delta_up, delta_down, window_start, reason, operator_id) \
             VALUES (nextval('traffic_ledger_id_seq'), $1, NULL, \
                     $2, $3, now(), $4, $5)",
        )
        .bind(body.user_id)
        .bind(actual_up)
        .bind(actual_down)
        .bind(reason)
        .bind(auth.id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }
    sqlx::query(
        "UPDATE users SET uploaded = $2, downloaded = $3, \
         spark_balance = $4 WHERE id = $1",
    )
    .bind(body.user_id)
    .bind(up)
    .bind(down)
    .bind(spark)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 火花调整写流水（余额权威在 spark_ledger；kind=admin，操作者入 ref_id）
    if let Some(delta) = body.spark_delta {
        if delta != 0 {
            // 审计修复（P0 错账）：流水必须记「实际发生的差额」而非请求 delta——
            // 余额不足扣成负数时被 clamp 到 0，此前流水仍记请求值，
            // sum(ledger) 与 balance 永久撕裂（对照 amountbonus / increment-bulk 已修口径）。
            let actual_delta = spark - spark0;
            if actual_delta != 0 {
                sqlx::query(
                    "INSERT INTO spark_ledger (id, user_id, amount, kind, ref_type, ref_id, idempotency_key, balance_after) \
                     VALUES (nextval('spark_ledger_id_seq'), $1, $2, 'admin', 'adjust', $3, $4, $5)",
                )
                .bind(body.user_id)
                .bind(actual_delta)
                .bind(auth.id)
                .bind(match idem {
                    Some(k) => format!("admin-adjust:{}:{}", k, body.user_id),
                    None => format!(
                        "admin-adjust-{}-{}",
                        body.user_id,
                        uuid::Uuid::new_v4().simple()
                    ),
                })
                .bind(spark)
                .execute(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
            }
        }
    }
    // 增发邀请：直接生成有效邀请码（30 天有效，NP takeinvite 口径）
    let mut granted_codes: Vec<String> = Vec::new();
    let mut revoked_invites: i64 = 0;
    if let Some(grant) = body.invite_grant {
        if grant > 0 {
            for _ in 0..grant {
                let code = uuid::Uuid::new_v4().simple().to_string();
                sqlx::query(
                    "INSERT INTO invites (inviter_id, \
                     code, expires_at) VALUES ($1, $2, now() + interval '30 \
                     days')",
                )
                .bind(body.user_id)
                .bind(&code)
                .execute(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
                granted_codes.push(code);
            }
        } else if grant < 0 {
            // 负数：回收最早到期的未用邀请
            revoked_invites = sqlx::query(
                "DELETE FROM invites WHERE ctid IN (\
                    SELECT ctid FROM invites WHERE inviter_id = $1 AND status = 0 \
                    ORDER BY expires_at LIMIT $2)",
            )
            .bind(body.user_id)
            .bind((-grant) as i64)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .rows_affected() as i64;
        }
    }
    state
        .repo
        .audit(Some(auth.id), "user.adjust", Some(body.user_id))
        .await;
    crate::admin_p3_http::modify_log(
        &state.repo.db,
        body.user_id,
        Some(auth.id),
        &format!(
            "数值调整 up{:+}/down{:+}/spark{:+}/invite{:+}",
            actual_up,
            actual_down,
            spark - spark0,
            granted_codes.len() as i64 - revoked_invites,
        ),
    )
    .await;
    Ok(ok(serde_json::json!({
        "user_id": body.user_id,
        "uploaded": up, "downloaded": down, "spark": spark,
        // 一律回**实际发生数**（0291）：旧版回显请求值，被 clamp 掉的差额
        // 在响应里看不出来，站长与库对不上账。
        "invite_granted": granted_codes.len() as i64,
        "invite_revoked": revoked_invites,
        "granted_codes": granted_codes,
        "note": body.note,
    })))
}
