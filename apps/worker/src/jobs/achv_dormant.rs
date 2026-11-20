//! 成就发放/休眠标记。
//! 从 jobs.rs 按域拆出。
//!
//! 指标口径（0507 运营体检三批参数化）：四族指标此前在本文三处
//! 硬编码同一份 SQL——加一个新成就类型要同步改三处，且与
//! me_achievements 的 CASE、arcade_stubs 的 metric_of 共四处漂移
//! 风险。现改为读 achievement_metrics 注册表（0507，0507 迁移播种
//! 全部既有指标）动态拼聚合 CTE：新增指标 = INSERT 一行注册表，
//! 零代码改动。gacha_draws/stub_count 两例派生类特判保留在代码内
//! （前者查独立表，后者依赖同轮授予结果）。

use sqlx::PgPool;

/// 读注册表拼「metrics CTE 体」：每行 `({expr}) AS {metric}`，
/// expr 里 {u} 占位替换为 users 别名 u。注册表为空时回退空串
/// （调用方 SQL 里保留 seeding_bytes 最小集，成就引擎静默空转）。
async fn metrics_cte(db: &PgPool) -> anyhow::Result<String> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT metric, sql_expr FROM achievement_metrics ORDER BY position",
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .iter()
        .map(|(m, expr)| {
            format!(
                ",\n                   ({}::bigint) AS {}",
                expr.replace("{u}", "u"),
                m
            )
        })
        .collect())
}

