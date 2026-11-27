//! 公开字典缓存原语（加速批五期 2026-11-26，从 section_public.rs 拆出守行数门）。
//!
//! `/section-dict` 每请求 10+ 条 SQL（逐维度字典 + 模式聚合）、`/tags-dict`
//! 每请求 1 条——两者是 browse/上传/筛选的公共依赖，匿名可读、内容只随
//! 管理端写口变化。Redis 60s 短 TTL（site-profile 是 300s：那边的写口
//! 集中且已全挂失效；本域写口分散在 4 个文件 12 个 handler，短 TTL 把
//! 漏挂失效点的影响窗压到一分钟内）。fail-open：Redis 挂了直落 DB 原路。
//!
//! 失效用 **generation 号**而非 SCAN：写口 INCR 一颗世代键，公开读把世代
//! 拼进缓存键——失效即旧键全体变冷（60s TTL 过期回收），无需扫描，
//! SCAN 失败时的部分删除问题不存在。世代读失败按 0 处理（fail-open：
//! 只是缓存键退化成固定键，TTL 兜底正确性）。

use crate::state::AppState;
use redis::AsyncCommands;

pub(super) const SECTION_DICT_CACHE_PREFIX: &str = "cache:section-dict:";
pub(super) const TAGS_DICT_CACHE_PREFIX: &str = "cache:tags-dict:";
const SECTION_DICT_GEN_KEY: &str = "cache:section-dict:gen";
pub(super) const SECTION_DICT_TTL: u64 = 60;

/// 写失效：本域任何字典/维度/模式写口调用（INCR 世代键，fail-open）。
pub(super) async fn invalidate_section_dict_cache(state: &AppState) {
    let mut c = state.redis.clone();
    let _: Result<(), _> =
        AsyncCommands::incr(&mut c, SECTION_DICT_GEN_KEY, 1).await;
}

/// 读当前世代（fail-open：失败按 0，缓存键退化固定键，TTL 兜底正确性）。
pub(super) async fn section_dict_generation(
    c: &mut redis::aio::ConnectionManager,
) -> i64 {
    AsyncCommands::get(c, SECTION_DICT_GEN_KEY)
        .await
        .unwrap_or(0i64)
}
