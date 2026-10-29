//! 注册自定义字段值落库（从 register.rs 按域拆出，行数门禁）。
//! 口径（P-135/P-140）：字段清单与 required 校验、`/register-fields`
//! 三处同源——模块关着的字段既不要求填，也不偷偷存。
//! 账号已建成后值落不上不回滚账号（附属数据），但必须留痕告警。

use crate::state::AppState;

pub(super) async fn store_field_values(
    state: &AppState,
    user_id: i64,
    fields: &std::collections::HashMap<String, serde_json::Value>,
) {
    // 与注册口的 required 校验、`/register-fields` 三处同一口径
    // （P-140）：模块关着的字段既不该要求填，也不该偷偷存下用户塞来的值
    let defs: Vec<String> = sqlx::query_scalar(&format!(
        "SELECT key FROM user_field_defs \
         WHERE enabled AND show_on_register AND {}",
        crate::modules::module_on_sql("")
    ))
    .fetch_all(&state.repo.db)
    .await
    .unwrap_or_default();
    for key in defs {
        if let Some(v) = fields.get(&key).filter(|v| !v.is_null()) {
            // 账号已建成、这个值没落上——静默吞掉就是「用户填了却查无此值」
            // （P-135 同族）：不回滚账号是既定口径，但必须留痕
            if let Err(e) = sqlx::query(
                "INSERT INTO user_field_values (user_id, field_key, value) \
                 VALUES ($1, $2, $3) \
                 ON CONFLICT (user_id, field_key) DO NOTHING",
            )
            .bind(user_id)
            .bind(&key)
            .bind(v)
            .execute(&state.repo.db)
            .await
            {
                tracing::warn!(
                    %e,
                    user = user_id,
                    field = %key,
                    "注册字段值落库失败（账号已建，该值丢失）"
                );
            }
        }
    }
}
