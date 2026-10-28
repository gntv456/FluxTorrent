//! 新手欢迎 PM（0424 P1-4 从 register.rs 拆出，300 行门禁）。
//!
//! 按 onboarding_preset 模板分支——考核淘汰制（strict）站的新人最该看到
//! 「转正考核与 H&R 红线」，宽进/未设走签到抽卡的游戏化口径。
//! 失败不回滚注册（附属数据，与字段值落库同口径）。

use sqlx::PgPool;

pub(super) async fn send_welcome_pm(db: &PgPool, user_id: i64) {
    let site_name: String = sqlx::query_scalar(
        "SELECT value FROM site_settings WHERE name = 'site_name'",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .unwrap_or_else(|| "站点".into());
    let preset: String = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings \
         WHERE name = 'onboarding_preset'), 'none')",
    )
    .fetch_one(db)
    .await
    .unwrap_or_else(|_| "none".into());
    let exam_days: i64 = sqlx::query_scalar(
        "SELECT COALESCE(NULLIF((SELECT value FROM site_settings \
         WHERE name = 'exam_onboard_days'), '')::bigint, 30)",
    )
    .fetch_one(db)
    .await
    .unwrap_or(30);
    let (subject, body) = if preset == "strict" {
        (
            format!("欢迎来到{site_name}！先看你的转正考核"),
            format!(
                "欢迎！你所在的是考核制站点，注册后 {exam_days} 天内会自动\
                 派发转正考核（「成长荣誉 → 我的考核」随时看进度）。起步三步：\
                 \n1. 先读「站点规则」与「客户端指南」（DHT/PEX 必关，开着\
                 会触发作弊判定）；\n2. 每天签到领魔力，下载后保持做种\
                 （H&R 口径见「我的 H&R」）；\n3. 做种赚魔力才是正道——\
                 上传你的第一颗种子吧！"
            ),
        )
    } else {
        (
            format!("欢迎来到{site_name}！你的起步三步"),
            "欢迎！起步三步：\n1. 每天「任务中心 → 每日签到」领魔力与抽卡券；\
             \n2. 用魔力去「娱乐屋」玩一把（刮刮乐 10 魔力起）；\
             \n3. 抽卡的首次单抽只要 1 张券（票面 25 券），试试手气。\n\n\
             做种赚魔力才是正道——上传你的第一颗种子吧！"
                .to_string(),
        )
    };
    let _ = sqlx::query(
        "INSERT INTO messages (sender_id, receiver_id, subject, body, \
         location, saved, unread) \
         VALUES (NULL, $1, $2, $3, 1, 1, true)",
    )
    .bind(user_id)
    .bind(subject)
    .bind(body)
    .execute(db)
    .await;
}
