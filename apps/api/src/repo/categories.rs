//! 分类树展开（torrent_http/list.rs 与 works_filter.rs 共用口径下沉）：
//! 给定一组分类 id，递归带出全部子孙 id——列表/作品聚合的筛选语义是
//! 「选中父分类 = 含所有子分类」。H 层不再内联 SQL（分层宪法 ①）。

use sqlx::PgPool;

/// 展开结果含输入自身；输入空返回 None（= 不筛选）。
pub async fn expand_with_children(db: &PgPool, ids: &[i32]) -> Vec<i32> {
    if ids.is_empty() {
        return Vec::new();
    }
    let rows: Vec<i32> = sqlx::query_scalar(
        "WITH RECURSIVE sub AS ( \
         SELECT id FROM categories WHERE id = ANY($1) UNION \
         SELECT c.id FROM categories c JOIN sub s ON c.parent_id = s.id \
         ) SELECT id FROM sub",
    )
    .bind(ids)
    .fetch_all(db)
    .await
    .unwrap_or_default();
    let mut all = ids.to_vec();
    for r in rows {
        if !all.contains(&r) {
            all.push(r);
        }
    }
    all
}
