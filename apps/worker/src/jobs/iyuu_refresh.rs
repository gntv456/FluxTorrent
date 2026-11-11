//! IYUU 辅种查重：定时批量刷新（0487）。
//!
//! 为什么不在详情页实时查：IYUU 查重是**配额制**的（实测 25 次/15 秒不触发
//! 频率限制，卡的是总量），而种子详情页是全站最热的页面。实时查有两宗罪：
//! ① 站点一热就超额；② 页面要等一次出站（最长 5 秒）。
//!
//! 所以走「定时批量 + 库缓存」，IYUU 生态里各站都是这个路子：
//! * 本 job 每小时跑一次，挑一批「最久没刷过」的已审核种子逐条查，写
//!   `iyuu_crossseed` 表；
//! * 详情页端点只读表，零出站。
//!
//! 节流四道闸：
//! 1. 每次最多 `iyuu_bridge_batch` 条（缺省 30，站长可调）；
//! 2. 条与条之间 sleep 300ms（≈3 次/秒，远低于实测不触发的阈值）；
//! 3. 连续 5 条失败（网络/配额/凭据失效）立刻停手，等下一轮——不烧配额；
//! 4. 站点表只在 Redis 缺失/过期时拉一次，不跟着每条查重走。

use anyhow::Result;
use redis::AsyncCommands;
use sqlx::PgPool;

/// 站点表在 Redis 的键（api 侧读同一份）。
const SITES_KEY: &str = "iyuu:site_table";
/// 站点表缓存时长（秒）。
const SITES_TTL: u64 = 86_400;
/// 单条之间的间隔：3 次/秒，留足余量。
const PACE_MS: u64 = 300;
/// 连续失败多少条就停手。
const MAX_CONSEC_FAIL: u32 = 5;
/// 缺省每轮条数（站长可用 `iyuu_bridge_batch` 覆盖）。
const DEFAULT_BATCH: i64 = 30;

/// 一轮刷新。返回本轮实际写入的行数。
pub(crate) async fn iyuu_refresh(
    db: &PgPool,
    redis: redis::aio::ConnectionManager,
) -> Result<u64> {
    let Some(cfg) = load_conf(db).await? else {
        return Ok(0);
    };
    let mut r = redis.clone();
    refresh_site_table(&cfg, &mut r).await;

    let rows: Vec<(i64, String)> = sqlx::query_as(
        "SELECT t.id, t.info_hash FROM torrents t \
         LEFT JOIN iyuu_crossseed c ON c.torrent_id = t.id \
         WHERE t.approval_status = 1 \
           AND COALESCE(t.info_hash, '') <> '' \
         ORDER BY c.fetched_at NULLS FIRST \
         LIMIT $1",
    )
    .bind(cfg.batch)
    .fetch_all(db)
    .await?;

    let mut written = 0u64;
    let mut consec_fail = 0u32;
    for (tid, info_hash) in rows {
        match flux_shared::iyuu::search(&cfg.cred, &info_hash).await {
            Some(hits) => {
                consec_fail = 0;
                flux_shared::iyuu::upsert_hits(db, tid, &hits).await?;
                written += 1;
            }
            None => {
                consec_fail += 1;
                flux_shared::iyuu::upsert_failed(db, tid).await?;
                tracing::warn!(
                    torrent_id = tid,
                    "IYUU 查重失败（网络/配额/凭据）"
                );
                if consec_fail >= MAX_CONSEC_FAIL {
                    tracing::warn!("IYUU 连续失败，本轮提前结束");
                    break;
                }
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(PACE_MS)).await;
    }
    if written > 0 {
        tracing::info!(n = written, "iyuu crossseed refreshed");
    }
    Ok(written)
}

/// 从站点设置读凭据与批量。
///
/// 未启用 / 凭据不全时返回 None —— job 直接跳过，不发任何请求。
async fn load_conf(db: &PgPool) -> Result<Option<Conf>> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT name, value FROM site_settings WHERE name = ANY($1)",
    )
    .bind(&[
        "iyuu_bridge_enabled".to_string(),
        "iyuu_bridge_appid".to_string(),
        "iyuu_bridge_secret".to_string(),
        "iyuu_bridge_batch".to_string(),
    ])
    .fetch_all(db)
    .await?;
    let mut enabled = false;
    let mut appid = String::new();
    let mut secret = String::new();
    let mut batch = DEFAULT_BATCH;
    for (name, value) in rows {
        let v = value.trim().to_string();
        match name.as_str() {
            "iyuu_bridge_enabled" => {
                enabled = v.eq_ignore_ascii_case("yes") || v == "1"
            }
            "iyuu_bridge_appid" => appid = v,
            "iyuu_bridge_secret" => secret = v,
            // 站长可能填 0（暂停但保留配置）或很小的数
            "iyuu_bridge_batch" => batch = v.parse().unwrap_or(DEFAULT_BATCH),
            _ => {}
        }
    }
    let cred = flux_shared::iyuu::Cred::new(appid, secret);
    if !enabled || !cred.ready() || batch <= 0 {
        return Ok(None);
    }
    Ok(Some(Conf {
        cred,
        batch: batch.clamp(1, 500),
    }))
}

struct Conf {
    cred: flux_shared::iyuu::Cred,
    batch: i64,
}

/// 站点表：Redis 里没有（或过期）才拉一次。
async fn refresh_site_table(
    cfg: &Conf,
    redis: &mut redis::aio::ConnectionManager,
) {
    let cached: Option<String> = redis.get(SITES_KEY).await.ok().flatten();
    if cached.is_some() {
        return;
    }
    let Some(map) = flux_shared::iyuu::sites(&cfg.cred).await else {
        tracing::warn!("IYUU 站点表同步失败，沿用内置表");
        return;
    };
    if let Ok(raw) = serde_json::to_string(&map) {
        let _: Result<(), _> = redis.set_ex(SITES_KEY, raw, SITES_TTL).await;
    }
}
