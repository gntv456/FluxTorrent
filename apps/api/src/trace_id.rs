//! traceId 中间件（0424，同类站对标 B 批）。
//!
//! 每个请求生成一个短随机 trace id，注入响应头 `X-Trace-Id` 并打一行
//! `trace_id` 索引日志——用户报障时报这个 id，运维按 `trace_id=xxx`
//! 一击检索到该请求的全部访问/错误日志（同类站 的报障流程同款口径：
//! 「请把响应头里的 traceid 发到 develop@」）。
//!
//! 为什么不走 access_log::Logger：actix Logger 的格式符不支持响应头
//! 注入值，且 exclude 逻辑已有一套（passkey 脱敏），不能推倒。本件是
//! 独立薄 Transform：只加头 + 一行 info 索引，不碰请求体。

use actix_web::{
    body::MessageBody,
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
};

/// 8 字节随机 hex（16 字符）——日志洪流里足够唯一定位，又短到用户可手抄。
fn short_trace_id() -> String {
    use rand::RngCore;
    let mut b = [0u8; 8];
    rand::thread_rng().fill_bytes(&mut b);
    b.iter().map(|x| format!("{x:02x}")).collect()
}

pub struct TraceId;

pub struct TraceIdMiddleware<S> {
    service: S,
}

impl<S, B> Transform<S, ServiceRequest> for TraceId
where
    S: Service<
        ServiceRequest,
        Response = ServiceResponse<B>,
        Error = actix_web::Error,
    >,
    S::Future: 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<B>;
    type Error = actix_web::Error;
    type InitError = ();
    type Transform = TraceIdMiddleware<S>;
    type Future = std::future::Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        std::future::ready(Ok(TraceIdMiddleware { service }))
    }
}

impl<S, B> Service<ServiceRequest> for TraceIdMiddleware<S>
where
    S: Service<
        ServiceRequest,
        Response = ServiceResponse<B>,
        Error = actix_web::Error,
    >,
    S::Future: 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<B>;
    type Error = actix_web::Error;
    type Future = futures_util::future::LocalBoxFuture<
        'static,
        Result<Self::Response, Self::Error>,
    >;

    fn poll_ready(
        &self,
        ctx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Result<(), Self::Error>> {
        self.service.poll_ready(ctx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let tid = short_trace_id();
        let path = req.path().to_string();
        let method = req.method().to_string();
        let fut = self.service.call(req);
        Box::pin(async move {
            tracing::info!(
                trace_id = %tid, method = %method, path = %path, "request"
            );
            let mut res = fut.await?;
            if let Ok(v) = actix_web::http::header::HeaderValue::from_str(&tid)
            {
                res.response_mut().headers_mut().insert(
                    actix_web::http::header::HeaderName::from_static(
                        "x-trace-id",
                    ),
                    v,
                );
            }
            Ok(res)
        })
    }
}
