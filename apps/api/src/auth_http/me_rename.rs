//! 用户自助改名（改名卡核销口，P-477 的收口侧）。
//!
//! 商店「改名卡」（`user_vouchers.kind='rename_card'`）自 P-477 起停售，
//! 原因是发了券没有任何口认它。本口即注释里说的「核销口」：
//! 一张未用未过期改名卡 + 新用户名 → 同事务 CAS 销券 + 改名 + 写
//! username_change_logs。验证尺与注册同源（3-24 字符、字母数字下划线或
//! 中文），与管理员改名口（user_rename.rs，1-32 无字符集限制）刻意不同：
//! 自助面收得紧，人工面留给站长裁量。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[derive(Deserialize, utoipa::ToSchema)]
pub struct RenameCardReq {
    pub new_name: String,
    /// 要核销的改名卡券 id；缺省取最早到期的一张可用券
    #[serde(default)]
    pub voucher_id: Option<i64>,
}

#[utoipa::path(
    post,
    path = "/api/v1/me/rename",
    request_body = RenameCardReq,
    responses((status = 200, description = "改名成功，返回新旧名与券 id"))
)]
#[post("/me/rename")]
pub async fn me_rename(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<RenameCardReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let new_name = body.new_name.trim().to_string();
    // 与 domain::validate_register 同尺的字符集校验（抽出来防两处漂移；
    // 注册那条在 mod.rs，本文件不引 domain 以免压它行数基线）
    let len = new_name.chars().count();
    if !(3..=24).contains(&len) {
        return Err(DomainError::Validation("用户名长度需 3-24 字符".into()));
    }
    if !new_name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || (c as u32) > 0x4e00)
    {
        return Err(DomainError::Validation(
            "用户名仅允许字母数字下划线或中文".into(),
        ));
    }
    let db = &state.repo.db;
    let old_name: String =
        sqlx::query_scalar("SELECT username FROM users WHERE id = $1")
            .bind(auth.id)
            .fetch_one(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    if old_name == new_name {
        return Err(DomainError::Validation("新用户名与当前相同".into()));
    }
    // 重名占位（事务内再查一次防并发窗口，此处先挡明显冲突）
    let taken: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM users WHERE username = $1 AND id <> $2)",
    )
    .bind(&new_name)
    .bind(auth.id)
    .fetch_one(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if taken {
        return Err(DomainError::Validation("用户名已被占用".into()));
    }

    // 券：显式指定或取最早到期的一张可用改名卡
    let vid: i64 = match body.voucher_id {
        Some(v) => {
            let kind: Option<String> = sqlx::query_scalar(
                "SELECT kind FROM user_vouchers WHERE id = $1 \
                 AND user_id = $2 AND used_torrent_id IS NULL \
                 AND used_at IS NULL AND expires_at > now()",
            )
            .bind(v)
            .bind(auth.id)
            .fetch_optional(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            if kind.as_deref() != Some("rename_card") {
                return Err(DomainError::Validation(
                    "改名卡不存在、已使用或已过期".into(),
                ));
            }
            v
        }
        None => {
            let v: Option<i64> = sqlx::query_scalar(
                "SELECT id FROM user_vouchers WHERE user_id = $1 \
                 AND kind = 'rename_card' AND used_torrent_id IS NULL \
                 AND used_at IS NULL AND expires_at > now() \
                 ORDER BY expires_at LIMIT 1",
            )
            .bind(auth.id)
            .fetch_optional(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            v.ok_or_else(|| {
                DomainError::Validation(
                    "没有可用的改名卡——可在商店购买（我的券页面自动抵扣）"
                        .into(),
                )
            })?
        }
    };

    // 券消费 + 改名 + 台账同事务（P-407 同型口径：任何一步失败整体回滚，
    // 券不蒸发）。改名 UPDATE 带旧名条件做乐观并发：两人同秒抢同名，
    // 后提交者 affected 0 行 → 回滚重试时报「已被占用」。
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let consumed = sqlx::query(
        "UPDATE user_vouchers SET used_at = now() \
         WHERE id = $1 AND user_id = $2 AND used_torrent_id IS NULL \
         AND used_at IS NULL AND expires_at > now()",
    )
    .bind(vid)
    .bind(auth.id)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if consumed == 0 {
        return Err(DomainError::Validation(
            "改名卡不存在、已使用或已过期".into(),
        ));
    }
    let renamed = sqlx::query(
        "UPDATE users SET username = $2 WHERE id = $1 AND username = $3",
    )
    .bind(auth.id)
    .bind(&new_name)
    .bind(&old_name)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if renamed == 0 {
        return Err(DomainError::Validation("用户名已被占用".into()));
    }
    sqlx::query(
        "INSERT INTO username_change_logs (uid, old_name, new_name, operator) \
         VALUES ($1, $2, $3, $1)",
    )
    .bind(auth.id)
    .bind(&old_name)
    .bind(&new_name)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

    state
        .repo
        .audit_detail(
            Some(auth.id),
            "user.self_rename",
            Some(vid),
            Some(&format!("{old_name} -> {new_name}")),
            None,
        )
        .await;
    Ok(ok(serde_json::json!({
        "voucher_id": vid,
        "old_name": old_name,
        "new_name": new_name,
    })))
}
