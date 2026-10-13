//! 管理组站内信的唯一出口（批五十二 P-197/198）。
//!
//! 三处自动告警原先都写成
//! `INSERT INTO staffmessages … SELECT MIN(id), … FROM users
//!  WHERE class_id>=90`。
//! —— 这是**把聚合当 EXISTS 用**：`SELECT MIN(id)` 没有 GROUP BY，过滤命中
//! 0 行时仍然返回**一行 NULL**，于是 INSERT 真的执行并撞 `user_id` 的
//! NOT NULL 约束。两种正常情况因此都以数据库错误收场：
//!   - `cheat_enforce` 的 7 天去重命中（同一个 (user,agent) 已告警过）；
//!   - 站点里暂时没有 class≥90 的在册账号。
//! 而调用侧用 `.unwrap_or(0)` / `let _: Result<_, _> =` 把错误吞掉
//! ⇒ 「告警没发出去」既看不见也不记账，只在 PG 日志里留一行错误。
//! 本件把「取一个来信人 → 插一条」收在一处：**没有可发对象时静默返回 0**，
//! 真的数据库错误则 `tracing::warn!` 报出来。
//!
//! 语义提示：`staffmessages.user_id` 是**来信人**（见
//! `apps/api/src/community_http/leak_bot.rs:92` 的同名修复注释），可见范围由
//! `permission` 决定 ⇒ 这里挑的 id 只是署名，不是收件人。署名要选**在册**
//! 账号（`status < 2`）：`admin_http/user_del.rs:96` 删号时会
//! `DELETE FROM staffmessages WHERE user_id = $1`，署在墓碑号上的告警会跟着
//! 被清掉。

use sqlx::PgPool;

/// 发一条管理组告警，返回真正写入的行数（0 = 此刻没有可署名的在册职员）。
/// 占位符顺序：`$2=subject`、`$3=body`、`$4=permission`。
pub(crate) async fn staff_alert(
    db: &PgPool,
    subject: &str,
    body: &str,
    permission: &str,
) -> u64 {
    // `t.uid IS NOT NULL` 是关键：把「没有收件署名」变成不插入，而不是插 NULL
    let r = sqlx::query(
        "INSERT INTO staffmessages (user_id, subject, body, permission) \
         SELECT t.uid, $2, $3, $4 \
         FROM (SELECT MIN(id) AS uid FROM users \
                WHERE class_id >= 90 AND status < 2) t \
         WHERE t.uid IS NOT NULL",
    )
    .bind(subject)
    .bind(body)
    .bind(permission)
    .execute(db)
    .await;
    match r {
        Ok(n) => {
            let wrote = n.rows_affected();
            if wrote == 0 {
                tracing::warn!(subject, "staff_alert: 无在册职员可署名，告警未发出");
            }
            wrote
        }
        Err(e) => {
            tracing::warn!(%e, subject, "staff_alert: 管理组告警写入失败");
            0
        }
    }
}

/// 7 天内是否已就同一前缀告警过（去重要读得出来，不能靠 INSERT 报错）。
pub(crate) async fn alerted_within_7d(
    db: &PgPool,
    permission: &str,
    subject: &str,
    body_prefix: &str,
) -> bool {
    sqlx::query_scalar::<_, i64>(
        "SELECT 1 FROM staffmessages \
          WHERE permission = $1 AND subject = $2 AND body LIKE $3 || '%' \
            AND created_at > now() - interval '7 days' LIMIT 1",
    )
    .bind(permission)
    .bind(subject)
    .bind(body_prefix)
    .fetch_optional(db)
    .await
    .unwrap_or(None)
    .is_some()
}
