//! Worker 侧邮件通道（0283 P1-8）：H&R 预警等批事件的邮件提醒。
//!
//! SMTP 配置口径与 api 侧 mailer::smtp_config 完全一致（site_settings 的
//! smtp_host/port/from + accountname/accountpassword/encryption 装配 URL），
//! 但 worker 无法引用 api crate（无 lib target），此处按同构 SQL 复读——
//! 两处装配逻辑如有变更须同步（与 push 的 outbox 解耦不同，邮件直接发）。

use sqlx::PgPool;

/// 装配 SMTP 发信参数；未配置返回 None（调用方降级跳过）。
async fn smtp_config(db: &PgPool) -> Option<(String, String)> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT name, value FROM site_settings \
         WHERE name IN ('smtp_host','smtp_port','smtp_from', \
         'accountname','accountpassword','encryption')",
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();
    let get = |k: &str| {
        rows.iter()
            .find(|(n, _)| n == k)
            .map(|(_, v)| v.trim().to_string())
            .filter(|v| !v.is_empty())
    };
    let (Some(host), Some(from)) = (get("smtp_host"), get("smtp_from")) else {
        return None;
    };
    let encryption = get("encryption").unwrap_or_else(|| "ssl".into());
    let default_port = if encryption == "tls" { "587" } else { "465" };
    let port = get("smtp_port").unwrap_or_else(|| default_port.to_string());
    let scheme = match encryption.as_str() {
        "tls" | "none" => "smtp",
        _ => "smtps",
    };
    let auth = match (get("accountname"), get("accountpassword")) {
        (Some(u), Some(p)) => {
            let enc = |s: &str| {
                s.replace('@', "%40")
                    .replace(':', "%3A")
                    .replace('/', "%2F")
            };
            format!("{}:{}@", enc(&u), enc(&p))
        }
        _ => String::new(),
    };
    Some((format!("{scheme}://{auth}{host}:{port}"), from))
}

/// 给单用户发邮件（尽力而为）：email 为 None / SMTP 未配置 / 投递失败都只记日志。
pub(crate) async fn send_user_mail(
    db: &PgPool,
    email: Option<&str>,
    subject: &str,
    body: &str,
) {
    let Some(to) = email else { return };
    let Some((url, from)) = smtp_config(db).await else {
        return;
    };
    if let Err(e) = send_via_smtp(&url, &from, to, subject, body).await {
        tracing::warn!(?e, to, subject, "worker 邮件发送失败");
    }
}

async fn send_via_smtp(
    smtp_url: &str,
    from: &str,
    to: &str,
    subject: &str,
    body: &str,
) -> anyhow::Result<()> {
    use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

    let url = url::Url::parse(smtp_url)?;
    let host = url
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("SMTP_URL 缺少主机"))?
        .to_string();
    let port =
        url.port()
            .unwrap_or(if url.scheme() == "smtps" { 465 } else { 25 });
    let builder = if url.scheme() == "smtps" {
        AsyncSmtpTransport::<Tokio1Executor>::relay(&host)?
    } else {
        AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&host)
    }
    .port(port);
    let builder = if !url.username().is_empty() {
        builder.credentials(
            lettre::transport::smtp::authentication::Credentials::new(
                url.username().to_string(),
                url.password().unwrap_or_default().to_string(),
            ),
        )
    } else {
        builder
    };
    let mailer = builder.build();
    let email = Message::builder()
        .from(from.parse()?)
        .to(to.parse()?)
        .subject(subject.to_string())
        .body(body.to_string())?;
    mailer.send(email).await?;
    Ok(())
}

/// mail_outbox 消费轮（0404，§17-2）：捞未发送行退避重试。
/// 幂等：sent_at 置位即完成；失败 attempts+1，attempts≥5 停试并在
/// staffmessages 落死信告警（一次）。由 run.rs 挂 60s 档。
pub(crate) async fn mail_outbox_drain(db: &PgPool) -> anyhow::Result<u64> {
    // 认领一批（FOR UPDATE SKIP LOCKED 防双 worker 重发；行内 attempts
    // 即重试计数，退避 = 60s * 2^attempts，由 WHERE 谓词表达）
    let rows: Vec<(i64, String, String, String)> = sqlx::query_as(
        "SELECT id, to_email, subject, body FROM mail_outbox \
         WHERE sent_at IS NULL AND attempts < 5 \
           AND (last_attempt_at IS NULL OR last_attempt_at < \
                now() - make_interval(secs => 60 * power(2, attempts)::int8)) \
         ORDER BY id LIMIT 20 FOR UPDATE SKIP LOCKED",
    )
    .bind(0)
    .fetch_all(db)
    .await?;
    if rows.is_empty() {
        return Ok(0);
    }
    let mut sent = 0u64;
    for (id, to, subject, body) in rows {
        let Some((url, from)) = smtp_config(db).await else {
            // SMTP 未配置：不算失败也不死信（配置问题，配置好了下一轮自然发）
            tracing::info!("mail_outbox: SMTP 未配置，待命");
            return Ok(sent);
        };
        let r = send_via_smtp(&url, &from, &to, &subject, &body).await;
        match r {
            Ok(()) => {
                let _ = sqlx::query(
                    "UPDATE mail_outbox SET sent_at = now(), attempts = \
                     attempts + 1 WHERE id = $1",
                )
                .bind(id)
                .execute(db)
                .await;
                sent += 1;
            }
            Err(e) => {
                let attempts: i32 = sqlx::query_scalar(
                    "UPDATE mail_outbox SET attempts = attempts + 1, \
                     last_error = $2, last_attempt_at = now() \
                     WHERE id = $1 RETURNING attempts",
                )
                .bind(id)
                .bind(e.to_string())
                .fetch_one(db)
                .await
                .unwrap_or(0);
                if attempts >= 5 {
                    tracing::warn!(id, to, "mail_outbox: 达重试上限，落死信告警");
                    // P-428：这里原先自己写
                    // `INSERT … SELECT MIN(id) … WHERE class_id >= 99`——
                    // 把聚合当 EXISTS 用：命中 0 行时 `MIN(id)` 仍返回**一行
                    // NULL**，于是 INSERT 真执行并撞 staffmessages.user_id 的
                    // NOT NULL；外面又套 `let _ =` 把数据库错误吞成静默
                    // ⇒ 邮件死信这种最该出声的告警，既发不出去也没人看得见。
                    // 收进唯一出口 staff_alert（无在册职员可署名时 warn + 返回 0）。
                    super::staff_alert::staff_alert(
                        db,
                        "邮件死信告警",
                        &format!(
                            "outbox #{} 发送 5 次失败：{}（收件已脱敏）",
                            id, e
                        ),
                        "staff_panel",
                    )
                    .await;
                }
            }
        }
    }
    Ok(sent)
}
