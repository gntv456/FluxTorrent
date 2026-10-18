//! 抽奖公平性验证（commit-reveal 的读面）。
//!
//! commit-reveal 的全部意义在于「第三方能独立复算」——只公布
//! commit/seed 而不给验证路径，等于让用户相信而不是让用户确认。
//!
//! 三层可查，缺一层都会让用户无法自证：
//!   1. **开售中**：只看得到 commit（seed 未揭晓，符合协议）；
//!   2. **已开奖**：拿到 seed + tickets_root + rand + 中奖票据哈希，
//!      用户可自行跑 SHA-256/HMAC 复算；
//!   3. **服务端也复算一遍**并回 `verified` —— 本站自己先验，
//!      用户不必先信任本站才会验证。

use actix_web::{get, web, HttpRequest, HttpResponse};

use crate::domain::lottery_fair as fair;
use crate::dto::ok;
use crate::errors::DomainResult;
use crate::http::require_auth;
use crate::repo::lottery_fair as repo_lf;
use crate::state::AppState;

/// 抽奖公平性验证材料。
///
/// 未登录也可查——验证页的前提是「任何人都能独立验证」，加登录门
/// 等于把验证权收回到站点手里，那这机制就白做了。
#[utoipa::path(
    get,
    path = "/api/v1/forums/lottery/{topic_id}/fairness",
    tag = "社区/论坛",
    description = "抽奖公平性验证材料（commit-reveal 的公开承诺与证明）。",
    params(
        ("topic_id" = i64, description = "抽奖主题 id"),
    ),
    responses(
        (status = 200, description = "验证材料（含服务端复算结果"),
        (status = 404, description = "抽奖不存在")))]
#[get("/forums/lottery/{topic_id}/fairness")]
pub(crate) async fn lottery_fairness(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    // 不强制登录：验证必须对所有人开放（见模块头）
    let _ = require_auth(&req, &state).await.ok();
    let topic_id = path.into_inner();

    let Some(mat) = repo_lf::public_commit(&state.repo, topic_id).await? else {
        return Ok(ok(serde_json::json!({
            "exists": false,
            "note": "该抽奖未登记可验证承诺（可能是老抽奖或未启用公平模式）",
        })));
    };
    let commit = mat.commit;
    let Some(proof) = mat.proof else {
        // 开售中：只公布 commit，seed 未揭晓
        return Ok(ok(serde_json::json!({
            "exists": true,
            "stage": "open",
            "commit": commit,
            "note": "开奖后公开 seed 与 rand，届时可独立复算。",
        })));
    };

    // 已开奖：把公开材料 + 服务端复算结果一起给出
    let tickets = repo_lf::ticket_hashes(&state.repo, topic_id).await?;
    let round_id = topic_id.to_string();
    let winners_claimed = proof
        .get("winner_ticket_hashes")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);

    // 服务端自验：把 proof 还原成结构体再跑 verify()。
    // 验不过也要返回（且明确 verified=false），而不是报错 500——
    // 「验不过」本身就是要告诉用户的事实。
    let (verified, verify_note) =
        match serde_json::from_value::<fair::DrawProof>(proof.clone()) {
            Ok(p) => {
                let n = p.winner_ticket_hashes.len();
                match fair::verify(&commit, &p, &tickets, n) {
                    Ok(v) => (
                        true,
                        format!(
                            "服务端复算通过：{} 名参与者 / {} 名中奖",
                            v.participants, v.winners
                        ),
                    ),
                    Err(e) => (false, format!("服务端复算失败：{e}")),
                }
            }
            Err(e) => (false, format!("证明材料无法解析：{e}")),
        };

    Ok(ok(serde_json::json!({
        "exists": true,
        "stage": "drawn",
        "commit": commit,
        "proof": proof,
        "ticket_count": tickets.len(),
        "winners_claimed": winners_claimed,
        "verified": verified,
        "verify_note": verify_note,
        "algorithm": {
            "ticket_hash": "SHA-256(seed_hex ‖ prev_hash ‖ nonce_be32)",
            "tickets_root": "最后一张票据的哈希",
            "rand": "HMAC-SHA-256(key=seed_hex, msg=round_id ‖ tickets_root)",
            "order_key": "HMAC-SHA-256(seed_hex, 0x6f726465725f|ticket_hash)",
            "pick": "按 order_key 升序取前 N 名",
            "note": "所有输入都是 hex 文本，无需自行解码；rand 绑 tickets_root，\
    事后改名单会让公开的 rand 对不上。",
        },
    })))
}
