//! SMTP 可达性探测（从 mailer.rs 按域拆出，300 行门禁）：
//! lettre 的 test_connection 会建立连接、完成 TLS/认证并发 NOOP——
//! 比裸 TCP 探测更接近「这封信发得出去」。

/// 4 秒超时防黑洞（DNS 失败/端口被封时不让请求线程一直等）。
pub(super) async fn smtp_reachable(url: &str) -> bool {
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
