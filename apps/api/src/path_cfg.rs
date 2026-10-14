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
        let raw = err.to_string();
        // 原文只进日志不进响应体：路径解析发生在鉴权之前，未登录请求也能
        // 读到这条响应，把 serde/actix 原文塞进 data 等于换个字段继续泄漏
        tracing::warn!("路径参数解析失败：{raw}");
        let body = serde_json::json!({
            "code": 1002,
            "message": format!("路径参数无效：{}", humanize_parse_error(&raw)),
            "data": serde_json::Value::Null,
            "request_id": crate::request_id::current(),
        });
        // 404 留给「真的没有这个资源」；格式坏是客户端的错
        actix_web::error::InternalError::from_response(
            raw,
            HttpResponse::BadRequest().json(body),
        )
        .into()
    })
}

/// JSON 载荷解析错误的中央口径（原 main.rs 内联块，逐字搬来；
/// P-304 把 serde 原文换成人话，原文进日志不进 message）。
pub fn json_config() -> web::JsonConfig {
    web::JsonConfig::default().error_handler(|err, _req| {
        let locale = crate::i18n::current();
        let raw = err.to_string();
        tracing::warn!("请求体解析失败：{raw}");
        let body = serde_json::json!({
            "code": 1002,
            "message": format!(
                "{}: {}",
                crate::i18n::localized_message(1002, locale),
                humanize_parse_error(&raw)
            ),
            "data": null,
            "request_id": crate::request_id::current()
        });
        // error_handler 需返回 actix_web::Error；InternalError 是标准包裹方式
        actix_web::error::InternalError::from_response(
            raw,
            HttpResponse::BadRequest().json(body),
        )
        .into()
    })
}

/// 解析原文里 `missing field \`email\`` 之类的反引号段
fn quoted(s: &str, open: &str) -> Option<String> {
    let rest = s.split(open).nth(1)?.trim_start();
    // 值紧跟分隔符，第一个字符就是开引号；要找的是它的**闭合**引号
    let delim = rest.chars().next().filter(|c| *c == '`' || *c == '"')?;
    let inner = &rest[delim.len_utf8()..];
    let end = inner.find(delim)?;
    let v = inner[..end].trim();
    if v.is_empty() {
        return None;
    }
    Some(v.to_string())
}

/// P-304：把 serde / actix 的解析原文换成人话。
///
/// 现网实测（10-14）：`POST /auth/register` 少带 email 时，1002 的 message
/// 是「参数校验失败: Json deserialize error: missing field \`email\` at line
/// 1 column 16」——粗标签后面挂的是**英文内部诊断**。这一族在前端有 43 处
/// `dict.errors[code] ?? message`，把英文摊出去比笼统粗标签更糟，也不能让
/// 前端按「含中文」猜（猜一次错一次）。所以在出口这一层翻译：认得的形状
/// 给人话，认不出的给通用中文，原文一律挪进 `data.detail` 供排障。
pub(crate) fn humanize_parse_error(raw: &str) -> String {
    let r = raw.trim();
    let body = r
        .strip_prefix("Json deserialize error: ")
        .or_else(|| r.strip_prefix("Query deserialize error: "))
        .unwrap_or(r);
    if body.contains("missing field") {
        if let Some(f) = quoted(body, "missing field ") {
            return format!("缺少必填字段：{f}");
        }
        return "缺少必填字段".to_string();
    }
    if body.contains("invalid type") {
        let want = body
            .split("expected ")
            .nth(1)
            .map(|w| {
                // 去掉「at line 1 column 20」这类位置尾巴，它不是给人看的
                w.split(" at line ").next().unwrap_or(w).trim_end_matches('.')
            })
            .unwrap_or("类型不符");
        return format!("字段类型不对：应为 {want}");
    }
    if body.contains("invalid value") || body.contains("expected variant") {
        return "取值不在允许范围内".to_string();
    }
    if body.contains("invalid digit") || body.contains("cannot parse")
        || body.contains("can not parse") {
        let num = quoted(body, "parse ");
        return match num {
            Some(v) => format!("应为数字：{v}"),
            None => "应为数字".to_string(),
        };
    }
    if body.contains("expected value") || body.contains("trailing characters")
        || body.contains("key must be a string") || body.contains("EOF") {
        return "请求体不是合法 JSON".to_string();
    }
    if body.contains("content type") || body.contains("Content-Type") {
        return "请求的 Content-Type 不是 application/json".to_string();
    }
    if body.contains("Stream error") || body.contains("payload") {
        return "请求体读取失败".to_string();
    }
    "请求参数格式不正确".to_string()
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

    #[test]
    fn serde_shapes_become_human_chinese() {
        // 现网实测的原句（1002，缺 email）
        let h = humanize_parse_error(
            "Json deserialize error: missing field `email` at line 1 \
             column 16",
        );
        assert_eq!(h, "缺少必填字段：email");
        assert_eq!(
            humanize_parse_error(
                "Json deserialize error: invalid type: string \"abc\", \
                 expected i64 at line 1 column 20",
            ),
            "字段类型不对：应为 i64"
        );
        assert_eq!(
            humanize_parse_error(
                "Query deserialize error: invalid digit found in string",
            ),
            "应为数字"
        );
        // 认不出的形状也必须给人话，而不是把原文留在 message 上
        assert_eq!(humanize_parse_error("wibbel wobble"), "请求参数格式不正确");
        for s in [
            "Json deserialize error: missing field `x`",
            "Json deserialize error: expected value at line 1 column 1",
            "Json payload error: Stream error",
            "can not parse \"abc\" to a i64",
            "wibbel",
        ] {
            assert!(
                humanize_parse_error(s).chars().any(|c| c > '\u{2e80}'),
                "{s} 的出口不是中文"
            );
        }
    }
}
