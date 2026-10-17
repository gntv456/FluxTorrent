//! 火花（魔力）流水的 SQL 居所（GET /me/spark/ledger）。
//!
//! 2026-10-17 从 `economy_http/voucher_use.rs` 下沉：那个文件压在 300 行
//! 门禁门口（299/300）且 sqlx 计数已超基线，任何扩展都得先把 SQL 搬走。
//! 搬完后 `voucher_use.rs` 只留券核销与做种收益估算。
//!
//! 口径要点：
//! * `spark_ledger.kind` 是**自由文本**（无 CHECK、无枚举），写入点 12 处
//!   约 55 个取值；中文名在前端 `lib/spark-kind-labels.ts` 收口，
//!   这里不做映射（R 层不碰展示）。
//! * 「摘要」列：`kind='shop'` 的行 JOIN 出商品名；其余行退到
//!   `ref_type`/`ref_id`（表里本来就有，旧查询没投影，这里补上）。
//! * 列表与计数共用同一段 WHERE 常量：可选筛选用「参数为 NULL 即不筛」
//!   的写法（与 `community_http/feed.rs` 的 `$n::timestamptz IS NULL`
//!   同形），这样参数位次固定为 $1..$7，两处不可能漂移。

use super::Repo;
use crate::errors::{DomainError, DomainResult};

/// 一行的读侧视图（字段名即 JSON 名）。
#[derive(sqlx::FromRow, serde::Serialize)]
pub struct LedgerRow {
    pub amount: i64,
    pub kind: String,
    pub balance_after: Option<i64>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// 消费对象名（仅商城流水 JOIN 得到）
    pub item_name: Option<String>,
    /// 关联对象类型/主键（任务、银行、悬赏等行的「摘要」回落）
    pub ref_type: Option<String>,
    pub ref_id: Option<i64>,
}

/// 筛选条件。`dir` 只认 "in"/"out"，其余值按不筛处理。
pub struct Filter<'a> {
    pub kind: Option<&'a str>,
    pub dir: Option<&'a str>,
    pub from: Option<chrono::DateTime<chrono::Utc>>,
    pub to: Option<chrono::DateTime<chrono::Utc>>,
}

impl<'a> Default for Filter<'a> {
    fn default() -> Self {
        Filter {
            kind: None,
            dir: None,
            from: None,
            to: None,
        }
    }
}

/// 方向谓词只接受两个白名单值，别的（含拼错的）一律不筛——
/// 宁可多给几行，也不要「筛了个空」演成「你没有这类流水」。
const WHERE: &str = " WHERE l.user_id = $1 \
    AND ($2::text IS NULL OR l.kind = $2) \
    AND ($3::text IS NULL \
         OR ($3 = 'in' AND l.amount > 0) \
         OR ($3 = 'out' AND l.amount < 0)) \
    AND ($4::timestamptz IS NULL OR l.created_at >= $4) \
    AND ($5::timestamptz IS NULL OR l.created_at < $5)";

const FROM: &str = " FROM spark_ledger l \
    LEFT JOIN shop_orders o \
      ON o.user_id = l.user_id AND o.idempotency_key = l.idempotency_key \
    LEFT JOIN shop_items si ON si.id = o.item_id";

pub async fn ledger(
    repo: &Repo,
    uid: i64,
    f: &Filter<'_>,
    limit: i64,
    offset: i64,
) -> DomainResult<Vec<LedgerRow>> {
    let sql = format!(
        "SELECT l.amount, l.kind, l.balance_after, l.created_at, \
            si.name AS item_name, l.ref_type, l.ref_id{FROM}{WHERE} \
         ORDER BY l.id DESC LIMIT $6 OFFSET $7"
    );
    // 绑定顺序 = WHERE 里 $1..$5 的出现顺序，与 count_ledger 逐字一致
    let rows = sqlx::query_as::<_, LedgerRow>(&sql)
        .bind(uid)
        .bind(f.kind)
        .bind(f.dir)
        .bind(f.from)
        .bind(f.to)
        .bind(limit)
        .bind(offset)
        .fetch_all(&repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(rows)
}

pub async fn count_ledger(
    repo: &Repo,
    uid: i64,
    f: &Filter<'_>,
) -> DomainResult<i64> {
    let sql = format!("SELECT count(*){FROM}{WHERE}");
    let n = sqlx::query_scalar::<_, i64>(&sql)
        .bind(uid)
        .bind(f.kind)
        .bind(f.dir)
        .bind(f.from)
        .bind(f.to)
        .fetch_one(&repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(n)
}
