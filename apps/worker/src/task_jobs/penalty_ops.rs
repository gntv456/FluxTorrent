//! 考核处置单项执行（0397，自 penalty.rs 拆出守 300 行门禁）。
//!
//! 列口径全部对齐后台人工入口（user_status.rs / user_flags.rs）：
//! 自动处置与人工处置写同一批列，不会出现两套真相。
//!
//! 【流量口径——P0 修复】users.uploaded/downloaded 只是**快照**，权威在
//! traffic_ledger：worker 的 reconcile_snapshots 每 6h 全量把快照重算成
//! `base_up/base_down + SUM(traffic_ledger.delta_*)`（同款坑见
//! admin_http/user_ops.rs 的 P0-2 注释、repo/cheat_reverse.rs 的
//! 「流水记实际值而非请求值」注释）。旧写法只 `UPDATE users`，实测
//! 处置后 6 小时内被对账抹平＝**处罚静默失效**，还会误触 reconcile 的
//! 「疑似绕过流水的流量变动源」告警（值班白跑一趟）。现在同事务内落
//! **负向流水** + 同步快照，流水记**实际扣掉**的数（GREATEST 夹到 0 之后
//! 的真值），文案与流水才对得上。

use sqlx::{Postgres, Transaction};

use super::penalty::PenaltyItem;
use super::settle::OpenClaim;

/// 执行单项处置，返回实况 JSON（实扣额等，落 exam_penalties.applied）。
///
/// `tx` 由调用方持有：处置与 exam_penalties 落账在同一事务里——
/// 崩溃重跑要么全成、要么全不成（不会出现「罚了一半、账没记」）。
pub(super) async fn apply_one(
    tx: &mut Transaction<'_, Postgres>,
    c: &OpenClaim,
    item: &PenaltyItem,
) -> anyhow::Result<serde_json::Value> {
    match item.kind.as_str() {
        super::penalty::KIND_BAN => ban(tx, c).await,
        super::penalty::KIND_SUSPEND => suspend(tx, c).await,
        super::penalty::KIND_RESTRICT_DOWNLOAD => {
            restrict_download(tx, c).await
        }
        super::penalty::KIND_MUTE => mute(tx, c).await,
        super::penalty::KIND_BAN_UPLOAD => {
            deny_perm(tx, c, "torrent.upload").await?;
            Ok(serde_json::json!({ "denied": "torrent.upload" }))
        }
        super::penalty::KIND_BAN_PAGES => {
            let pages = item.pages.clone().unwrap_or_default();
            let mut denied: Vec<String> = Vec::new();
            for p in &pages {
                if deny_perm(tx, c, p).await? {
                    denied.push(p.clone());
                }
            }
            Ok(serde_json::json!({ "denied": denied }))
        }
        super::penalty::KIND_SPARK_FINE => {
            spark_fine(tx, c, item.amount.unwrap_or(0)).await
        }
        super::penalty::KIND_TRAFFIC_FINE => traffic_fine(tx, c, item).await,
        _ => Ok(serde_json::json!({})),
    }
}

/// 封禁：status=2 + forumpost=FALSE + 回收未用邀请码，
/// 与 user_status.rs 的人工封禁逐位同口径
async fn ban(
    tx: &mut Transaction<'_, Postgres>,
    c: &OpenClaim,
) -> anyhow::Result<serde_json::Value> {
    sqlx::query(
        "UPDATE users SET status = 2, forumpost = FALSE \
         WHERE id = $1 AND status < 2",
    )
    .bind(c.user_id)
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        "UPDATE invites SET status = 2 \
         WHERE inviter_id = $1 AND status = 0 AND expires_at > now()",
    )
    .bind(c.user_id)
    .execute(&mut **tx)
    .await?;
    Ok(serde_json::json!({ "status": 2 }))
}

/// 挂起：suspended=TRUE（下载计费闸拦截，可继续做种保分享）
async fn suspend(
    tx: &mut Transaction<'_, Postgres>,
    c: &OpenClaim,
) -> anyhow::Result<serde_json::Value> {
    sqlx::query("UPDATE users SET suspended = TRUE WHERE id = $1")
        .bind(c.user_id)
        .execute(&mut **tx)
        .await?;
    Ok(serde_json::json!({ "suspended": true }))
}

/// 禁下载：与 user_flags.rs 同口径——download_locked_by 置 'exam'
/// （人工恢复入口按此列判断锁来源，不与人工锁混淆）
async fn restrict_download(
    tx: &mut Transaction<'_, Postgres>,
    c: &OpenClaim,
) -> anyhow::Result<serde_json::Value> {
    sqlx::query(
        "UPDATE users SET download_enabled = FALSE, \
         download_locked_by = 'exam' \
         WHERE id = $1 AND download_enabled",
    )
    .bind(c.user_id)
    .execute(&mut **tx)
    .await?;
    Ok(serde_json::json!({ "download_enabled": false }))
}

/// 禁言：status=1 + forumpost=FALSE（forum_perm.rs 读的就是 forumpost，
/// 单改 status 论坛侧不认——0381 曾踩过）
async fn mute(
    tx: &mut Transaction<'_, Postgres>,
    c: &OpenClaim,
) -> anyhow::Result<serde_json::Value> {
    sqlx::query(
        "UPDATE users SET status = 1, forumpost = FALSE \
         WHERE id = $1 AND status = 0",
    )
    .bind(c.user_id)
    .execute(&mut **tx)
    .await?;
    Ok(serde_json::json!({ "status": 1, "forumpost": false }))
}

