//! 首页公告渲染（从 home.rs 拆出，300 门禁）：公告 JSON 组装 + 视频白名单消毒。
//!
//! 公告 body 为管理员撰写的富文本 HTML（NP 口径）。0190 起支持视频内嵌：
//! ammonia 默认白名单放行 `iframe`/`video`/`source`，但 src/poster 经
//! attribute_filter 按 `video_embed_rules.enabled` 的 embed_origin 集合校验
//! —— 绝对 URL 比对 authority（防 `player.bilibili.com@evil.com` 前缀逃逸），
//! 站内相对路径比对 origin 前缀。规则集为空时 src 一律剥除（fail-closed）。
//! 其余标签维持 ammonia 默认消毒（剥 script/事件属性/javascript:）。

use crate::errors::{DomainError, DomainResult};
use std::borrow::Cow;

/// enabled 规则的 embed_origin 集合（含站内相对前缀 /api/v1/attachments/）。
async fn video_embed_origins(db: &sqlx::PgPool) -> Vec<String> {
    sqlx::query_as::<_, (String,)>(
        "SELECT embed_origin FROM video_embed_rules WHERE enabled",
    )
    .fetch_all(db)
    .await
    .map(|v| v.into_iter().map(|(o,)| o).collect())
    .unwrap_or_default()
}

/// src/poster 域校验：绝对 URL 比对 authority（整段 host，防子域/伪装前缀
/// 逃逸），站内相对路径比对以 / 开头的 origin 前缀。
pub(super) fn embed_src_allowed(src: &str, origins: &[String]) -> bool {
    let s = src.trim();
    if s.is_empty() {
        return false;
    }
    if let Some(rest) = s.strip_prefix('/') {
        let abs = format!("/{rest}");
        return origins
            .iter()
            .any(|o| o.starts_with('/') && abs.starts_with(o.as_str()));
    }
    let Some((scheme, rest)) = s.split_once("://") else {
        return false;
    };
    if !scheme.eq_ignore_ascii_case("https")
        && !scheme.eq_ignore_ascii_case("http")
    {
        return false;
    }
    let authority = rest
        .split('/')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    origins.iter().any(|o| {
        let Some((os, or)) = o.split_once("://") else {
            return false;
        };
        os.eq_ignore_ascii_case(scheme)
            && or.split('/').next().unwrap_or_default() == authority
    })
}

/// 公告消毒：默认白名单 + iframe/video/source，src/poster 过 embed_origin。
pub(super) fn announcement_clean(body: &str, origins: &[String]) -> String {
    let mut b = ammonia::Builder::default();
    b.add_tags(["iframe", "video", "source"]);
    b.add_tag_attributes(
        "iframe",
        [
            "src",
            "allow",
            "allowfullscreen",
            "sandbox",
            "loading",
            "referrerpolicy",
            "scrolling",
            "frameborder",
            "width",
            "height",
        ],
    );
    b.add_tag_attributes(
        "video",
        [
            "src",
            "controls",
            "poster",
            "preload",
            "playsinline",
            "muted",
            "loop",
            "width",
            "height",
        ],
    );
    b.add_tag_attributes("source", ["src", "type"]);
    let origins = origins.to_vec();
    b.attribute_filter(move |tag, attr, val| {
        let is_url = match tag {
            "iframe" | "source" => attr == "src",
            "video" => attr == "src" || attr == "poster",
            _ => false,
        };
        if is_url && !embed_src_allowed(val, &origins) {
            None // 剥属性：标签壳保留但加载不了任何外域内容
        } else {
            Some(Cow::Borrowed(val))
        }
    });
    b.clean(body).to_string()
}

/// 公告（home-news）：最新一条为头条 + 其余为列表；返回 (news_json, 最新 id)。
/// 出站前 ammonia 白名单消毒——管理员账号被盗也不构成全站存储 XSS。
pub(super) async fn home_news_json(
    db: &sqlx::PgPool,
) -> DomainResult<(Vec<serde_json::Value>, i32)> {
    #[rustfmt::skip]
    let news: Vec<(
        i32, String, String, String, chrono::DateTime<chrono::Utc>,
    )> = sqlx::query_as(
        "SELECT id, title, body, badge, \
         created_at FROM announcements ORDER BY id DESC LIMIT 8",
    )
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let latest = news.first().map(|(id, _, _, _, _)| *id).unwrap_or(0);
    let origins = video_embed_origins(db).await;
    let news_json = news
        .iter()
        .map(|(id, title, body, badge, ts)| {
            serde_json::json!({
                "id": id, "title": title,
                "body": announcement_clean(body, &origins),
                "badge": badge,
                "date": ts.format("%m-%d").to_string(),
            })
        })
        .collect();
    Ok((news_json, latest))
}
