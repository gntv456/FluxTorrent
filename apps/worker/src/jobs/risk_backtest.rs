//! 反作弊规则集回测（0391，自 risk_score.rs 拆出守 300 行门禁）。
//!
//! 回答「换成 to_ver 这版权重，过去 N 天会多判/少判谁」。这是
//! 「敢改反作弊权重」的前置：改完先回测，看看会不会把一批正常
//! 用户送进案件队列，再决定是否切版本。
//!
//! 两条路径共用一个计算体（compute_backtest）：
//!   * 手触（manual.rs）：面板直接跑，INSERT 一行结论；
//!   * 排队（drain_pending_backtests）：API `/admin/cheat/backtest`
//!     只落一行 total_users=-1 的占位（回测重，不能在请求里同步算），
//!     由 risk_score 10m tick 顺带消费——此前没有任何消费者，面板
//!     「排一次回测」永远停在排队中。

use super::risk_score::{ruleset_params, Fam, RiskCfg};
use sqlx::PgPool;
use std::collections::HashMap;

/// 回测结论（排队路径回填用；手触路径复用计算体）。
pub(super) struct BacktestOut {
    pub(super) total_users: i64,
    pub(super) would_open: i64,
    pub(super) band_up: i64,
    pub(super) band_down: i64,
    pub(super) rows: Vec<serde_json::Value>,
}

/// 规则集回测（手触路径）：算完直接 INSERT 一行结论。
///
/// 做法：把窗口内每个用户当前家族的 hits/decay 算一遍，分别用
/// from_ver 与 to_ver 的参数打分，比档位。**只记会开案或会掉档的行**——
/// 全量差异既无意义又占空间。
pub async fn backtest(
    db: &PgPool,
    from_ver: i32,
    to_ver: i32,
    window_days: i32,
) -> anyhow::Result<i64> {
    let from_params = ruleset_params(db, from_ver).await?;
    let to_params = ruleset_params(db, to_ver).await?;
    let (Some(from_cfg), Some(to_cfg)) = (from_params, to_params) else {
        anyhow::bail!("规则集版本不存在：{from_ver} / {to_ver}");
    };
    let out = compute_backtest(db, &from_cfg, &to_cfg, window_days).await?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO cheat_backtest_runs \
         (from_ver, to_ver, window_days, total_users, would_open, \
          band_up, band_down, diffs) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8::jsonb) RETURNING id",
    )
    .bind(from_ver)
    .bind(to_ver)
    .bind(window_days)
    .bind(out.total_users)
    .bind(out.would_open)
    .bind(out.band_up)
    .bind(out.band_down)
    .bind(serde_json::Value::Array(out.rows).to_string())
    .fetch_one(db)
    .await?;

    tracing::info!(
        id,
        from_ver,
        to_ver,
        window_days,
        users = out.total_users,
        would_open = out.would_open,
        band_up = out.band_up,
        band_down = out.band_down,
        "反作弊规则集回测完成"
    );
    Ok(id)
}

/// 消费已排队的回测占位（排队路径）：认领 → 算 → UPDATE 回填。
///
/// 抢坑 `FOR UPDATE SKIP LOCKED`：多实例下同轮只有一个赢。
/// 失败时回填原因到 diffs（面板看得到而不是永远「排队中」）；
/// 可重试错误归还占位（-1）下一轮再跑，版本不存在则终态（-3）。
pub async fn drain_pending_backtests(db: &PgPool) -> u64 {
    let pending: Vec<(i64, i32, i32, i32)> = match sqlx::query_as(
        "UPDATE cheat_backtest_runs \
           SET total_users = -2 \
         WHERE id IN (SELECT id FROM cheat_backtest_runs \
                      WHERE total_users = -1 \
                      ORDER BY id LIMIT 1 FOR UPDATE SKIP LOCKED) \
         RETURNING id, from_ver, to_ver, window_days",
    )
    .fetch_all(db)
    .await
    {
        Ok(rows) => rows,
        Err(e) => {
            tracing::error!(?e, "回测占位行认领失败");
            return 0;
        }
    };
    let mut done = 0u64;
    for (id, from, to, days) in pending {
        match run_backtest_row(db, id, from, to, days).await {
            Ok(()) => done += 1,
            Err(e) => {
                let msg = format!("{e:#}");
                let ver_missing = msg.contains("规则集版本不存在");
                let set_to = if ver_missing { -3 } else { -1 };
                let _ = sqlx::query(
                    "UPDATE cheat_backtest_runs \
                       SET total_users = $2, band_up = 0, band_down = 0, \
                           would_open = 0, diffs = $3::jsonb \
                     WHERE id = $1",
                )
                .bind(id)
                .bind(set_to)
                .bind(serde_json::json!([{ "error": msg }]).to_string())
                .execute(db)
                .await;
                tracing::error!(id, "排队回测执行失败（已回填原因）");
            }
        }
    }
    done
}

