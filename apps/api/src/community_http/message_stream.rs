//! GET /me/messages/stream：未读数 SSE 流（M16 补）。
//! EventSource 不带 Authorization 头，走 flux_token Cookie 鉴权。
//! 推送节奏：首帧立即推 → 每 15s 轮询、变化才推；每 30s `: ping` 心跳；
//! 连接 10 分钟正常收尾（EventSource 自动重连），断连即清理退出。

use std::sync::Arc;
use std::time::Duration;

use actix_web::web::Bytes;
use actix_web::{get, web, HttpRequest, HttpResponse};
use futures_util::stream::Stream;

use crate::errors::DomainResult;
use crate::http::require_auth;
use crate::state::AppState;

/// 轮询间隔（count 走 idx_messages_receiver_unread 部分索引，成本可忽略）
const POLL: Duration = Duration::from_secs(15);
/// 心跳间隔（注释行防中间层掐空闲连接）
const PING: Duration = Duration::from_secs(30);
/// 单连接生命周期上限：到点流正常结束，前端 EventSource 自动重连
const MAX_LIFE: Duration = Duration::from_secs(600);

/// 主收件箱未读数：谓词与 /me（auth_http/me.rs 的 unread 查询）一字不差——
/// `unread + location=1 + folder IS NULL`（角标数字必须等于点进主收件箱
/// 看到的条数，两处口径分叉会在同一屏上出现两个「未读数」）
async fn unread_inbox(db: &sqlx::PgPool, uid: i64) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM messages WHERE receiver_id = $1 AND \
         unread = true AND location = 1 AND folder IS NULL",
    )
    .bind(uid)
    .fetch_one(db)
    .await
    .unwrap_or(0)
}

/// 待答复工单数：staffmessages 无 unread 谓词（答复状态走 ticket_status），
/// 与 messages 的未读谓词不同域，故 staff 字段恒 0 占位——事件签名与
/// /me 约定一致，后续如需工单角标在此换真查询
async fn staff_pending(_db: &sqlx::PgPool, _uid: i64) -> i64 {
    0
}

/// SSE 帧：unread 事件 + JSON 单行 data（前端按事件名分发后 JSON.parse）
fn unread_frame(n: i64, m: i64) -> Bytes {
    Bytes::from(format!(
        "event: unread\ndata: {{\"count\":{n},\"staff\":{m}}}\n\n"
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/me/messages/stream",
    tag = "社区/消息",
    description = "站内信 SSE 长连接（有新信时推事件）。",
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）")))]
#[get("/me/messages/stream")]
pub(crate) async fn message_stream(
    req: HttpRequest,
    state: web::Data<Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    // EventSource 不能带 Authorization 头；token_from_request 已按
    // Cookie flux_token 回落（auth_infra.rs 双轨），require_auth 直接可用。
    // must_reset 闸门只放行 /me、/me/password*、/auth/logout——本路径
    // 对未改密账号同样 400，与其它 /me/* 端点口径一致
    let auth = require_auth(&req, &state).await?;
    let db = state.repo.db.clone();
    let uid = auth.id;
    Ok(HttpResponse::Ok()
        .content_type("text/event-stream")
        // no-transform：web 容器（Next 15）内置 gzip 会把 text/* 压成
        // chunk 流，但 zlib 要攒够数据才出块——47 字节的 unread 帧会一直
        // 卡在压缩器里，浏览器直到连接结束（10 分钟上限）才拿到未读数。
        // Next 的压缩器遇 Cache-Control 含 no-transform 即跳过（见
        // next/dist/compiled/compression 的 shouldTransform），故这里
        // 声明不可变换；X-Accel-Buffering 只管 nginx，管不到 Next。
        .insert_header(("Cache-Control", "no-cache, no-transform"))
        .insert_header(("X-Accel-Buffering", "no"))
        .streaming(unfold_stream(db, uid)))
}

/// unfold 状态：上次已推的 (count, staff)（None = 首帧必推）+ 下次心跳时刻
struct StreamState {
    db: sqlx::PgPool,
    uid: i64,
    last: Option<(i64, i64)>,
    next_ping: tokio::time::Instant,
    deadline: tokio::time::Instant,
}

/// 事件流：interval 首拍立即到期 ⇒ 连上即推当前未读数；此后每 15s 查一次、
/// 变化才发 unread 帧；每到 30s 发 `: ping` 注释；过 10 分钟期限流返回
/// None 正常收尾。客户端断连时 poll_next 返回 Err/None，actix drop 掉流，
/// 下层 future 随之析构——无泄漏挂起
fn unfold_stream(
    db: sqlx::PgPool,
    uid: i64,
) -> impl Stream<Item = Result<Bytes, actix_web::Error>> {
    let poll = tokio::time::interval(POLL);
    let now = tokio::time::Instant::now();
    let st = StreamState {
        db,
        uid,
        last: None,
        next_ping: now + PING,
        deadline: now + MAX_LIFE,
    };
    futures_util::stream::unfold((st, poll), |(mut st, mut poll)| async move {
        loop {
            if tokio::time::Instant::now() >= st.deadline {
                return None; // 10 分钟到点：正常结束，浏览器自动重连
            }
            tokio::select! {
                _ = poll.tick() => {}
                _ = tokio::time::sleep_until(st.next_ping) => {
                    st.next_ping += PING;
                    return Some((
                        Ok(Bytes::from_static(b": ping\n\n")),
                        (st, poll),
                    ));
                }
            }
            let n = unread_inbox(&st.db, st.uid).await;
            let m = staff_pending(&st.db, st.uid).await;
            if st.last != Some((n, m)) {
                st.last = Some((n, m));
                return Some((Ok(unread_frame(n, m)), (st, poll)));
            }
            // 值未变：本轮不发帧，回到 select 等下一拍
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// SSE 帧格式契约：前端 EventSource 按 `event: unread` 分发、JSON.parse(data)
    #[test]
    fn unread_frame_shape() {
        let f = unread_frame(3, 0);
        let s = String::from_utf8(f.to_vec()).unwrap();
        assert!(s.starts_with("event: unread\n"));
        assert!(s.contains("\"count\":3"));
        assert!(s.contains("\"staff\":0"));
        assert!(s.ends_with("\n\n"), "SSE 帧必须空行收尾");
    }

    /// 节奏契约：轮询 15s / 心跳 30s / 生命周期 10min（文档口径）
    #[test]
    fn timing_contract() {
        assert_eq!(POLL, Duration::from_secs(15));
        assert_eq!(PING, Duration::from_secs(30));
        assert_eq!(MAX_LIFE, Duration::from_secs(600));
    }

    /// 心跳帧契约：SSE 注释行（冒号开头），不产生事件
    #[test]
    fn ping_frame_is_comment() {
        let s = String::from_utf8(Bytes::from_static(b": ping\n\n").to_vec())
            .unwrap();
        assert!(s.starts_with(":"));
        assert!(s.ends_with("\n\n"));
    }

    /// 首推语义：StreamState.last 初始 None ⇒ 首帧无论计数是否为 0 都会推
    /// （前端连上即拿到当前值，不必等变化）。unfold 分支的判定就是
    /// `last != Some((n, m))`，这里直接断言该表达式对 (0,0) 成立
    #[test]
    fn first_frame_always_emits() {
        let last: Option<(i64, i64)> = None;
        assert!(last != Some((0, 0)));
        // 推过一次 (0,0) 后：值未变不再推（变化才推的口径）
        let last = Some((0, 0));
        assert!(!last.filter(|l| *l != (0, 0)).is_some());
    }
}
