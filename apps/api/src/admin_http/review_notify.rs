//! 审核结果通知（0285 从 `review.rs` 拆出——该文件顶在 300 行门禁上）。
//!
//! 拆出时一并修掉的 P0（本机实测）：原取数 SQL 写了 `t.title` / `d.label`
//! 两个**不存在的列**（真身是 `torrents.name` / `torrent_deny_reasons.reason`），
//! 语句在 prepare 期就失败，却被 `.ok().flatten()` 吞成 `None`
//! ⇒ 过审/被拒的站内信 + 邮件双通道 100% 静默失效（messages 条数审核前后不变）。
//! 现在列名对齐，失败也不再静默：一律 WARN 出声，通知仍不打断审核动作本身
//! （通过/驳回已落库，因通知失败而 500 会让版主重复操作）。

use actix_web::web;
use std::sync::Arc;

use crate::state::AppState;

/// 向发布者发审核结果（PM + 尽力邮件）。被拒时优先用拒绝字典条目，其次自由文本。
pub(crate) async fn notify_review_result(
    state: &web::Data<Arc<AppState>>,
    torrent_id: i64,
    approve: bool,
    free_reason: &str,
) {
    let db = &state.repo.db;
    let row =
        sqlx::query_as::<_, (i64, String, Option<String>, Option<String>)>(
            "SELECT t.owner_id, t.name, d.reason, t.deny_note \
         FROM torrents t \
         LEFT JOIN torrent_deny_reasons d ON d.id = t.deny_reason_id \
         WHERE t.id = $1",
        )
        .bind(torrent_id)
        .fetch_optional(db)
        .await;
    let Ok(Some((owner, title, deny_label, deny_note))) = row else {
        if let Err(e) = row {
            tracing::warn!(?e, torrent = torrent_id, "审核通知取数失败");
        }
        return;
    };
    let email = sqlx::query_scalar::<_, String>(
        "SELECT email FROM users WHERE id = $1 AND email <> ''",
    )
    .bind(owner)
    .fetch_optional(db)
    .await
    .ok()
    .flatten();
    // 0286：文案键 + 参数一并落库（P1-9）——切语言后这条通知仍能被重新渲染
    // 拒因先算一次：正文与 params 共用，不两边各拼一遍（两处口径会漂）
    let reason_text: Option<String> = if approve {
        None
    } else {
        Some(
            deny_label
                .clone()
                .or_else(|| deny_note.clone())
                .unwrap_or_else(|| {
                    let r = free_reason.trim();
                    if r.is_empty() {
                        "未注明".to_string()
                    } else {
                        r.to_string()
                    }
                }),
        )
    };
    let (kind, subject, body_text) = if approve {
        (
            "review_approved",
            format!("种子过审：{title}"),
            format!("你发布的种子已通过审核：#{torrent_id} {title}。"),
        )
    } else {
        let why = reason_text.clone().unwrap_or_else(|| "未注明".into());
        (
            "review_rejected",
            format!("种子被拒：{title}"),
            format!(
                "你发布的种子未通过审核：#{torrent_id} {title}\n\
                 原因：{why}\n可修改后重新发布。"
            ),
        )
    };
    crate::mailer::notify_keyed(
        db,
        owner,
        email,
        &subject,
        &body_text,
        kind,
        serde_json::json!({
            "id": torrent_id,
            "name": title,
            "reason": reason_text,
        }),
    )
    .await;
}
