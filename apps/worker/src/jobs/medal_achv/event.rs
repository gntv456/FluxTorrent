//! event 模式：业务侧记事件 → 累加进度 → 达标授予。

use sqlx::PgPool;

use super::{award, awarded, prog, rules_of, save_progress};
use super::api_domain as md;

/// 记一条可计数事件（业务侧调用；event 模式成就的输入）。
///
/// 返回是否触发了授予。业务侧无需关心细节，只需在动作成功后调一次。
pub(crate) async fn record_event(
    db: &PgPool,
    user_id: i64,
    event: &str,
    amount: i64,
) -> anyhow::Result<bool> {
    if amount <= 0 {
        return Ok(false);
    }
    sqlx::query(
        "INSERT INTO medal_events (user_id, event, amount) \
         VALUES ($1, $2, $3)",
    )
    .bind(user_id)
    .bind(event)
    .bind(amount)
    .execute(db)
    .await?;
    let all = rules_of(db, Some("event")).await?;
    let rules: Vec<_> = all
        .into_iter()
        .filter(|r| r.auto_rule.as_deref() == Some(event))
        .collect();
    let mut granted = false;
    for r in &rules {
        if awarded(db, user_id, r.id).await {
            continue;
        }
        let cur = prog(db, user_id, r.id).await;
        let rule = md::Rule {
            mode: md::Mode::Event,
            period: md::Period::Total,
            target: r.auto_target.unwrap_or(0),
        };
        match md::judge_event(&rule, cur, amount) {
            md::Verdict::Award => {
                if award(db, user_id, r, "event").await? {
                    granted = true;
                }
            }
            md::Verdict::Progressed { current } => {
                save_progress(db, user_id, r.id, current).await?;
            }
            _ => {}
        }
    }
    Ok(granted)
}
