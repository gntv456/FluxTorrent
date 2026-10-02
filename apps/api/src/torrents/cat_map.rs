//! 对外分类映射（0267）：内部 category_id → 名称 / NP 4xx 号 / Newznab 号。
//!
//! 第三方工具需要「可读的分类名」与「标准分类号」；内部自增 id（1..10）
//! 对它们没有意义（Torznab 客户端按 Newznab 号归位，NP 口径客户端按 4xx
//! 号归位）。映射数据落 `categories.legacy_id / newznab_id`，站长可在后台
//! 调整，代码只读不写死——避免「站长改了分类，对外还是老号」的漂移。
//!
//! 同文件另附 `MediaMap`（媒介名）：同样是小字典，同一次对外响应里一起用。

use sqlx::PgPool;
use std::collections::HashMap;

/// Newznab 兜底分类号（Other）：映射缺失时对外层必须仍有值可发。
pub const NEWZNAB_OTHER: i32 = 8000;

#[derive(Debug, Clone)]
pub struct CatInfo {
    pub name: String,
    /// NexusPHP 4xx 口径分类号
    pub legacy_id: i32,
    /// Newznab / Torznab 标准分类号
    pub newznab_id: i32,
}

#[derive(Debug, Default)]
pub struct CatMap(HashMap<i32, CatInfo>);

impl CatMap {
    /// 一次查表载入全部分类（分类是小表；对外端点每请求一次可接受，
    /// 未来若成热点可换 Redis 缓存）。
    pub async fn load(db: &PgPool) -> Self {
        let rows: Vec<(i32, String, Option<i32>, Option<i32>)> =
            sqlx::query_as(
                "SELECT id, name, legacy_id, newznab_id FROM categories",
            )
            .fetch_all(db)
            .await
            .unwrap_or_default();
        let mut m = HashMap::with_capacity(rows.len());
        for (id, name, legacy_id, newznab_id) in rows {
            m.insert(
                id,
                CatInfo {
                    name,
                    // 未配置时回落内部 id / Other，保证对外恒有值
                    legacy_id: legacy_id.unwrap_or(id),
                    newznab_id: newznab_id.unwrap_or(NEWZNAB_OTHER),
                },
            );
        }
        Self(m)
    }

    pub fn get(&self, id: i32) -> Option<&CatInfo> {
        self.0.get(&id)
    }

    pub fn newznab(&self, id: i32) -> i32 {
        self.get(id).map(|c| c.newznab_id).unwrap_or(NEWZNAB_OTHER)
    }

    pub fn legacy(&self, id: i32) -> i32 {
        self.get(id).map(|c| c.legacy_id).unwrap_or(id)
    }

    /// 展示名；分类已不存在（站长删过）时回落 "Other"，不给工具留空洞。
    pub fn name(&self, id: i32) -> String {
        self.get(id)
            .map(|c| c.name.clone())
            .unwrap_or_else(|| "Other".into())
    }

    /// 分类数 / 空表判断（自检与单测用；运行时不依赖）
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// 站点分类实际用到的 Newznab 号（去重升序）——Torznab caps 声明用。
    pub fn newznab_ids(&self) -> Vec<i32> {
        let mut v: Vec<i32> = self.0.values().map(|c| c.newznab_id).collect();
        v.sort_unstable();
        v.dedup();
        v
    }

    /// 反查：给定一组 Newznab 号，返回命中的内部 category_id 集合。
    /// 空入参 = 不筛（调用方自行判断）。
    pub fn internal_ids_for_newznab(&self, want: &[i32]) -> Vec<i32> {
        let mut v: Vec<i32> = self
            .0
            .iter()
            .filter(|(_, c)| want.contains(&c.newznab_id))
            .map(|(k, _)| *k)
            .collect();
        v.sort_unstable();
        v
    }
}

/// Newznab/Torznab 标准分类号 → 官方英文名（caps 与 item 都要用）。
/// 只枚举本项目映射会落到的号；其余一律视为 Other（宁可归类到 Other，
/// 也不要在 item 里发一个客户端读不懂的分类名）。
pub fn newznab_name(id: i32) -> &'static str {
    match id {
        2000 => "Movies",
        2010 => "Movies/Foreign",
        2040 => "Movies/HD",
        2045 => "Movies/UHD",
        3000 => "Audio",
        3030 => "Audio/MP3",
        3040 => "Audio/Lossless",
        4000 => "PC",
        4050 => "PC/Games",
        5000 => "TV",
        5030 => "TV/WEB-DL",
        5040 => "TV/HD",
        5060 => "TV/Sport",
        5070 => "TV/Anime",
        5080 => "TV/Documentary",
        5090 => "TV/Other",
        7000 => "Books",
        7020 => "Books/EBook",
        8000 => "Other",
        _ => "Other",
    }
}

/// 媒介字典（media 表，8 行左右的固定小字典）：id → 名称。
/// 第三方工具拿 medium_id 渲染不了标签，故对外层补 medium_name。
#[derive(Debug, Default)]
pub struct MediaMap(HashMap<i32, String>);

impl MediaMap {
    pub async fn load(db: &PgPool) -> Self {
        let rows: Vec<(i32, String)> =
            sqlx::query_as("SELECT id, name FROM media")
                .fetch_all(db)
                .await
                .unwrap_or_default();
        Self(rows.into_iter().collect())
    }

    /// 未命中（medium_id 为空或已删）→ None，调用方决定是否发 null。
    pub fn name(&self, id: i32) -> Option<&str> {
        self.0.get(&id).map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 回落口径：未配置的分类号不能变成 0/NULL —— 对外层必须恒有值
    #[test]
    fn fallbacks_are_stable() {
        let m = CatMap::default();
        assert_eq!(m.newznab(7), NEWZNAB_OTHER);
        assert_eq!(m.legacy(7), 7);
        assert_eq!(m.name(7), "Other");
        assert!(m.is_empty());
    }

    #[test]
    fn media_map_misses_are_none() {
        let m = MediaMap::default();
        assert!(m.name(1).is_none());
    }

    /// caps/item 的分类名必须有值：未知号回落 Other，绝不发空串
    #[test]
    fn newznab_names_are_total() {
        assert_eq!(newznab_name(2000), "Movies");
        assert_eq!(newznab_name(5070), "TV/Anime");
        assert_eq!(newznab_name(99999), "Other");
        assert_eq!(newznab_name(NEWZNAB_OTHER), "Other");
    }

    /// 反查与去重：caps 不能出现重复分类行
    #[test]
    fn newznab_ids_dedup() {
        let mut m = CatMap::default();
        for (id, nz) in [(1, 2000), (2, 2000), (3, 5000)] {
            m.0.insert(
                id,
                CatInfo {
                    name: format!("c{id}"),
                    legacy_id: 400 + id,
                    newznab_id: nz,
                },
            );
        }
        assert_eq!(m.newznab_ids(), vec![2000, 5000]);
        assert_eq!(m.internal_ids_for_newznab(&[5000]), vec![3]);
        assert!(m.internal_ids_for_newznab(&[9999]).is_empty());
    }
}
