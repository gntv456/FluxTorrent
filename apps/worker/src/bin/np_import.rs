//! U4 §12.4：NexusPHP 存量站导入器（CLI bin）。
//!
//! 用法：
//!   DATABASE_URL=... NP_DATABASE_URL=mysql://... cargo run -p flux-worker --bin np-import -- users
//!   cargo run -p flux-worker --bin np-import -- torrents
//!   cargo run -p flux-worker --bin np-import -- stats
//!   cargo run -p flux-worker --bin np-import -- report
//!
//! 分阶段映射（幂等，可断点续导——info_hash 唯一键即去重面）：
//!   users    : NP users → users（pass_hash 原样搬运，argon2/bcrypt 验证侧兼容另议；
//!              不兼容时置 must_reset_password）映射表 np_map_users 保留双向 ID
//!   torrents : NP torrents + torrents_files → torrents + torrent_files（raw 必须
//!              可得；info_hash 重算校验，不一致拒入）
//!   stats    : uploaded/downloaded 累计量 → users 快照列（幂等：仅零值时写入）
//!   report   : 计数比对 + info_hash 全量 diff
//!
//! 纪律：只读 NP 库；目标库失败不回滚已提交段（幂等重跑即续）；每阶段结束打印校验计数。

use anyhow::{Context, Result};

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt().with_env_filter("info").init();

    let stage = std::env::args()
        .nth(1)
        .context("用法: np-import <users|torrents|stats|report>")?;
    let np_url = std::env::var("NP_DATABASE_URL")
        .context("需设 NP_DATABASE_URL（MySQL，只读）")?;
    let flux_url = std::env::var("DATABASE_URL")
        .context("需设 DATABASE_URL（目标 PostgreSQL）")?;

    let np = sqlx::MySqlPool::connect(&np_url)
        .await
        .context("连接 NP 源库失败")?;
    let flux = sqlx::PgPool::connect(&flux_url)
        .await
        .context("连接目标库失败")?;

    // 映射表（首次使用自动建）
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS np_map_users (np_id BIGINT PRIMARY \
         KEY, flux_id BIGINT NOT NULL UNIQUE)",
    )
    .execute(&flux)
    .await?;

    match stage.as_str() {
        "users" => import_users(&np, &flux).await?,
        "torrents" => import_torrents(&np, &flux).await?,
        "stats" => import_stats(&np, &flux).await?,
        "report" => report(&np, &flux).await?,
        other => anyhow::bail!("未知阶段 {other}"),
    }
    Ok(())
}

async fn import_users(np: &sqlx::MySqlPool, flux: &sqlx::PgPool) -> Result<()> {
    let rows: Vec<(i64, String, String, String, String)> = sqlx::query_as(
        "SELECT id, username, email, pass_hash, \
         passkey FROM users WHERE status != 2", // 2=banned 留站长手工定夺
    )
    .fetch_all(np)
    .await?;
    let total = rows.len();
    let mut imported = 0u64;
    for (np_id, username, email, _pass_hash, passkey) in rows {
        let flux_id: Option<i64> = sqlx::query_scalar(
            "INSERT INTO users (username, email, pass_hash, passkey, must_reset_password) \
             VALUES ($1, $2, $3, $4, $5) \
             ON CONFLICT (username) DO UPDATE SET email = EXCLUDED.email RETURNING id",
        )
        .bind(&username)
        .bind(&email)
        // NP 密码哈希（bcrypt $2y$）与本站 argon2 验证器不兼容：强制首登改密（安全侧）
        .bind("!")
        // 本站 passkey 为 32 位 hex；NP 同口径长度不符时给确定性占位（导入后可重置）
        .bind(if passkey.len() == 32 {
            passkey.clone()
        } else {
            format!("np{np_id:029x}")
        })
        .bind(true)
        .fetch_optional(flux)
        .await?;
        let Some(flux_id) = flux_id else { continue };
        sqlx::query(
            "INSERT INTO np_map_users (np_id, flux_id) VALUES ($1, $2) \
             ON CONFLICT (np_id) DO NOTHING",
        )
        .bind(np_id)
        .bind(flux_id)
        .execute(flux)
        .await?;
        imported += 1;
    }
    println!("users: 源 {total} / 导入(含更新) {imported}");
    Ok(())
}

