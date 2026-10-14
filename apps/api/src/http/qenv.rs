//! 站内信封版查询参数提取器（批七十六 P-280）。
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

use std::future::{ready, Ready};
use std::ops::Deref;

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
