//! 论坛视频内嵌规则的入参结构与保存侧校验（0189 规则表）。
//! CRUD 处理器留在 embed_rules.rs（300 行门禁按域拆分）。
//!
//! 校验全部在保存时完成，坏配置不留到渲染层。

use serde::Deserialize;

use crate::errors::{DomainError, DomainResult};

#[derive(Deserialize)]
pub struct EmbedRuleBody {
    pub provider: String,
    pub name_zh: String,
    pub url_pattern: String,
    pub embed_template: String,
    pub embed_origin: String,
    #[serde(default = "default_iframe")]
    pub render_kind: String,
    #[serde(default = "default_aspect")]
    pub aspect: String,
    #[serde(default)]
    pub extra_params: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_sort")]
    pub sort: i32,
}
pub fn default_iframe() -> String {
    "iframe".into()
}
pub fn default_aspect() -> String {
    "16:9".into()
}
pub fn default_true() -> bool {
    true
}
pub fn default_sort() -> i32 {
    100
}

/// url_pattern 必须以 ^https?:// + 域名字面量开头（或站内 ^/api/v1/ 锚定），
/// 杜绝开放正则匹配任意 URL 的情况。
pub fn anchored_domain(pattern: &str) -> bool {
    let rest = pattern
        .strip_prefix("^https?://")
        .or_else(|| pattern.strip_prefix("^https://"))
        .or_else(|| pattern.strip_prefix("^http://"));
    let Some(rest) = rest else {
        return pattern.starts_with("^/api/v1/");
    };
    // 域名字面量段：字母数字点连字符，允许 (www\.)? 形态的少量转义
    let head: String = rest
        .chars()
        .take_while(|c| {
            c.is_ascii_alphanumeric()
                || matches!(c, '.' | '-' | '\\' | '(' | ')')
        })
        .collect();
    !head.is_empty() && head.chars().any(|c| c.is_ascii_alphanumeric())
}

pub fn validate_body(b: &EmbedRuleBody) -> DomainResult<()> {
    if b.provider.trim().is_empty() || b.provider.len() > 40 {
        return Err(DomainError::Validation("provider 需 1-40 字符".into()));
    }
    if b.name_zh.trim().is_empty() || b.name_zh.chars().count() > 60 {
        return Err(DomainError::Validation("展示名需 1-60 字符".into()));
    }
    if !matches!(b.render_kind.as_str(), "iframe" | "video") {
        return Err(DomainError::Validation(
            "render_kind 只允许 iframe/video".into(),
        ));
    }
    if !matches!(b.aspect.as_str(), "16:9" | "4:3" | "1:1") {
        return Err(DomainError::Validation(
            "aspect 只允许 16:9/4:3/1:1".into(),
        ));
    }
    if b.url_pattern.len() > 500 || b.embed_template.len() > 1000 {
        return Err(DomainError::Validation("正则/模板超长".into()));
    }
    if !anchored_domain(&b.url_pattern) {
        return Err(DomainError::Validation(
            "url_pattern 须 ^https?:// 域名字面量锚定（或 ^/api/v1/ 站内）"
                .into(),
        ));
    }
    // P-55（批二十八实测）：连续两个反斜杠在正则里是「一个字面反斜杠 + 任意
    // 字符」，而 URL 永不含反斜杠 ⇒ 语法能编译、语义永不命中。0189 的四条内置
    // 规则就是这么静默死掉的（数据侧修复见迁移 0345），这里按形态直接拦。
    if b.url_pattern.contains(r"\\") {
        return Err(DomainError::Validation(
            "url_pattern 有连续两个反斜杠：正则转义只需一个，URL 中不会出现 \
             反斜杠，这样的规则永不命中"
                .into(),
        ));
    }
    // 正则可编译 + 捕获组清点（渲染侧换行不敏感，统一多行模式编译）
    let re = regex::Regex::new(&b.url_pattern).map_err(|e| {
        DomainError::Validation(format!("url_pattern 正则无效: {e}"))
    })?;
    // ReDoS 兜底：限长 + 禁嵌套量词的保守检查（catastrophic 回溯常见形态）
    if b.url_pattern.contains("(.*)") || b.url_pattern.contains("(.+)") {
        return Err(DomainError::Validation(
            "url_pattern 不允许 (.*)/(.+) 开放捕获".into(),
        ));
    }
    // 模板 $n 引用必须 ≤ 捕获组数；生成物必须落在 embed_origin 前缀内
    let groups = re.captures_len() - 1;
    for i in 1..=9 {
        if b.embed_template.contains(&format!("${i}")) && i > groups {
            return Err(DomainError::Validation(format!(
                "模板 ${i} 超出捕获组数（{groups}）"
            )));
        }
    }
    if !b.embed_template.starts_with(&b.embed_origin) {
        return Err(DomainError::Validation(
            "embed_template 必须以 embed_origin 开头".into(),
        ));
    }
    if let Some(extra) = &b.extra_params {
        if extra.len() > 200 || extra.contains(' ') {
            return Err(DomainError::Validation(
                "extra_params 需 ≤200 字符且不含空格".into(),
            ));
        }
    }
    Ok(())
}
