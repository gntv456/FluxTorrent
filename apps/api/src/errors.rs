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
    #[error("余额不足")]
    InsufficientSpark,
    #[error("流水冲突，请重试")]
    LedgerConflict,
    #[error("操作太频繁啦，休息一下")]
    RateLimited,
    #[error("已感谢过")]
    AlreadyThanked,
    #[error("种子文件无效: {0}")]
    TorrentInvalid(String),
    #[error("种子重复")]
    TorrentDuplicate,
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
            DomainError::InsufficientSpark => 4001,
            DomainError::LedgerConflict => 4002,
            DomainError::RateLimited => 1015,
            DomainError::AlreadyThanked => 5002,
            DomainError::TorrentInvalid(_) => 3003,
            DomainError::TorrentDuplicate => 3004,
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
            | DomainError::RateLimited
            | DomainError::TorrentInvalid(_)
            | DomainError::TorrentDuplicate
            | DomainError::Validation(_) => StatusCode::BAD_REQUEST,
            DomainError::InsufficientSpark | DomainError::LedgerConflict => StatusCode::CONFLICT,
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
        HttpResponse::build(status).json(json!({
            "code": self.code(),
            "message": self.to_string(),
            "data": null,
            "request_id": uuid::Uuid::new_v4().to_string(),
        }))
    }
}

pub type DomainResult<T> = Result<T, DomainError>;
