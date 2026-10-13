//! 0355 发布信用分免审——阈值读取与口径锁定单测。
//! 从 upload_precheck.rs 拆出（300 行门禁）；生产路径三处：
//!   * 阈值读取：credits_threshold（upload.rs 免审第五条）
//!   * 累积写入：review.rs / review_batch.rs 两处 UPDATE 的
//!     `GREATEST(0, LEAST(5, publish_credits + CASE ...))`
//! 迁移 0355 注释与本文件的 credits_step 互为口径镜像。

/// 信用分免审线缺省（publish_credits ≥ threshold 即免审）。
/// 阈值进 site_settings `publish_credits_threshold`，可调 6（超列上限）临时关闭。
pub(super) const CREDITS_SKIP_DEFAULT: i32 = 5;

/// 信用分免审阈值（读 site_settings，缺省 5；与 upload.rs 读
/// `upload_deny_approval_deny_count` 同款 COALESCE 范式）。
pub(super) async fn credits_threshold(db: &sqlx::PgPool) -> i32 {
    sqlx::query_scalar(
        "SELECT COALESCE((SELECT value::int FROM site_settings WHERE \
         name = 'publish_credits_threshold'), $1)",
    )
    .bind(CREDITS_SKIP_DEFAULT)
    .fetch_one(db)
    .await
    .unwrap_or(CREDITS_SKIP_DEFAULT)
}

/// 信用分单步累积（review 双口 SQL 的 Rust 镜像，口径锁定用）：
/// 过审 +1（上限 5）、被拒 -2（下限 0）。
fn credits_step(current: i32, approved: bool) -> i32 {
    let next = if approved { current + 1 } else { current - 2 };
    next.clamp(0, CREDITS_SKIP_DEFAULT)
}

/// 免审判定审计（0355）：把命中的维度拼成可 grep 的 CSV。
/// perm 通道在 authz 内部判定，这里拿不到结果，不列入（审计面以四条可观测
/// 维度为准；perm 命中者本就有日志）。
pub(super) fn audit_dims(
    cat_auto: bool,
    streak_skip: bool,
    credits_skip: bool,
    class_hit: bool,
) -> String {
    [
        cat_auto.then_some("category_auto"),
        streak_skip.then_some("approve_streak"),
        credits_skip.then_some("publish_credits"),
        class_hit.then_some("class"),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(",")
}

#[cfg(test)]
mod tests {
    use super::{credits_step, CREDITS_SKIP_DEFAULT};

    /// 过审 +1，封顶 5（与迁移 0355 CHECK 上限一致）
    #[test]
    fn credits_approve_increment_capped() {
        assert_eq!(credits_step(0, true), 1);
        assert_eq!(credits_step(4, true), 5);
        assert_eq!(credits_step(5, true), 5, "已达上限不得再涨");
    }

    /// 被拒 -2，下限 0（1 分被拒直接归零，不再负）
    #[test]
    fn credits_deny_decrement_floored() {
        assert_eq!(credits_step(5, false), 3);
        assert_eq!(credits_step(2, false), 0);
        assert_eq!(credits_step(1, false), 0, "下限 0");
        assert!(credits_step(0, false) >= 0);
    }

    /// 免审第五条命中：credits ≥ threshold（缺省 5）
    #[test]
    fn credits_threshold_hit_skips_review() {
        assert!(credits_step(4, true) >= CREDITS_SKIP_DEFAULT);
        // 连续被拒清零后不可能命中：0 < 5
        assert!(credits_step(credits_step(3, false), false) < 5);
    }

    /// 审计维度串：命中按序拼接，无命中为空串
    #[test]
    fn audit_dims_lists_hits_in_order() {
        use super::audit_dims;
        assert_eq!(audit_dims(true, false, false, false), "category_auto");
        assert_eq!(
            audit_dims(false, true, true, false),
            "approve_streak,publish_credits"
        );
        assert_eq!(audit_dims(false, false, false, true), "class");
        assert_eq!(audit_dims(false, false, false, false), "");
    }
}
