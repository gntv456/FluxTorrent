//! with_lock 分布式锁 + job_module/module_on/init_bank_day。
//! 从 jobs.rs 按域拆出。

use sqlx::PgPool;
use std::sync::OnceLock;
use tokio::sync::Semaphore;

/// job 硬超时：超时只掐业务 future，不掐持锁连接（见 with_lock）。
/// 超时文案（日志 + job_status.last_result）的数值由此常量派生，改一处即全同步。
pub(crate) const JOB_HARD_TIMEOUT_SECS: u64 = 900;

/// 一条 job 在 `with_lock` 里占**两条**连接：持 advisory lock 的连接
/// （整个 job 期间不放，否则 session 锁会跟着泄）+ 业务体自己从池里再取一条。
/// 调度器一分钟能扇出 15 条 job（2026-10-20 现网 `job_status` 实测：
/// 同一分钟内最多 15 行 last_started_at 相同），16 条池必然有一半的 job
/// 抢不到连接 ⇒ `pool timed out while waiting for an open connection`，
/// 被掐掉的是 hr_punish / hr_warn_ladder / cheat_enforce / collusion_check /
/// class_auto_adjust / sweep_stale_peers 这一族「该执行没执行」的任务。
///
/// 这里把「抢连接抢到超时」换成「抢槽位，抢不到就排队」：排队期间不动账，
/// 超过排队宽限就按既有 Skip 语义跳过本轮（下一 tick 自然补跑，job 都有幂等键）。
const JOB_SLOT_PER_JOB: usize = 2;
const SLOT_QUEUE_GRACE_SECS: u64 = 60;

/// 并发槽位数 = 连接池容量的一半（`with_lock` 每条 job 占 2 条连接）。
/// 池容量在 main.rs（缺省 `DEFAULT_POOL_SIZE`=32，可经 DB_POOL_SIZE 覆盖）；
/// 两处数字必须成倍数关系，闸门 pt_audit_worker_job_health 的 J3 就是钉这一条的
/// （历史上「改了一处忘了另一处」在这仓里反复出现过，见 P-426 一族）。
/// 运维若把 DB_POOL_SIZE 调大，这里可以跟着调；不调也不会失败，只是并发用不满。
const JOB_SLOTS_MAX: usize = 16;

static JOB_SLOTS: OnceLock<Semaphore> = OnceLock::new();

fn job_slots() -> &'static Semaphore {
    JOB_SLOTS.get_or_init(|| {
        tracing::info!(
            slots = JOB_SLOTS_MAX,
            "job 并发槽位（每 job 占 2 条连接）"
        );
        Semaphore::new(JOB_SLOTS_MAX)
    })
}

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
    // 槽位先于连接：抢不到槽位就按既有 Skip 语义跳过本轮，
    // 而不是把「等待连接」变成一次失败（失败会让 job_status 亮红且整轮动作没做）
    let _permit = match tokio::time::timeout(
        std::time::Duration::from_secs(SLOT_QUEUE_GRACE_SECS),
        job_slots().acquire(),
    )
    .await
    {
        Ok(Ok(p)) => p,
        Ok(Err(_)) => {
            tracing::error!(key, "job 槽位信号量已关闭，跳过本轮");
            return None;
        }
        Err(_) => {
            tracing::warn!(
                key,
                secs = SLOT_QUEUE_GRACE_SECS,
                "并发槽位排队超时，跳过本轮（下一 tick 自然补跑）"
            );
            return None;
        }
    };
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
    let job = key.strip_prefix("job:").unwrap_or(key);
    mark_start(db, job).await;
    let outcome = tokio::time::timeout(
        std::time::Duration::from_secs(JOB_HARD_TIMEOUT_SECS),
        fut,
    )
    .await;
    // 同一连接上解锁（连接归还池前必须释放，否则锁随连接泄漏到复用方）
    let unlock: Result<bool, _> =
        sqlx::query_scalar("SELECT pg_advisory_unlock(hashtext($1))")
            .bind(key)
            .fetch_one(&mut *conn)
            .await;
    if let Err(e) = unlock {
        tracing::error!(?e, key, "advisory unlock 失败（锁将随连接关闭释放）");
    }
    match &outcome {
        Ok(Ok(_)) => mark_end(db, job, None).await,
        Ok(Err(e)) => mark_end(db, job, Some(&format!("{e:#}"))).await,
        Err(_) => {
            mark_end(db, job, Some(&timeout_msg())).await;
        }
    }
    match outcome {
        Ok(Ok(v)) => Some(v),
        Ok(Err(e)) => {
            tracing::error!(?e, key, "job 执行失败");
            None
        }
        Err(_) => {
            tracing::error!(key, "job {}", timeout_msg());
            None
        }
    }
}

