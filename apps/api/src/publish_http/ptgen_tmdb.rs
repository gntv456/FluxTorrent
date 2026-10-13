//! TMDB 第五源适配（0358）：themoviedb.org 链接 → 官方 v3 API 直连。
//! 从 ptgen.rs 拆出（300 行门禁）；fetch_meta 按 host 分流到这里。
//! 直连路线（不走 WASM/PT-Gen 上游）：TMDB 一发拿全标题/简介/海报，
//! key 走 site_settings.tmdb_api_key（空 = 禁用，报可操作文案）。

use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

/// 从 themoviedb.org URL 提取 `(类型, id)`。
/// 只认条目页 `/movie/{id}` 或 `/tv/{id}`（含语言前缀 `/zh-CN/tv/…`）；
/// 搜索页 / 人物页不支持（fetch 需要 id 定位，搜索页给不了）。
fn parse_tmdb_url(url: &str) -> DomainResult<(&'static str, String)> {
    let parsed = url::Url::parse(url.trim())
        .map_err(|_| DomainError::Validation("链接无效".into()))?;
    let segs: Vec<&str> =
        parsed.path().split('/').filter(|s| !s.is_empty()).collect();
    // 语言前缀（/zh-CN/tv/12345）：首段含 '-' 且非 movie/tv 即剥掉
    let segs = if segs.len() >= 3
        && segs[0].contains('-')
        && segs[0] != "movie"
        && segs[0] != "tv"
    {
        &segs[1..]
    } else {
        &segs[..]
    };
    let kind = match segs.first().copied() {
        Some("movie") => "movie",
        Some("tv") => "tv",
        _ => {
            return Err(DomainError::Validation(
                "只支持 TMDB 条目链接（/movie/{id} 或 /tv/{id}），\
                 搜索页/人物页请先进入条目再复制"
                    .into(),
            ));
        }
    };
    let id = segs
        .get(1)
        .copied()
        .filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
        .ok_or_else(|| {
            DomainError::Validation("TMDB 链接里没有条目 id".into())
        })?;
    Ok((kind, id.to_string()))
}

/// 折叠标题：`title || name`，年拼进 name（`(2024)` 形态，无年不拼）。
fn fold_name(
    title: Option<&str>,
    name: Option<&str>,
    year: Option<i64>,
) -> Option<String> {
    let base = title
        .filter(|s| !s.is_empty())
        .or_else(|| name.filter(|s| !s.is_empty()))?;
    Some(match year {
        Some(y) => format!("{base} ({y})"),
        None => base.to_string(),
    })
}

