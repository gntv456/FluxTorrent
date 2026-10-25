//! 论坛行为奖励的额度来源（后台「行为奖励」面板的 `starttopic` /
//! `makepost`）。
//!
//! P-456（2026-10-21）：这两把键 0034 就进了 `site_settings`，0039 又给
//! 它们登记了面板元数据（分组「行为奖励」、单位火花），站长在
//! `/admin/settings` 改得动、保存后也回显——可发放侧写的是字面量：发主题
//! 恒 +2、回帖恒 +1，删帖收回也照同一组字面量扣。面板是台摆设。
//! 现网两把键的值本来就分别是 2 / 1，接线当天一分钱行为都不变。
//!
//! 收回为什么不读面板值而读流水：面板随时能改，而当年那笔火花是按旧值
//! 发的。按现值收回会让「站长把 2 调成 5 之后删一个老主题」多扣楼主 3
//! 火花——P-415 立的规矩是「能收多少收多少、与发放对称」，对称只能是
//! 「收回当时真发的那笔」。`forum-topic:{uid}:{tid}` 与
//! `forum-reply:{uid}:{pid}` 是发放时就确定、事后能重算的幂等键，
//! `spark_ledger` 上的 `idx_spark_idem`（UNIQUE (idempotency_key,
//! created_at)）正好命中。
//!
//! 顺带修掉一处多扣：楼主的首帖也存于 `posts`，但它拿的是发帖奖励，
//! 从来没有任何回帖流水；旧写法在单独删首帖时照扣 1 火花。

use super::super::Repo;
use crate::errors::{DomainError, DomainResult};

/// 键缺失/整列读不出时的老口径（与接线前的字面量一致）。
const TOPIC_DEFAULT: i64 = 2;
const REPLY_DEFAULT: i64 = 1;
/// `0039` 给这两把键登记的 max 是 NULL（不封顶）。发火花是铸币，这里留
/// 一道与付费定价同型的上限（`torrent_http::manage.rs` 的
/// `price.clamp(0, 1_000_000)`），免得后台一个手滑的位数把论坛变印钞机。
const AMOUNT_MAX: i64 = 1_000_000;

/// 面板值归一。读不出（缺失、空串、`0.5` 这类火花给不出的小数、负数、
/// 超长位数）一律回到老口径或夹进 `[0, AMOUNT_MAX]`：面板 min=0，负数
/// 不是「惩罚发帖」的合法表达，夹 0 比按字面量扣钱安全。
pub fn amount_of(raw: Option<&str>, fallback: i64) -> i64 {
    match raw.and_then(|v| v.trim().parse::<i64>().ok()) {
        Some(v) => v.clamp(0, AMOUNT_MAX),
        None => fallback,
    }
}

/// 发主题奖励（面板键 `starttopic`）。
pub async fn topic_reward(repo: &Repo) -> i64 {
    amount_of(
        crate::repo::settings::read(repo, "starttopic")
            .await
            .as_deref(),
        TOPIC_DEFAULT,
    )
}

/// 回帖奖励（面板键 `makepost`）。
pub async fn reply_reward(repo: &Repo) -> i64 {
    amount_of(
        crate::repo::settings::read(repo, "makepost")
            .await
            .as_deref(),
        REPLY_DEFAULT,
    )
}

/// 当初真发放了多少（无流水行 = 当时没发过，回 0 = 没有可收回的）。
/// 读失败必须出声：吞掉错误会退化成「不收回」，白拿火花没人知道。
pub async fn granted_by_idem(repo: &Repo, key: &str) -> DomainResult<i64> {
    let v: Option<i64> = sqlx::query_scalar(
        "SELECT sum(amount) FROM spark_ledger WHERE idempotency_key = $1",
    )
    .bind(key)
    .fetch_one(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(v.unwrap_or(0))
}

/// 发放用的幂等键（收回侧要按同一形状重算，两处共用这一份定义）。
pub fn topic_idem(uid: i64, tid: i64) -> String {
    format!("forum-topic:{uid}:{tid}")
}

/// 回帖发放的幂等键。
pub fn reply_idem(uid: i64, pid: i64) -> String {
    format!("forum-reply:{uid}:{pid}")
}

#[cfg(test)]
mod tests {
    use super::amount_of;

    #[test]
    fn panel_value_is_used_as_is() {
        assert_eq!(amount_of(Some("5"), 2), 5);
        assert_eq!(amount_of(Some(" 0 "), 2), 0);
        assert_eq!(amount_of(Some("3"), 1), 3);
    }

    #[test]
    fn unreadable_panel_value_falls_back_to_old_literal() {
        // 键缺失、空串、小数（火花给不出半个）、非数字：都回老口径
        assert_eq!(amount_of(None, 2), 2);
        assert_eq!(amount_of(Some(""), 2), 2);
        assert_eq!(amount_of(Some("0.5"), 2), 2);
        assert_eq!(amount_of(Some("abc"), 1), 1);
    }

    #[test]
    fn out_of_domain_is_clamped_not_trusted() {
        // 负数不许变成「发帖先扣钱」
        assert_eq!(amount_of(Some("-7"), 2), 0);
        // 解析得动但超上限：夹到上限（面板 max 登记为 NULL，不夹就是铸币口）
        assert_eq!(amount_of(Some("5000000"), 2), 1_000_000);
        // 位数长到 i64 都装不下：按「读不出」处理，回老口径。这里不能夹成
        // 上限——一栏 20 位数字不是任何站长的真实意图，回 2 比发 100 万安全。
        assert_eq!(amount_of(Some("99999999999999999999"), 2), 2);
    }

    #[test]
    fn idem_keys_match_the_shape_already_in_the_ledger() {
        // 现网 spark_ledger 里的键就是这两形状（实测
        // `forum-topic:7064:474` / `forum-reply:7064:643`），改形状＝
        // 收回侧读不到旧流水，白拿
        assert_eq!(super::topic_idem(7064, 474), "forum-topic:7064:474");
        assert_eq!(super::reply_idem(7064, 643), "forum-reply:7064:643");
    }
}
