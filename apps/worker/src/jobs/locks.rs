//! with_lock 分布式锁 + job_module/module_on/init_bank_day。
//! 从 jobs.rs 按域拆出。

use sqlx::PgPool;

pub(crate) async fn with_lock<F, T>(db: &PgPool, key: &str, fut: F) -> Option<T>
where
    F: std::future::Future<Output = anyhow::Result<T>>,
{
    // U1 §5.4 模块守卫：job 声明归属模块则按开关整轮跳过（debug 日志，不动账）；
    // 核心任务（announce 计费/快照/清理/反作弊）不在表内 = 不受开关影响。
    // 跳过不报错，恢复开启后靠既有幂等键自然补跑。
    if let Some(module) = job_module(key) {
        if !module_on(db, module).await {
            tracing::debug!(key, module, "module off, skip job");
            return None;
        }
    }
    let mut conn = match db.acquire().await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(?e, key, "advisory lock 连接获取失败，跳过本轮");
            return None;
        }
    };
    let locked: bool =
        sqlx::query_scalar("SELECT pg_try_advisory_lock(hashtext($1))")
            .bind(key)
            .fetch_one(&mut *conn)
            .await
            .unwrap_or(false);
    if !locked {
        tracing::debug!(key, "advisory lock 未抢到（他实例执行中），跳过本轮");
        return None;
    }
    // 业务 future 与锁连接解耦：超时只掐业务，不掐持锁连接
    let outcome =
        tokio::time::timeout(std::time::Duration::from_secs(900), fut).await;
    // 同一连接上解锁（连接归还池前必须释放，否则锁随连接泄漏到复用方）
    let unlock: Result<bool, _> =
        sqlx::query_scalar("SELECT pg_advisory_unlock(hashtext($1))")
            .bind(key)
            .fetch_one(&mut *conn)
            .await;
    if let Err(e) = unlock {
        tracing::error!(?e, key, "advisory unlock 失败（锁将随连接关闭释放）");
    }
    match outcome {
        Ok(Ok(v)) => Some(v),
        Ok(Err(e)) => {
            tracing::error!(?e, key, "job 执行失败");
            None
        }
        Err(_) => {
            tracing::error!(key, "job 超时（900s）被掐断");
            None
        }
    }
}

/// U1 §5.4：job key → 模块键映射（与 API 网关表同口径）。
/// 未列出的 job 属核心层（计费/快照/清理/反作弊/等级），不受模块开关影响。
/// 二审 G8 补登：magic_pool/funding/subtitles/论坛抽奖此前漏挂锁——模块关闭后
/// 仍开全站促销/退款动账/验收交稿/开奖发奖。
fn job_module(job_key: &str) -> Option<&'static str> {
    Some(match job_key {
        "job:bank_daily" => "bank",
        "job:task_settle" => "tasks",
        "job:exam_assign" => "exams",
        "job:jixiao_settle" => "jixiao",
        "job:social_team_settle" | "job:social_team_expire" => "social",
        "job:preserve_exit" | "job:preserve_settle" | "job:preserve_seed" => {
            "preserve"
        }
        "job:resurrection_settle" => "resurrections",
        "job:wishlist_notify" => "wishlist",
        "job:magic_pool_promo" | "job:funding_settle" => "magic_pool",
        "job:subreq_sweep" | "job:subawards" | "job:subcert_sweep" => {
            "subtitles"
        }
        "job:lottery_settle" => "forums",
        _ => return None,
    })
}

/// 模块开关判定（worker 侧直查，无缓存——每分钟 tick 一次，查询代价可忽略；
/// 与 API 的 ModuleFlags::default_on 保持同一缺省口径：缺键=general 中立形态，0178）。
pub(crate) async fn module_on(db: &PgPool, module: &str) -> bool {
    let v: Option<String> =
        sqlx::query_scalar("SELECT value FROM site_settings WHERE name = $1")
            .bind(format!("module_{module}"))
            .fetch_optional(db)
            .await
            .unwrap_or(None);
    match v {
        // 显式配置按配置（no = 关）；查询失败/未配置回落默认值
        // （0178 缺省 = general 中立矩阵：教育考核与重度娱乐关）
        Some(raw) => raw.trim() == "yes",
        None => !matches!(
            module,
            "textbooks"
                | "showcase"
                | "social"
                | "farm"
                | "gomoku"
                | "contests"
                | "jixiao"
                | "exams"
        ),
    }
}

/// 启动基线：当日（站点时区）已由上一进程结算过则不重跑，取健康游标最近记录日期。
pub(crate) async fn init_bank_day(db: &PgPool) -> chrono::NaiveDate {
    let site_today =
        (chrono::Utc::now() + chrono::Duration::hours(8)).date_naive();
    let last_run: Option<chrono::NaiveDate> =
        sqlx::query_scalar("SELECT max(run_date) FROM bank_settle_runs")
            .fetch_one(db)
            .await
            .ok()
            .flatten();
    match last_run {
        // 今日已结 → 以今日为基线（当日不重跑）；更早/无记录 → 昨日（跨日即触发）
        Some(d) if d >= site_today => d,
        _ => site_today - chrono::Duration::days(1),
    }
}
