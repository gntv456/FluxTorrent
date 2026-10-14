//! 图形验证码（0020）：签发与校验。
//! 从 gaps_http.rs 按域拆出。

use actix_web::{get, web, HttpRequest, Responder};
use redis::AsyncCommands;
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::DomainResult;
use crate::state::AppState;

// ============ 图形验证码（注册防机） ============

#[derive(Deserialize)]
struct CaptchaVerify {
    #[serde(default)]
    _code: String, // 预留：当前简单算术题方案在 issue 时校验答案
}

/// 简易验证码：服务端出算术题（a+b=?），答案存 Redis 5 分钟。
/// 注册时带 captcha_id + captcha_answer 校验（见 register 流程注释；当前先供前端展示与校验闭环）。
#[get("/auth/captcha")]
pub async fn captcha_issue(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    _q: crate::http::Q<CaptchaVerify>,
) -> DomainResult<impl Responder> {
    // 防刷限流（审计 P2）：20+20 算术题可脚本化批量取题，30 次/分钟/ISP 按来源 IP
    let ip = crate::http::client_ip(&req);
    // P-73：本维度是**匿名 IP**，旧写法两处同族病被放大成整片故障：
    // `unwrap_or(0)` 让 Redis 抖动时批量取题不设限；`if n == 1 { expire }`
    // 一失败就留下无 TTL 的计数键，而运营商 CGNAT 后成千用户共用一个出口 IP
    // ⇒ 那个 IP 从此拿不到验证码，连带注册/登录全部走不下去。
    crate::http::rate_window(&state, &format!("captcha:{ip}"), 30, 60).await?;
    use rand::Rng;
    let a: i32 = rand::thread_rng().gen_range(1..=20);
    let b: i32 = rand::thread_rng().gen_range(1..=20);
    let id = uuid::Uuid::new_v4().to_string();
    let mut c = state.redis.clone();
    let _: () =
        AsyncCommands::set_ex(&mut c, format!("captcha:{id}"), a + b, 300)
            .await
            .unwrap_or(());
    Ok(ok(serde_json::json!({
        "captcha_id": id,
        "question": format!("{a} + {b} = ?"),
    })))
}

/// 供注册等内部路径校验验证码。
/// 审计修复（P1 防穷举）：同一 captcha_id 答错即作废 —— 1+20 算术题答案空间仅 2..40，
/// 旧版答错不删键可对同一 id 平均 ~20 次穷举命中，一次性语义只防答对后重放。
#[allow(dead_code)] // 注册流程集成点：前端接入验证码后启用
pub async fn captcha_verify(state: &AppState, id: &str, answer: i32) -> bool {
    let mut c = state.redis.clone();
    let key = format!("captcha:{id}");
    // GETDEL 原子取删：无论对错都消费，杜绝同 id 反复试错
    let expect: Option<i32> = redis::cmd("GETDEL")
        .arg(&key)
        .query_async(&mut c)
        .await
        .unwrap_or(None);
    expect == Some(answer)
}
