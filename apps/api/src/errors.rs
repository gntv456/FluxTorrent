//! 领域错误 → HTTP 状态码与 API 信封错误码的映射（§8.2）。
//! 错误码分段与 packages/domain-types 的 ErrorCode 保持一致。

use actix_web::{http::StatusCode, HttpResponse, ResponseError};
use serde_json::json;

#[allow(dead_code)]
#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    #[error("资源不存在")]
    NotFound(i64),
    #[error("未认证")]
    Unauthorized,
    #[error("无权限")]
    Forbidden,
    #[error("凭证无效")]
    InvalidCredentials,
    #[error("邀请码无效或已过期")]
    InviteInvalid,
    #[error("邀请码已被使用")]
    InviteUsed,
    #[error("用户名已被占用")]
    UsernameTaken,
    #[error("参数校验失败: {0}")]
    Validation(String),
    /// 字段级校验失败（站点设定批量保存 / 预校验，§4.1）：(字段名, 错误说明)
    #[error("字段校验失败")]
    FieldErrors(Vec<(String, String)>),
    #[error("余额不足")]
    InsufficientSpark,
    #[error("流水冲突，请重试")]
    LedgerConflict,
    #[error("操作太频繁啦，休息一下")]
    RateLimited,
    #[error("已感谢过")]
    AlreadyThanked,
    /// 登录需两步验证：密码已对但未提供验证码（前端应展开 2FA 输入框）
    #[error("账号已开启两步验证，请填写动态验证码")]
    TwoFactorRequired,
    /// 登录两步验证码不正确（已填但校验失败）
    #[error("两步验证码不正确，请核对验证器当前 6 位数字")]
    TwoFactorInvalid,
    #[error("种子文件无效: {0}")]
    TorrentInvalid(String),
    #[error("种子重复")]
    TorrentDuplicate,
    /// 模块未开启（U1 §5.1）：本站未开放该功能，与 403 权限区分（先模块后权限，§5.5）
    #[error("本站未开放此功能")]
    ModuleDisabled(String),
    /// 抽卡池经济守卫（方案 §5 G31-B）：含保底综合返还率 >100%，运行时拒抽——
    /// 后台保存拦截不够，站长直连改库也要被拦（样张：合成价后门 202.8%）
    #[error("卡池经济守卫触发，抽取已停止")]
    PoolGuard(String),
    #[error("内部错误")]
    Internal(#[from] anyhow::Error),
}

