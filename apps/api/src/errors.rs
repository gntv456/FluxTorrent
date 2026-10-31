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
    /// 登录连续失败后需先完成工作量证明（P-502）：客户端重新取题求解后重提
    #[error("登录尝试较多，需先完成工作量证明")]
    PowRequired,
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
    /// 站点邮件服务当前不可用（SMTP 未配置、连不上或站点基址未配）：
    /// 找回密码/邮箱激活必须发信的动作，此前用 `Internal` 表达 ⇒ 500
    /// 「内部错误」，用户与运维都看不出是配置状态（P-102）。语义是
    /// 「服务当前不可用」，故 HTTP 503。P-690：判据从「配置键非空」
    /// 收紧到「真能连上」，文案同时覆盖未配置与连接失败。
    #[error("邮件服务当前不可用，请稍后重试或联系管理组")]
    MailUnconfigured,
    /// 乐观并发冲突（site_settings expected updated_at 校验，5003）：
    /// 提交携带的版本戳与库中现值不一致 = 有人先改过，整批拒绝防静默覆盖；
    /// fields 随信封 data 带冲突键列表，前端提示刷新后重试
    #[error("配置已被其他人修改，请刷新后重试")]
    VersionConflict(Vec<String>),
    #[error("内部错误")]
    Internal(#[from] anyhow::Error),
}

