//! M15 论坛公共件（违禁词/通知/提及/标签）。
//! 从 community_http.rs 按域拆出。

use actix_web::{get, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

pub(crate) async fn forum_banned_words(db: &sqlx::PgPool) -> Vec<String> {
    let raw: Option<String> = sqlx::query_scalar(
        "SELECT value FROM site_settings WHERE name = 'forum_banned_words'",
    )
    .fetch_optional(db)
    .await
    .unwrap_or(None)
    .flatten();
    raw.unwrap_or_default()
        .split(['\n', '\r'])
        .map(str::trim)
        .filter(|s| s.len() >= 2)
        .map(str::to_string)
        .collect()
}

/// 命中检测：大小写不敏感（中文无大小写，天然不受影响）；返回第一个命中的词。
pub(crate) fn hit_banned_word<'a>(
    text: &str,
    words: &'a [String],
) -> Option<&'a str> {
    let lower = text.to_lowercase();
    words
        .iter()
        .find(|w| lower.contains(&w.to_lowercase()))
        .map(String::as_str)
}

/// 发主题 / 回帖 / 编辑共用的敏感词闸门（Phase3 治理：先挡再落库，不做「发后删」）
pub(crate) async fn check_banned_words(
    db: &sqlx::PgPool,
    text: &str,
) -> DomainResult<()> {
    let words = forum_banned_words(db).await;
    if words.is_empty() {
        return Ok(());
    }
    if let Some(w) = hit_banned_word(text, &words) {
        return Err(DomainError::Validation(format!(
            "内容包含敏感词「{w}」，请修改后重发"
        )));
    }
    Ok(())
}

pub(crate) async fn notify_user(
    db: &sqlx::PgPool,
    to: i64,
    subject: &str,
    body: &str,
) {
    if to <= 0 {
        return;
    }
    let _ = sqlx::query(
        "INSERT INTO messages (sender_id, receiver_id, subject, body) VALUES (NULL, $1, $2, $3)",
    )
    .bind(to)
    .bind(subject)
    .bind(body)
    .execute(db)
    .await;
}

/// 从 Markdown 正文里抽出 @提及 的用户名（与 forum-markdown.tsx 的 @ 正则同口径：
/// 字母/数字/下划线/连字符/中日韩汉字，2..=30 字符）。返回去重后的名字，最多 10 个。
pub(crate) fn extract_mentions(src: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0usize;
    while i < chars.len() {
        if chars[i] != '@' {
            i += 1;
            continue;
        }
        // @ 前一个字符若是字母/数字/汉字，说明是邮箱等场景，跳过
        if i > 0 {
            let prev = chars[i - 1];
            if prev.is_alphanumeric() || prev == '_' || prev == '-' {
                i += 1;
                continue;
            }
        }
        let mut j = i + 1;
        while j < chars.len() {
            let c = chars[j];
            if c.is_alphanumeric() || c == '_' || c == '-' {
                j += 1;
            } else {
                break;
            }
        }
        let name: String = chars[i + 1..j].iter().collect();
        let n = name.chars().count();
        if (2..=30).contains(&n)
            && !out.iter().any(|x| x.eq_ignore_ascii_case(&name))
        {
            out.push(name);
        }
        i = j.max(i + 1);
    }
    out.truncate(10);
    out
}

/// 给「关注了 target_type/target_id」的所有人发系统通知，`exclude` 用于排除自己/已单独通知过的人。
/// 关注者可能很多，故批量一次查出来再逐条插；单条插入失败不影响其余（notify_user 本身 best-effort）。
pub(crate) async fn notify_followers(
    db: &sqlx::PgPool,
    target_type: &str,
    target_id: i64,
    subject: &str,
    body: &str,
    exclude: &[i64],
) {
    let rows: Vec<i64> = sqlx::query_scalar(
        "SELECT user_id FROM follows WHERE target_type = $1 AND target_id = $2 LIMIT 500",
    )
    .bind(target_type)
    .bind(target_id)
    .fetch_all(db)
    .await
    .unwrap_or_default();
    for uid in rows {
        if exclude.contains(&uid) {
            continue;
        }
        notify_user(db, uid, subject, body).await;
    }
}

