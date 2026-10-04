//! 注册/邀请邮箱后缀策略（0282）：email_policy 三模式 × email_policy_list。
//! 从 register.rs 拆公共件——注册与邀请邮件发送两路同口径（发到不允许的
//! 邮箱等于白发邀请）。与 email_bans 分工：bans 是运营个案封禁，本策略是
//! 准入整类后缀门，两层都过才放行。

use actix_web::web;

use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

/// 校验邮箱后缀是否放行。读取 site_settings 两键：
///   email_policy      none / allow_list / deny_list（缺省 none）
///   email_policy_list 逗号分隔 @domain 清单
/// 返回 Err(Validation) 时 message 已是可直接给前端的中文。
pub(crate) async fn check_email_policy(
    state: &web::Data<std::sync::Arc<AppState>>,
    email: &str,
) -> DomainResult<()> {
    let kv: Vec<(String, String)> = sqlx::query_as(
        "SELECT name, value FROM site_settings WHERE name IN \
         ('email_policy', 'email_policy_list')",
    )
    .fetch_all(&state.repo.db)
    .await
    .unwrap_or_default();
    let kv: std::collections::HashMap<_, _> = kv.into_iter().collect();
    let policy = kv.get("email_policy").map(String::as_str).unwrap_or("none");
    if policy == "none" {
        return Ok(());
    }
    let list: Vec<String> = kv
        .get("email_policy_list")
        .map(|s| {
            s.split(',')
                .map(|p| p.trim().trim_start_matches('@').to_lowercase())
                .filter(|p| !p.is_empty())
                .collect()
        })
        .unwrap_or_default();
    if list.is_empty() {
        // 配了模式却没配清单：allow_list 会拦掉所有人，属配置错误——
        // 宁可放行也不误伤（管理端 hint 已写明配对使用）
        return Ok(());
    }
    let suffix = email.rsplit('@').next().unwrap_or("").to_lowercase();
    let hit = list.iter().any(|s| *s == suffix);
    let denied = match policy {
        "allow_list" => !hit,
        "deny_list" => hit,
        _ => false,
    };
    if denied {
        let mode = if policy == "allow_list" {
            "不在允许清单"
        } else {
            "已被站点禁止"
        };
        return Err(DomainError::Validation(format!(
            "该邮箱后缀{mode}，无法注册（如有疑问请联系管理组）"
        )));
    }
    Ok(())
}
