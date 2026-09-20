//! 任务系统结算（0051）：领取时记指标基线，达标发奖、超时失败。
//!
//! 按域拆分：settle（认领结算主循环）、reward（达标/失败发奖与罚金）、
//! exam（考核自动派发）。

mod exam;
mod reward;
mod settle;

pub use exam::exam_assign;
pub use settle::task_settle;

#[cfg(test)]
mod tests {
    use super::settle::TaskMetric;

    /// seed_points 口径（P1-6 厘清）：1 积分 = 1 小时做种 = 3600 秒，
    /// 与 class_rules.min_seed_hours 同源。旧公式 v*3600/100（v×36 秒）无站内依据，已废弃。
    #[test]
    fn seed_points_one_point_per_hour() {
        let m: TaskMetric = serde_json::from_value(
            serde_json::json!({ "seed_points_delta": 10 }),
        )
        .unwrap();
        assert_eq!(m.seed_points_delta, Some(10));
        // v=10 积分 → 门槛 10×3600=36000 秒（10 小时）
        assert_eq!(m.seed_points_delta.unwrap().saturating_mul(3600), 36000);
    }
}
