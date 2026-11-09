//! 标签与官种迁移（从 np_import.rs 按 300 行门禁拆出）。

use anyhow::Result;
use sqlx::{MySqlPool, PgPool};

/// 标签迁移：NP torrent_tags × tags 的名字 → 新站 tag_dict 同名标签。
/// 按 (info_hash, tag 名) 关联（迁移器无 torrent id 映射表，info_hash 唯一可靠）。
/// 只迁新站 tag_dict 里**已存在同名**的标签（官方/首发/禁转/中字… 等），
/// 新站没有的（如学科标签）跳过。迁完调 refresh_official_tag 全量重算官种。
pub async fn import_tags(np: &MySqlPool, flux: &PgPool) -> Result<()> {
    // NP：info_hash(40hex 小写) + tag 名
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT LOWER(HEX(t.info_hash)), tg.name \
         FROM torrent_tags tt \
         JOIN torrents t ON t.id = tt.torrent_id \
         JOIN tags tg ON tg.id = tt.tag_id",
    )
    .fetch_all(np)
    .await?;
    let total = rows.len();
    // 新站 tag_dict：name → id（同名匹配）
    let dict: Vec<(String, i64)> =
        sqlx::query_as("SELECT name, id FROM tag_dict")
            .fetch_all(flux)
            .await?;
    let by_name: std::collections::HashMap<String, i64> =
        dict.into_iter().collect();
    let mut linked = 0u64;
    let mut unmapped = 0u64;
    for (ih, tag_name) in rows {
        let Some(&tag_id) = by_name.get(&tag_name) else {
            unmapped += 1;
            continue;
        };
        // 落一条标签关联（种子按 info_hash 定位；不存在/已有关联都安全跳过）
        let n = sqlx::query(
            "INSERT INTO tags (torrent_id, tag_id) \
             SELECT t.id, $2 FROM torrents t WHERE lower(t.info_hash) = $1 \
             ON CONFLICT (torrent_id, tag_id) DO NOTHING",
        )
        .bind(&ih)
        .bind(tag_id)
        .execute(flux)
        .await?
        .rows_affected();
        linked += n;
    }
    // 官种物化列重算（官方标签已落库 ⇒ official_tag 随之变 true）
    let refreshed: i32 =
        sqlx::query_scalar("SELECT refresh_official_tag(NULL)")
            .fetch_one(flux)
            .await?;
    println!(
        "tags: 源关联 {total} / 新关联 {linked} / 无同名标签跳过 {unmapped}；\
         official_tag 重算扫描 {refreshed} 行"
    );
    Ok(())
}