impl DomainError {
    /// 信封 code：与前端 ErrorCode 常量一一对应
    ///
    /// 每个码一行的触发条件注释（W-13 注册表口径；`scripts/error_codes_gen.mjs`
    /// 按本格式抓取生成 docs/api/error-codes.md，格式：`// <code> <变体>：触发条件`）
    fn code(&self) -> i32 {
        match self {
            // 1004 NotFound：按 id 查资源（种子/评论/工单/订单…）无行命中
            DomainError::NotFound(_) => 1004,
            // 2001 Unauthorized：JWT 缺失/过期/签名不对，或匿名访问需登录端点
            DomainError::Unauthorized => 2001,
            // 2003 Forbidden：已认证但等级/权限不足（先过模块开关再查权限，§5.5）
            DomainError::Forbidden => 2003,
            // 2004 InvalidCredentials：登录用户名或密码不匹配
            DomainError::InvalidCredentials => 2004,
            // 2005 InviteInvalid：注册邀请码不存在或已过有效期
            DomainError::InviteInvalid => 2005,
            // 2006 InviteUsed：注册邀请码已被消费（一码一用）
            DomainError::InviteUsed => 2006,
            // 2007 UsernameTaken：注册/改名的用户名与他人冲突
            DomainError::UsernameTaken => 2007,
            // 1002 Validation：字段校验不通过或外键引用了不存在的 id（db_to_domain 23503）
            DomainError::Validation(_) => 1002,
            // 1002 FieldErrors：站点设定批量保存预校验多字段不通过（§4.1）
            DomainError::FieldErrors(_) => 1002,
            // 2010 TwoFactorRequired：密码已对但未提交 2FA 验证码（登录第二步）
            DomainError::TwoFactorRequired => 2010,
            // 2011 TwoFactorInvalid：已提交 2FA 验证码但校验不匹配
            DomainError::TwoFactorInvalid => 2011,
            // 2012 PowRequired：登录失败达到触发线，缺令牌或令牌失效应重新取题
            DomainError::PowRequired => 2012,
            // 4001 InsufficientSpark：经济操作（购买/抽卡/打赏…）火花余额不足
            DomainError::InsufficientSpark => 4001,
            // 4002 LedgerConflict：spark_ledger 乐观并发版本冲突，客户端应重试
            DomainError::LedgerConflict => 4002,
            // 1015 RateLimited：命中限流桶（openapi 登录/下载 passkey，60s 窗口）
            DomainError::RateLimited => 1015,
            // 5002 AlreadyThanked：对同一资源重复提交感谢
            DomainError::AlreadyThanked => 5002,
            // 3003 TorrentInvalid：种子文件解析失败或不合规（缺 info_hash 等）
            DomainError::TorrentInvalid(_) => 3003,
            // 3004 TorrentDuplicate：info_hash 与库中已有种子重复
            DomainError::TorrentDuplicate => 3004,
            // 4101 ModuleDisabled：站点关闭了该功能模块（先模块后权限，§5.5）
            DomainError::ModuleDisabled(_) => 4101,
            // 4102 PoolGuard：抽卡池含保底综合返还率 >100% 被运行时拒绝（G31-B）
            DomainError::PoolGuard(_) => 4102,
            // 4103 MailUnconfigured：站点未配邮件服务却走了必须发信的链路（P-102）
            DomainError::MailUnconfigured => 4103,
            // 5xxx 社区段：5003 = 设定乐观并发冲突（5001/5002 已占）
            // 5003 VersionConflict：设定保存携带的 expected updated_at 与库中不一致
            DomainError::VersionConflict(_) => 5003,
            // 1000 Internal：未分类的服务端故障（含 db_to_domain 兜底的数据库错误）
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
            | DomainError::TwoFactorInvalid
            | DomainError::PowRequired => StatusCode::BAD_REQUEST,
            // 429 与 openapi 文档（openapi_http.rs 429 描述）及 _ratelimit_check 口径一致
            DomainError::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            DomainError::InsufficientSpark | DomainError::LedgerConflict => {
                StatusCode::CONFLICT
            }
            // 4101 语义上更贴近「资源被移除」；用 403 会与权限混淆、404 会误导前端重试逻辑
            DomainError::ModuleDisabled(_) => StatusCode::NOT_FOUND,
            // 经济守卫：409 语义是「资源当前状态冲突」，与 429 限频区分
            DomainError::PoolGuard(_) => StatusCode::CONFLICT,
            // 设定乐观并发：携带的版本戳已过期（他人先改），409 让前端提示刷新
            DomainError::VersionConflict(_) => StatusCode::CONFLICT,
            // 邮件链路依赖的外部配置缺失：503 比 500 诚实（不是我们的 bug）
            DomainError::MailUnconfigured => StatusCode::SERVICE_UNAVAILABLE,
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
            // 版本冲突：冲突键列表随信封回传（前端高亮哪些键被人改过）
            DomainError::VersionConflict(keys) => {
                serde_json::json!(keys)
            }
            _ => serde_json::Value::Null,
        };
        let mut resp = HttpResponse::build(status).json(json!({
            "code": self.code(),
            // 术语表（0205 / 四审 L7）：错误详情里写死的「种子 / 魔力 / 保种」这类
            // 固有词，在出口处按站长注册的规则改写——390 条校验串不用逐条改，
            // 也不用给每条配三语译文，改的是「词」本身。零规则时 apply 原样返回。
            "message": crate::terms::apply(&message),
            "data": data,
            // 贯穿修复（P2）：与响应头/日志同源（request_id_mw task-local）
            "request_id": crate::request_id::current(),
        }));
        // 0267：限流响应必须自带 Retry-After —— 第三方客户端据此退避，
        // 否则只能盲目重试。本项目所有限流桶都是 60s 固定窗口
        // （openapi 按 token、download.php 按 passkey、登录按用户名/IP）。
        if matches!(self, DomainError::RateLimited) {
            resp.headers_mut().insert(
                actix_web::http::header::RETRY_AFTER,
                actix_web::http::header::HeaderValue::from_static("60"),
            );
        }
        resp
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

#[cfg(test)]
mod tests {
    use super::*;

    /// P-502：登录 PoW 闸门必须发 2012，前端据此重新取题并自动重提
    #[test]
    fn pow_required_maps_to_2012() {
        let e = DomainError::PowRequired;
        assert_eq!(e.code(), 2012);
        assert_eq!(e.status(), StatusCode::BAD_REQUEST);
    }
}
