//! 论坛视频内嵌的服务端计数口径（0189 语法 + 0190 每帖上限）。
//! 规则表本身的读写在 admin_http/embed_rules.rs，公开只读端点在
//! community_http/embed_rules.rs。

use sqlx::PgPool;

use crate::errors::{DomainError, DomainResult};

/// 前端 `forum-markdown.tsx::videoLine()` 的两条识别式同口径（P-54 实测）：
/// ① `!video(URL)` 独占一行（VIDEO_RE），② 整行恰为一个 http(s)/站内路径
/// URL（BARE_URL_RE，Onebox 式自动转换）。二者都要再过 SAFE_URL。
/// 只数 `!video(` 会让裸链接绕过 post_video_max（实测上限 3 却发出 10 个播放器）。
fn video_candidate_url(line: &str) -> Option<&str> {
    let t = line.trim();
    if let Some(inner) = t.strip_prefix("!video(") {
        let url = inner.strip_suffix(')')?;
        // 前端 VIDEO_RE 是 ^!video\(([^)\s]+)\)$：URL 里不许有 ')' 或空白，
        // 否则整行按普通文本渲染，不该计入视频数。
        if url.is_empty()
            || url.contains(')')
            || url.chars().any(char::is_whitespace)
        {
            return None;
        }
        // SAFE_URL：http(s)/mailto/站内绝对路径
        if !(url.starts_with("http://")
            || url.starts_with("https://")
            || url.starts_with("mailto:")
            || url.starts_with('/'))
        {
            return None;
        }
        return Some(url);
    }
    let bare = t.starts_with("http://")
        || t.starts_with("https://")
        || t.starts_with('/');
    if bare && !t.chars().any(char::is_whitespace) {
        return Some(t);
    }
    None
}

/// 正文里「可能被渲染成播放器」的候选 URL（逐行，代码围栏内不算——
/// 与 parseBlocks 先吃围栏、再判 videoLine 的顺序一致）。
fn video_candidate_urls(body: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut in_fence = false;
    for line in body.lines() {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        if let Some(u) = video_candidate_url(line) {
            out.push(u);
        }
    }
    out
}

/// 启用规则的 url_pattern，按前端 `new RegExp(p, "i")` 同口径编译；
/// 总开关 forum_video_embed=no 时前端拿到空规则表（一切降级为链接），
/// 这里同步返回空。编译失败的坏规则跳过（保存侧已拦，运行时兜底）。
async fn compiled_rule_patterns(
    db: &PgPool,
) -> DomainResult<Vec<regex::Regex>> {
    let embed_on: bool = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings WHERE name = \
         'forum_video_embed'), 'yes') = 'yes'",
    )
    .fetch_one(db)
    .await
    .unwrap_or(true);
    if !embed_on {
        return Ok(Vec::new());
    }
    let pats: Vec<String> = sqlx::query_scalar(
        "SELECT url_pattern FROM video_embed_rules WHERE enabled ORDER BY \
         sort, id",
    )
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(pats
        .iter()
        .filter_map(|p| {
            regex::RegexBuilder::new(p)
                .case_insensitive(true)
                .build()
                .ok()
        })
        .collect())
}

/// 真正会渲染成播放器的行数 = 候选 URL ∩ 命中白名单规则。
pub async fn video_block_count(db: &PgPool, body: &str) -> DomainResult<i64> {
    let cands = video_candidate_urls(body);
    if cands.is_empty() {
        return Ok(0);
    }
    let rules = compiled_rule_patterns(db).await?;
    Ok(cands
        .iter()
        .filter(|u| rules.iter().any(|re| re.is_match(u)))
        .count() as i64)
}

/// 每帖视频块上限（0190：post_video_max，0=不限）——发主题、回帖、
/// 改帖共用（改帖漏计是 P-54 的一半：先发一条再编辑堆满）。
pub async fn check_video_count(db: &PgPool, body: &str) -> DomainResult<()> {
    let max: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings WHERE name = \
         'post_video_max'), '3')::bigint",
    )
    .fetch_one(db)
    .await
    .unwrap_or(3);
    if max <= 0 {
        return Ok(());
    }
    let n = video_block_count(db, body).await?;
    if n > max {
        return Err(DomainError::Validation(format!("每帖最多 {max} 个视频")));
    }
    Ok(())
}
