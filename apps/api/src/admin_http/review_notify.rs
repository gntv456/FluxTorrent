//! 审核结果通知（0285 从 `review.rs` 拆出——该文件顶在 300 行门禁上）。
//!
//! 拆出时一并修掉的 P0（本机实测）：原取数 SQL 写了 `t.title` / `d.label`
//! 两个**不存在的列**（真身是 `torrents.name` / `torrent_deny_reasons.reason`），
//! 语句在 prepare 期就失败，却被 `.ok().flatten()` 吞成 `None`
//! ⇒ 过审/被拒的站内信 + 邮件双通道 100% 静默失效（messages 条数审核前后不变）。
//! 现在列名对齐，失败也不再静默：一律 WARN 出声，通知仍不打断审核动作本身
//! （通过/驳回已落库，因通知失败而 500 会让版主重复操作）。

use crate::state::AppState;

/// 向发布者发审核结果（PM + 尽力邮件）。被拒时优先用拒绝字典条目，其次自由文本。
/// F2（FRAME §4）：服务层 services/review_decide.rs 复用本函数——签名从
/// `web::Data<Arc<AppState>>` 放宽为 `&AppState`（同一对象的两种视图，
/// SQL 原样，行为零变）。
pub(crate) async fn notify_review_result(
    state: &AppState,
    torrent_id: i64,
    approve: bool,
    free_reason: &str,
) {
    let db = &state.repo.db;
    let row = sqlx::query_as::<
        _,
        (Option<i64>, String, Option<String>, Option<String>),
    >(
        "SELECT t.owner_id, t.name, d.reason, t.deny_note \
         FROM torrents t \
         LEFT JOIN torrent_deny_reasons d ON d.id = t.deny_reason_id \
         WHERE t.id = $1",
    )
    .bind(torrent_id)
    .fetch_optional(db)
    .await;
    // owner_id 可空（站方代发 / 系统导入的种子没有上传者）：以前按 i64 解码，
    // 这类种子一过审就走「取数失败」分支，日志里全是假警告。
    let (owner, title, deny_label, deny_note) = match row {
        Ok(Some((Some(o), t, dl, dn))) => (o, t, dl, dn),
        Ok(_) => return,
        Err(e) => {
            tracing::warn!(?e, torrent = torrent_id, "审核通知取数失败");
            return;
        }
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
    // 卡片 5（0400）：站长自定义邮件主题模板（auto_review_mail_*_subject），
    // 变量 {site}/{username}/{torrent}/{reason}；未配置回落缺省文案。
    let tmpl: Vec<(String, String)> = sqlx::query_as(
        "SELECT name, value FROM site_settings WHERE name IN \
           ('auto_review_mail_pass_subject', \
            'auto_review_mail_deny_subject', 'site_name')",
    )
    .bind(0)
    .fetch_all(db)
    .await
    .unwrap_or_default();
    let get = |k: &str| {
        tmpl.iter()
            .find(|(n, _)| n == k)
            .map(|(_, v)| v.trim().to_string())
            .filter(|v| !v.is_empty())
    };
    let site_name = get("site_name").unwrap_or_else(|| "站点".into());
    let render = |t: &str, reason: &str| {
        t.replace("{site}", &site_name)
            .replace("{username}", "")
            .replace("{torrent}", &title)
            .replace("{reason}", reason)
    };
    let (kind, subject, body_text) = if approve {
        (
            "review_approved",
            get("auto_review_mail_pass_subject")
                .map(|t| render(&t, ""))
                .unwrap_or_else(|| format!("种子过审：{title}")),
            format!("你发布的种子已通过审核：#{torrent_id} {title}。"),
        )
    } else {
        let why = reason_text.clone().unwrap_or_else(|| "未注明".into());
        (
            "review_rejected",
            get("auto_review_mail_deny_subject")
                .map(|t| render(&t, why.as_str()))
                .unwrap_or_else(|| format!("种子被拒：{title}")),
            format!(
                "你发布的种子未通过审核：#{torrent_id} {title}\n\
                 原因：{why}\n\
                 修改建议：请按原因补充或修正内容（描述可补充资源介绍/截图/MediaInfo；标题请按站规命名）。\n\
                 修改后可在「我的发布」中重新提交；如有异议可通过站内工单申诉。"
            ),
        )
    };
    // 0400 §4.2：自动拒因（字典 sort 91-96）的邮件走 1h/3 封限频——
    // 超限只发站内信；人工拒因不限频（人工逐条裁决自带节奏）。
    let email = if approve {
        email
    } else {
        let is_auto_deny: bool = sqlx::query_scalar(
            "SELECT COALESCE((SELECT d.sort BETWEEN 91 AND 96 \
               FROM torrent_deny_reasons d \
               JOIN torrents t ON t.deny_reason_id = d.id \
               WHERE t.id = $1), false)",
        )
        .bind(torrent_id)
        .fetch_one(db)
        .await
        .unwrap_or(false);
        if is_auto_deny
            && !crate::mailer::mail_rate_allow(&mut state.redis.clone(), owner)
                .await
        {
            tracing::info!(owner, torrent = torrent_id,
                "自动拒邮件超频（1h>3），本封只发站内信");
            None
        } else {
            email
        }
    };
    // 自动拒因邮件走 outbox（0404：丢了=用户不知情被拒，须重试）；
    // 过审邮件保持尽力直发（丢了无实害）。
    let is_auto_deny_mail = !approve
        && sqlx::query_scalar::<_, bool>(
            "SELECT COALESCE((SELECT d.sort BETWEEN 91 AND 96 \
               FROM torrent_deny_reasons d \
               JOIN torrents t ON t.deny_reason_id = d.id \
               WHERE t.id = $1), false)",
        )
        .bind(torrent_id)
        .fetch_one(db)
        .await
        .unwrap_or(false);
    if is_auto_deny_mail {
        if let Some(to) = email.clone() {
            crate::mailer::enqueue_mail_outbox(
                db,
                owner,
                &to,
                &subject,
                &body_text,
                &format!("auto_deny:{torrent_id}"),
            )
            .await;
        }
    }
    crate::mailer::notify_keyed(
        db,
        owner,
        if is_auto_deny_mail { None } else { email },
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