/// 对一行已认领的回测占位执行计算并回填。
async fn run_backtest_row(
    db: &PgPool,
    id: i64,
    from_ver: i32,
    to_ver: i32,
    window_days: i32,
) -> anyhow::Result<()> {
    let from_params = ruleset_params(db, from_ver).await?;
    let to_params = ruleset_params(db, to_ver).await?;
    let (Some(from_cfg), Some(to_cfg)) = (from_params, to_params) else {
        anyhow::bail!("规则集版本不存在：{from_ver} / {to_ver}");
    };
    let d = compute_backtest(db, &from_cfg, &to_cfg, window_days).await?;
    sqlx::query(
        "UPDATE cheat_backtest_runs \
           SET total_users = $2, would_open = $3, band_up = $4, \
               band_down = $5, diffs = $6::jsonb \
         WHERE id = $1",
    )
    .bind(id)
    .bind(d.total_users)
    .bind(d.would_open)
    .bind(d.band_up)
    .bind(d.band_down)
    .bind(serde_json::Value::Array(d.rows).to_string())
    .execute(db)
    .await?;
    tracing::info!(
        id,
        from_ver,
        to_ver,
        window_days,
        users = d.total_users,
        would_open = d.would_open,
        "排队回测完成"
    );
    Ok(())
}

/// 计算体（排队/手触两路共用，只读不写结论表）。
async fn compute_backtest(
    db: &PgPool,
    from_cfg: &RiskCfg,
    to_cfg: &RiskCfg,
    window_days: i32,
) -> anyhow::Result<BacktestOut> {
    // 窗口内每个用户按家族的 hits 与最近一次证据时间。
    // cheat_events 无 status 列（用户状态在 users 表）——不 join users
    // 过滤是因为回测只做参数对比，口径与正式评分的差集对运营无意义。
    let rows: Vec<(i64, String, i64, f64)> = sqlx::query_as(
        "SELECT user_id, \
                CASE WHEN agent LIKE 'deal:%' THEN 'self_deal' \
                     WHEN agent IN ('down_under','bitthief') THEN agent \
                     ELSE split_part(agent, ':', 1) END AS fam, \
                count(*) AS hits, \
                EXTRACT(EPOCH FROM (now() - max(last_seen))) \
                  / 86400.0::float8 AS days \
         FROM cheat_events \
         WHERE resolved_at IS NULL \
           AND last_seen > now() - make_interval(days => $1::int) \
         GROUP BY 1, 2",
    )
    .bind(window_days)
    .fetch_all(db)
    .await?;

    let mut agg: HashMap<i64, Vec<Fam>> = HashMap::new();
    for (uid, fam, hits, days) in rows {
        agg.entry(uid).or_default().push(Fam {
            family: fam,
            hits,
            days,
        });
    }

    let mut diffs: Vec<serde_json::Value> = vec![];
    let (mut would_open, mut band_up, mut band_down) = (0i64, 0i64, 0i64);
    for (uid, fams) in &agg {
        let (s_from, b_from) = score_with(from_cfg, fams);
        let (s_to, b_to) = score_with(to_cfg, fams);
        if s_from == s_to && b_from == b_to {
            continue;
        }
        let opens_to = matches!(b_to.as_str(), "high" | "critical");
        let opened_from = matches!(b_from.as_str(), "high" | "critical");
        if opens_to && !opened_from {
            would_open += 1;
        }
        let rank = |b: &str| match b {
            "critical" => 4,
            "high" => 3,
            "watch" => 2,
            "low" => 1,
            _ => 0,
        };
        if rank(&b_to) > rank(&b_from) {
            band_up += 1;
        } else if rank(&b_to) < rank(&b_from) {
            band_down += 1;
        }
        // 只留会开案或会掉档的：其余差异对运营无意义
        if (opens_to && !opened_from)
            || (opened_from && !opens_to)
            || rank(&b_to) != rank(&b_from)
        {
            diffs.push(serde_json::json!({
                "user_id": uid,
                "from": {"score": s_from, "band": b_from},
                "to":   {"score": s_to,   "band": b_to},
            }));
        }
    }

    Ok(BacktestOut {
        total_users: agg.len() as i64,
        would_open,
        band_up,
        band_down,
        rows: diffs,
    })
}

/// 用给定参数对某用户的家族集合打分（与 risk_score 主流程同公式；
/// 回测要「用另一版权重重放」，不能复用读 site_settings 的 risk_cfg）。
fn score_with(cfg: &RiskCfg, fams: &[Fam]) -> (i32, String) {
    let norm = cfg.f("risk_norm", 8.0).max(0.5);
    let half = cfg.f("risk_decay_days", 14.0).clamp(0.5, 3650.0);
    let total: f64 = fams
        .iter()
        .map(|f| {
            let w = cfg.weight(&f.family);
            let fh = 1.0 + ((f.hits.clamp(1, 16) as f64).log2());
            let decay = 2f64.powf(-f.days.max(0.0) / half);
            w * fh * decay * 100.0 / norm
        })
        .sum();
    let score = (total.round() as i32).clamp(0, 100);
    (score, cfg.band(score).to_string())
}
