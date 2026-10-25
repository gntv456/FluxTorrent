//! 求字幕撤销与退款（P-43，本批补）。
//!
//! 0146 建表时词表就写了 `status=2 已撤销`，但全仓没有实现它的端点：
//! 池子只有「交付/验收」两条出账路，没人结的单（发起人反悔、删号、被遗忘）
//! 冻结的火花永久悬在 subtitle_requests 里 —— 与已修的 P-26「求种悬赏
//! 蒸发」同型。这里补上唯一缺失的出口：未被认领时可撤销，按 contributors
//! 明细逐人退款。已认领（3）/已交稿（4）不走这条路：译者已投入劳动，
//! 撤销会把承诺的赏金抽走，那两种状态只由交付/验收/超时回池收尾。

use actix_web::{post, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::economy_http::{earn_spark_tx, SpendOutcome};
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 撤销（仅 status=0）+ 按入池明细退款（幂等键 sub-req-refund:{rid}:{uid}，
/// 同单同人多笔合并成一笔退，重放不双退）。
#[post("/subtitles/requests/{id}/cancel")]
pub(super) async fn subtitle_request_cancel(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let rid = path.into_inner();
    let staff = auth.class_id >= 90;
    // P-406：CAS 与退款必须同生共死。旧写法 `UPDATE ... SET status=2` 先
    // autocommit 落库，再在事务外逐个退款——任一 `earn_spark` 失败（`.await?`
    // 直接上抛）就退到一半停住：单子已经是 status=2，而这条 UPDATE 只认
    // status=0 ⇒ **撤销无法重试、没退到的人永远拿不回火花**。同型口径见
    // P-364（钱到账才落标记）与 `bank_withdraw`（销单与入账同事务）。
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let row: Option<(i64, serde_json::Value)> = sqlx::query_as(
        "UPDATE subtitle_requests SET status = 2 WHERE id = $1 AND status = 0 \
         AND ($2 OR user_id = $3) RETURNING user_id, contributors",
    )
    .bind(rid)
    .bind(staff)
    .bind(auth.id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((owner, contributors)) = row else {
        return Err(DomainError::Validation(
            "该求字幕不存在、已认领或已处理（仅未被认领的单可撤销）".into(),
        ));
    };
    let mut totals: std::collections::HashMap<i64, i64> =
        std::collections::HashMap::new();
    if let Some(list) = contributors.as_array() {
        for m in list {
            let (Some(uid), Some(amount)) = (
                m.get("user_id").and_then(|v| v.as_i64()),
                m.get("amount").and_then(|v| v.as_i64()),
            ) else {
                continue;
            };
            if amount > 0 {
                *totals.entry(uid).or_insert(0) += amount;
            }
        }
    }
    let mut refunded = Vec::new();
    let mut already = Vec::new();
    for (uid, amount) in &totals {
        let idem = format!("sub-req-refund:{}:{}", rid, uid);
        // 有意容忍的重放：历史上「退款已落账但状态位被回滚」的单，这次只补销单。
        // P-406b：但**不许把这种情形写进 `refunded`**——那一笔钱这次并没有动账，
        // 照 totals 列出来就是告诉会员「已退 20」而账面什么都没有。重放单另列
        // `already_settled` 并出声，界面与对账读的是同一件事。
        let outcome = earn_spark_tx(
            &mut tx,
            *uid,
            *amount,
            "subtitle_request_refund",
            &idem,
        )
        .await?;
        let item = serde_json::json!({"user_id": uid, "amount": amount});
        if matches!(outcome, SpendOutcome::Replayed) {
            tracing::warn!(
                rid = rid,
                user = uid,
                refund_key = idem.as_str(),
                "subtitle cancel refund replayed: settling the row only"
            );
            already.push(item);
        } else {
            refunded.push(item);
        }
    }
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "subreq.cancel", Some(rid))
        .await;
    Ok(ok(serde_json::json!({
        "id": rid, "status": 2, "owner": owner, "refunded": refunded,
        "already_settled": already,
    })))
}
