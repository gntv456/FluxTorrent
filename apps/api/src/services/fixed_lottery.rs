//! 固定抽签用例层（FRAME-PLAN-01：一个写用例一个函数）。
//!
//! 副作用清单（FRAME §4.3）：
//!   join:    报名行 + 票价扣款（spark_ledger `flx-join:{cid}:{uid}`）
//!   draw:    CAS open→drawn + winners + fulfillments + draw_meta + audit
//!   fulfill: 奖励发放（earn_spark/apply_item_effect/user_medals/arcade_items）
//!            + fulfillment 状态 + winners 状态
//!   claim:   领取资料提交/审核 + 状态翻转 + audit
//!
//! 两段式纪律：开奖事务只落 pending 行，奖励发放走独立用例（fulfill_one）
//! ——apply_item_effect 是 pool 版非事务口，塞进开奖事务会在失败时连带
//! 回滚整个开奖结果（与 tangpt 插件同款权衡）。幂等锚：
//!   票价 flx-join:{cid}:{uid} / 奖励 flx-win:{cid}:{uid}:{prize}

use crate::domain::fixed_lottery as dom;
use crate::domain::lottery_fair::new_seed;
use crate::economy_http::{spend_spark_tx, SpendOutcome};
use crate::errors::{DomainError, DomainResult};
use crate::repo::fixed_lottery as rp;
use crate::repo::fixed_lottery_read as rr;
use crate::state::AppState;
use serde_json::json;

/// 报名（已 open 且在窗口内）。票价 > 0 时同事务扣火花；已报名幂等返回。
pub(crate) async fn join(
    state: &std::sync::Arc<AppState>,
    uid: i64,
    username: &str,
    class_id: i32,
    code: &str,
    ip: Option<&str>,
) -> DomainResult<()> {
    let repo = &state.repo;
    let c = rp::campaign_by_code(repo, code)
        .await?
        .ok_or(DomainError::NotFound(0))?;
    if c.status != "open" {
        return Err(DomainError::Validation("活动不在报名期".into()));
    }
    let now = chrono::Utc::now();
    if let Some(s) = c.starts_at {
        if now < s {
            return Err(DomainError::Validation("报名尚未开始".into()));
        }
    }
    if c.trigger_type == "scheduled" {
        if let Some(d) = c.scheduled_draw_at {
            if now >= d {
                return Err(DomainError::Validation("报名已截止".into()));
            }
        }
    } else if let Some(f) = c.fallback_draw_at {
        if now >= f {
            return Err(DomainError::Validation("报名已截止".into()));
        }
    }
    check_eligibility(&repo, &c.eligibility, uid, class_id).await?;

    let mut tx = repo.db.begin().await.map_err(tx_err)?;
    if c.ticket_price > 0 {
        let idem = format!("flx-join:{}:{}", c.id, uid);
        let outcome = spend_spark_tx(
            &mut tx,
            uid,
            c.ticket_price,
            "lottery",
            &idem,
            "lottery",
            c.id,
        )
        .await?;
        // Replayed = 重放报名（已扣过价）：直接按已报名收尾
        if matches!(outcome, SpendOutcome::Replayed) {
            tx.commit().await.map_err(tx_err)?;
            return Ok(());
        }
    }
    rp::join_tx(&mut tx, c.id, uid, username, ip).await?;
    tx.commit().await.map_err(tx_err)?;
    repo.audit(Some(uid), "lottery.join", Some(c.id)).await;
    // threshold 达标同步触发开奖（对齐 tangpt 报名落库即查达标；
    // 失败不回滚报名——worker 60s tick 兜底重试）
    if c.trigger_type == "threshold" {
        if let Some(t) = c.threshold_count {
            if t >= 1 {
                let n = rp::participant_count(repo, c.id).await?;
                if n >= t as i64 {
                    if let Err(e) = draw(state, c.id, None).await {
                        tracing::warn!(
                            campaign = c.id,
                            error = %e,
                            "threshold 达标同步开奖失败（worker 兜底）"
                        );
                    }
                }
            }
        }
    }
    Ok(())
}

