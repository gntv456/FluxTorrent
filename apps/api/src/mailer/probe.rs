//! SMTP 配置装配 + 可达性探测（从 mailer.rs 按域拆出，300 行门禁）：
//! 配置（smtp_config）与探测（smtp_reachable）同居一文件——装配产物
//! URL 正是探测的输入，拆开反而要在两文件间传「URL 形状」的隐式契约。

/// 4 秒超时防黑洞（DNS 失败/端口被封时不让请求线程一直等）。
use sqlx::PgPool;

use super::SmtpConfig;

pub(crate) async fn smtp_reachable(url: &str) -> bool {
    let Ok(transport) = crate::gaps_http::build_smtp(url) else {
        tracing::warn!("SMTP 探测跳过：配置无法装配");
        return false;
    };
    let probe = tokio::time::timeout(
        std::time::Duration::from_secs(4),
        transport.test_connection(),
    )
    .await;
    match probe {
        Ok(Ok(true)) => true,
        Ok(Ok(false)) => {
            tracing::warn!("SMTP 探测未通过（NOOP 失败）");
            false
        }
        Ok(Err(e)) => {
            tracing::warn!(error = %e, "SMTP 探测失败");
            false
        }
        Err(_) => {
            tracing::warn!("SMTP 探测超时");
            false
        }
    }
}

/// 读取 SMTP 配置：site_settings.smtp_host/smtp_port/smtp_from 优先；
/// 未配置时回退环境变量 SMTP_URL/SMTP_FROM（compose 注入口，兼容既有部署）。
/// 凭据/加密（0211）：accountname/accountpassword/encryption 三键接入装配——
/// encryption=ssl（缺省，465）/tls（587 STARTTLS，拼 smtp:// 由 lettre 升级）/none
///（内网中继 smtp:// 明文）；账密嵌入 URL userinfo，与 SMTP_URL 形状一致。
pub async fn smtp_config(db: &PgPool) -> Option<SmtpConfig> {
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
    // 后台三键齐（host + from）才算「后台已配置」；端口缺省 465（ssl 假定）
    if let (Some(host), Some(from)) = (get("smtp_host"), get("smtp_from")) {
        let encryption = get("encryption").unwrap_or_else(|| "ssl".into());
        let default_port = match encryption.as_str() {
            "tls" => "587",
            // A 方案（2026-11-18）：中继常用 2525（机房只封 25/465/587 时
            // 唯一活路）；明文中继方言走 smtp-plain（build_smtp 明文分支）
            "plain" => "2525",
            _ => "465",
        };
        let port = get("smtp_port").unwrap_or_else(|| default_port.to_string());
        let scheme = match encryption.as_str() {
            // tls=STARTTLS（587 或中继 2525 均要求）；plain=明文中继
            "tls" => "smtp",
            "none" | "plain" => "smtp-plain",
            _ => "smtps",
        };
        let (user, pass) = (get("accountname"), get("accountpassword"));
        let auth = match (user, pass) {
            // userinfo 百分号编码（@ : / 三个保留字即可覆盖绝大多数账密）
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
        let url = format!("{scheme}://{auth}{host}:{port}");
        return Some(SmtpConfig { url, from });
    }
    // 兜底：环境变量（smtp:// 形状自带账号密码；后台无凭据时保持 env 优先的凭据口）
    let url = std::env::var("SMTP_URL").unwrap_or_default();
    let from = std::env::var("SMTP_FROM").unwrap_or_default();
    if url.is_empty() || from.is_empty() {
        None
    } else {
        Some(SmtpConfig { url, from })
    }
}
