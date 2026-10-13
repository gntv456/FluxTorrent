//! 促销 intent 共享核心（0356）：快照哈希 / kind→effect 折算 / 直挂留痕。
//! 审批端点在 promo_intent_approve.rs，队列读在 promo_intent_list.rs。
//! 系统入口（worker 自动促销/字幕联动）无审批语义，不经 promo_intents 表。

use crate::errors::{DomainError, DomainResult};
use sha3::Digest;

/// 审批快照：sha3-256(kind|starts_at|ends_at|决定时点|请求者) hex。
/// kind 落 promotions 时取 promo_kinds.effect（注册表 0213 口径），
/// 这里对「申请的 kind」打快照——请求与裁决的同字段比对。
pub(super) fn snapshot_hash(
    kind: &str,
    starts_at: &chrono::DateTime<chrono::Utc>,
    ends_at: &chrono::DateTime<chrono::Utc>,
    decided_at: &chrono::DateTime<chrono::Utc>,
    requested_by: i64,
) -> String {
    let mut h = sha3::Sha3_256::new();
    h.update(kind.as_bytes());
    h.update(b"|");
    h.update(starts_at.to_rfc3339().as_bytes());
    h.update(b"|");
    h.update(ends_at.to_rfc3339().as_bytes());
    h.update(b"|");
    h.update(decided_at.to_rfc3339().as_bytes());
    h.update(b"|");
    h.update(requested_by.to_string().as_bytes());
    let out = h.finalize();
    out.iter().map(|b| format!("{b:02x}")).collect()
}

/// kind → promotions 落地效果（promo_kinds.effect；sticky* 走 pos_state
/// 不落 promotions，intent 审批只受理促销类 effect）。
pub(super) async fn promo_effect(
    tx: &mut sqlx::PgConnection,
    kind: &str,
) -> DomainResult<String> {
    let effect: Option<String> = sqlx::query_scalar(
        "SELECT effect FROM promo_kinds WHERE kind = $1 AND enabled",
    )
    .bind(kind)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    match effect.as_deref() {
        // 与 promotions.kind 枚举同域的 effect 才可落 promotions；
        // sticky* 不支持走 intent（自购链路已有，直挂场景管理组用置顶工具）
        Some(e) if e != "sticky1" && e != "sticky2" => Ok(e.to_string()),
        _ => Err(DomainError::Validation(
            "该促销类型不可走审批（不存在/未启用/置顶类）".into(),
        )),
    }
}

/// 管理组直挂留痕：freeleech_set 直接写 promotions 的同时落一条
/// approved 状态的 intent 行（快照哈希同口径），「为什么是这个倍率」可查。
/// 失败不阻塞直挂——留痕是审计面，不是业务前置（与 audit 同语义）。
pub(super) async fn log_direct_intent(
    tx: &mut sqlx::PgConnection,
    scope: &str,
    torrent_id: Option<i64>,
    category_id: Option<i32>,
    kind: &str,
    starts_at: &chrono::DateTime<chrono::Utc>,
    ends_at: &chrono::DateTime<chrono::Utc>,
    decided_at: &chrono::DateTime<chrono::Utc>,
    operator: i64,
) {
    let snap = snapshot_hash(kind, starts_at, ends_at, decided_at, operator);
    let r = sqlx::query(
        "INSERT INTO promo_intents \
         (scope, torrent_id, category_id, kind, starts_at, ends_at, \
          status, requested_by, decided_by, decided_at, snapshot_hash) \
         VALUES ($1, $2, $3, $4, $5, $6, 'approved', $7, $7, $8, $9)",
    )
    .bind(scope)
    .bind(torrent_id)
    .bind(category_id)
    .bind(kind)
    .bind(starts_at)
    .bind(ends_at)
    .bind(operator)
    .bind(decided_at)
    .bind(&snap)
    .execute(&mut *tx)
    .await;
    if let Err(e) = r {
        tracing::warn!(error = %e, "promo direct-write intent log failed");
    }
}

#[derive(serde::Deserialize)]
pub(super) struct PromoIntentBody {
    /// promo_kinds.kind（如 free/x2/x2free/half/x2half/p30 或站方自定义）
    pub(super) kind: String,
    pub(super) hours: i32,
    #[serde(default)]
    pub(super) torrent_id: Option<i64>,
    #[serde(default)]
    pub(super) scope: Option<String>,
    #[serde(default)]
    pub(super) category_id: Option<i32>,
    #[serde(default)]
    pub(super) starts_at: Option<String>,
    #[serde(default)]
    pub(super) ends_at: Option<String>,
}