/// 资格谓词（与 repo::candidate_uids 的 SQL 同一把尺，报名侧快筛）。
async fn check_eligibility(
    repo: &crate::repo::Repo,
    elig: &serde_json::Value,
    uid: i64,
    class_id: i32,
) -> DomainResult<()> {
    let min = elig.get("min_class").and_then(|v| v.as_i64());
    let max = elig.get("max_class").and_then(|v| v.as_i64());
    if let Some(m) = min {
        if (class_id as i64) < m {
            return Err(DomainError::Validation("等级不满足参与门槛".into()));
        }
    }
    if let Some(m) = max {
        if (class_id as i64) > m {
            return Err(DomainError::Validation("等级超出参与范围".into()));
        }
    }
    let excl: Vec<i64> = elig
        .get("exclude_uids")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_i64()).collect())
        .unwrap_or_default();
    if excl.contains(&uid) {
        return Err(DomainError::Validation("你不在本活动的参与名单内".into()));
    }
    if let Some(days) = elig.get("registered_days_min").and_then(|v| v.as_i64())
    {
        if !rp::registration_ok(repo, uid, days).await? {
            return Err(DomainError::Validation(
                "注册时长不满足参与门槛".into(),
            ));
        }
    }
    Ok(())
}

fn tx_err(e: sqlx::Error) -> DomainError {
    DomainError::Internal(e.into())
}

/// 手动/worker 共用开奖入口。CAS 放事务首步并持有行锁，
/// 候选集在锁后、同一事务内读取——杜绝报名挤进「快照→CAS」窗口。
/// 返回 (already_drawn, winner_count)。
pub(crate) async fn draw(
    state: &std::sync::Arc<AppState>,
    cid: i64,
    operator: Option<i64>,
) -> DomainResult<(bool, usize)> {
    let repo = &state.repo;
    let c = rp::campaign_by_id(repo, cid)
        .await?
        .ok_or(DomainError::NotFound(cid))?;
    if c.status == "drawn" {
        return Ok((true, 0));
    }
    if c.status != "open" {
        return Err(DomainError::Validation("活动不在可开奖状态".into()));
    }

    let mut tx = repo.db.begin().await.map_err(tx_err)?;
    // CAS 先落锁：0 行 = 对手已开奖，让位；其后任何 ? 提前返回
    // 都由 sqlx 事务 Drop 自动回滚，不会留下半开状态。
    let n = sqlx::query(
        "UPDATE fixed_lottery_campaigns \
         SET status = 'drawn', drawn_at = now(), drawn_by = $2 \
         WHERE id = $1 AND status = 'open'",
    )
    .bind(cid)
    .bind(operator)
    .execute(&mut *tx)
    .await
    .map_err(tx_err)?
    .rows_affected();
    if n == 0 {
        tx.rollback().await.ok();
        return Ok((true, 0));
    }

    let prizes = rr::prizes_of_tx(&mut tx, cid).await?;
    if prizes.is_empty() {
        return Err(DomainError::Validation("活动未配置有效奖项".into()));
    }
    let uids = rr::candidate_uids_tx(&mut tx, cid).await?;
    let slot_total: i64 = prizes.iter().map(|p| p.3 as i64).sum();

    // seed 在候选集冻结之后才生成；快照哈希与 seed 落 meta 供复算
    let seed = new_seed();
    let sel = dom::selection_of(&seed, cid, &uids);
    let prize_slots: Vec<(i64, i32)> =
        prizes.iter().map(|p| (p.0, p.3)).collect();
    let slots = dom::assign_slots(&sel, &prize_slots);
    let mut rows: Vec<(i64, i32, i64, i64, String, String)> = Vec::new();
    for &(pid, sno, pos, uid) in &slots {
        let uname = sqlx::query_scalar::<_, String>(
            "SELECT username FROM users WHERE id = $1",
        )
        .bind(uid)
        .fetch_one(&mut *tx)
        .await
        .map_err(tx_err)?;
        let pname = prizes
            .iter()
            .find(|p| p.0 == pid)
            .map(|p| p.2.clone())
            .unwrap_or_default();
        rows.push((pid, sno, pos, uid, uname, pname));
    }
    let meta = json!({
        "algorithm": "hmac-sha256-bottom-k-v1",
        "seed": seed,
        "seed_hash": sha256_hex(seed.as_bytes()),
        "candidate_snapshot_hash": dom::snapshot_hash(&uids),
        "candidate_count": uids.len(),
        "prize_slot_count": slot_total,
        "winner_count": rows.len(),
        "voided": uids.is_empty(),
        "trigger": if operator.is_some() { "manual" } else { "auto" },
    });

    rp::insert_winners_tx(&mut tx, cid, &seed, &rows, &meta).await?;
    tx.commit().await.map_err(tx_err)?;

    repo.audit_detail(
        operator,
        "lottery.drawn",
        Some(cid),
        None,
        Some(meta.clone()),
    )
    .await;
    Ok((false, rows.len()))
}

fn sha256_hex(b: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(b);
    h.finalize().iter().map(|x| format!("{x:02x}")).collect()
}

pub(crate) mod claim;
#[cfg(test)]
mod db_tests;
pub(crate) mod fulfill;
