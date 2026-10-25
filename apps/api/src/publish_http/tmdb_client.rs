//! TMDB 官方 v3 客户端（0358 直连路线的公共部分）。
//! ptgen_tmdb（发种 PT-Gen 抓取）与 work_meta（作品元数据缓存）共用：
//! key 读 site_settings.tmdb_api_key（空 = 未启用）、统一超时、
//! append_to_response 参数化（种子补全只要 external_ids，
//! 作品缓存还要 credits 折导演/主演）。

use crate::errors::{DomainError, DomainResult};
use crate::http::OUTBOUND_FETCH_TIMEOUT_SECS;
use crate::state::AppState;

/// TMDB details 原始 JSON（作品元数据缓存 work_meta 用）：zh-CN 一次，
/// 带 external_ids + credits（导演/主演从这里折）。
pub(super) async fn details_raw(
    state: &AppState,
    kind: &str,
    id: &str,
) -> DomainResult<serde_json::Value> {
    let key = tmdb_key(state).await?;
    let client = reqwest::Client::new();
    let secs = std::time::Duration::from_secs(
        crate::http::OUTBOUND_FETCH_TIMEOUT_SECS,
    );
    tmdb_timeout_get(
        &client,
        &key,
        kind,
        id,
        "zh-CN",
        secs,
        "external_ids,credits",
    )
    .await
}

/// site_settings.tmdb_api_key（空 = 未启用，报可操作文案）
pub(super) async fn tmdb_key(state: &AppState) -> DomainResult<String> {
    let key: String = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings WHERE name = \
         'tmdb_api_key'), '')",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or_default();
    if key.trim().is_empty() {
        return Err(DomainError::Validation(
            "TMDB 源未启用（未配置 tmdb_api_key，请在后台站点设定中填写\
             或改用其他元数据源）"
                .into(),
        ));
    }
    Ok(key.trim().to_string())
}

/// 单次 API 请求（zh-CN 与英文回退共用；404 单独给文案）。
async fn tmdb_get(
    client: &reqwest::Client,
    key: &str,
    kind: &str,
    id: &str,
    lang: &str,
    append: &str,
) -> DomainResult<serde_json::Value> {
    let url = format!(
        "https://api.themoviedb.org/3/{kind}/{id}?api_key={key}\
         &language={lang}&append_to_response={append}"
    );
    let resp = client.get(url).send().await.map_err(|e| {
        DomainError::Validation(format!(
            "TMDB 服务不可达（{e}），请稍后重试或手动填写简介"
        ))
    })?;
    if resp.status().as_u16() == 404 {
        return Err(DomainError::Validation(
            "TMDB 上没有该条目（404），请核对链接".into(),
        ));
    }
    if !resp.status().is_success() {
        return Err(DomainError::Validation(format!(
            "TMDB 接口异常（HTTP {}，常见原因是 api_key 无效）",
            resp.status().as_u16()
        )));
    }
    resp.json()
        .await
        .map_err(|e| DomainError::Internal(e.into()))
}

/// TMDB 直连抓取：GET /3/{type}/{id}（zh-CN，overview 空回退 en-US）。
/// 出口与 PT-Gen 同形 `(name, descr, poster)`；poster 无则 None。
#[allow(clippy::too_many_arguments)]
pub(super) async fn tmdb_timeout_get(
    client: &reqwest::Client,
    key: &str,
    kind: &str,
    id: &str,
    lang: &str,
    secs: std::time::Duration,
    append: &str,
) -> DomainResult<serde_json::Value> {
    let fut = tmdb_get(client, key, kind, id, lang, append);
    tokio::time::timeout(secs, fut).await.map_err(|_| {
        DomainError::Validation("TMDB 请求超时，请稍后重试".into())
    })?
}