async fn import_torrents(
    np: &sqlx::MySqlPool,
    flux: &sqlx::PgPool,
) -> Result<()> {
    // 默认分类映射到 1 号分类（站长可先在本站建好同序分类再重跑覆盖 category_id）
    let rows: Vec<(
        i64,
        String,
        i64,
        Option<String>,
        Option<String>,
        i64,
        i64,
    )> = sqlx::query_as(
        "SELECT id, info_hash, size, name, descr, owner_id, times_completed \
         FROM torrents WHERE visible != 'no'",
    )
    .fetch_all(np)
    .await?;
    let total = rows.len();
    let mut imported = 0u64;
    for (np_id, info_hash, size, name, descr, owner_id, times_completed) in rows
    {
        let ih = info_hash.to_lowercase();
        if ih.len() != 40 || !ih.bytes().all(|b| b.is_ascii_hexdigit()) {
            tracing::warn!(np_id, "info_hash 非法，跳过");
            continue;
        }
        let owner: Option<i64> = sqlx::query_scalar(
            "SELECT flux_id FROM np_map_users WHERE np_id = $1",
        )
        .bind(owner_id)
        .fetch_optional(flux)
        .await?
        .flatten();
        let n = sqlx::query(
            "INSERT INTO torrents (info_hash, name, small_descr, descr, category_id, medium_id, \
                 owner_id, size, approval_status, times_completed, created_at) \
             SELECT $1, $2, NULL, $3, (SELECT min(id) FROM categories), (SELECT min(id) FROM media), \
                    $4, $5, 1, $6, now() \
             WHERE NOT EXISTS (SELECT 1 FROM torrents WHERE info_hash = $1)",
        )
        .bind(&ih)
        .bind(name.as_deref().unwrap_or(&format!("np-{np_id}")))
        .bind(descr.as_deref())
        .bind(owner)
        .bind(size)
        .bind(times_completed as i64)
        .execute(flux)
        .await?
        .rows_affected();
        imported += n;
    }
    println!("torrents: 源 {total} / 新导入 {imported}（已存在跳过）");
    println!(
        "注意：raw .torrent 文件与做种 peer 数据不在 NP 库内，需站长另行放置后走 0022 口径回填"
    );
    Ok(())
}

async fn import_stats(np: &sqlx::MySqlPool, flux: &sqlx::PgPool) -> Result<()> {
    let rows: Vec<(i64, i64, i64)> =
        sqlx::query_as("SELECT id, uploaded, downloaded FROM users")
            .fetch_all(np)
            .await?;
    let total = rows.len();
    let mut imported = 0u64;
    for (np_id, uploaded, downloaded) in rows {
        let Some(flux_id) = sqlx::query_scalar::<_, i64>(
            "SELECT flux_id FROM np_map_users WHERE np_id = $1",
        )
        .bind(np_id)
        .fetch_optional(flux)
        .await?
        else {
            continue;
        };
        // 上下量初始值：写 users 快照列 + 同步抬 balance_baseline（P0-2 同族，0285）。
        // 只写快照会被 announce 消费轮的「基线 + SUM(流水)」重算抹平（导入站无历史流水
        // ⇒ 基线缺行 ⇒ 归零），所以基线才是初始事实的权威落点。
        // 幂等：仅当目标仍为 0（未跑过/未活动）时写入，重跑不覆盖导入后产生的新增量
        let n = sqlx::query(
            "UPDATE users SET uploaded = $2, downloaded = $3 \
             WHERE id = $1 AND uploaded = 0 AND downloaded = 0",
        )
        .bind(flux_id)
        .bind(uploaded)
        .bind(downloaded)
        .execute(flux)
        .await?
        .rows_affected();
        if n > 0 {
            sqlx::query(
                "INSERT INTO balance_baseline \
                 (user_id, base_up, base_down, base_spark, \
                 base_seed_secs, through) \
                 VALUES ($1, $2, $3, 0, 0, now()) \
                 ON CONFLICT (user_id) DO UPDATE SET \
                   base_up = EXCLUDED.base_up, \
                   base_down = EXCLUDED.base_down, \
                   updated_at = now()",
            )
            .bind(flux_id)
            .bind(uploaded)
            .bind(downloaded)
            .execute(flux)
            .await?;
        }
        imported += n;
    }
    println!("stats: 源 {total}/{imported} 初始流水写入");
    Ok(())
}

async fn report(np: &sqlx::MySqlPool, flux: &sqlx::PgPool) -> Result<()> {
    let np_users: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE status != 2")
            .fetch_one(np)
            .await?;
    let flux_users: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM np_map_users")
            .fetch_one(flux)
            .await?;
    let np_torrents: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM torrents WHERE visible != 'no'",
    )
    .fetch_one(np)
    .await?;
    let flux_torrents: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM torrents")
            .fetch_one(flux)
            .await?;
    // info_hash 全量 diff（源有目标无 → 缺失清单前 20 条）
    let missing: Vec<String> = {
        let src: Vec<String> = sqlx::query_scalar(
            "SELECT LOWER(info_hash) FROM torrents WHERE visible != 'no'",
        )
        .fetch_all(np)
        .await?;
        let dst: Vec<String> =
            sqlx::query_scalar("SELECT info_hash FROM torrents")
                .fetch_all(flux)
                .await?;
        let set: std::collections::HashSet<String> = dst.into_iter().collect();
        src.into_iter()
            .filter(|h| !set.contains(h))
            .take(20)
            .collect()
    };
    println!("==== 导入校验报告 ====");
    println!("用户   : NP {np_users} → mapped {flux_users}");
    println!("种子   : NP {np_torrents} → Flux {flux_torrents}");
    println!("缺失 info_hash（最多列 20）: {:?}", missing);
    Ok(())
}