/// 显式拒绝一个权限位（禁发布 / 禁页面共用）。
/// 用户级覆盖优先于角色判定（authz.rs），等级再高也拦得住。
///
/// 返回是否真的落到一条权限位上。**未知权限位不能炸结算**：
/// user_permissions.permission_key 对 permissions.key 有外键，直接
/// INSERT 一个库里没有的键会报 FK 违例 → 整个结算批次中断 → 全站考核
/// 认领卡在 status=0（人工解开全部超时才算完）。现在用
/// `INSERT … SELECT … FROM permissions WHERE key = $2`，未知权限位
/// 静默跳过并喊日志，处置其余项照常执行。
async fn deny_perm(
    tx: &mut Transaction<'_, Postgres>,
    c: &OpenClaim,
    key: &str,
) -> anyhow::Result<bool> {
    let n = sqlx::query(
        "INSERT INTO user_permissions \
         (user_id, permission_key, granted, note) \
         SELECT $1, p.key, FALSE, 'exam penalty' \
         FROM permissions p WHERE p.key = $2 \
         ON CONFLICT (user_id, permission_key) DO UPDATE SET \
         granted = FALSE, note = 'exam penalty'",
    )
    .bind(c.user_id)
    .bind(key)
    .execute(&mut **tx)
    .await?
    .rows_affected();
    if n == 0 {
        tracing::error!(
            key,
            user_id = c.user_id,
            "考核处置：权限位在 permissions 表里不存在，已跳过（配置错误请在后台改正）"
        );
    }
    Ok(n > 0)
}

/// 扣魔力：流水驱动（spark_ledger）+ 只扣到 0，与 settle_fail 罚金
/// 同款——直接改 users 余额会被余额对账回滚（0086 教训）
async fn spark_fine(
    tx: &mut Transaction<'_, Postgres>,
    c: &OpenClaim,
    want: i64,
) -> anyhow::Result<serde_json::Value> {
    let want = want.max(0);
    let balance: i64 = sqlx::query_scalar(
        "SELECT spark_balance FROM users WHERE id = $1 FOR UPDATE",
    )
    .bind(c.user_id)
    .fetch_one(&mut **tx)
    .await?;
    let take = balance.min(want).max(0);
    if take > 0 {
        let idem = format!("exam_penalty:{}:spark_fine", c.id);
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM spark_ledger \
             WHERE idempotency_key = $1)",
        )
        .bind(&idem)
        .fetch_one(&mut **tx)
        .await?;
        if !exists {
            sqlx::query(
                "INSERT INTO spark_ledger (id, user_id, amount, kind, \
                 ref_type, ref_id, idempotency_key, balance_after) \
                 VALUES (nextval('spark_ledger_id_seq'), $1, $2, \
                 'exam_penalty', 'task', $3, $4, $5)",
            )
            .bind(c.user_id)
            .bind(-take)
            .bind(c.task_id)
            .bind(&idem)
            .bind(balance - take)
            .execute(&mut **tx)
            .await?;
            sqlx::query(
                "UPDATE users SET spark_balance = spark_balance - $2 \
                 WHERE id = $1",
            )
            .bind(c.user_id)
            .bind(take)
            .execute(&mut **tx)
            .await?;
        }
    }
    Ok(serde_json::json!({ "took": take, "wanted": want }))
}

/// 扣上传/下载量：GREATEST 钳到 0（不负量——负量会破坏分享率
/// 计算的分子分母语义）。
///
/// 【P0】必须落 traffic_ledger：users.uploaded/downloaded 是快照，
/// reconcile 每 6h 会按「基线 + 流水」重算覆盖它，只改快照＝处罚
/// 6 小时后自动消失。流水记**实际扣掉**的数（不是请求值），
/// 快照同步减去同一个数，两边才对得上（口径同
/// repo/cheat_reverse.rs 的 apply_reclaim_tx）。
async fn traffic_fine(
    tx: &mut Transaction<'_, Postgres>,
    c: &OpenClaim,
    item: &PenaltyItem,
) -> anyhow::Result<serde_json::Value> {
    let want = item.amount.unwrap_or(0).max(0);
    let (col, is_down) = if item.target.as_deref() == Some("downloaded") {
        ("downloaded", true)
    } else {
        ("uploaded", false)
    };
    if want <= 0 {
        return Ok(serde_json::json!({
            "target": col, "took": 0, "wanted": want
        }));
    }
    // 锁行读现值 → 算实扣 → 落负向流水 + 同步快照（同一事务）
    let cur: i64 = sqlx::query_scalar(&format!(
        "SELECT {col} FROM users WHERE id = $1 FOR UPDATE"
    ))
    .bind(c.user_id)
    .fetch_one(&mut **tx)
    .await?;
    let take = cur.max(0).min(want);
    if take > 0 {
        let (du, dd) = if is_down {
            (0i64, -take)
        } else {
            (-take, 0i64)
        };
        sqlx::query(
            "INSERT INTO traffic_ledger (id, user_id, torrent_id, \
             delta_up, delta_down, window_start, reason) \
             VALUES (nextval('traffic_ledger_id_seq'), $1, 0, $2, $3, \
                     now(), $4)",
        )
        .bind(c.user_id)
        .bind(du)
        .bind(dd)
        .bind(format!("exam penalty: claim #{} {col}", c.id))
        .execute(&mut **tx)
        .await?;
        sqlx::query(&format!(
            "UPDATE users SET {col} = GREATEST({col} - $2, 0) WHERE id = $1"
        ))
        .bind(c.user_id)
        .bind(take)
        .execute(&mut **tx)
        .await?;
    }
    Ok(serde_json::json!({
        "target": col, "took": take, "wanted": want
    }))
}
