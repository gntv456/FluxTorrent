//! 火花余额累加的溢出护栏（批一百五十三，与 api 侧 economy_http::spend::checked_balance 同源）。
//!
//! `balance_after` 这类「先在 Rust 里算出新余额、再当参数绑给 SQL」的写法，
//! 在 release（无 `overflow-checks`）下**静默回绕**：`i64::MAX + 300` 变成
//! 一个大负数，账本里于是写着「余额 -9223372036854775309」；debug 下直接
//! `attempt to add with overflow` panic，把 worker 线程打掉。
//!
//! PG 自己有防护（bigint 溢出会报错并回滚），问题是我们在 Rust 侧提前
//! 就把和算完了——把一个该由数据库拒绝的业务错误变成了静默错账或 panic。
//! 统一改成 checked_add：越界即返回错误，本轮作业回滚、余额不动。
pub(crate) fn checked_balance(
    balance: i64,
    delta: i64,
) -> anyhow::Result<i64> {
    balance.checked_add(delta).ok_or_else(|| {
        tracing::error!(
            balance,
            delta, "余额累加溢出 bigint：拒绝入账（不静默回绕、不 panic）"
        );
        anyhow::anyhow!(
            "spark 余额越界：{} + {} 超出 int64 范围",
            balance,
            delta
        )
    })
}
