//! 匿名使用统计上报（0276，opt-in 默认关）。
//!
//! 背景：项目零遥测、GHCR 无拉取计数，发布者无法知道外部部署规模。
//! 站长在后台设定（grp=ops）开启 stats_report_enabled 后，本任务每天
//! 最多上报一次：匿名站点哈希 + 版本 + 用户/种子/活跃 peer 三项计数。
//!
//! 隐私口径（刻意收窄，宁可少收不可多收）：
//!   * 站点标识 = sha256(JWT_SECRET + site_name) 前 16 hex——不含 JWT_SECRET
//!     本体，不可逆推站名/域名；作用只是让收集端能区分「同一站每天心跳」
//!     与「新站」。收集端换了 JWT_SECRET 会被当成新站（计数略偏高，可接受）。
//!   * 不上报：域名、IP、站名、邮箱、任何用户数据。
//!   * 开关默认关；stats_report_url 置空也静默跳过。失败只落日志，永不重试堆积。

use sqlx::PgPool;

pub async fn usage_stats_report(db: &PgPool) -> anyhow::Result<()> {
    let kv: Vec<(String, String)> = sqlx::query_as(
        "SELECT name, value FROM site_settings WHERE name IN \
         ('stats_report_enabled', 'stats_report_url', 'site_name')",
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();
    let kv: std::collections::HashMap<_, _> = kv.into_iter().collect();
    if kv.get("stats_report_enabled").map(String::as_str) != Some("1") {
        return Ok(()); // opt-in 未开：正常路径，不算错误也不记日志
    }
    let Some(url) = kv
        .get("stats_report_url")
        .filter(|u| !u.trim().is_empty())
        .cloned()
    else {
        tracing::info!("usage_stats：stats_report_url 为空，跳过上报");
        return Ok(());
    };

    // 站点哈希：JWT_SECRET 做盐（不离开本机），site_name 参与混合让同机多站可分
    let jwt = std::env::var("JWT_SECRET").unwrap_or_default();
    let site = kv.get("site_name").cloned().unwrap_or_default();
    let site_id = {
        use sha2::Digest;
        let mut hasher = sha2::Sha256::new();
        hasher.update(b"fluxtorrent-stats-v1:");
        hasher.update(jwt.as_bytes());
        hasher.update(site.as_bytes());
        hex(&hasher.finalize())[..16].to_string()
    };

    let (users, torrents, peers): (i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM users), (SELECT count(*) FROM torrents), \
                (SELECT count(*) FROM snatches WHERE seeding OR leeching)",
    )
    .fetch_one(db)
    .await?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;
    let resp = client
        .post(&url)
        .json(&serde_json::json!({
            "site_id": site_id,
            "version": env!("CARGO_PKG_VERSION"),
            "users": users,
            "torrents": torrents,
            "active_peers": peers,
        }))
        .send()
        .await;
    match resp {
        Ok(r) if r.status().is_success() => {
            tracing::info!(users, torrents, "usage_stats：已上报匿名统计");
        }
        Ok(r) => {
            tracing::warn!(status = %r.status(), "usage_stats：收集端返回非 2xx");
        }
        Err(e) => {
            tracing::warn!(?e, "usage_stats：上报失败（下次 24h 后重试）");
        }
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn site_id_is_stable_and_salted() {
        use sha2::Digest;
        let mk = |salt: &str, name: &str| {
            let mut h = sha2::Sha256::new();
            h.update(b"fluxtorrent-stats-v1:");
            h.update(salt.as_bytes());
            h.update(name.as_bytes());
            hex(&h.finalize())[..16].to_string()
        };
        // 同输入同输出
        assert_eq!(mk("secret", "MySite"), mk("secret", "MySite"));
        // 换盐（换 JWT_SECRET）→ 换身份
        assert_ne!(mk("secret", "MySite"), mk("other-secret", "MySite"));
    }
}
