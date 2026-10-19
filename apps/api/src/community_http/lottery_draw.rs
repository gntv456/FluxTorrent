//! 抽奖开奖核心（自 lottery.rs 抽出，守 300 行门禁）。
//!
//! 手动入口（`lottery_draw`）与 worker 的到点 tick 共用本函数——
//! CAS `open→drawn` 保证只有一方赢对局，两条入口不会重复发奖。
//!
//! commit-reveal（0379）：公平场用 seed 派生的次序定中奖并落公开证明；
//! 老抽奖（无 seed）保持 `RANDOM()` 洗牌，行为与迁移前逐位一致。

use super::*;
use crate::community_http::forum_perm::forum_access;
use crate::community_http::forum_util::notify_user;
use crate::domain::lottery_fair as fair;
use crate::dto::ok;
use crate::economy_http::earn_spark;
use crate::errors::{DomainError, DomainResult};
use crate::repo::lottery_draw as repo_draw;
use crate::repo::lottery_fair as repo_fair;
use crate::repo::Repo;

/// 开奖核心（手动入口与 worker 共用）：CAS open→drawn 后随机抽 winners 名，
/// 每人发 prize_per_winner（幂等键 `forum-lottery-win:{tid}:{uid}`）。
/// 参与人数不足名额时全中奖（钱不留在池里）；零参与则奖金池退回楼主。
pub async fn lottery_draw_core(
    repo: &Repo,
    topic_id: i64,
) -> DomainResult<serde_json::Value> {
    let db = &repo.db;
    let Some((winners, prize, _ticket, status)) =
        repo_draw::meta(repo, topic_id).await?
    else {
        return Err(DomainError::NotFound(topic_id));
    };
    if status != "open" {
        return Err(DomainError::Validation("抽奖不在进行中".into()));
    }
    if repo_draw::cas_drawn(repo, topic_id).await? == 0 {
        return Err(DomainError::LedgerConflict);
    }
    // 参与者全表捞出来在应用层抽（数量级 ≤ 数百，RANDOM() 洗牌即可）
    let mut entries: Vec<i64> =
        repo_draw::participant_ids(repo, topic_id).await?;
    if entries.is_empty() {
        // 无人参与：奖金池退回楼主
        let op = repo_draw::topic_owner(repo, topic_id).await?;
        let refund = winners as i64 * prize;
        if refund > 0 {
            // P-366：无人参与时奖池退回楼主——旧版 `let _ =` 吞失败，
            // 而调用方拿到的仍是「已退回」的返回值（假成功）。
            // 键 `forum-lottery-refund:{topic}` 稳定，失败时出声并让上层
            // 看见 `refunded=false`，别把没到账说成到账。
            let refunded = earn_spark(
                db,
                op,
                refund,
                "forum_lottery_refund",
                &format!("forum-lottery-refund:{topic_id}"),
            )
            .await
            .is_ok();
            if !refunded {
                tracing::warn!(
                    topic = topic_id,
                    owner = op,
                    amount = refund,
                    "lottery pool refund failed"
                );
            }
        }
        return Ok(serde_json::json!({
            "topic_id": topic_id,
            "winners": [],
            "refunded": refund,
        }));
    }
    // commit-reveal（0379）：公平场用 seed 派生的次序定中奖，公开证明
    // 供第三方复算；老抽奖（seed 为空）保持 RANDOM() 洗牌不变。
    let picked: Vec<i64> = match repo_fair::seed_of(repo, topic_id).await? {
        Some(seed) => {
            let pairs = repo_fair::ticket_to_user(repo, topic_id).await?;
            let tickets: Vec<String> =
                pairs.iter().map(|(h, _)| h.clone()).collect();
            let root =
                tickets.last().cloned().unwrap_or_else(fair::genesis_root);
            let order = fair::draw_order(&seed, &tickets, winners as usize);
            let winner_hashes: Vec<String> = order
                .iter()
                .filter(|(_, won)| *won)
                .map(|(h, _)| (*h).clone())
                .collect();
            let proof = fair::build_proof(
                &topic_id.to_string(),
                &seed,
                &root,
                &tickets,
                winners as usize,
            );
            // P-396：证明写失败旧版 `let _ =` 吞掉——`draw_proof` 留 NULL ⇒
            // 验证页永远停在「开售中，开奖后公开 seed」，而奖其实已经开完了。
            // 出声留下现场（P-323 同族），别把没落库说成已揭晓。
            if let Err(e) = repo_fair::store_proof(
                repo,
                topic_id,
                &root,
                &serde_json::to_value(&proof)
                    .unwrap_or(serde_json::Value::Null),
            )
            .await
            {
                tracing::warn!(
                    err = %e,
                    topic = topic_id,
                    "lottery draw proof not stored: fairness page stuck at open"
                );
            }
            winner_hashes
                .iter()
                .filter_map(|h| {
                    pairs.iter().find(|(th, _)| th == h).map(|(_, uid)| *uid)
                })
                .collect()
        }
        None => {
            use rand::seq::SliceRandom;
            entries.shuffle(&mut rand::thread_rng());
            let take = (winners as usize).min(entries.len());
            entries.into_iter().take(take).collect()
        }
    };
    for uid in &picked {
        // P-367 换序：**先派彩，后落「已中奖」标记**，通知也只发给真到账的人。
        // 旧顺序是 mark_won 先落、`let _ = earn_spark` 后走且吞失败 ⇒ 派彩出错时
        // 会员已被记成「已中奖」（参与行已被判过，不可重试），奖金却没到账——
        // 把一次可重试的失败固化成永久假成功。prize==0 的抽奖（不发魔力）照旧标记。
        if prize > 0 {
            if let Err(e) = earn_spark(
                db,
                *uid,
                prize,
                "forum_lottery",
                &format!("forum-lottery-win:{topic_id}:{uid}"),
            )
            .await
            {
                tracing::warn!(
                    err = %e,
                    topic = topic_id,
                    winner = *uid,
                    "lottery payout failed: winner left unmarked for retry"
                );
                continue;
            }
            notify_user(
                db,
                *uid,
                "comment_reply",
                "抽奖中奖",
                &format!("您在 [/forums/topic/{topic_id}] 的抽奖中中奖，获得 {prize} 魔力！"),
            )
            .await;
        }
        // 钱已到账、但「已中奖」标记写失败 ⇒ 界面仍是未中奖，而参与行已判过
        // 无法重试：出声留下可补现场（同 P-364 口径）。
        if let Err(e) = repo_draw::mark_won(repo, topic_id, *uid).await {
            tracing::warn!(
                err = %e,
                topic = topic_id,
                winner = *uid,
                "lottery mark_won failed after payout"
            );
        }
    }
    Ok(serde_json::json!({
        "topic_id": topic_id,
        "winners": picked,
        "prize": prize,
    }))
}
