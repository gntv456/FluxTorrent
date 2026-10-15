//! `site_settings` 单键读的唯一居所（FRAME §2：SQL 只住 repo 层）。
//!
//! 此前同一个「按 name 读一列 text」的查询在仓里有 **5 份拷贝**：
//! `repo/reg_quiz.rs::read_setting`、`repo/register_gates.rs::setting`
//! （后者无调用方，是纯死重复），以及 H 层的
//! `torrent_http/magnet.rs`、`publish_http/download.rs` 各自内嵌的
//! 局部 `setting()`。五份逐字相同，只有「trim + 去掉尾斜杠」这层
//! 后处理因调用方而异——那层留在 H 层，这里只管取原值。
//!
//! 语义（与全部旧拷贝一致，搬动时不得改）：缺行 / 查询失败都返回
//! `None`，由调用方自带默认值。站点设置读不到不该让列表或下载 500。

use super::Repo;

/// 按 name 读 `site_settings.value`（原样返回，不 trim）。
pub async fn read(repo: &Repo, name: &str) -> Option<String> {
    sqlx::query_scalar("SELECT value FROM site_settings WHERE name = $1")
        .bind(name)
        .fetch_optional(&repo.db)
        .await
        .ok()
        .flatten()
}

/// `read` + trim + 去尾斜杠，空串视为无值。
///
/// tracker / announce 类基址的既有口径（`magnet.rs` 与 `download.rs`
/// 两处局部 `setting()` 都是这么做的）：首尾空白与结尾 `/` 都会让
/// 拼出来的 URL 多出双斜杠或让判等失效。
pub async fn read_base(repo: &Repo, name: &str) -> Option<String> {
    read(repo, name)
        .await
        .map(|v| v.trim().trim_end_matches('/').to_string())
        .filter(|v| !v.is_empty())
}
