//! 抽取失败也要走站内信封（P-280 的续集 P-282）。
//!
//! actix 的 `web::Path<T>` / JSON 载荷在**进入 handler 之前**就失败，抛的是
//! `HttpRequestError`/`PathError`，默认渲染成 `text/plain` 的英文句子。
//! 现网实测（镜像 `api:aa1025c`，2026-10-14）：
//!
//! ```text
//! GET /users/abc         → 404 text/plain  can not parse "abc" to a i64
//! GET /torrents/notanid  → 404 text/plain  can not parse "notanid" to a i64
//! ```
//!
//! 两个后果：
//!   ① 违反「所有 HTTP 响应都是 {code,message,data,request_id}」——前端
//!      api-client 按 content-type 判非 JSON，只能显示「服务异常」；
//!   ② 路径解析发生在**鉴权之前**，未登录请求也能拿到内部解析细节
//!      （对照：同一条 URL 用合法 id 打，会先走 `require_auth`）。
//!
//! 查询侧走 `super::qenv::Q<T>`（显式换提取器，87 文件 109 处）；路径侧有
//! `web::Path<…>` 形参 **199 处 / 115 个文件**，逐个换不划算，这里用 actix 的
//! 中央机制 `PathConfig::error_handler` 一次接住。
//! JSON 载荷侧的口径原内联在 main.rs，为腾出 main.rs 的行数预算原样搬来
//! （语义零变化；状态码仍 400，只有路径侧从 actix 的 404 归位到 400）。

use actix_web::{web, HttpResponse};

/// `web::Path<T>` 形参的中央错误口径（P-282 新增）。
pub fn path_config() -> web::PathConfig {
    web::PathConfig::default().error_handler(|err, _req| {
        let body = serde_json::json!({
            "code": 1002,
            "message": format!("路径参数无效：{}", err),
            "data": serde_json::Value::Null,
            "request_id": crate::request_id::current(),
        });
        // 404 留给「真的没有这个资源」；格式坏是客户端的错
        actix_web::error::InternalError::from_response(
            err.to_string(),
            HttpResponse::BadRequest().json(body),
        )
        .into()
    })
}

/// JSON 载荷解析错误的中央口径（原 main.rs 内联块，逐字搬来）。
pub fn json_config() -> web::JsonConfig {
    web::JsonConfig::default().error_handler(|err, _req| {
        let locale = crate::i18n::current();
        let body = serde_json::json!({
            "code": 1002,
            "message": format!(
                "{}: {}",
                crate::i18n::localized_message(1002, locale),
                err
            ),
            "data": null,
            "request_id": crate::request_id::current()
        });
        // error_handler 需返回 actix_web::Error；InternalError 是标准包裹方式
        actix_web::error::InternalError::from_response(
            err.to_string(),
            HttpResponse::BadRequest().json(body),
        )
        .into()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 两个中央口径都必须能构造出来（注册缺失的话 main.rs 编译不过，
    /// 这条留给「装配点被误删」之外的形状核对）。
    #[test]
    fn both_configs_build() {
        let _p: web::PathConfig = path_config();
        let _j: web::JsonConfig = json_config();
    }
}
