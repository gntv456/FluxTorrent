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
        request_id: uuid::Uuid::new_v4().to_string(),
    })
}
