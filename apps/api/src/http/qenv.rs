//! 站内信封版提取器：查询参数 `Q<T>`（P-280）与可选 JSON 请求体
//! `OptJson<T>`（P-478）。两者治的是同一种病——actix 原生提取器失败时
//! 要么抛 `text/plain` 英文句子，要么（`Option<…>` 形式）把错误折成
//! `Ok(None)` 继续往下跑；本文件的两个类型把它们收回站内 1002 信封。
//!
//! actix 的 `web::Query<T>` 形参在**抽取失败**时直接回
//! `400 text/plain` + 英文 `Query deserialize error: invalid digit found
//! in string`：既没有 `code/message`，也不是中文。前端把它显示成「未知错误」，
//! 对外脚本（RSS / Torznab / NexusPHP 兼容层）按 JSON 信封解析则直接崩。
//! 本仓所有业务错误都走 `DomainError` ⇒ `{code, message, request_id}` 信封，
//! 唯独这一类绕过了它——`/torrents` 在 2026-10-10 已逐口改成 `from_query`
//! 并写明理由（「重复键 `?alive=0&alive=2`、`?limit=abc` 全中」），
//! 但同样的形状还散在几十个入口上。
//!
//! `Q<T>` 是 `web::Query<T>` 的替代类型：用法完全一致（`Deref` +
//! `into_inner()`），差别只在失败时**交回 `DomainError::Validation`**，
//! 于是错误码、文案、request_id 与其它所有错误同源。

use std::future::{ready, Future, Ready};
use std::ops::Deref;
use std::pin::Pin;

use actix_web::dev::Payload;
use actix_web::http::StatusCode;
use actix_web::{web, FromRequest, HttpRequest, HttpResponse, ResponseError};
use serde::de::DeserializeOwned;

use crate::errors::DomainError;

/// 查询参数（站内信封版）：`q: Q<MyQuery>`，取值仍写 `q.foo`。
pub struct Q<T>(pub T);

impl<T> Q<T> {
    /// 与 `web::Query::into_inner` 同义（少数入口按这个形状写）。
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> Deref for Q<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

/// 包装错误：交给 `DomainError` 去生成响应，保证信封与业务错误完全同源。
#[derive(Debug)]
pub struct QError(DomainError);

impl std::fmt::Display for QError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl ResponseError for QError {
    fn error_response(&self) -> HttpResponse {
        self.0.error_response()
    }
    fn status_code(&self) -> StatusCode {
        self.0.status_code()
    }
}

impl<T: DeserializeOwned> FromRequest for Q<T> {
    type Error = QError;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _pl: &mut Payload) -> Self::Future {
        ready(match web::Query::<T>::from_query(req.query_string()) {
            Ok(q) => Ok(Q(q.into_inner())),
            Err(e) => {
                let raw = e.to_string();
                // P-304：查询侧与 JSON 体/路径侧同一口径——serde 原文不进
                // message（这里还多亏包了一层中文前缀，原文照抄等于把
                // 「invalid digit found in string」摊给成员）
                tracing::warn!("查询参数解析失败：{raw}");
                Err(QError(DomainError::Validation(format!(
                    "查询参数格式无效：{}",
                    crate::path_cfg::humanize_parse_error(&raw)
                ))))
            }
        })
    }
}

/// 可选 JSON 请求体（P-478）：形参写 `body: OptJson<MyReq>`，取值仍写
/// `body.and_then(..)` / `body.map(..)` / `body.into_inner()`。
///
/// 它是 `Option<web::Json<T>>` 的替代类型，差别只有一件事：actix 的 Option
/// 提取器（`FromRequestOpt`）把内层**任何**错误折成 `Ok(None)`，于是「载荷写
/// 坏了」不报错，而是静默降级成**一次没带参数的调用**。各端口的代价实测是：
///   * 扣费口（九宫格 / 扭蛋 / 大转盘 / 宠物投喂 / 口粮券核销）幂等键丢失，
///     `idem_key` 回落随机键 ⇒ 网络重试或双击**再扣一次票**；
///   * 贷款还款口 `amount` 丢失 ⇒ 「还 500」被演成**整笔结清**，扣走全部应还；
///   * 临时下载凭证吊销口 `torrent_id` 丢失 ⇒ 「只撤这一种」放大成
///     **撤本人全部**（别的设备的下载链接一起失效）；
///   * 宠物改名口 `name`/`species` 丢失 ⇒ 回 200 报「成功」，一个字没改。
///
/// 空体与裸 `null` 仍算「没带参数」——那是第三方工具与旧客户端在用的兼容
/// 口径，本类型只把**写坏的载荷**从「缺省放行」改回「拒绝」。出口与 `Q<T>`、
/// `path_cfg::json_config()` 同源：1002 + `humanize_parse_error` 的人话细节，
/// serde 原文只进日志不进响应。单口手工版先例：
/// `torrent_http/torrent_delete.rs::parse_body`。
pub struct OptJson<T>(pub Option<T>);

