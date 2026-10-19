//! 答谢魔力转账（P-360）：给付方与收礼方必须在同一笔事务里守恒。
//!
//! 旧版 `/torrents/{id}/thanks` 带 `amount` 时**只给种主入账，没有向答谢人扣款**
//! ——`torrents::thank` 只落一行 `thanks`，钱是凭空调出来的；界面
//! `torrent-social.tsx` 的 +1..+10000 按钮组就是一条活的增发口（每个
//! 「一人一种」组合白送一笔，双号互刷无上限）。同时幂等键里塞了
//! `chrono::Utc::now().timestamp()`，秒级时间戳使每个重试都是新键，重放
//! 闸门形同不存在。
//!
//! 现在：扣款、入账、落感谢行同一事务，任一步失败整体回滚；幂等键锚在
//! 「谁谢了哪种种」这个事实上，365 天内同一对键不可能第二次转账。
//!
//! kind 暂借 `forum_tip`（流水页译名「打赏」，见 apps/web/i18n/zh-CN.ts
//! 的 ledger kinds 块）：`torrent_thanks` 这个新键还没有译名，而三份 i18n
//! 文件正被站方改着，我不把他们的在途改动带进本批。机器可读的来源判定
//! 走 `ref_type`，所以「论坛打赏」与「种子答谢」在流水里仍可区分。

use sqlx::{PgPool, Transaction};

use crate::economy_http::{earn_spark_tx, spend_spark_tx, SpendOutcome};
use crate::errors::{DomainError, DomainResult};

/// 这条种的发布者（匿名发布也按 `owner_id` 记账，钱要给真账号）。
/// 读不到就是种不存在/已删 → 404，不能让调用方拿着 `None` 继续往下走：
/// 旧版正是 `owner=None` 时静默跳过转账，却照样落一条「已答谢」。
pub(super) async fn owner_of(
    db: &PgPool,
    torrent_id: i64,
) -> DomainResult<i64> {
    let owner: Option<i64> =
        sqlx::query_scalar("SELECT owner_id FROM torrents WHERE id = $1")
            .bind(torrent_id)
            .fetch_optional(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .flatten();
    owner.ok_or(DomainError::NotFound(torrent_id))
}

/// 允许的答谢档位（馒头口径按钮组）。0 由调用方先行拦下（免费感谢不进来）。
pub(super) fn is_valid_amount(amount: i64) -> bool {
    matches!(amount, 1 | 10 | 100 | 500 | 1000 | 10000)
}

/// 把 `amount` 从答谢人转到种主，返回真实转出的数额。
///
/// 调用方负责把这条与 `thanks` 行放进同一个事务；这里不再自己 begin，
/// 否则又回到「感谢落了账、钱没动」的两段写。
pub(super) async fn transfer(
    tx: &mut Transaction<'_, sqlx::Postgres>,
    giver: i64,
    owner: i64,
    amount: i64,
    torrent_id: i64,
) -> DomainResult<i64> {
    let idem = format!("thank-spark:{}:{}", giver, torrent_id);
    // 扣款在前：余额不足由 spend 自己拒单，「先送钱再发现扣不动」不存在
    if !matches!(
        spend_spark_tx(
            tx,
            giver,
            amount,
            "forum_tip",
            &idem,
            "torrent_thanks",
            torrent_id,
        )
        .await?,
        SpendOutcome::Spent
    ) {
        return Err(DomainError::Validation(
            "该笔答谢已受理，魔力未重复转出".into(),
        ));
    }
    // spend 已闸 Spent，earn 幂等键锚在本次新 spend 的派生键，必为 Spent；
    // 但仍要判——丢弃结果就是「账面说送了、种主没收到」
    if !matches!(
        earn_spark_tx(tx, owner, amount, "forum_tip", &format!("{idem}:to"))
            .await?,
        SpendOutcome::Spent
    ) {
        return Err(DomainError::LedgerConflict);
    }
    Ok(amount)
}