/// 解析 @提及 到真实用户（精确匹配用户名，避免「张小」误伤「张小三」）。
/// `exclude` 用于排除自己（自己 @ 自己不发通知）。
pub(crate) async fn notify_mentions(
    db: &sqlx::PgPool,
    text: &str,
    from: i64,
    topic_id: i64,
    topic_title: &str,
    where_txt: &str,
    exclude: &[i64],
) {
    let names = extract_mentions(text);
    if names.is_empty() {
        return;
    }
    let rows: Vec<(i64, String)> =
        sqlx::query_as("SELECT id, username FROM users WHERE username = ANY($1) AND status < 2")
            .bind(&names)
            .fetch_all(db)
            .await
            .unwrap_or_default();
    for (uid, _uname) in rows {
        if uid == from || exclude.contains(&uid) {
            continue;
        }
        // 幂等：同一人对同一主题的重复提及只提醒一次。
        // 判重锚定在链接尾部的 `)`，避免 topic/12 误匹配 topic/123；
        // 用私信存在性判重，避免为通知单独加表/加列。
        let dup: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM messages WHERE receiver_id = $1 AND sender_id IS NULL \
             AND subject = '论坛提及' AND body LIKE $2)",
        )
        .bind(uid)
        .bind(format!("%/forums/topic/{})%", topic_id))
        .fetch_one(db)
        .await
        .unwrap_or(false);
        if dup {
            continue;
        }
        notify_user(
            db,
            uid,
            "论坛提及",
            &format!(
                "用户 #{} 在{}提到了你：[{}](/forums/topic/{})",
                from, where_txt, topic_title, topic_id
            ),
        )
        .await;
    }
}

// ---- 论坛标签（0123）：词表复用 tag_dict（带样式的通用标签字典），关联表 topic_tags ----

#[derive(sqlx::FromRow, serde::Serialize)]
pub(crate) struct TagChipRow {
    id: i32,
    name: String,
    /// 官方标签（仅 kind=official；论坛打标签不区分权限，仅样式语义）
    kind: String,
    /// 0138：论坛域标签（scope=forum），种子域标签不再进论坛
    #[sqlx(default)]
    scope: String,
    /// 以下为 0063 的样式列，前端 TagChip 直接吃
    #[sqlx(default)]
    bg_color: String,
    #[sqlx(default)]
    color: String,
    #[sqlx(default)]
    font_size: String,
    #[sqlx(default)]
    margin: String,
    #[sqlx(default)]
    padding: String,
    #[sqlx(default)]
    border_radius: String,
}

/// 论坛标签字典（发帖选择器 + TagChip 渲染样式源）：只出启用中的论坛域标签（0138 scope=forum）。
/// 公开读——种子页/版块页 SSR 要在登录前渲染标签筛选，与 /tags-dict（种子口径）同级的公开词表。
#[get("/forums/tags")]
pub(crate) async fn forum_tags_dict(
    _req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let rows: Vec<TagChipRow> = sqlx::query_as(
        "SELECT id, name, kind, scope, bg_color, color, font_size, margin, padding, border_radius \
         FROM tag_dict WHERE COALESCE(enabled, TRUE) AND scope = 'forum' ORDER BY sort, id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::to_value(rows).unwrap_or_default()))
}

/// 发帖选的标签写入 topic_tags：只收启用中的论坛域字典 id（0138 scope=forum；
/// 禁用/已删/种子域 id 不写，防私插），去重后逐条插入（PK 幂等，量级 ≤5 无需批量）。
pub(crate) async fn attach_topic_tags(
    tx: &mut sqlx::PgTransaction<'_>,
    topic_id: i64,
    tag_ids: &[i32],
) -> Result<(), sqlx::Error> {
    let mut seen = std::collections::HashSet::new();
    for tid in tag_ids.iter().take(5) {
        if !seen.insert(*tid) {
            continue;
        }
        let ok: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM tag_dict \
              WHERE id = $1 AND COALESCE(enabled, TRUE) AND scope = 'forum')",
        )
        .bind(*tid)
        .fetch_one(&mut **tx)
        .await?;
        if !ok {
            continue;
        }
        sqlx::query(
            "INSERT INTO topic_tags (topic_id, tag_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
        )
        .bind(topic_id)
        .bind(*tid)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}
