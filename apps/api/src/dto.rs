//! API 信封（§8.2）：统一 { code, message, data, request_id }。

use actix_web::HttpResponse;
use serde::Serialize;

#[derive(Serialize)]
struct Envelope<T> {
    code: i32,
    message: &'static str,
    data: T,
    request_id: String,
}

pub fn ok<T: Serialize>(data: T) -> HttpResponse {
    HttpResponse::Ok().json(Envelope {
        code: 0,
        message: "ok",
        data,
        // 贯穿修复（P2）：从 request_id_mw 的 task-local 取本次请求同一 id，
        // 与响应头/日志同源；无中间件上下文（测试）时 current() 内部回退自生成
        request_id: crate::request_id::current(),
    })
}

/// 分页参数统一钳制（审计 P1）：per_page 1..=100、page 1..=100_000；
/// offset 用 checked_mul 防溢出——旧式 `(page.max(1) - 1) * per_page` 在极端参数下
/// debug 构建 panic（500 面）、release 构建得到负 OFFSET。返回 (offset, limit)。
pub fn page_window(page: i64, per_page: i64) -> (i64, i64) {
    let limit = per_page.clamp(1, 100);
    let page = page.clamp(1, 100_000);
    let offset = page
        .checked_sub(1)
        .and_then(|p| p.checked_mul(limit))
        .unwrap_or(0);
    (offset, limit)
}
