//! 群发邮件。
//! 从 staff_http.rs 按域拆出。

use actix_web::{get, post, web, HttpRequest, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

// ============ staffpanel 管理工具（同类站 faqmanage/modrules/catmanage/bans/massmail 口径） ============

#[get("/admin/massmail")]
pub async fn massmail_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::MASSMAIL)
        .await?;
    let rows: Vec<MassMailRow> = sqlx::query_as(
        "SELECT m.id, m.subject, m.recipients, m.created_at, \
                 u.username AS sender, m.audience \
         FROM mass_mails m LEFT JOIN users u ON u.id = m.sent_by ORDER BY m.id DESC LIMIT 50",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[post("/admin/massmail")]
pub async fn massmail_send(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<MassMailBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::MASSMAIL)
        .await?;
    if body.subject.trim().is_empty() || body.body.trim().is_empty() {
        return Err(DomainError::Validation("主题和正文不能为空".into()));
    }
    // 0497 圈选：class:N 特殊解析（N 白名单 1-12），其余走枚举片段
    let aud = body.audience.as_deref().unwrap_or("all");
    let (pred, aud_note): (String, String) =
        if let Some(n) = aud.strip_prefix("class:") {
            let n: i32 = n.parse().map_err(|_| {
                DomainError::Validation("audience class:N 的 N 须为数字".into())
            })?;
            if !(1..=12).contains(&n) {
                return Err(DomainError::Validation(
                    "audience class:N 的 N 须在 1-12".into(),
                ));
            }
            (format!(" AND u.class_id >= {n}"), aud.to_string())
        } else {
            (audience_pred(Some(aud)), aud.to_string())
        };
    let users: i64 = {
        // 0499：预统计套圈选——recipients 反映目标人群而非全员
        let sql = format!(
            "SELECT count(*) FROM users u WHERE u.status < 2 \
             AND u.email IS NOT NULL{pred}"
        );
        sqlx::query_scalar(&sql)
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or(0)
    };
    let id: i32 = sqlx::query_scalar(
        "INSERT INTO mass_mails \
             (subject, body, sent_by, recipients, audience) \
         VALUES ($1,$2,$3,$4,$5) RETURNING id",
    )
    .bind(body.subject.trim())
    .bind(&body.body)
    .bind(auth.id)
    .bind(users as i32)
    .bind(&aud_note)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "massmail_send", None).await;
    // 0208 P0：SMTP「站点设定优先，env 兜底」（与 mailer::smtp_config 同源）。
    // 0501：投递改 mail_outbox 入队（worker 消费，自带幂等/退避/频控），
    // 「当场逐户直发」形态退役——那条 P1 修复在反代 60s 超时面前不成立。
    let mut delivered: i64 = 0;
    let cfg = crate::mailer::smtp_config(&state.repo.db).await;
    let note = match cfg {
        None => "SMTP 未配置（站点设定与 SMTP_URL 均为空，邮件未入队，仅留档）"
            .into(),
        Some(cfg) => {
            // 0501 投递本体修复（体检四轮 A1）：同步逐户直发改为
            // mail_outbox 入队——单请求内几千封必超反代 60s；失败即
            // break 无续发；重试重发无幂等。outbox 的 worker 消费自带
            // 幂等（dedupe_key）/退避/死信告警/单用户频控全套，且
            // enqueue 失败自带「退化为尽力直发」兜底。SMTP 构建成功与
            // 否不再决定本请求成败（build 在 worker 侧执行）。
            let _ = cfg; // url 仅用于探测可达性提示，不强求
            let scope =
                crate::mailer::send::mail_ctx(&state.repo.db, Some(auth.id))
                    .await;
            // 圈选 + 0501 补两闸：
            //  · email_bans 过滤（与注册/邀请同口径——此前群发/收件
            //    两处都不查，被封邮箱仍收群发信）
            //  · 仍要求有邮箱
            let rows: Vec<(i64, String, Option<String>)> = {
                let sql = format!(
                    "SELECT u.id, u.email, u.site_language \
                     FROM users u WHERE u.status < 2 \
                     AND u.email IS NOT NULL \
                     AND NOT EXISTS (SELECT 1 FROM email_bans e \
                         WHERE e.mode <> 'allow' \
                           AND (lower(u.email) = lower(e.pattern) \
                                OR (e.pattern LIKE '@%' \
                                    AND lower(u.email) \
                                        LIKE '%' || lower(e.pattern)) \
                                OR (e.pattern LIKE '%@' \
                                    AND lower(u.email) \
                                        LIKE lower(e.pattern) || '%'))) \
                     {pred} ORDER BY u.id",
                );
                sqlx::query_as(&sql)
                    .fetch_all(&state.repo.db)
                    .await
                    .map_err(|e| DomainError::Internal(e.into()))?
            };
            let total = rows.len();
            for (uid, to, lang) in &rows {
                let ctx = flux_shared::mailtpl::Ctx {
                    site: &scope.site,
                    base: &scope.base,
                    lang: flux_shared::mailtpl::Lang::from_db(lang.as_deref()),
                };
                let mail = flux_shared::mailtpl::generic_notice(
                    &ctx,
                    body.subject.trim(),
                    &body.body,
                    "站 内 通 知",
                    flux_shared::mailtpl::Tone::Gold,
                );
                // dedupe 锚 mass:{批次}:{uid}——管理员重试同主题会生成
                // 新批次 id（新行），跨批次不吞；同批次重放安全。
                let key = format!("mass:{}:{}", id, uid);
                crate::mailer::enqueue_mail_outbox(
                    &state.repo.db,
                    *uid,
                    to,
                    &mail,
                    &key,
                )
                .await;
            }
            delivered = total as i64;
            format!("已入队 {total} 封，由邮件通道按频控投递（含退避重试）")
        }
    };
    if delivered > 0 {
        let _ =
            sqlx::query("UPDATE mass_mails SET recipients = $2 WHERE id = $1")
                .bind(id)
                .bind(delivered as i32)
                .execute(&state.repo.db)
                .await;
    }
    Ok(ok(serde_json::json!({
        "id": id, "delivered": delivered,
        "queued": 0,
        "audience": aud_note,
        "note": note,
    })))
}

