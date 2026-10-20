//! 开放 API 列表口的分页与计数（P-444）。
//!
//! 单独一个文件而不是塞在 `me.rs`：这一层是**四个口共用**的形状
//! （`/open/me/messages`、`/open/me/hr`、`/open/me/seeding`、
//! `/open/me/history`），写在其中一家里另三家就得反向 import。

use actix_web::web;
use std::collections::HashMap;
use std::sync::Arc;

use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::me::MAX_ROWS;

/// `limit`（缺省 50，夹到 [1, MAX_ROWS]）与 `offset`（缺省 0，负数夹到 0）。
///
/// 这四个口此前**只认 limit**：客户端按网页端同名的 `offset`/`page` 传参
/// 不会报错、也不会有 400——拿回的永远是第一页。现网实测：root 有 153 封信，
/// `?limit=100&offset=100` 与 `?limit=100` 返回**完全相同的 100 条**，
/// 后 53 条对客户端永久不可达，而响应里没有任何字段说明「被截断了」。
pub(super) fn page_params(q: &HashMap<String, String>) -> (i64, i64) {
    let limit = q
        .get("limit")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(50)
        .clamp(1, MAX_ROWS);
    let offset = q
        .get("offset")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(0)
        .max(0);
    (limit, offset)
}

/// 这一页之后还有没有。客户端不必拿 `count == limit` 去猜是不是截断。
pub(super) fn has_more(offset: i64, got: i64, total: i64) -> bool {
    offset + got < total
}

/// `total` 的取数：`frag` 必须是**列表那一条的 FROM/WHERE**（`$1` = uid），
/// 两处共用同一个变量，否则总数与能翻出来的行数不是一回事——同仓
/// `/me/torrentlist` 的注释早就写着这条。
pub(super) async fn count_with(
    state: &web::Data<Arc<AppState>>,
    uid: i64,
    frag: &str,
) -> DomainResult<i64> {
    let sql = format!("SELECT count(*) FROM {}", frag);
    Ok(sqlx::query_scalar::<_, i64>(&sql)
        .bind(uid)
        .fetch_one(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?)
}

#[cfg(test)]
mod tests {
    use super::{has_more, page_params};
    use std::collections::HashMap;

    fn q(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    /// 缺省、夹界、脏值：`limit` 越界是夹（不 400），`offset` 负数当 0。
    #[test]
    fn page_params_clamps() {
        assert_eq!(page_params(&q(&[])), (50, 0), "两个都不给");
        assert_eq!(page_params(&q(&[("limit", "200"), ("offset", "100")])),
                   (100, 100), "limit 夹到上限、offset 生效");
        assert_eq!(page_params(&q(&[("limit", "0")])), (1, 0), "0 夹到 1");
        assert_eq!(page_params(&q(&[("offset", "-5")])), (50, 0),
                   "负 offset 不往前翻");
        assert_eq!(page_params(&q(&[("limit", "abc"), ("offset", "x")])),
                   (50, 0), "脏值回落缺省，不 400 打断工具");
    }

    /// `has_more` 只说「后面还有没有」，不负责说总数。
    #[test]
    fn has_more_is_about_the_rest() {
        assert!(has_more(0, 100, 153), "第一页后面还有 53 条");
        assert!(!has_more(100, 53, 153), "最后一页");
        assert!(!has_more(0, 1, 1), "一条都没有截断");
        assert!(!has_more(500, 0, 153), "翻过头给空页也不谎报还有");
    }
}
