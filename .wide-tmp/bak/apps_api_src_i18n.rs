//! 后端消息本地化（zh-CN / zh-TW / en）。
//! 机制：`locale_mw` 中间件解析 Accept-Language → tokio task-local 注入；
//! `error_response()` 在 handler future poll 内同步调用，task-local 必然可见。

use actix_web::body::MessageBody;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::http::header::ACCEPT_LANGUAGE;
use actix_web::middleware::Next;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Locale {
    ZhCn,
    ZhTw,
    En,
}

tokio::task_local! {
    /// 当前请求语言（由 `locale_mw` 注入，handler/错误转换路径内可见）
    pub static LOCALE: Locale;
}

/// Accept-Language → Locale：按 q 值取最优可识别项，不可识别回 zh-CN。
pub fn from_accept_language(header: &str) -> Locale {
    let mut best: Option<(f32, Locale)> = None;
    for part in header.split(',') {
        let mut it = part.trim().splitn(2, ';');
        let tag = it.next().unwrap_or("").trim();
        if tag.is_empty() {
            continue;
        }
        let q: f32 = it
            .next()
            .and_then(|p| p.trim().strip_prefix("q="))
            .and_then(|v| v.parse().ok())
            .unwrap_or(1.0);
        let tag = tag.to_ascii_lowercase();
        let locale = if tag.starts_with("zh-tw")
            || tag.starts_with("zh-hk")
            || tag.starts_with("zh-mo")
            || tag.starts_with("zh-hant")
        {
            Some(Locale::ZhTw)
        } else if tag.starts_with("zh") {
            Some(Locale::ZhCn)
        } else if tag.starts_with("en") {
            Some(Locale::En)
        } else {
            None
        };
        if let Some(l) = locale {
            if best.is_none_or(|(bq, _)| q > bq) {
                best = Some((q, l));
            }
        }
    }
    best.map(|(_, l)| l).unwrap_or(Locale::ZhCn)
}

/// 错误码 → 三语消息（与 errors.rs `code()` / packages/domain-types 对齐）
const MESSAGES: &[(i32, &str, &str, &str)] = &[
    (1000, "内部错误", "內部錯誤", "Internal error"),
    // 1001 保留码（前端 ErrorCode.BAD_REQUEST 已定义；后端 malformed JSON 统一走 JsonConfig→1002）
    (1001, "请求格式错误", "請求格式錯誤", "Bad request"),
    (1002, "参数校验失败", "參數校驗失敗", "Validation failed"),
    (1004, "资源不存在", "資源不存在", "Resource not found"),
    (
        1015,
        "操作太频繁啦，休息一下",
        "操作太頻繁啦，休息一下",
        "Too many requests, take a break",
    ),
    (2001, "未认证", "未認證", "Not authenticated"),
    // 2008 保留码（前端 ErrorCode.MUST_RESET_PASSWORD；登录响应以 must_reset_password 布尔下发，
    // 该码为强制改密态下访问其他端点时的兜底文案）
    (
        2008,
        "请先重置密码",
        "請先重置密碼",
        "Password reset required",
    ),
    (2003, "无权限", "無權限", "Permission denied"),
    // 登录两步验证（UX 修复：区分未填/填错，替代旧 1002「参数校验失败」）
    (
        2010,
        "账号已开启两步验证，请填写动态验证码",
        "帳號已開啟兩步驗證，請填寫動態驗證碼",
        "This account has 2FA enabled; enter the 6-digit code",
    ),
    (
        2011,
        "两步验证码不正确，请核对验证器当前 6 位数字",
        "兩步驗證碼不正確，請核對驗證器當前 6 位數字",
        "Incorrect 2FA code; check the current 6 digits",
    ),
    (2004, "凭证无效", "憑證無效", "Invalid credentials"),
    (
        2005,
        "邀请码无效或已过期",
        "邀請碼無效或已過期",
        "Invite code is invalid or expired",
    ),
    (
        2006,
        "邀请码已被使用",
        "邀請碼已被使用",
        "Invite code already used",
    ),
    (
        2007,
        "用户名已被占用",
        "用戶名已被佔用",
        "Username already taken",
    ),
    (3003, "种子文件无效", "種子檔案無效", "Invalid torrent file"),
    (3004, "种子重复", "種子重複", "Duplicate torrent"),
    (4001, "余额不足", "餘額不足", "Insufficient balance"),
    (
        4002,
        "流水冲突，请重试",
        "流水衝突，請重試",
        "Ledger conflict, please retry",
    ),
    (
        4101,
        "本站未开放此功能",
        "本站未開放此功能",
        "This feature is not enabled on this site",
    ),
    (5002, "已感谢过", "已感謝過", "Already thanked"),
];

