//! 站型包的术语段（0206）：apply 时把包声明的术语规则还原到站点。
//!
//! 口径与 sections/tags 完全一致，尤其两条：
//!   1) **NULL = 本包不声明术语** ⇒ 一律不动站方现有规则。内置预置包都没有这一列
//!      （0206 之前不存在），所以切换站型不会莫名改写谁的词汇表；
//!   2) 空数组是**显式声明「就是要清空」**，与 NULL 不同——「另存快照」如实捕获
//!      一个没有术语的站，apply 回去也该是没有术语。
//!
//! 覆盖式重建（先删后插）而不是 merge：术语是「本站叫什么」的整体表态，
//! 半并半留会产生同一个原词两条规则的歧义（主键会直接拒，等于 apply 半途失败）。

use super::sitetype::SiteTypePack;
use crate::errors::{DomainError, DomainResult};
use sqlx::PgPool;

/// 应用包声明的术语段，返回写回的规则条数（未声明返回 -1，便于回执区分）。
pub(crate) async fn apply_pack_terms(
    db: &PgPool,
    pack: &SiteTypePack,
) -> DomainResult<i64> {
    let Some(arr) = pack.terms.as_ref().and_then(serde_json::Value::as_array)
    else {
        return Ok(-1);
    };
    let mut tx = db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query("DELETE FROM site_terms")
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let mut n = 0i64;
    for item in arr {
        let canonical = item.get("canonical").and_then(|v| v.as_str());
        let replacement = item.get("replacement").and_then(|v| v.as_str());
        let (Some(from), Some(to)) = (canonical, replacement) else {
            // 包载荷是站长另存出来的，形状不对就是脏包：整段放弃而不是半套写回
            return Err(DomainError::Validation(
                "本站的包数据已损坏，请联系站长".into(),
            ));
        };
        let sort = item.get("sort").and_then(serde_json::Value::as_i64);
        // 缺 enabled 字段 = 老包载荷，按启用处理（0206 之前不存在这段）
        let enabled = item
            .get("enabled")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true);
        sqlx::query(
            "INSERT INTO site_terms \
             (canonical, replacement, enabled, descr, sort) \
             VALUES ($1, $2, $3, '', COALESCE($4, 100)) \
             ON CONFLICT (canonical) DO NOTHING",
        )
        .bind(from)
        .bind(to)
        .bind(enabled)
        .bind(sort.map(|s| s as i32))
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        n += 1;
    }
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 写侧统一在落库后刷快照（与后台面板 CRUD 同一口径），否则 apply 完
    // 还要等下一次重启才看到效果。
    crate::terms::reload(db).await;
    Ok(n)
}
