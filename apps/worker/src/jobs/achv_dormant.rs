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

/// 读注册表：返回 `(metrics CTE 体, 指标名序列)`。每行拼
/// `({expr}) AS {metric}`，expr 里 {u} 占位替换为 users 别名 u。
/// 名字直接从注册表行带出（旧版从拼好的 CTE 字符串 split(" AS ")
/// 反解析——指标表达式自带 AS/别名时中段被切飞，UNION 里拼出
/// `'name,\n (expr)::bigint)'` 这种字符串字面量与括号残肢，运行期
/// syntax error，成就引擎因此从未跑通过一轮）。注册表为空时回退
/// 空串（调用方 SQL 里保留 seeding_bytes 最小集，成就引擎静默空转）。
async fn metrics_cte(db: &PgPool) -> anyhow::Result<(String, Vec<String>)> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT metric, sql_expr FROM achievement_metrics ORDER BY position",
    )
    .fetch_all(db)
    .await?;
    let names: Vec<String> = rows.iter().map(|(m, _)| m.clone()).collect();
    let cols = rows
        .iter()
        .map(|(m, expr)| {
            format!(
                ",\n                   ({}::bigint) AS {}",
                expr.replace("{u}", "u"),
                m
            )
        })
        .collect();
    Ok((cols, names))
}

/// 成就授予（0079 G6，U3D 口径教育站收敛版）：注册表指标聚合 → 达标授予 + 火花奖励。
/// 幂等：PK (user_id, def_id) 天然防重；奖励走幂等键 achievement:{def_code}:{uid}。
pub(crate) async fn achievement_grant(db: &PgPool) -> anyhow::Result<u64> {
    let (metric_cols, metric_names) = metrics_cte(db).await?;
    let unions = unions_sql(&metric_names);
    // 「达标行」生成器：metrics 宽表 → m 纵表 (user_id, metric, val) → 与 defs 比对。
    // 三条查询共用同一结构——旧版第二条/第三条把宽表内联后直接引用 m.metric/m.val
    // （宽表没有这两列），写成那天起就是坏的；旧版第一条 INSERT 因 split 损坏
    // 先炸，永远轮不到它们，本次修复第一条后才暴露（live_db_tests 实测复现）。
    let due_sql = |select: &str, tail: &str| {
        format!(
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
        {select}
        FROM due d{tail}
        "#
        )
    };
    // 授予：注册表指标逐行比对 def.metric/threshold
    let insert_sql = due_sql(
        "INSERT INTO user_achievements (user_id, def_id, metric_value)
        SELECT user_id, def_id, val",
        "
        ON CONFLICT (user_id, def_id) DO NOTHING",
    );
    let _res = sqlx::query(&insert_sql).fetch_all(db).await?;
    // RETURNING 只能引用目标表列（code/reward_sparks 属于 achievement_defs）。
    // 改为插入后反查达标行。
    let res: Vec<(i64, i64, String, i64, i64)> = sqlx::query_as(&due_sql(
        "SELECT d.user_id, d.def_id, d.code, d.reward_sparks, d.val",
        "",
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
                SELECT nextval('spark_ledger_id_seq'), $1, $2,
                       'achievement', $3,
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
    // 语义：due（本轮达标）∩ user_achievements（已落库）∩ 未发过「成就达成」PM。
    // 独立 format!：主体是 ua 驱动，due 只作 IN 子查询；CTE 结构与 due_sql 同源。
    let newly: Vec<(i64, String, i64)> = sqlx::query_as(&format!(
        r#"
        WITH metrics AS (
            SELECT u.id AS user_id{metric_cols}
            FROM users u WHERE u.status < 2
        ),
        m AS (
            {unions}
        ),
        due AS (
            SELECT m.user_id, d.id AS def_id
            FROM m JOIN achievement_defs d
              ON d.metric = m.metric AND m.val >= d.threshold
        )
        SELECT ua.user_id, d.code, d.reward_sparks
        FROM user_achievements ua
        JOIN achievement_defs d ON d.id = ua.def_id
        WHERE (ua.user_id, ua.def_id) IN (SELECT user_id, def_id FROM due)
        AND NOT EXISTS (
            SELECT 1 FROM messages msg
            WHERE msg.receiver_id = ua.user_id
              AND msg.subject = '成就达成'
              AND msg.body LIKE '%「' || d.code || '」%'
        )
        "#,
        metric_cols = metric_cols,
        unions = unions
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
fn unions_sql(metric_names: &[String]) -> String {
    metric_names
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

#[cfg(test)]
mod live_db_tests {
    //! DB 级实测（`#[ignore]` 门控：CI 无数据库；本地对准隧道后的真库跑）。
    //! 用途：2026-11-23 dev.44 上线后 achievement_grant 报
    //! `column m.metric does not exist`，而同形状 SQL 手跑通过——
    //! 需要在真函数上复现才能定位。

    use super::*;

    async fn pool() -> sqlx::PgPool {
        let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
            "postgres://flux:flux@127.0.0.1:5432/fluxtorrent".into()
        });
        sqlx::postgres::PgPoolOptions::new()
            .max_connections(2)
            .connect(&url)
            .await
            .expect("connect")
    }

    #[tokio::test]
    #[ignore]
    async fn achievement_grant_on_live_db() {
        let db = pool().await;
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT metric, sql_expr
             FROM achievement_metrics ORDER BY position",
        )
        .fetch_all(&db)
        .await
        .unwrap();
        eprintln!("registry rows = {}", rows.len());
        eprintln!(
            "names = {:?}",
            rows.iter().map(|(m, _)| m).collect::<Vec<_>>()
        );
        let n = achievement_grant(&db).await.expect("grant must succeed");
        eprintln!("grant ok, n = {n}");
    }
}
