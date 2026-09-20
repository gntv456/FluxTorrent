//! 图形验证码（0020）：签发与校验。
//! 从 gaps_http.rs 按域拆出。

use actix_web::{get, web, HttpRequest, Responder};
use redis::AsyncCommands;
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
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
    _q: web::Query<CaptchaVerify>,
) -> DomainResult<impl Responder> {
    // 防刷限流（审计 P2）：20+20 算术题可脚本化批量取题，30 次/分钟/ISP 按来源 IP
    let ip = crate::http::client_ip(&req);
    {
        let mut c = state.redis.clone();
        let key = format!("rl:captcha:{ip}");
        let n: i64 = AsyncCommands::incr(&mut c, &key, 1).await.unwrap_or(0);
        if n == 1 {
            let _: () =
                AsyncCommands::expire(&mut c, &key, 60).await.unwrap_or(());
        }
        if n > 30 {
            return Err(DomainError::RateLimited);
        }
    }
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