/// 成就授予（0079 G6，U3D 口径教育站收敛版）：注册表指标聚合 → 达标授予 + 火花奖励。
/// 幂等：PK (user_id, def_id) 天然防重；奖励走幂等键 achievement:{def_code}:{uid}。
pub(crate) async fn achievement_grant(db: &PgPool) -> anyhow::Result<u64> {
    let metric_cols = metrics_cte(db).await?;
    // 授予：注册表指标逐行比对 def.metric/threshold（JOIN 侧动态拼）
    let insert_sql = format!(
        r#"
        WITH metrics AS (
            SELECT u.id AS user_id{metric_cols}
            FROM users u WHERE u.status < 2
        ),
        m AS (
            {unions}
        ),
        due AS (
            SELECT m.user_id, d.id AS def_id, d.code, d.reward_sparks, m.val
            FROM m JOIN achievement_defs d
              ON d.metric = m.metric AND m.val >= d.threshold
        )
        INSERT INTO user_achievements (user_id, def_id, metric_value)
        SELECT user_id, def_id, val FROM due
        ON CONFLICT (user_id, def_id) DO NOTHING
        "#,
        metric_cols = metric_cols,
        unions = unions_sql(&metric_cols),
    );
    let _res = sqlx::query(&insert_sql).fetch_all(db).await?;
    // RETURNING 只能引用目标表列（code/reward_sparks 属于 achievement_defs）。
    // 0079 上线以来因 RETURNING 语法错误，成就系统从未授予过 —— 改为插入后反查达标行。
    let res: Vec<(i64, i64, String, i64, i64)> = sqlx::query_as(&format!(
        r#"
        SELECT ua.user_id, ua.def_id, d.code, d.reward_sparks, ua.metric_value
        FROM user_achievements ua
        JOIN achievement_defs d ON d.id = ua.def_id
        WHERE (ua.user_id, ua.def_id) IN (
            SELECT m.user_id, d2.id FROM (
                SELECT u.id AS user_id{metric_cols}
                FROM users u WHERE u.status < 2
            ) m JOIN achievement_defs d2
              ON d2.metric = m.metric AND m.val >= d2.threshold
        )
        "#,
        metric_cols = metric_cols
    ))
    .fetch_all(db)
    .await?;
    for &(uid, def_id, ref code, reward, _val) in &res {
        if reward > 0 {
            let idem = format!("achievement:{code}:{uid}");
            // 审计修复（原子性）：INSERT 流水与 UPDATE 余额包进同一事务——
            // 两语句分离时中途崩溃会出现「流水已落、余额未加」（或反之），对账永久撕裂。
            // 幂等护栏只在 INSERT 上判断：事务内已插入的行对本事务可见，
            // UPDATE 再带同款 NOT EXISTS 会恒 0 行 → 整体回滚 → 奖励静默
            // 丢失（链路缺陷 #10 同款）。INSERT 未插入 = 已发过，跳过余额。
            let mut tx = db.begin().await?;
            let inserted = sqlx::query(
                r#"
                INSERT INTO spark_ledger
                    (id, user_id, amount, kind, idempotency_key, balance_after)
                SELECT nextval('spark_ledger_id_seq'), $1, $2, 'achievement', $3,
                  u.spark_balance + $2 FROM users u WHERE u.id = $1
                AND NOT EXISTS (SELECT 1 FROM spark_ledger
                  WHERE idempotency_key = $3)
                "#,
            )
            .bind(uid)
            .bind(reward)
            .bind(&idem)
            .execute(&mut *tx)
            .await?
            .rows_affected()
                > 0;
            if inserted {
                sqlx::query(
                    "UPDATE users SET spark_balance = spark_balance + $2 \
                     WHERE id = $1",
                )
                .bind(uid)
                .bind(reward)
                .execute(&mut *tx)
                .await?;
            }
            tx.commit().await?;
        }
        let _ = def_id;
    }
    if !res.is_empty() {
        tracing::info!(n = res.len(), "achievements grant pass done");
    }
    // 新授予（本轮才落 user_achievements 的行）补发站内信；历史达标行只走上面的
    // 幂等发奖路径，不重复发信（修复每小时向全部历史达标成就重发通知的轰炸）。
    let newly: Vec<(i64, String, i64)> = sqlx::query_as(&format!(
        r#"
        SELECT ua.user_id, d.code, d.reward_sparks
        FROM user_achievements ua
        JOIN achievement_defs d ON d.id = ua.def_id
        WHERE (ua.user_id, ua.def_id) IN (
            SELECT m.user_id, d2.id FROM (
                SELECT u.id AS user_id{metric_cols}
                FROM users u WHERE u.status < 2
            ) m JOIN achievement_defs d2
              ON d2.metric = m.metric AND m.val >= d2.threshold
        )
        AND NOT EXISTS (
            SELECT 1 FROM messages msg
            WHERE msg.receiver_id = ua.user_id
              AND msg.subject = '成就达成'
              AND msg.body LIKE '%「' || d.code || '」%'
        )
        "#,
        metric_cols = metric_cols
    ))
    .fetch_all(db)
    .await?;
    for &(uid, ref code, reward) in &newly {
        let _ = sqlx::query(
            "INSERT INTO messages (sender_id, receiver_id, \
             subject, body) VALUES (NULL, $1, $2, $3)",
        )
        .bind(uid)
        .bind("成就达成")
        .bind(format!(
            "恭喜达成成就「{code}」！奖励 {reward} 魔力已入账。"
        ))
        .execute(db)
        .await;
    }
    if !newly.is_empty() {
        tracing::info!(n = newly.len(), "achievements granted");
    }
    Ok(res.len() as u64)
}

/// m CTE 的 UNION ALL 体：注册表每指标一行 (user_id, metric, val)。
fn unions_sql(metric_cols: &str) -> String {
    let metrics: Vec<&str> =
        metric_cols.split(" AS ").skip(1).filter_map(seg).collect();
    metrics
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let head = if i == 0 {
                "SELECT user_id, "
            } else {
                "UNION ALL SELECT user_id, "
            };
            format!("{head}'{m}' AS metric, {m} AS val FROM metrics")
        })
        .collect::<Vec<_>>()
        .join("\n            ")
}

/// 从片段 `..., (expr) AS name` 序列提取末段指标名。
fn seg(seg: &str) -> Option<&str> {
    let t = seg.trim();
    if t.is_empty() {
        None
    } else {
        Some(t)
    }
}
