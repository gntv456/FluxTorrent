//! GET /register-mode：注册模式公开下发（0204，0227 扩展）。
//! open 模式下前端不强制邀请码；0227 附带验证码驱动配置与申请通道开关。

use actix_web::{get, web, HttpResponse, Responder};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

/// GET /register-mode：注册模式公开下发（0204）——open 模式下前端
/// 不再强制邀请码（与后端 registration_mode 判定同步）。
#[get("/register-mode")]
pub async fn register_mode_endpoint(
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    async fn setting(db: &sqlx::PgPool, name: &str, dflt: &str) -> String {
        sqlx::query_scalar(
            "SELECT COALESCE((SELECT value FROM site_settings \
             WHERE name = $1), $2)",
        )
        .bind(name)
        .bind(dflt)
        .fetch_one(db)
        .await
        .unwrap_or_else(|_| dflt.to_string())
    }
    let mode =
        setting(&state.repo.db, "registration_mode", "invite_only").await;
    // 与注册口同一口径：未知驱动归一为 none，前端据此渲染算术题，
    // 不会出现「驱动不认识 ⇒ 无组件但后端仍在等 token」的死路。
    let provider =
        crate::gaps_http::captcha_drivers::provider(&state.repo.db).await;
    let site_key = setting(&state.repo.db, "captcha_site_key", "").await;
    let app_open = setting(&state.repo.db, "application_signup", "off").await;
    // 0372 M5：答题闸的公开开关——前端据此决定是否渲染答题块
    // （`reg_quiz_category` 不下发：抽题时由 quiz-challenge 自己按站点
    //  分类过滤，前端无需知道分类名）。
    let quiz_on = setting(&state.repo.db, "reg_quiz_enabled", "no").await;
    let quiz_need =
        setting(&state.repo.db, "reg_quiz_pass_required", "yes").await;
    // 开放轮窗口额度（P-684）：reg_window JSON 的公开只读投影——
    // 注册页/登录页据此显示「还剩 N 个名额」与截止时间。窗口外/名额满
    // 投影为 closed，前端按邀请制渲染（读侧惰性判定，无须 worker 关闸）。
    let window = reg_window_public(&state.repo.db, &mode).await;
    // 窗口状态对外的 mode 投影（P-684/P-686）：已开窗口到期/满额 → 回落
    // invite_only（登录页横幅/注册页必填全部跟着回退，站长无须记得关；
    // registration_mode 库值不动——窗口重开时无需再设置）。未开始的
    // 预约窗口不回退（open 在未来时闸门自动拦，open 时刻一到自然放行，
    // 预告信息走 reg_window.not_started + open_at）。
    // P-689：反向也要投影——站点仍是 invite_only、但预约窗口已经到点时，
    // 对外必须报 open。旧写法只做「关」不做「开」，后端已按窗口放行直接
    // 注册，注册页却仍把邀请码标成必填、登录页仍挂邀请制横幅，预约开放
    // 在界面上根本不存在（实测：到点后 mode 仍为 invite_only）。
    let window_closed = window.get("closed") == Some(&serde_json::json!(true))
        && window.get("not_started").is_none();
    let window_active = window.get("closed") == Some(&serde_json::json!(false));
    let mode = if window_active && mode == "invite_only" {
        "open".to_string()
    } else if window_closed && mode != "invite_only" {
        "invite_only".to_string()
    } else {
        mode
    };
    Ok(ok(serde_json::json!({
        "mode": mode,
        "captcha_provider": provider,
        "captcha_site_key": site_key,
        "application_signup": app_open,
        "reg_quiz_enabled": quiz_on == "yes",
        "reg_quiz_pass_required": quiz_need == "yes",
        "reg_window": window,
    })))
}

/// reg_window 的公开投影：{label, slots_left, close_at, closed}。
/// 仅在开放模式（open/email_verify）下有意义；名额口径与闸门一致——
/// 只数直接注册（invited_by IS NULL）。读失败投影 closed=true（fail-closed，
/// 不会把已满的窗口显示成还有名额）。
async fn reg_window_public(db: &sqlx::PgPool, mode: &str) -> serde_json::Value {
    let raw: Option<String> = sqlx::query_scalar(
        "SELECT value FROM site_settings WHERE name = 'reg_window'",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten();
    let Some(raw) = raw.filter(|r| !r.trim().is_empty() && r.trim() != "null")
    else {
        let closed = mode != "open" && mode != "email_verify";
        return serde_json::json!({ "closed": closed });
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return serde_json::json!({ "closed": true });
    };
    let parse = |k: &str| -> Option<chrono::DateTime<chrono::Utc>> {
        v.get(k)
            .and_then(|x| x.as_str())
            .filter(|s| !s.trim().is_empty())
            .and_then(|s| {
                chrono::DateTime::parse_from_rfc3339(s.trim())
                    .ok()
                    .map(|d| d.with_timezone(&chrono::Utc))
            })
    };
    let label = v
        .get("label")
        .and_then(|l| l.as_str())
        .unwrap_or("本轮开放")
        .to_string();
    let now = chrono::Utc::now();
    if let Some(open) = parse("open") {
        if now < open {
            let open_at = parse("open");
            return serde_json::json!({ "label": label, "closed": true,
                "not_started": true,
                "open_at": open_at.map(|t| t.to_rfc3339()) });
        }
    }
    if let Some(close) = parse("close") {
        if now >= close {
            return serde_json::json!({ "label": label, "closed": true });
        }
    }
    let slots = v.get("slots").and_then(|x| x.as_i64()).unwrap_or(0);
    if slots <= 0 {
        // P-689：slots=0 是「不限名额」，旧写法用 i64::MAX 充当剩余数，
        // 界面如实印出「剩余 9223372036854775807 个名额」。不限名额就
        // 不下发 slots_left，前端按 windowNoLimit 文案渲染。
        return serde_json::json!({
            "label": label,
            "close_at": parse("close").map(|c| c.to_rfc3339()),
            "closed": false,
        });
    }
    let since = parse("open").unwrap_or(now - chrono::Duration::days(3650));
    let used: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM users \
         WHERE created_at >= $1 AND invited_by IS NULL",
    )
    .bind(since)
    .fetch_one(db)
    .await
    .unwrap_or(slots);
    let left = slots - used;
    if left <= 0 {
        return serde_json::json!({ "label": label, "closed": true });
    }
    serde_json::json!({
        "label": label,
        "slots_left": left,
        "slots_total": slots,
        "close_at": parse("close").map(|c| c.to_rfc3339()),
        "closed": false,
    })
}
