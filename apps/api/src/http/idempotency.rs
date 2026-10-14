//! Idempotency-Key 头贯穿（任务2：头兜底，体优先）。
//!
//! 幂等键现状是业务体字段（invite/bank/shop/medal/gacha/games 等 12 处），
//! 网关/SDK 重试侧想注入键时无处落手——只能改各 DTO。机制与 request_id
//! 同款 task-local 范式：`idempotency_mw` 读 `Idempotency-Key` 头 →
//! 合法形状（8..=128 位可见 ASCII，0x21..=0x7E）注入 task-local，
//! `current()` 供各取用点兜底读取。
//!
//! 纪律：
//! - **体字段优先，头兜底**：body.idempotency_key 非空时头不参与；
//! - 头形状不合法（过短/过长/含空白控制字符）**直接忽略**，不拒绝请求
//!   （幂等键缺失语义由各端点既有校验/回退链负责）；
//! - 取用点原有长度校验（如 gacha draw 的 8..=120）对头来源同样适用：
//!   不满足视同未提供。

use actix_web::body::MessageBody;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::http::header::HeaderName;
use actix_web::middleware::Next;

pub const IDEMPOTENCY_KEY_HEADER: HeaderName =
    HeaderName::from_static("idempotency-key");
/// 合法长度（与 gacha draw 的 8..=120 取交集再放宽到通用 128 上限）
const MIN_LEN: usize = 8;
const MAX_LEN: usize = 128;

tokio::task_local! {
    /// 当前请求的 Idempotency-Key（由 `idempotency_mw` 注入；无头/非法头时无值）
    pub static IDEMPOTENCY_KEY: Option<String>;
}

/// 读取当前请求头幂等键；无中间件上下文（测试/后台任务）返回 None。
pub fn current() -> Option<String> {
    IDEMPOTENCY_KEY.try_with(|k| k.clone()).unwrap_or(None)
}

/// 头形状校验：8..=128 位可见 ASCII（0x21..=0x7E，无空白无控制字符）。
/// 公开给取用点复用（体字段也按同一把尺时可直接调）。
pub fn valid_shape(v: &str) -> bool {
    (MIN_LEN..=MAX_LEN).contains(&v.len())
        && v.bytes().all(|b| (0x21..=0x7E).contains(&b))
}

/// 中间件：读头 → 形状校验（不合法忽略，不拒绝）→ task-local 注入
pub async fn idempotency_mw(
    req: ServiceRequest,
    next: Next<impl MessageBody>,
) -> actix_web::Result<ServiceResponse<impl MessageBody>> {
    let key = req
        .headers()
        .get(&IDEMPOTENCY_KEY_HEADER)
        .and_then(|v| v.to_str().ok())
        .filter(|v| valid_shape(v))
        .map(str::to_string);
    IDEMPOTENCY_KEY.scope(key, next.call(req)).await
}

/// 统一取键口径（体优先、头兜底）：body 字段 trim 后非空即用，
/// 否则读头 task-local；两侧都空返回 None（调用方走既有必填校验/回退）。
pub fn body_or_header(body: &Option<String>) -> Option<String> {
    body.clone()
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty())
        .or_else(current)
}

#[cfg(test)]
mod tests {
    use actix_web::{test, web, App, HttpResponse};

    #[actix_web::test]
    async fn injects_valid_header_key() {
        let app = test::init_service(
            App::new()
                .wrap(actix_web::middleware::from_fn(super::idempotency_mw))
                .route(
                    "/ping",
                    web::to(|| async {
                        let k = super::current();
                        HttpResponse::Ok().json(serde_json::json!({ "key": k }))
                    }),
                ),
        )
        .await;
        let req = test::TestRequest::get()
            .uri("/ping")
            .insert_header((super::IDEMPOTENCY_KEY_HEADER, "abc12345-6789"))
            .to_request();
        let res = test::call_service(&app, req).await;
        let body: serde_json::Value =
            serde_json::from_slice(&test::read_body(res).await).expect("json");
        assert_eq!(
            body["key"].as_str(),
            Some("abc12345-6789"),
            "合法头必须注入 task-local"
        );
    }

    #[actix_web::test]
    async fn ignores_invalid_header_key() {
        let app = test::init_service(
            App::new()
                .wrap(actix_web::middleware::from_fn(super::idempotency_mw))
                .route(
                    "/ping",
                    web::to(|| async {
                        let k = super::current();
                        HttpResponse::Ok().json(serde_json::json!({ "key": k }))
                    }),
                ),
        )
        .await;
        // 过短（<8）/ 过长（>128）/ 含空格：一律忽略且不拒绝请求
        for bad in ["short", &"x".repeat(129), "has space in it"] {
            let req = test::TestRequest::get()
                .uri("/ping")
                .insert_header((super::IDEMPOTENCY_KEY_HEADER, bad))
                .to_request();
            let res = test::call_service(&app, req).await;
            assert_eq!(res.status().as_u16(), 200, "非法头不拒绝请求");
            let body: serde_json::Value =
                serde_json::from_slice(&test::read_body(res).await)
                    .expect("json");
            assert!(body["key"].is_null(), "非法头必须被忽略：{bad:?}");
        }
    }
}

/// 头兜底取键口径：体空 → 头；体非空 → 体（trim）
/// （单独模块避免 tests 里 `use actix_web::test` 把属性宏 `#[test]`
/// 解析成 actix 的异步测试宏——request_id.rs safe_path_tests 同款处理）
#[cfg(test)]
mod sync_tests {
    #[test]
    fn body_or_header_prefers_body() {
        assert_eq!(
            super::body_or_header(&Some("  body-key-1  ".into())),
            Some("body-key-1".into())
        );
        assert_eq!(
            super::body_or_header(&Some("   ".into())),
            None,
            "纯空白体视同缺失，走回退"
        );
        // 无 task-local 上下文时头侧为 None
        assert_eq!(super::body_or_header(&None), None);
    }

    /// 形状校验边界：8/128 位合法，7/129/含空白非法
    #[test]
    fn shape_validation_bounds() {
        assert!(super::valid_shape(&"a".repeat(8)));
        assert!(super::valid_shape(&"a".repeat(128)));
        assert!(!super::valid_shape(&"a".repeat(7)));
        assert!(!super::valid_shape(&"a".repeat(129)));
        assert!(!super::valid_shape("abcd 123"));
        assert!(!super::valid_shape("abcd\t1234"));
    }
}
