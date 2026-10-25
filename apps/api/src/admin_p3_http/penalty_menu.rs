//! 0397 考核处置菜单校验（自 tasks.rs 拆出守 300 行门禁）。
//!
//! 菜单键与 worker `penalty.rs` 的 KINDS 是同一份菜单的手工镜像
//! （serde 层无法跨 crate 复用常量）。两处不一致会在 CI 两侧暴露——
//! worker 侧 parse 过滤未知键，api 侧保存时拒绝未知键。
//!
//! 【为什么校验要查库】`ban_pages` 的权限位最终写进
//! `user_permissions.permission_key`，而该列对 `permissions.key` 有
//! 外键：存一个库里没有的键，结算那一夜会撞 FK 违例 → 整个结算批次
//! 中断 → 全站考核认领卡在 status=0（人工解开全部超时才算完）。
//! 而且用户界面给的示例键（forum.view / games.play）本来就不在
//! `permissions` 表里＝按提示填就必然踩雷。所以**保存时查库校验**，
//! 配置错误当场拦下，不留到结算夜。

use sqlx::PgPool;

use crate::errors::{DomainError, DomainResult};

/// 处置菜单键（worker penalty.rs / web admin-tasks-shared.ts 三处镜像）
pub const PENALTY_KINDS: &[&str] = &[
    "ban",
    "suspend",
    "restrict_download",
    "mute",
    "ban_upload",
    "ban_pages",
    "spark_fine",
    "traffic_fine",
];

/// traffic_fine 的扣减对象（worker penalty_ops.rs 同款）
const TRAFFIC_TARGETS: &[&str] = &["uploaded", "downloaded"];

/// 保存时校验：kind 在菜单内、数值项 amount 合规、页面权限位真实存在。
///
/// 配置错误在保存时拦下，而不是等到结算夜静默不生效。
pub async fn validate_fail_penalty(
    db: &PgPool,
    v: &serde_json::Value,
) -> DomainResult<()> {
    let Some(arr) = v.as_array() else {
        return Err(DomainError::Validation("fail_penalty 需为数组".into()));
    };
    let mut page_keys: Vec<String> = Vec::new();
    for it in arr {
        let Some(kind) = it.get("kind").and_then(|k| k.as_str()) else {
            return Err(DomainError::Validation(
                "fail_penalty 每项需含 kind".into(),
            ));
        };
        if !PENALTY_KINDS.contains(&kind) {
            return Err(DomainError::Validation(format!(
                "fail_penalty 含未知处置项 {kind}"
            )));
        }
        match kind {
            "spark_fine" | "traffic_fine" => {
                // 数额型处置：数额必须 > 0。缺省 0 ＝「配了但不扣」，
                // 结算时记一条 applied={took:0} 还会给用户发「扣除魔力 0」
                // 的通知——看着像扣了款其实没扣，是最典型的假绿。
                let a = it.get("amount").and_then(|a| a.as_i64());
                match a {
                    None => {
                        return Err(DomainError::Validation(format!(
                            "{kind} 需填写数额"
                        )));
                    }
                    Some(v) if v <= 0 => {
                        return Err(DomainError::Validation(format!(
                            "{kind} 数额需大于 0"
                        )));
                    }
                    Some(_) => {}
                }
                if kind == "traffic_fine" {
                    if let Some(t) = it.get("target").and_then(|t| t.as_str()) {
                        if !TRAFFIC_TARGETS.contains(&t) {
                            return Err(DomainError::Validation(format!(
                                "traffic_fine 的 target 只能是 uploaded / \
                                 downloaded，收到 {t}"
                            )));
                        }
                    }
                }
            }
            "ban_pages" => {
                let pages = it
                    .get("pages")
                    .and_then(|p| p.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str())
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                            .map(String::from)
                            .collect::<Vec<String>>()
                    })
                    .unwrap_or_default();
                if pages.is_empty() {
                    return Err(DomainError::Validation(
                        "ban_pages 需至少填写一个权限位".into(),
                    ));
                }
                page_keys.extend(pages);
            }
            _ => {}
        }
    }
    if !page_keys.is_empty() {
        let known: Vec<String> = sqlx::query_scalar(
            "SELECT key FROM permissions WHERE key = ANY($1::text[])",
        )
        .bind(&page_keys)
        .fetch_all(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        let unknown: Vec<String> = page_keys
            .iter()
            .filter(|k| !known.contains(k))
            .cloned()
            .collect();
        if !unknown.is_empty() {
            return Err(DomainError::Validation(format!(
                "ban_pages 含不存在的权限位：{}（权限位取自 permissions 表，\
                 如 torrent.upload / announce.publish / forums.manage）",
                unknown.join("、")
            )));
        }
    }
    Ok(())
}
