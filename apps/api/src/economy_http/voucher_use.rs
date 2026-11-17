//! 免费券使用（0073）。
//! 从 economy_http.rs 按域拆出。

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;
use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

/// 用券：把一张未用未过期的券绑定到种子（CAS 防并发双用）。
/// 生效在 worker 计费侧：该种该用户的下载增量按 0 计（free）/上下行均 0 计（neutral），
/// 当累计下载超过种子大小 4% 时核销（Gazelle slop 口径——防买了券只下 1% 就转移给别人用）。
#[post("/me/vouchers/use")]
async fn voucher_use(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<VoucherUseReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    // temp_invite 券（0204 修死库存）：核销即发一枚邀请码，天数取
    // invite_admin_ttl_days（30 天口径），不绑种子。其余券种维持绑种子语义。
    let vkind: Option<String> = sqlx::query_scalar(
        "SELECT kind FROM user_vouchers WHERE id = $1 AND user_id = $2 \
         AND used_torrent_id IS NULL AND used_at IS NULL AND expires_at > now()",
    )
    .bind(body.voucher_id)
    .bind(auth.id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    match vkind.as_deref() {
        Some("temp_invite") => {
            let ttl_days = invite_admin_ttl_days(&state.repo.db).await;
            let code = crate::domain::new_invite_code();
            let expires = chrono::Utc::now() + chrono::Duration::days(ttl_days);
            // P-407：旧顺序是「CAS 销券（autocommit）→ 再发邀请码」。发码那步
            // 一旦失败（DB 抖动/约束），券已经是 used_at=now()，而这条 CAS 只认
            // used_at IS NULL ⇒ 重试被「券不存在、已使用或已过期」挡回：会员手里
            // 既没有券也没有码，那张券花出去的火花永久蒸发。同型口径见 P-364
            // （钱到账才落标记）与刚修的 P-406（撤销的 CAS 与退款同事务）。
            // 现在两条写进同一事务：任何一步失败 ⇒ 整体回滚，券与码一个都不动。
            let mut tx = state
                .repo
                .db
                .begin()
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
            let id = issue_invite_tx(&mut tx, auth.id, &code, expires).await?;
            let n = sqlx::query(
                "UPDATE user_vouchers SET used_at = now() WHERE id = $1 \
                 AND user_id = $2 AND used_at IS NULL \
                 AND used_torrent_id IS NULL AND expires_at > now()",
            )
            .bind(body.voucher_id)
            .bind(auth.id)
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .rows_affected();
            if n == 0 {
                // 券在这瞬间被另一路用掉/过期：回滚，不把刚生成的码留在库里
                return Err(DomainError::Validation(
                    super::vouchers::why_unusable(
                        &state.repo.db,
                        body.voucher_id,
                        auth.id,
                    )
                    .await,
                ));
            }
            tx.commit()
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
            state
                .repo
                .audit(Some(auth.id), "voucher.temp_invite", Some(id))
                .await;
            return Ok(ok(serde_json::json!({
                "voucher_id": body.voucher_id,
                "invite_id": id,
                "code": code,
                "expires_at": expires.to_rfc3339(),
            })));
        }
        None => {
            return Err(DomainError::Validation(
                super::vouchers::why_unusable(
                    &state.repo.db,
                    body.voucher_id,
                    auth.id,
                )
                .await,
            ));
        }
        // 0211 H2：绑种子语义只属于免费/中性券——rename_card 等其它券种
        // 误入此口 = 花钱买空气（绑上种子后既不生效也无消费出口），显式拒绝。
        Some(k) if k != "free" && k != "neutral" => {
            return Err(DomainError::Validation(format!(
                "该券类型（{k}）不能绑定种子；请在对应的入口使用"
            )));
        }
        _ => {}
    }
    // 绑定即生效但不置 used_at：worker 计费侧以「used_torrent_id 已绑定 + used_at IS NULL」
    // 判定生效中的券，下载量过阈值后由核销语句置 used_at。旧版绑定时就写 used_at，
    // 导致券永远不被计费侧匹配（用户花钱买的权益确定性为 0，真实资损）。
    let n = sqlx::query(
        "UPDATE user_vouchers SET used_torrent_id = $3 \
         WHERE id = $1 AND user_id = $2 AND used_torrent_id IS NULL AND used_at IS NULL AND expires_at > now()",
    )
    .bind(body.voucher_id)
    .bind(auth.id)
    .bind(body.torrent_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        // P-459：CAS 没打中不等于「用过或过期」——最常见的真因是这张券
        // 已经绑定在某张种子上生效中（面板还把它列在「可用券」里）。
        return Err(DomainError::Validation(
            super::vouchers::why_unusable(
                &state.repo.db,
                body.voucher_id,
                auth.id,
            )
            .await,
        ));
    }
    state
        .repo
        .audit(Some(auth.id), "voucher.use", Some(body.torrent_id))
        .await;
    Ok(ok(serde_json::json!({
        "voucher_id": body.voucher_id,
        "torrent_id": body.torrent_id,
        "note": "已对该种子生效；下载量超过种子大小 4% 后自动核销"
    })))
}

/// 事务内发一枚邀请码（P-407）。
///
/// 与 `Repo::issue_invite` 是同一条 INSERT：之所以在本地再写一遍，是券核销必须
/// 与发码同事务，而 `repo/mod.rs` 已压在行数基线上（多一行就红）。
/// 动任何一侧请同步另一侧——只有本口走事务版。
async fn issue_invite_tx(
    tx: &mut sqlx::PgTransaction<'_>,
    inviter_id: i64,
    code: &str,
    expires_at: chrono::DateTime<chrono::Utc>,
) -> DomainResult<i64> {
    Ok(sqlx::query_scalar::<_, i64>(
        "INSERT INTO invites (inviter_id, code, expires_at) \
         VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(inviter_id)
    .bind(code)
    .bind(expires_at)
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?)
}

/// 管理端直发邀请码 TTL（0204 配置化）：site_settings.invite_admin_ttl_days，
/// 缺省 30，clamp 1..=365。temp_invite 券核销与 admin 直发同口径。
pub async fn invite_admin_ttl_days(db: &sqlx::PgPool) -> i64 {
    sqlx::query_scalar(
        "SELECT value FROM site_settings WHERE name = 'invite_admin_ttl_days'",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .and_then(|v: String| v.parse().ok())
    .map(|v: i64| v.clamp(1, 365))
    .unwrap_or(30)
}

#[derive(Deserialize)]
pub(super) struct VoucherUseReq {
    pub(super) voucher_id: i64,
    /// 免费券/中性券绑种子必填；temp_invite 券核销不消费此字段（可传 0）
    #[serde(default)]
    pub(super) torrent_id: i64,
}