impl<T> OptJson<T> {
    /// 与 `web::Query::into_inner` 同义（拿到内层 `Option<T>`）。
    pub fn into_inner(self) -> Option<T> {
        self.0
    }
    /// 以下三个转发是为了**调用点形状零改动**（换提取器不给文件加行）：
    /// 语义与 `Option` 的同名方法逐字一致。
    pub fn and_then<U>(self, f: impl FnOnce(T) -> Option<U>) -> Option<U> {
        self.0.and_then(f)
    }
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Option<U> {
        self.0.map(f)
    }
    pub fn unwrap_or(self, default: T) -> T {
        self.0.unwrap_or(default)
    }
}

impl<T: DeserializeOwned + 'static> FromRequest for OptJson<T> {
    type Error = actix_web::Error;
    type Future =
        Pin<Box<dyn Future<Output = Result<Self, Self::Error>> + 'static>>;

    fn from_request(req: &HttpRequest, pl: &mut Payload) -> Self::Future {
        let req = req.clone();
        let mut pl = pl.take();
        Box::pin(async move {
            let raw = web::Bytes::from_request(&req, &mut pl).await?;
            if raw.is_empty() {
                return Ok(OptJson(None));
            }
            // 按 `Option<T>` 解：裸 `null` 天然落成 None，`{…}` 落成 Some
            let parsed = serde_json::from_slice::<Option<T>>(&raw);
            parsed.map(OptJson).map_err(|e| {
                let detail = e.to_string();
                tracing::warn!(
                    "可选请求体解析失败，已拒绝而非按缺省放行：{detail}"
                );
                DomainError::Validation(crate::path_cfg::humanize_parse_error(
                    &detail,
                ))
                .into()
            })
        })
    }
}

#[cfg(test)]
mod opt_json_tests {
    use super::OptJson;
    use actix_web::{web, App, HttpResponse};
    use serde::Deserialize;

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Req {
        #[serde(default)]
        amount: Option<i64>,
        #[serde(default)]
        idempotency_key: Option<String>,
    }

    async fn ping(body: OptJson<Req>) -> HttpResponse {
        let b = body.into_inner();
        HttpResponse::Ok().json(serde_json::json!({
            "absent": b.is_none(),
            "amount": b.and_then(|r| r.amount),
        }))
    }

    async fn post(payload: &'static [u8]) -> (u16, serde_json::Value) {
        let app = actix_web::test::init_service(
            App::new().route("/ping", web::post().to(ping)),
        )
        .await;
        let req = actix_web::test::TestRequest::post()
            .uri("/ping")
            .insert_header(("content-type", "application/json"))
            .set_payload(payload)
            .to_request();
        let res = actix_web::test::call_service(&app, req).await;
        let code = res.status().as_u16();
        let json: serde_json::Value =
            serde_json::from_slice(&actix_web::test::read_body(res).await)
                .expect("必须回站内信封");
        (code, json)
    }

    /// P-478 本体：坏载荷必须**报错**，不许折成「没带参数」。
    #[actix_web::test]
    async fn malformed_optional_body_is_refused_not_defaulted() {
        // ① 没带载荷＝兼容口径，照旧 None（空体与裸 null 两条都算）
        for absent in [b"".as_slice(), b"null".as_slice()] {
            let (code, j) = post(absent).await;
            assert_eq!((code, j["absent"].as_bool()), (200, Some(true)));
        }
        // ② 合法载荷照旧受理
        let (code, j) = post(br#"{"amount":500}"#).await;
        assert_eq!((code, j["amount"].as_i64()), (200, Some(500)));
        // ③ 类型不对 ⇒ 旧写法会折成「amount 当没填」，现在必须 400 + 人话
        let (code, j) = post(br#"{"amount":"500"}"#).await;
        assert_eq!(code, 400, "类型不符必须拒：{j}");
        assert_eq!(j["code"].as_i64(), Some(1002));
        assert!(
            j["message"].as_str().unwrap_or("").contains("字段类型不对"),
            "出口该是人话：{j}"
        );
        // ④ 多带的键（deny_unknown_fields）⇒ 旧写法在此丢幂等键 ⇒ 重试双扣
        let (code, j) = post(br#"{"count":5}"#).await;
        assert_eq!(code, 400, "未知键必须拒：{j}");
        assert!(
            j["message"]
                .as_str()
                .unwrap_or("")
                .contains("含未识别的字段"),
            "未知键要点名：{j}"
        );
        // ⑤ JSON 截断 ⇒ 1002，不是「当没填」
        let (code, j) = post(b"{\"amount\":").await;
        assert_eq!(code, 400, "截断必须拒：{j}");
    }
}