/// 超时文案：数值跟 JOB_HARD_TIMEOUT_SECS 走，避免常量与文案漂移。
fn timeout_msg() -> String {
    format!("超时（{}s）被掐断", JOB_HARD_TIMEOUT_SECS)
}

/// 运行登记（0218 G7）：自动/手动共用本函数，后台「任务面板」的
/// 「最近运行」列以此为准——手动触发与定时触发在同一处留痕。
pub(crate) async fn mark_start(db: &PgPool, job: &str) {
    let _ = sqlx::query(
        "INSERT INTO job_status (job, last_started_at,
                 updated_at, executed_by) VALUES ($1, now(), now(), $2)
                 ON CONFLICT (job) DO UPDATE SET last_started_at
                     = now(), updated_at = now(), executed_by = $2",
    )
    .bind(job)
    .bind(crate::shutdown::instance_tag())
    .execute(db)
    .await;
}
pub(crate) async fn mark_end(db: &PgPool, job: &str, err: Option<&str>) {
    let _ = sqlx::query(
        "UPDATE job_status SET last_finished_at = now(), last_ok = $2, \
         last_result = $3, updated_at = now() WHERE job = $1",
    )
    .bind(job)
    .bind(err.is_none())
    .bind(err)
    .execute(db)
    .await;
}

/// U1 §5.4：job key → 模块键映射（与 API 网关表同口径）。
/// 未列出的 job 属核心层（计费/快照/清理/反作弊/等级），不受模块开关影响。
/// 二审 G8 补登：magic_pool/funding/subtitles/论坛抽奖此前漏挂锁——模块关闭后
/// 仍开全站促销/退款动账/验收交稿/开奖发奖。
pub(crate) fn job_module(job_key: &str) -> Option<&'static str> {
    // 收尾类 settle/expire **不挂**模块开关（见 _doc/契约失败流转与信誉.md
    // §3.4）：组队契约（P-38，2026-10-11）与单人复活（P-40，同日）都只
    // 处理既有契约——settle 不给任何人新开契约、expire 只做终止——关模块
    // 后不增加新玩法面，但一支开着的契约若被判「模块已关闭」而整轮跳过，
    // 会永远悬在「进行中」/open，成员的做种增量与奖励拿不到。worker 侧
    // 一律不拦，「办新契约」只在 API 入口做开关。
    Some(match job_key {
        "job:bank_daily" => "bank",
        "job:task_settle" => "tasks",
        "job:exam_assign" => "exams",
        // P-475：jixiao_settle 曾挂 jixiao 模块门，与本函数上面那条口径
        // 自相矛盾——现网 module_jixiao=no ⇒ 它一次 mark_start 都没有，
        // 已登记的绩效契约会永远不结算（今天 jixiao_claims 0 行才没咬人）。
        // 收尾类 settle 一律不挂模块开关，回归哨兵见
        // pt_audit_an_team_contract.py 的 S5（P-38）。
        "job:preserve_exit" | "job:preserve_settle" | "job:preserve_seed" => {
            "preserve"
        }
        "job:wishlist_notify" => "wishlist",
        "job:magic_pool_promo" | "job:funding_settle" => "magic_pool",
        "job:subreq_sweep" | "job:subawards" | "job:subcert_sweep" => {
            "subtitles"
        }
        "job:lottery_settle" => "forums",
        // 四审 S4：模块键是 medals（user_medals 是表名，恒查不到 → 锁失效）
        "job:expire_medals" => "medals",
        "job:expire_invites" => "invites",
        // 四审 S5：口粮券按 games_coupon_seed_hours 发放（games 模块的运营
        // 动作），关 games 后不应继续发券
        "job:game_coupons" => "games",
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 池容量与并发槽位必须成倍数（闸门 pt_audit_worker_job_health 的 J3
    /// 在编译期的那一份）：一条 job 占 `JOB_SLOT_PER_JOB` 条连接，
    /// 槽位给多了就又回到「抢连接抢到超时」，给少了是白养连接。
    /// 另钉一次初值：信号量确实按 JOB_SLOTS_MAX 建起来（不是 1 或 0）。
    #[test]
    fn job_slots_is_half_the_default_pool() {
        assert_eq!(
            crate::JOB_POOL_DEFAULT as usize,
            JOB_SLOTS_MAX * JOB_SLOT_PER_JOB
        );
        assert_eq!(job_slots().available_permits(), JOB_SLOTS_MAX);
    }
}
