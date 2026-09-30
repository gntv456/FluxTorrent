//! 大厅玩法清单（`arcade_games`）读取：顺序 / 显隐 / 图标 / 色系 / 角标来源。
//!
//! 纯展示清单，不承载经济参数。表为空（旧库未跑迁移）时返回空 Vec ——
//! 前端有兜底注册表，大厅照常出卡，不会白屏。

use sqlx::{PgPool, Row};

use crate::errors::DomainError;

use super::pool::dberr;

/// 读启用中的玩法清单（按 sort）。缺表/空表都返回空，由前端兜底。
pub(super) async fn load_registry(
    db: &PgPool,
) -> Result<Vec<serde_json::Value>, DomainError> {
    let rows = sqlx::query(
        "SELECT key, title_key, icon, href, tone, grp, module_gate, badge, \
                hot \
           FROM arcade_games WHERE enabled ORDER BY sort, key",
    )
    .fetch_all(db)
    .await
    .map_err(dberr)?;
    Ok(rows
        .into_iter()
        .map(|r| {
            serde_json::json!({
                "key": r.get::<String, _>("key"),
                "title_key": r.get::<String, _>("title_key"),
                "icon": r.get::<String, _>("icon"),
                "href": r.get::<String, _>("href"),
                "tone": r.get::<String, _>("tone"),
                "grp": r.get::<String, _>("grp"),
                "module": r.get::<String, _>("module_gate"),
                "badge": r.get::<String, _>("badge"),
                "hot": r.get::<bool, _>("hot"),
            })
        })
        .collect())
}