/// 单次 API 请求（zh-CN 与英文回退共用；404 单独给文案）。
async fn tmdb_get(
    client: &reqwest::Client,
    key: &str,
    kind: &str,
    id: &str,
    lang: &str,
) -> DomainResult<serde_json::Value> {
    let url = format!(
        "https://api.themoviedb.org/3/{kind}/{id}?api_key={key}\
         &language={lang}&append_to_response=external_ids"
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
pub(super) async fn fetch_tmdb(
    state: &AppState,
    url: &str,
) -> DomainResult<(String, String, Option<String>)> {
    let (kind, id) = parse_tmdb_url(url)?;
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
    let client = reqwest::Client::new();
    let secs = std::time::Duration::from_secs(
        crate::http::OUTBOUND_FETCH_TIMEOUT_SECS,
    );
    let zh =
        tmdb_timeout_get(&client, key.trim(), kind, &id, "zh-CN", secs).await?;
    let zh_empty = zh
        .get("overview")
        .and_then(|x| x.as_str())
        .map(|s| s.trim().is_empty())
        .unwrap_or(true);
    let en = if zh_empty {
        tmdb_timeout_get(&client, key.trim(), kind, &id, "en-US", secs)
            .await
            .ok()
            .and_then(|v2| {
                v2.get("overview")
                    .and_then(|x| x.as_str())
                    .map(String::from)
            })
    } else {
        None
    };
    fold_tmdb_json(&zh, en.as_deref()).ok_or_else(|| {
        DomainError::Validation(
            "TMDB 返回缺标题或简介（无中文也无英文 overview），\
             请手动填写"
                .into(),
        )
    })
}

/// tmdb_get + 统一超时（对齐 fetch_meta 的 OUTBOUND_FETCH_TIMEOUT_SECS）。
async fn tmdb_timeout_get(
    client: &reqwest::Client,
    key: &str,
    kind: &str,
    id: &str,
    lang: &str,
    secs: std::time::Duration,
) -> DomainResult<serde_json::Value> {
    let fut = tmdb_get(client, key, kind, id, lang);
    tokio::time::timeout(secs, fut).await.map_err(|_| {
        DomainError::Validation("TMDB 请求超时，请稍后重试".into())
    })?
}

/// 纯函数折三元组（单元测试覆盖回退链；IO 与折解耦）。
/// `en_overview`：zh-CN 简介为空时的英文回退（由调用方二次请求拿到）。
fn fold_tmdb_json(
    v: &serde_json::Value,
    en_overview: Option<&str>,
) -> Option<(String, String, Option<String>)> {
    let str_of =
        |k: &str| v.get(k).and_then(|x| x.as_str()).filter(|s| !s.is_empty());
    let year = v
        .get("release_date")
        .or_else(|| v.get("first_air_date"))
        .and_then(|d| d.as_str())
        .and_then(|d| d.get(0..4))
        .and_then(|y| y.parse::<i64>().ok());
    let name = fold_name(str_of("title"), str_of("name"), year)?;
    // overview：zh-CN 空 → 英文回退；仍空则整体报错（上层折 Validation）
    let descr = str_of("overview")
        .or_else(|| en_overview.filter(|s| !s.trim().is_empty()))?
        .to_string();
    let poster = str_of("poster_path")
        .map(|p| format!("https://image.tmdb.org/t/p/w500{p}"));
    Some((name, descr, poster))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn case(kind: &str, id: &str) -> DomainResult<(String, String)> {
        parse_tmdb_url(&format!("https://www.themoviedb.org/{kind}/{id}"))
            .map(|(k, i)| (k.to_string(), i))
    }

    #[test]
    fn parse_movie_tv_and_lang_prefix() {
        assert_eq!(
            case("movie", "693134").unwrap(),
            ("movie".into(), "693134".into())
        );
        assert_eq!(case("tv", "94605").unwrap(), ("tv".into(), "94605".into()));
        // 语言前缀形态（站内切换语言后的 URL）
        let (k, i) = parse_tmdb_url(
            "https://www.themoviedb.org/zh-CN/tv/94605/season/1",
        )
        .unwrap();
        assert_eq!((k, i.as_str()), ("tv", "94605"));
    }

    #[test]
    fn parse_rejects_search_person_and_bad_id() {
        // 搜索页
        assert!(
            parse_tmdb_url("https://www.themoviedb.org/search?query=dune")
                .is_err()
        );
        // 人物页
        assert!(
            parse_tmdb_url("https://www.themoviedb.org/person/500").is_err()
        );
        // 非数字 / 缺失 id
        assert!(case("movie", "abc").is_err());
        assert!(parse_tmdb_url("https://www.themoviedb.org/movie/").is_err());
    }

    #[test]
    fn fold_movie_prefers_title_and_appends_year() {
        let v = serde_json::json!({
            "title": "Dune: Part Two",
            "name": "沙丘2",
            "release_date": "2024-02-27",
            "overview": "保罗·厄崔迪崛起",
            "poster_path": "/xOMo8BRK7PfcJv9JCnx7s5hj0PX.jpg"
        });
        let (n, d, p) = fold_tmdb_json(&v, None).unwrap();
        assert_eq!(n, "Dune: Part Two (2024)");
        assert_eq!(d, "保罗·厄崔迪崛起");
        assert_eq!(
            p.unwrap(),
            "https://image.tmdb.org/t/p/w500\
             /xOMo8BRK7PfcJv9JCnx7s5hj0PX.jpg"
        );
    }

    #[test]
    fn fold_tv_uses_name_and_first_air_date() {
        let v = serde_json::json!({
            "name": "Arcane",
            "first_air_date": "2021-11-06",
            "overview": "o",
            "poster_path": "/x.jpg"
        });
        assert_eq!(fold_tmdb_json(&v, None).unwrap().0, "Arcane (2021)");
    }

    #[test]
    fn fold_overview_falls_back_to_english() {
        // zh-CN 无简介 → 英文回退命中
        let v = serde_json::json!({
            "title": "Dune",
            "overview": "",
            "poster_path": "/x.jpg"
        });
        assert_eq!(
            fold_tmdb_json(&v, Some("A noble family...")).unwrap().1,
            "A noble family..."
        );
    }

    #[test]
    fn fold_fails_without_any_overview_and_allows_no_poster() {
        let no_ov = serde_json::json!({
            "title": "Dune",
            "poster_path": "/x.jpg"
        });
        assert!(fold_tmdb_json(&no_ov, None).is_none());
        assert!(fold_tmdb_json(&no_ov, Some("  ")).is_none());
        // 无海报不致命：poster 走 None（对齐 PT-Gen 三元组口径）
        let no_po = serde_json::json!({
            "title": "Dune",
            "overview": "o"
        });
        assert!(fold_tmdb_json(&no_po, None).unwrap().2.is_none());
    }
}
