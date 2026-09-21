//! M15 论坛权限（三层判定/防刷/文本净化）。
//! 从 community_http.rs 按域拆出。

use crate::errors::{DomainError, DomainResult};

// ============ M15 论坛（forums.php 三层权限口径） ============
//
// 三层判定叠加：
//   1) 版块三档门槛：minclassread 看 / minclasswrite 回 / minclasscreate 发主题（逐版块独立）
//   2) 版主：forum_mods（forumid+userid，无需等级，仅本版块有效）
//   3) 全局：postmanage = class≥90 全版块管帖；forummanage（≥93，见 admin_http）管版块
// 反直觉点照搬：作者不能删自己的帖（删帖只对版主/postmanage 开放）；
// 删主题收回发帖 +2 火花；管理员编辑他人帖自动 PM 通知并留 edited_by；
// forumpost=FALSE 账户级禁言；发帖 10 秒防刷（postmanage 豁免）；
// 受保护版块 2 楼起正文替换为提示（class≥90/发帖人本人/楼主/本版版主放行）。

/// 三层权限判定结果
#[derive(serde::Serialize, sqlx::FromRow, Clone, Copy)]
pub struct ForumPerm {
    pub can_read: bool,
    pub can_write: bool,
    pub can_create: bool,
    pub can_mod: bool,
}

pub async fn forum_access(
    db: &sqlx::PgPool,
    user_id: i64,
    class_id: i32,
    forum_id: i64,
) -> DomainResult<ForumPerm> {
    let none = ForumPerm {
        can_read: false,
        can_write: false,
        can_create: false,
        can_mod: false,
    };
    let row: Option<(i32, i32, i32)> = sqlx::query_as(
        "SELECT minclassread, minclasswrite, minclasscreate FROM forums WHERE id = $1",
    )
    .bind(forum_id)
    .fetch_optional(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((min_read, min_write, min_create)) = row else {
        return Ok(none);
    };
    // 全局 postmanage：全版块放行
    if class_id >= 90 {
        return Ok(ForumPerm {
            can_read: true,
            can_write: true,
            can_create: true,
            can_mod: true,
        });
    }
    // 版主：本版块全放行（任命不需要等级）
    let is_mod: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM forum_mods WHERE forum_id = $1 AND user_id = $2)",
    )
    .bind(forum_id)
    .bind(user_id)
    .fetch_one(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 账户级禁言（forumpost='no' 口径）：与等级、版块门槛无关，一律不能发帖回帖
    let can_post: bool = sqlx::query_scalar(
        "SELECT COALESCE(forumpost, TRUE) FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_one(db)
    .await
    .unwrap_or(true);
    let can_read = is_mod || class_id >= min_read;
    let can_write = can_read && can_post && (is_mod || class_id >= min_write);
    let can_create = can_write && (is_mod || class_id >= min_create);
    Ok(ForumPerm {
        can_read,
        can_write,
        can_create,
        can_mod: is_mod,
    })
}

/// 发帖 10 秒防刷（postmanage/版主豁免）
pub async fn forum_flood_check(
    db: &sqlx::PgPool,
    user_id: i64,
    class_id: i32,
) -> DomainResult<()> {
    if class_id >= 90 {
        return Ok(());
    }
    let last: Option<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar(
        "SELECT last_sent_at FROM forum_flood WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .flatten();
    if let Some(t) = last {
        if (chrono::Utc::now() - t).num_seconds() < 10 {
            return Err(DomainError::Validation(
                "发送过于频繁，请 10 秒后再试".into(),
            ));
        }
    }
    Ok(())
}

pub async fn forum_flood_mark(db: &sqlx::PgPool, user_id: i64) {
    let _ = sqlx::query(
        "INSERT INTO forum_flood (user_id, last_sent_at) VALUES ($1, now()) \
         ON CONFLICT (user_id) DO UPDATE SET last_sent_at = now()",
    )
    .bind(user_id)
    .execute(db)
    .await;
}

/// 把 Markdown 正文压成纯文本摘要（写入 posts.body_text，供搜索/列表预览）。
/// 不追求完美解析，够用即可：去代码围栏/标题/引用/列表符号 + 行内强调与链接语法。
pub fn strip_markdown(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut in_fence = false;
    for line in src.lines() {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue; // 代码块内容不进摘要，避免噪音
        }
        let mut l = t.trim_start_matches('#').trim_start();
        while let Some(rest) = l.strip_prefix('>') {
            l = rest.trim_start();
        }
        if let Some(rest) = l
            .strip_prefix("- ")
            .or_else(|| l.strip_prefix("* "))
            .or_else(|| l.strip_prefix("+ "))
        {
            l = rest;
        } else {
            let digits = l.chars().take_while(|c| c.is_ascii_digit()).count();
            if digits > 0 {
                let rest = &l[digits..];
                if let Some(r2) =
                    rest.strip_prefix(". ").or_else(|| rest.strip_prefix(") "))
                {
                    l = r2;
                }
            }
        }
        let l = l.trim();
        if l.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(l);
    }
    // 行内：![alt](url)/[text](url) 保留文字去 URL
    let mut s = strip_inline_links(&out);
    for pat in ["**", "__", "`", "~~", "*", "_"] {
        s = s.replace(pat, "");
    }
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn strip_inline_links(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0usize;
    while i < b.len() {
        if b[i] == b'!' && b.get(i + 1) == Some(&b'[') {
            i += 1; // 跳过 ! 保留 [ 交给下面分支
            continue;
        }
        if b[i] == b'[' {
            if let Some(close) = s[i + 1..].find(']') {
                let text = &s[i + 1..i + 1 + close];
                let after = i + 1 + close + 1;
                if b.get(after) == Some(&b'(') {
                    if let Some(pc) = s[after + 1..].find(')') {
                        out.push_str(text);
                        i = after + 1 + pc + 1;
                        continue;
                    }
                }
                out.push_str(text);
                i = after;
                continue;
            }
        }
        let ch = s[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}
