//! 促销 freeleech 设置。
//! 从 staff_http.rs 按域拆出。

use crate::errors::{DomainError, DomainResult};

// ============ staffpanel 管理工具（hxpt faqmanage/modrules/catmanage/bans/massmail 口径） ============

/// 促销参数校验：kind/scope 合法性 + 起止时间解析（starts 缺省 now，ends 缺省 starts+hours）
pub fn promo_parse(
    kind_in: &str,
    scope_in: Option<&str>,
    hours: i32,
    starts_at: &Option<String>,
    ends_at: &Option<String>,
) -> DomainResult<(
    String,
    String,
    chrono::DateTime<chrono::Utc>,
    chrono::DateTime<chrono::Utc>,
)> {
    let kind = match kind_in {
        "free" | "x2" | "x2free" | "half" | "x2half" | "p30" => {
            kind_in.to_string()
        }
        _ => return Err(DomainError::Validation("促销类型无效".into())),
    };
    if ends_at.is_none() && !(1..=720).contains(&hours) {
        return Err(DomainError::Validation("时长需在 1-720 小时".into()));
    }
    let scope = scope_in.unwrap_or("global").to_string();
    match scope.as_str() {
        "global" | "official" | "non_official" | "category" => {}
        _ => return Err(DomainError::Validation("促销范围无效".into())),
    }
    let starts_at = match starts_at {
        Some(s) => chrono::DateTime::parse_from_rfc3339(s)
            .map_err(|_| DomainError::Validation("开始时间格式无效".into()))?
            .with_timezone(&chrono::Utc),
        None => chrono::Utc::now(),
    };
    let ends_at = match ends_at {
        Some(e) => chrono::DateTime::parse_from_rfc3339(e)
            .map_err(|_| DomainError::Validation("结束时间格式无效".into()))?
            .with_timezone(&chrono::Utc),
        None => starts_at + chrono::Duration::hours(hours as i64),
    };
    if ends_at <= starts_at {
        return Err(DomainError::Validation("结束时间需晚于开始时间".into()));
    }
    if (ends_at - starts_at) > chrono::Duration::hours(24 * 90) {
        return Err(DomainError::Validation("促销时长不可超过 90 天".into()));
    }
    Ok((kind, scope, starts_at, ends_at))
}
