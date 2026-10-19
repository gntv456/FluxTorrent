//! 抽奖可验证公平的 SQL 唯一居所（FRAME §2 H 层不下沉 sqlx）。
//!
//! 消费方：`community_http/lottery.rs`（开售承诺 / 售票 / 验证页）与
//! `worker/jobs/lottery.rs`（结算 + 揭晓）。哈希派生与排序判定在
//! `domain/lottery_fair.rs`（纯函数），本文件只负责读写与链式推进。
//!
//! 【链式推进为什么放在 repo】票据哈希要把「上一条的哈希」喂给下一条，
//! 这个状态机必须在事务里串行推进，否则并发售票会各自从 genesis 起链、
//! 产生两条分叉——验证页无法判断哪条是真的。

use super::Repo;
use crate::domain::lottery_fair as fair;
use crate::errors::{DomainError, DomainResult};

/// 开售：登记承诺（commit + seed），并标记该场走公平路径。
pub async fn open_fair_round(
    repo: &Repo,
    topic_id: i64,
    seed_hex: &str,
    commit_hex: &str,
) -> DomainResult<()> {
    sqlx::query(
        "UPDATE topic_lotteries \
         SET commit_hash = $2, seed_cipher = $3, fair_enabled = TRUE \
         WHERE topic_id = $1 AND status = 'open' AND commit_hash IS NULL",
    )
    .bind(topic_id)
    .bind(commit_hex)
    .bind(seed_hex)
    .execute(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(())
}

/// 该场是否走公平路径（老抽奖为 false → 走旧随机逻辑）。
pub async fn is_fair(repo: &Repo, topic_id: i64) -> DomainResult<bool> {
    let v: Option<bool> = sqlx::query_scalar(
        "SELECT fair_enabled FROM topic_lotteries WHERE topic_id = $1",
    )
    .bind(topic_id)
    .fetch_optional(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(v.unwrap_or(false))
}

/// 结算用：取 seed（NULL = 老抽奖或未登记）。
pub async fn seed_of(
    repo: &Repo,
    topic_id: i64,
) -> DomainResult<Option<String>> {
    Ok(sqlx::query_scalar(
        "SELECT seed_cipher FROM topic_lotteries \
         WHERE topic_id = $1 AND fair_enabled",
    )
    .bind(topic_id)
    .fetch_optional(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .flatten())
}

/// 售票（P-396：必须在调用方事务内）：给这张票编号并接上哈希链。
///
/// 旧版自己 `begin()`，且写的是 `ON CONFLICT (topic_id, user_id) DO NOTHING`
/// ——而调用方 `lottery_join` 已经先落了占位行，这条 INSERT **必然冲突**
/// ⇒ 哈希从不落库。实测 2 人参与的公平抽奖：库里 `ticket_hash`/`ticket_seq`
/// 全 NULL，开奖后验证页却回「服务端复算通过：0 名参与者 / 0 名中奖」，
/// 而 `picked` 为空 ⇒ 奖池既不派彩也不再退回（status 已 drawn）。
/// 不是坏了不响，是响亮地给出假证明。
///
/// 现在占位行由同一事务先行落库，本函数只负责给它补上 `(ticket_seq,
/// ticket_hash)`；补不上就是链没接上，宁可报错也不静默返回空串。
/// 链尾推进靠调用方持有的 advisory 锁串行（并发售票各自从 genesis
/// 起链会产生两条分叉，验证页无法判断哪条是真的）。
pub async fn issue_ticket_tx(
    tx: &mut sqlx::PgTransaction<'_>,
    topic_id: i64,
    user_id: i64,
    seed_hex: &str,
) -> DomainResult<String> {
    // 链尾 = 该场最大的 ticket_seq 对应的哈希（无则 genesis）
    let prev: Option<(Option<i64>, Option<String>)> = sqlx::query_as(
        "SELECT ticket_seq, ticket_hash FROM lottery_entries \
         WHERE topic_id = $1 AND ticket_hash IS NOT NULL \
         ORDER BY ticket_seq DESC NULLS LAST LIMIT 1",
    )
    .bind(topic_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let (seq, prev_hash) = prev
        .map(|(s, h)| (s.unwrap_or(-1), h.unwrap_or_default()))
        .unwrap_or((-1, String::new()));
    let next_seq = seq + 1;
    let root = if seq < 0 {
        fair::genesis_root()
    } else {
        prev_hash
    };
    let hash = fair::ticket_hash(seed_hex, &root, next_seq as u32);
    let written: Option<String> = sqlx::query_scalar(
        "UPDATE lottery_entries SET ticket_hash = $3, ticket_seq = $4 \
         WHERE topic_id = $1 AND user_id = $2 AND ticket_hash IS NULL \
         RETURNING ticket_hash",
    )
    .bind(topic_id)
    .bind(user_id)
    .bind(&hash)
    .bind(next_seq)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    written.ok_or_else(|| {
        // 占位行刚在本事务里插入，补不上哈希只可能是链断了（不是并发，
        // 场次锁已串行）——留 500 而不是回一张空票。
        DomainError::Internal(anyhow::anyhow!("抽奖票据未能接上哈希链"))
    })
}

/// 停售：锁死 tickets_root（链尾哈希）。
pub async fn seal_root(
    repo: &Repo,
    topic_id: i64,
) -> DomainResult<Option<String>> {
    Ok(sqlx::query_scalar(
        "SELECT ticket_hash FROM lottery_entries \
         WHERE topic_id = $1 AND ticket_hash IS NOT NULL \
         ORDER BY ticket_seq DESC NULLS LAST LIMIT 1",
    )
    .bind(topic_id)
    .fetch_optional(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .flatten())
}

/// 结算用：按序取全部票据哈希（重建链 + 排序）。
pub async fn ticket_hashes(
    repo: &Repo,
    topic_id: i64,
) -> DomainResult<Vec<String>> {
    Ok(sqlx::query_scalar(
        "SELECT ticket_hash FROM lottery_entries \
         WHERE topic_id = $1 AND ticket_hash IS NOT NULL \
         ORDER BY ticket_seq",
    )
    .bind(topic_id)
    .fetch_all(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?)
}

/// 结算用：票据 → 参与者映射（排序后回填中奖）。
pub async fn ticket_to_user(
    repo: &Repo,
    topic_id: i64,
) -> DomainResult<Vec<(String, i64)>> {
    Ok(sqlx::query_as(
        "SELECT ticket_hash, user_id FROM lottery_entries \
         WHERE topic_id = $1 AND ticket_hash IS NOT NULL \
         ORDER BY ticket_seq",
    )
    .bind(topic_id)
    .fetch_all(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?)
}

/// 揭晓：落锁死的 root 与完整证明。
pub async fn store_proof(
    repo: &Repo,
    topic_id: i64,
    tickets_root: &str,
    proof_json: &serde_json::Value,
) -> DomainResult<()> {
    sqlx::query(
        "UPDATE topic_lotteries \
         SET tickets_root = $2, draw_proof = $3::jsonb \
         WHERE topic_id = $1",
    )
    .bind(topic_id)
    .bind(tickets_root)
    .bind(proof_json.to_string())
    .execute(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(())
}

/// 验证页：取公开材料（commit + 证明）。
pub async fn public_commit(
    repo: &Repo,
    topic_id: i64,
) -> DomainResult<Option<PublicMaterial>> {
    let row: Option<(String, Option<serde_json::Value>)> = sqlx::query_as(
        "SELECT COALESCE(commit_hash, ''), draw_proof \
         FROM topic_lotteries WHERE topic_id = $1",
    )
    .bind(topic_id)
    .fetch_optional(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(row.map(|(commit, proof)| PublicMaterial { commit, proof }))
}

/// 验证页的公开材料。
#[derive(Debug, Clone, serde::Serialize)]
pub struct PublicMaterial {
    /// 开售时公布的承诺（`SHA-256(seed)`）。
    pub commit: String,
    /// 结算后的完整证明；未开奖时为 NULL。
    pub proof: Option<serde_json::Value>,
}
