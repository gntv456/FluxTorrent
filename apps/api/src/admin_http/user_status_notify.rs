//! 状态变更通知当事人（0209 P1-9，0482 自 user_status.rs 拆出守行数门禁）：
//! 渲染统一母版邮件 + 封禁 PM/邮件必发、其余 notify 双通道 + E12 生命周期广播。

use actix_web::web;

use crate::state::AppState;

/// 状态变更 → 当事人通知与运营广播（尽力而为：查询失败静默跳过）。
pub(super) async fn notify_status_change(
    state: &web::Data<std::sync::Arc<AppState>>,
    user_id: i64,
    status: i16,
    label: &str,
    reason_txt: &str,
) {
    let target: Option<(String, Option<String>)> =
        sqlx::query_as("SELECT username, email FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(&state.repo.db)
            .await
            .ok()
            .flatten();
    let Some((username, email)) = target else {
        return;
    };
    // 统一母版（警示档）：封禁=朱砂、禁言/警告=琥珀、恢复=石绿。
    let ctx =
        crate::mailer::send::mail_ctx(&state.repo.db, Some(user_id)).await;
    let kind = match status {
        2 => flux_shared::mailtpl::StatusKind::Ban,
        1 => flux_shared::mailtpl::StatusKind::Mute,
        _ => flux_shared::mailtpl::StatusKind::Restore,
    };
    let subject = format!("[{}] 账号状态变更：{label}", ctx.site);
    let mail = flux_shared::mailtpl::account_status(
        &ctx.ctx(),
        &username,
        kind,
        label,
        reason_txt,
        None,
        Some(&subject),
    );
    // 封禁：PM 也写（解封后可见历史）+ 邮件必发；其余：notify 双通道
    if status == 2 {
        let db = &state.repo.db;
        let _ =
            crate::mailer::site_message(db, user_id, &mail.subject, &mail.text)
                .await;
        if let Some(to) = email {
            // 0461：配置与 Reply-To 先取好，随任务带走
            let db = &state.repo.db;
            let cfg = crate::mailer::smtp_config(db).await;
            let rt = crate::mailer::send::reply_to(db).await;
            crate::mailer::send::spawn_send(cfg, to, mail.clone(), rt);
        }
    } else {
        crate::mailer::notify(&state.repo.db, user_id, email, &mail).await;
    }
    // E12 生命周期事件：状态变更广播管理侧（Discord/TG，尽力而为）
    crate::ops_webhook::broadcast_ops_spawn(
        state,
        format!("用户 {username}（#{user_id}）{label}：{reason_txt}"),
    );
}