impl DomainError {
    /// 信封 code：与前端 ErrorCode 常量一一对应
    fn code(&self) -> i32 {
        match self {
            DomainError::NotFound(_) => 1004,
            DomainError::Unauthorized => 2001,
            DomainError::Forbidden => 2003,
            DomainError::InvalidCredentials => 2004,
            DomainError::InviteInvalid => 2005,
            DomainError::InviteUsed => 2006,
            DomainError::UsernameTaken => 2007,
            DomainError::Validation(_) => 1002,
            DomainError::FieldErrors(_) => 1002,
            DomainError::TwoFactorRequired => 2010,
            DomainError::TwoFactorInvalid => 2011,
            DomainError::InsufficientSpark => 4001,
            DomainError::LedgerConflict => 4002,
            DomainError::RateLimited => 1015,
            DomainError::AlreadyThanked => 5002,
            DomainError::TorrentInvalid(_) => 3003,
            DomainError::TorrentDuplicate => 3004,
            DomainError::ModuleDisabled(_) => 4101,
            DomainError::PoolGuard(_) => 4102,
            DomainError::Internal(_) => 1000,
        }
    }
    fn status(&self) -> StatusCode {
        match self {
            DomainError::NotFound(_) => StatusCode::NOT_FOUND,
            DomainError::Unauthorized => StatusCode::UNAUTHORIZED,
            DomainError::Forbidden => StatusCode::FORBIDDEN,
            DomainError::InvalidCredentials
            | DomainError::InviteInvalid
            | DomainError::InviteUsed
            | DomainError::UsernameTaken
            | DomainError::AlreadyThanked
            | DomainError::TorrentInvalid(_)
            | DomainError::TorrentDuplicate
            | DomainError::Validation(_)
            | DomainError::FieldErrors(_)
            | DomainError::TwoFactorRequired
            | DomainError::TwoFactorInvalid => StatusCode::BAD_REQUEST,
            // 429 与 openapi 文档（openapi_http.rs 429 描述）及 _ratelimit_check 口径一致
            DomainError::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            DomainError::InsufficientSpark | DomainError::LedgerConflict => {
                StatusCode::CONFLICT
            }
            // 4101 语义上更贴近「资源被移除」；用 403 会与权限混淆、404 会误导前端重试逻辑
            DomainError::ModuleDisabled(_) => StatusCode::NOT_FOUND,
            // 经济守卫：409 语义是「资源当前状态冲突」，与 429 限频区分
            DomainError::PoolGuard(_) => StatusCode::CONFLICT,
            DomainError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl ResponseError for DomainError {
    fn error_response(&self) -> HttpResponse {
        let status = self.status();
        if let DomainError::Internal(e) = self {
            tracing::error!(error = ?e, "internal error");
        }
        // 本地化消息：Accept-Language 经 locale_mw 注入 task-local（i18n.rs）
        let locale = crate::i18n::current();
        let message = match self {
            DomainError::Validation(d) => {
                format!(
                    "{}: {}",
                    crate::i18n::localized_message(self.code(), locale),
                    // 详情按原句查译文表（i18n/validation_details.tsv）；
                    // 不在表里就原样透出，不现场编译文
                    crate::i18n::localized_detail(d, locale)
                )
            }
            DomainError::TorrentInvalid(d) => {
                format!(
                    "{}: {}",
                    crate::i18n::localized_message(self.code(), locale),
                    crate::i18n::localized_detail(d, locale)
                )
            }
            DomainError::FieldErrors(list) => format!(
                "{}: {} 个字段未通过校验",
                crate::i18n::localized_message(self.code(), locale),
                list.len()
            ),
            _ => {
                crate::i18n::localized_message(self.code(), locale).to_string()
            }
        };
        // 字段级错误随信封 data 返回（前端据此内联红字），其余错误 data 保持 null
        let data = match self {
            DomainError::FieldErrors(list) => serde_json::json!(list
                .iter()
                .map(|(f, e)| serde_json::json!({
                    "field": f,
                    "error": crate::i18n::localized_detail(e, locale)
                }))
                .collect::<Vec<_>>()),
            _ => serde_json::Value::Null,
        };
        HttpResponse::build(status).json(json!({
            "code": self.code(),
            // 术语表（0205 / 四审 L7）：错误详情里写死的「种子 / 魔力 / 保种」这类
            // 固有词，在出口处按站长注册的规则改写——390 条校验串不用逐条改，
            // 也不用给每条配三语译文，改的是「词」本身。零规则时 apply 原样返回。
            "message": crate::terms::apply(&message),
            "data": data,
            // 贯穿修复（P2）：与响应头/日志同源（request_id_mw task-local）
            "request_id": crate::request_id::current(),
        }))
    }
}

pub type DomainResult<T> = Result<T, DomainError>;

/// 数据库错误 → 领域错误。
///
/// 外键违规（SQLSTATE 23503）是调用方给了不存在的 id（分类/字典项/用户…），
/// 属于可修正的输入问题：此前一路 `.map_err(Internal)` 冒成 500，客户端既看不懂
/// 也重试不好。其余数据库错误仍是 500（真故障，不该伪装成用户错误）。
pub fn db_to_domain(e: sqlx::Error, what: &str) -> DomainError {
    if let sqlx::Error::Database(ref d) = e {
        match d.code().as_deref() {
            Some("23503") => {
                return DomainError::Validation(format!(
                    "{what}引用了不存在的分类或字典项"
                ));
            }
            // 自定义异常（0188 分类防环等触发器）→ 400 带原话
            Some("P0001") => {
                return DomainError::Validation(d.message().to_string());
            }
            _ => {}
        }
    }
    DomainError::Internal(e.into())
}
