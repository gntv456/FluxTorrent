//! request_id 贯穿（审计 P2 修复）。
//! 旧状：信封里的 request_id 由 `errors.rs`/`dto.rs` 各自现场 `Uuid::new_v4()` 随机生成，
//! 与日志毫无关联——前端拿 ID 报障后后端无法检索对应请求日志，字段形同装饰。
//!
//! 机制（与 i18n::locale_mw 同款 task-local 范式）：
//! `request_id_mw` 中间件优先沿用入站 `X-Request-Id`（反代/网关已生成的链路 ID，
//! 便于跨系统串联；长度与字符集校验防日志注入），否则生成 UUIDv4 →
//! ① 写入响应头 `X-Request-Id`（前端/网关可直接读取）；
//! ② 以 tracing::info! 记录「请求开始」行（method/path/status/耗时在 actix Logger 侧）；
//! ③ 经 task-local 注入，`current()` 供信封构造路径同步读取。
//!
//! 中间件 wrap 顺序：须在 Logger 与业务 handler 之外层（main.rs 中紧邻 locale_mw 之前），
//! 保证 error_response()/ok() 在 handler future poll 内调用时 task-local 可见。

use actix_web::body::MessageBody;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::http::header::HeaderName;
use actix_web::middleware::Next;
use uuid::Uuid;

pub const REQUEST_ID_HEADER: HeaderName = HeaderName::from_static("x-request-id");
/// 入站 request_id 的长度与字符集约束（防日志注入/超长行）
const INBOUND_MAX_LEN: usize = 64;

tokio::task_local! {
    /// 当前请求的 request_id（由 `request_id_mw` 注入，handler/错误转换路径内可见）
    pub static REQUEST_ID: String;
}

/// 读取当前请求 request_id；无中间件上下文（测试/后台任务）时现场生成并返回。
pub fn current() -> String {
    REQUEST_ID
        .try_with(|r| r.clone())
        .unwrap_or_else(|_| Uuid::new_v4().to_string())
}

/// 入站头合法性：1..=64 个安全字符（字母数字 + 连字符），否则忽略改用自生成
fn valid_inbound(v: &str) -> bool {
    !v.is_empty()
        && v.len() <= INBOUND_MAX_LEN
        && v.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

/// 中间件：生成/沿用 request_id → 响应头 + task-local 贯穿
pub async fn request_id_mw(
    req: ServiceRequest,
    next: Next<impl MessageBody>,
) -> actix_web::Result<ServiceResponse<impl MessageBody>> {
    let rid = req
        .headers()
        .get(&REQUEST_ID_HEADER)
        .and_then(|v| v.to_str().ok())
        .filter(|v| valid_inbound(v))
        .map(str::to_string)
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let method = req.method().as_str().to_string();
    let path = req.path().to_string();
    let mut res = REQUEST_ID.scope(rid.clone(), next.call(req)).await?;
    // 请求开始行：与 access log（Logger）互补—— Logger 不含 request_id 字段
    tracing::info!(request_id = %rid, method = %method, path = %path, "request start");
    res.headers_mut().insert(
        REQUEST_ID_HEADER.clone(),
        rid.parse().expect("uuid 解析为HeaderValue必然成功"),
    );
    Ok(res)
}

#[cfg(test)]
mod tests {
    use actix_web::{test, web, App, HttpResponse};

    #[actix_web::test]
    async fn generates_and_echoes_request_id() {
        let app = test::init_service(
            App::new()
                .wrap(actix_web::middleware::from_fn(super::request_id_mw))
                .route(
                    "/ping",
                    web::to(|| async {
                        // 信封构造路径能读到同一个 id（task-local 注入验证）
                        let rid = super::current();
                        HttpResponse::Ok().json(serde_json::json!({ "request_id": rid }))
                    }),
                ),
        )
        .await;
        let req = test::TestRequest::get().uri("/ping").to_request();
        let res = test::call_service(&app, req).await;
        let header = res
            .headers()
            .get(super::REQUEST_ID_HEADER)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let body = test::read_body(res).await;
        let echoed: serde_json::Value = serde_json::from_slice(&body).expect("json");
        assert!(header.is_some(), "响应头必须回写 X-Request-Id");
        assert_eq!(
            header.as_deref(),
            echoed["request_id"].as_str(),
            "信封与响应头同源"
        );
    }

    #[actix_web::test]
    async fn reuses_inbound_request_id() {
        let app = test::init_service(
            App::new()
                .wrap(actix_web::middleware::from_fn(super::request_id_mw))
                .route("/ping", web::to(|| async { HttpResponse::Ok().finish() })),
        )
        .await;
        let req = test::TestRequest::get()
            .uri("/ping")
            .insert_header((super::REQUEST_ID_HEADER, "gateway-abc-123"))
            .to_request();
        let res = test::call_service(&app, req).await;
        assert_eq!(
            res.headers()
                .get(super::REQUEST_ID_HEADER)
                .and_then(|v| v.to_str().ok()),
            Some("gateway-abc-123"),
            "入站合法 id 必须沿用（跨系统串联）"
        );
    }

    #[actix_web::test]
    async fn rejects_log_injection_inbound_id() {
        let app = test::init_service(
            App::new()
                .wrap(actix_web::middleware::from_fn(super::request_id_mw))
                .route("/ping", web::to(|| async { HttpResponse::Ok().finish() })),
        )
        .await;
        // 含空格的非法 id 必须被忽略改自生成（换行等控制字符在 HTTP 头解析层已被拒）
        let req = test::TestRequest::get()
            .uri("/ping")
            .insert_header((super::REQUEST_ID_HEADER, "bad id with spaces"))
            .to_request();
        let res = test::call_service(&app, req).await;
        let rid = res
            .headers()
            .get(super::REQUEST_ID_HEADER)
            .and_then(|v| v.to_str().ok());
        assert_ne!(
            rid,
            Some("bad id with spaces"),
            "非法入站 id 必须忽略改自生成"
        );
        assert!(rid.is_some_and(|v| !v.is_empty() && v != "bad id with spaces"));
    }
}
