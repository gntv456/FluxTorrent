//! 安装向导状态端点（从 setup_http.rs 拆出，300 行门禁）。

use actix_web::{get, web, Responder};

use crate::dto::ok;
use crate::state::AppState;

/// 向导状态（公开）
#[get("/setup/status")]
pub(super) async fn setup_status(
    state: web::Data<std::sync::Arc<AppState>>,
) -> impl Responder {
    let done: String = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings WHERE name = \
         'setup_done'), '')",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or_default();
    let packs: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT code, name, \
         COALESCE(description, '') FROM site_type_packs ORDER BY sort",
    )
    .fetch_all(&state.repo.db)
    .await
    .unwrap_or_default();
    let has_admin: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM users WHERE class_id = 99)",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    // 引导 root 仍是临时密码（G12 复验发现）：临时密码下 must_reset 闸门只
    // 放行改密/登出/自身信息，POST /setup 必被拦（「请先修改密码后再操作」）。
    // 向导据此显示「就地改密」引导，否则站长在最后一步无路可走。
    //
    // 同时回传账号名与「是否仍是引导默认口令」——向导第二步据此把
    // 「旧密码」自动填好、只让站长设新密码。此前那两个框要站长自己填
    // 用户名和旧密码，而两者都是系统已知的常量（0017 引导 root +
    // password123），于是「不知道该填什么」成为 top 困惑，填错直接
    // 报「凭证无效」。is_default_pw 由哈希比对得出（argon2 哈希不可
    // 逆推，比对的是 0017 写入的那个已知常量），不是「用户名==root
    // 就假定默认密码」——后者在管理员被 ipcheck 重置过临时密码后会
    // 让向导静默用错口令。
    let temp_admin: Option<(String, String)> = sqlx::query_as(
        "SELECT username, pass_hash FROM users WHERE class_id = 99 \
         AND must_reset_password ORDER BY id LIMIT 1",
    )
    .fetch_optional(&state.repo.db)
    .await
    .unwrap_or(None);
    let root_temp = temp_admin.is_some();
    let temp_admin_username = temp_admin
        .as_ref()
        .map(|(u, _)| u.clone())
        .unwrap_or_default();
    // 0017_bootstrap_root.sql 写入的引导口令哈希（明文 password123）。
    // 哈希变了（已改密 / 被 ipcheck 重置）就不再自动填，交回站长手填。
    const BOOTSTRAP_ROOT_HASH: &str = concat!(
        "$argon2id$v=19$m=19456,t=2,p=1$",
        "XAMwi8WTuzejCBPhdilR6w$",
        "xUb/nkW8/iUYTMb+dCPsstkyeldF5LuM2sOIK4m8++c",
    );
    let temp_admin_default_pw = temp_admin
        .as_ref()
        .map(|(_, h)| h == BOOTSTRAP_ROOT_HASH)
        .unwrap_or(false);
    // 开站 checklist（0209 P2-12）：三项「不配也能跑，但配错会坑」的检查。
    // announce_url 默认 127.0.0.1 → 下载的 .torrent 带内网地址；
    // SMTP 空 → 找回密码/邀请信静默降级；注册默认 invite_only。
    let (announce, smtp_host, reg_mode, site_name): (
        String,
        String,
        String,
        String,
    ) = sqlx::query_as(
        "SELECT \
             COALESCE(MAX(value) FILTER \
                 (WHERE name = 'announce_url'), ''), \
             COALESCE(MAX(value) FILTER \
                 (WHERE name = 'smtp_host'), ''), \
             COALESCE(MAX(value) FILTER \
                 (WHERE name = 'registration_mode'), 'invite_only'), \
             COALESCE(MAX(value) FILTER \
                 (WHERE name = 'site_name'), '') \
             FROM site_settings WHERE name IN \
                 ('announce_url', 'smtp_host', 'registration_mode', \
                  'site_name')",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or_default();
    ok(serde_json::json!({
        "done": done == "done",
        "has_admin": has_admin,
        // 引导 root 仍用临时密码 → 前端就地改密（见上方注释）
        "root_temp_password": root_temp,
        // 该临时密码管理员的账号名 + 其口令是否仍为引导默认值：
        // 两者皆已知时前端自动填，只让站长设新密码（少两个无从填的框）
        "temp_admin_username": temp_admin_username,
        "temp_admin_default_pw": temp_admin_default_pw,
        // 当前站名（G12）：向导 Step2 站点名「可留空」要看得到留空的后果；
        // 空由前端回落到字典默认文案
        "site_name": site_name,
        "packs": packs.into_iter().map(|(code, name, descr)| {
            serde_json::json!({
                "code": code,
                "name": name,
                "description": descr
            })
        }).collect::<Vec<_>>(),
        "checklist": {
            "announce_local": announce.contains("127.0.0.1")
                || announce.contains("localhost"),
            "smtp_unset": smtp_host.trim().is_empty(),
            "registration_mode": reg_mode,
        },
    }))
}