/// 按错误码取本地化消息（未知码回内部错误文案）
pub fn localized_message(code: i32, locale: Locale) -> &'static str {
    for (c, cn, tw, en) in MESSAGES {
        if *c == code {
            return match locale {
                Locale::ZhCn => cn,
                Locale::ZhTw => tw,
                Locale::En => en,
            };
        }
    }
    localized_message(1000, locale)
}

/// default_service 的 404 文案
pub fn endpoint_not_found(locale: Locale) -> &'static str {
    match locale {
        Locale::ZhCn => "接口不存在",
        Locale::ZhTw => "介面不存在",
        Locale::En => "API endpoint not found",
    }
}

/// 当前请求语言（无中间件上下文时回退 zh-CN）
pub fn current() -> Locale {
    LOCALE.try_with(|l| *l).unwrap_or(Locale::ZhCn)
}

/// 中间件：解析 Accept-Language 并以 task-local 贯穿本次请求
pub async fn locale_mw(
    req: ServiceRequest,
    next: Next<impl MessageBody>,
) -> actix_web::Result<ServiceResponse<impl MessageBody>> {
    let locale = req
        .headers()
        .get(ACCEPT_LANGUAGE)
        .and_then(|v| v.to_str().ok())
        .map(from_accept_language)
        .unwrap_or(Locale::ZhCn);
    LOCALE.scope(locale, next.call(req)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_accept_language() {
        let cases = [
            ("", Locale::ZhCn),
            ("en", Locale::En),
            ("en-US,en;q=0.9", Locale::En),
            ("zh-TW,zh;q=0.8", Locale::ZhTw),
            ("zh-HK", Locale::ZhTw),
            ("zh-Hant-TW", Locale::ZhTw),
            ("zh-CN,zh;q=0.9,en;q=0.8", Locale::ZhCn),
            ("zh", Locale::ZhCn),
            ("fr-FR,fr;q=0.9", Locale::ZhCn),
            ("en-US;q=0.5,zh-TW;q=0.9", Locale::ZhTw), // q 值降序取最优
        ];
        for (header, expect) in cases {
            assert_eq!(from_accept_language(header), expect, "header={header}");
        }
    }

    #[test]
    fn message_table_covers_all_locales() {
        for (_, cn, tw, en) in MESSAGES {
            assert!(!cn.is_empty() && !tw.is_empty() && !en.is_empty());
        }
        assert_eq!(
            localized_message(2005, Locale::En),
            "Invite code is invalid or expired"
        );
        assert_eq!(localized_message(9999, Locale::ZhCn), "内部错误");
    }

    /// task_local 时序验证：错误转换发生在中间件 scope 内，message 随 Accept-Language 切换
    #[actix_web::test]
    async fn error_message_localizes_by_accept_language() {
        use actix_web::{test, web, App, HttpResponse};

        let app = test::init_service(
            App::new()
                .wrap(actix_web::middleware::from_fn(locale_mw))
                .route(
                    "/err",
                    web::to(|| async {
                        Err::<HttpResponse, crate::errors::DomainError>(
                            crate::errors::DomainError::NotFound(1),
                        )
                    }),
                ),
        )
        .await;

        for (lang, expect) in [
            (Some("en-US,en;q=0.9"), "Resource not found"),
            (Some("zh-TW,zh;q=0.8"), "資源不存在"),
            (None, "资源不存在"),
        ] {
            let mut req = test::TestRequest::get().uri("/err");
            if let Some(l) = lang {
                req = req.insert_header(("Accept-Language", l));
            }
            let resp = test::call_service(&app, req.to_request()).await;
            assert_eq!(resp.status(), 404);
            let body: serde_json::Value = test::read_body_json(resp).await;
            assert_eq!(body["message"], expect, "lang={lang:?}");
        }
    }
}
