//! 任务2（0351）：超频临时封禁键的小工具与单测
//!（自 guard.rs 拆出，300 行门禁）。SET/GET 两侧共用同一拼法与闸门。

/// `ban:ann:u:{user_id}`：SET/EXPIRE 与 EXISTS 共用的同一拼法——
/// 两处各写一份 format! 迟早拼岔（写 `ban:ann:u:42`、读 `ban:u:ann:42`），
/// 封禁就变成「写了永远读不到」的静默无效配置。
pub(crate) fn ban_key(user_id: i64) -> String {
    format!("ban:ann:u:{user_id}")
}

/// ban_secs 闸门：<=0 = 关闭封禁（SET 与 GET 两侧同判，关了就都不碰 Redis）。
pub(crate) fn ban_enabled(ban_secs: i64) -> bool {
    ban_secs > 0
}

#[cfg(test)]
mod tests {
    use super::{ban_enabled, ban_key};

    /// 键拼法是 SET/GET 两侧的隐式契约：变更键格式必须两侧同变，
    /// 这条断言钉住格式，改键名的人会先撞到这里。
    #[test]
    fn ban_key_shape_is_shared_by_set_and_get() {
        assert_eq!(ban_key(42), "ban:ann:u:42");
        assert_eq!(ban_key(0), "ban:ann:u:0");
        assert_eq!(ban_key(-7), "ban:ann:u:-7");
    }

    /// ban_secs=0/负 = 关闭：SET 侧不写键、GET 侧不查键（两侧同一闸门，
    /// 只写侧判 0 会出现「不写但每请求多打一次 EXISTS」的半开状态）。
    #[test]
    fn ban_secs_zero_or_negative_disables_both_sides() {
        assert!(!ban_enabled(0));
        assert!(!ban_enabled(-1));
        assert!(ban_enabled(1));
        assert!(ban_enabled(300)); // 出厂默认 ANN_RATE_USER_BAN_SECS=300
    }
}
