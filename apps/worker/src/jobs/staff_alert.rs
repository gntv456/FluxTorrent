//! 管理组站内信的唯一出口（批五十二 P-197/198/199）。
//!
//! 四处自动告警原先各写各的：三处（`cheat_enforce` / `audit` 的差额告警 /
//! `sweep` 的积压告警）写成
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
/// 占位符顺序：`$1=subject`、`$2=body`、`$3=permission`。sqlx 按**绑定顺序**
/// 编号，写 `$2..$4` 会让 PG 认为要 4 个参数而只收到 3 个（bind message
/// supplies 3 parameters ... requires 4）——本件首版就中过这一枪，
/// 是它自己新加的 `warn!` 在一轮里把它暴露出来的。
pub(crate) async fn staff_alert(
    db: &PgPool,
    subject: &str,
    body: &str,
    permission: &str,
) -> u64 {
    staff_alert_as(db, subject, body, permission, 90).await
}

/// 同上，但**署名档位**可指定：默认 90（老口径：class_id >= 90 的在册职员），
/// 自动审种批次用的是 99（与 leak_bot 的署名口径一致：class_id >= 99）。
/// 两档都收在**这个出口**里写，闸门 BQ 的 R1d（worker 里写 staffmessages 的
/// 地方只许有出口件一处）才站得住——散落的第二处会各自长出不同的守卫。
pub(crate) async fn staff_alert_as(
    db: &PgPool,
    subject: &str,
    body: &str,
    permission: &str,
    min_class: i32,
) -> u64 {
    // `t.uid IS NOT NULL` 是关键：把「没有收件署名」变成不插入，而不是插 NULL
    let r = sqlx::query(
        "INSERT INTO staffmessages (user_id, subject, body, permission) \
         SELECT t.uid, $1, $2, $3 \
         FROM (SELECT MIN(id) AS uid FROM users \
                WHERE class_id >= $4 AND status < 2) t \
         WHERE t.uid IS NOT NULL",
    )
    .bind(subject)
    .bind(body)
    .bind(permission)
    .bind(min_class)
    .execute(db)
    .await;
    match r {
        Ok(n) => {
            let wrote = n.rows_affected();
            if wrote == 0 {
                tracing::warn!(
                    subject,
                    "staff_alert: 无在册职员可署名，告警未发出"
                );
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
///
/// ⚠ 返回类型必须是 `bool`：本件首版写 `SELECT 1` + `query_scalar::<_, i64>`，
/// PG 的 `1` 是 int4，sqlx 按 i64 解码报错 ⇒ 错误被当成「没告警过」，
/// 去重整条失效，每个 tick 重发一封（闸门 BQ 的 R4a/R4c 抓到并钉住）。
pub(crate) async fn alerted_within_7d(
    db: &PgPool,
    permission: &str,
    subject: &str,
    body_prefix: &str,
) -> bool {
    let r = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM staffmessages \
          WHERE permission = $1 AND subject = $2 AND body LIKE $3 || '%' \
            AND created_at > now() - interval '7 days')",
    )
    .bind(permission)
    .bind(subject)
    .bind(body_prefix)
    .fetch_one(db)
    .await;
    match r {
        Ok(hit) => hit,
        Err(e) => {
            tracing::warn!(
                %e,
                subject,
                "alerted_within_7d: 去重读取失败，本轮按未告警处理"
            );
            false
        }
    }
}

/// 每用户每日闸（2026-11-15 洪水纠偏第二级）：7 天前缀去重的粒度是
/// 「每个 (user,agent)/band 前缀各一封」，96 个 gating 用户 × 多前缀
/// 三天灌了 8161 封。同用户 24h 内已就**任意**该类告警过则静默，
/// 与前缀闸叠加生效。读失败按未告警处理（同上，宁重发勿丢告警）。
pub(crate) async fn alerted_within_24h(
    db: &PgPool,
    permission: &str,
    subject: &str,
    uid: i64,
) -> bool {
    let r = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM staffmessages \
          WHERE permission = $1 AND subject = $2 \
            AND body LIKE '[[%]]用户 #' || $3 || ' %' \
            AND created_at > now() - interval '24 hours')",
    )
    .bind(permission)
    .bind(subject)
    .bind(uid.to_string())
    .fetch_one(db)
    .await;
    r.unwrap_or(false)
}

/// 同主题时间窗闸（2026-11-17 四口收编）：不解析 body 形态，只看
/// 「同一 (permission, subject) 在窗口内是否已发过」。给 body 无统一
/// 前缀锚点的发射口用（黑白名单/差额审计/Ratio Watch/邮件死信）——
/// 这些口的事件键天然分散（每颗种子/每封信各一 key），按前缀去重
/// 等于不去重，主题级窗口才是有效闸。
///
/// ⚠ `$3::int` 不能省：make_interval 的 hours 形参是 int4，Rust i64 绑成
/// int8 无隐式转换——首版没强转，每调必报 42883（function make_interval
/// (hours => bigint) does not exist），被下方 unwrap_or(false) 静默吞成
/// 「没告警过」⇒ 闸恒开。同坑见 cheat_enforce.rs / audit.rs / class_adj.rs
/// 的事故注释，那里都是因此补的强转。
pub(crate) async fn alerted_subject_within(
    db: &PgPool,
    permission: &str,
    subject: &str,
    hours: i64,
) -> bool {
    let r = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM staffmessages \
          WHERE permission = $1 AND subject = $2 \
            AND created_at > now() - make_interval(hours => $3::int))",
    )
    .bind(permission)
    .bind(subject)
    .bind(hours)
    .fetch_one(db)
    .await;
    match r {
        Ok(v) => v,
        // 查询失败要显性（warn），不能静默当「没发过」——那等于关闸。
        Err(e) => {
            tracing::warn!(error = %e, subject, "staff_alert: 主题窗闸查询失败（按未发处理）");
            false
        }
    }
}

/// 组合件（2026-11-17 四口收编）：同主题窗口内未告警过才发，返回是否真发。
/// 与 alerted_subject_within 配套，调用点一行可写：
/// `staff_alert_gated(db, "cheater", "主题", 24, &body).await;`
pub(crate) async fn staff_alert_gated(
    db: &PgPool,
    permission: &str,
    subject: &str,
    hours: i64,
    body: &str,
) -> bool {
    if alerted_subject_within(db, permission, subject, hours).await {
        tracing::debug!(subject, hours, "staff_alert: 主题窗闸静默");
        return false;
    }
    staff_alert(db, subject, body, permission).await;
    true
}
