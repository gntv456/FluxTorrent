//! 审种规则集版本（0400 §7/A25）：保存 auto_review 组任一键 → 版本 +1。
//! 台账行的 `ruleset_ver` 是裁决时点快照，版本递增让「当时配置」与
//! 「现在配置」可区分——回放与审计据此定位配置代际。从 groups.rs 拆出
//! （300 行门禁）；幂等：首次写入即种下初始键 1。

use sqlx::PgPool;

pub(crate) async fn bump(db: &PgPool) {
    let _ = sqlx::query(
        "INSERT INTO site_settings (name, value, descr, grp) \
           VALUES ('auto_review_ruleset_ver', '1', \
             '自动审种规则集版本（保存组内任一键自动 +1，系统维护）', \
             'auto_review') \
         ON CONFLICT (name) DO UPDATE SET \
           value = ((site_settings.value::int + 1)::text)",
    )
    .execute(db)
    .await;
}