#[derive(Deserialize)]
pub(super) struct MassMailBody {
    pub(super) subject: String,
    pub(super) body: String,
    /// 0497 圈选人群（缺省 all 保持旧行为）：
    /// all=全员 | active30=30天内活跃 | dormant90=90天未活跃（召回）
    /// | vip=捐赠/VIP | class:N=指定等级及以上（N 1-12）
    #[serde(default)]
    pub(super) audience: Option<String>,
}

/// audience → SQL WHERE 片段（只拼白名单枚举，无自由文本进 SQL）。
fn audience_pred(audience: Option<&str>) -> String {
    match audience.unwrap_or("all") {
        "active30" => " AND COALESCE(u.last_seen_at, u.created_at) \
             > now() - interval '30 days'"
            .into(),
        "dormant90" => " AND COALESCE(u.last_seen_at, u.created_at) \
             <= now() - interval '90 days'"
            .into(),
        "vip" => " AND (u.donor OR u.vip_until > now() \
                 OR u.donor_until > now())"
            .into(),
        _ => String::new(),
    }
}

#[derive(serde::Serialize, sqlx::FromRow)]
pub(super) struct MassMailRow {
    pub(super) id: i32,
    pub(super) subject: String,
    pub(super) recipients: i32,
    pub(super) created_at: chrono::DateTime<chrono::Utc>,
    #[sqlx(default)]
    pub(super) sender: Option<String>,
    /// 0499 圈选留痕（dev.22 前旧行为 NULL）
    #[sqlx(default)]
    pub(super) audience: Option<String>,
}
