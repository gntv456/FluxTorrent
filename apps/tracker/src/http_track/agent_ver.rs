//! 0351 agent_rules 版本区间匹配的纯函数工具（自 guard.rs 拆出，
//! 300 行门禁 + 便于单测）。tracker 侧匹配语义：
//! 带 version_min 的规则要求 UA 版本 >= min，带 version_max 要求 <= max
//!（含边界）；两字段都空 = 不限版本；提取失败的 UA 对带区间的规则不命中。

use super::helpers::AgentRule;

/// 从 UA 字符串提取第一个 `\d+(\.\d+){0,2}` 形态的版本号，解析为
/// (major, minor, patch) 三元组（缺省段补 0），按数字段字典序比较。
/// 例：`Transmission/3.00` → (3,0,0)。提取失败返回 None。
pub(crate) fn ua_version(ua: &str) -> Option<(u64, u64, u64)> {
    let bytes = ua.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if !bytes[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let mut segs: Vec<u64> = Vec::with_capacity(3);
        loop {
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            // 超长数字段：parse 失败封顶而非丢规则（u64::MAX 视同无穷大）
            segs.push(ua[start..i].parse().unwrap_or(u64::MAX));
            // 只有紧跟 `.` 且后面还是数字才继续收段（最多 3 段）
            if segs.len() < 3
                && i + 1 < bytes.len()
                && bytes[i] == b'.'
                && bytes[i + 1].is_ascii_digit()
            {
                i += 1;
            } else {
                break;
            }
        }
        return Some((
            segs.first().copied().unwrap_or(0),
            segs.get(1).copied().unwrap_or(0),
            segs.get(2).copied().unwrap_or(0),
        ));
    }
    None
}

/// 点分数字串 → (u64,u64,u64)。写入口已挡非数字形状（api 侧
/// check_version_shape），这里对畸形串按 (0,0,0) 兜底 = 不设门槛。
fn ver_tuple(s: &str) -> (u64, u64, u64) {
    let mut seg = (0u64, 0u64, 0u64);
    for (i, p) in s.split('.').take(3).enumerate() {
        let v = p.parse().unwrap_or(0);
        match i {
            0 => seg.0 = v,
            1 => seg.1 = v,
            _ => seg.2 = v,
        }
    }
    seg
}

/// 规则版本区间判定：两字段都空 = 不限版本；带 min 要求 ua_ver >= min，
/// 带 max 要求 ua_ver <= max（含边界）。**版本提取失败的 UA（ua_ver=None）
/// 对带任何版本区间的规则视为不命中**——提取不到就放行等于区间形同虚设。
pub(crate) fn version_in_range(
    r: &AgentRule,
    ua_ver: Option<(u64, u64, u64)>,
) -> bool {
    if r.version_min.is_empty() && r.version_max.is_empty() {
        return true;
    }
    let Some(v) = ua_ver else {
        return false;
    };
    if !r.version_min.is_empty() && v < ver_tuple(&r.version_min) {
        return false;
    }
    if !r.version_max.is_empty() && v > ver_tuple(&r.version_max) {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::{ua_version, ver_tuple, version_in_range, AgentRule};

    fn rule(vmin: &str, vmax: &str) -> AgentRule {
        AgentRule {
            deny: true,
            agent_re: None,
            peer_re: None,
            version_min: vmin.into(),
            version_max: vmax.into(),
            priority: 0,
        }
    }

    #[test]
    fn ua_version_extracts_first_dotted_number() {
        assert_eq!(ua_version("Transmission/3.00 (macOS)"), Some((3, 0, 0)));
        assert_eq!(ua_version("qBittorrent/4.6.2"), Some((4, 6, 2)));
        assert_eq!(ua_version("qBittorrent/4.6"), Some((4, 6, 0)));
        // 收满 3 段即停：`2.0.5-1` 后面的 `-1` 不并入
        assert_eq!(
            ua_version("Deluge 2.0.5-1 libtorrent/1.2.x"),
            Some((2, 0, 5))
        );
        assert_eq!(ua_version("-XL0014-"), Some((14, 0, 0)));
        // 无数字 UA 提取失败
        assert_eq!(ua_version("libtorrent (none)"), None);
        assert_eq!(ua_version(""), None);
    }

    /// 4.6 vs 4.10 必须按数字段比较：字符串序会把 "4.6" 判大于 "4.10"
    ///（'6' > '1'），令 [4.10,∞) 的封禁放走 4.9、误伤 4.10 本尊。
    #[test]
    fn version_compare_is_numeric_not_string() {
        assert!(ver_tuple("4.6") < ver_tuple("4.10"));
        let r = rule("4.10", "");
        assert!(!version_in_range(&r, Some((4, 6, 0))));
        assert!(version_in_range(&r, Some((4, 10, 0))));
    }

    /// 边界含等号：ua_ver == min / == max 都算命中区间。
    #[test]
    fn version_bounds_are_inclusive() {
        let r = rule("4.6", "5.0");
        assert!(version_in_range(&r, Some((4, 6, 0)))); // == min
        assert!(version_in_range(&r, Some((5, 0, 0)))); // == max
        assert!(!version_in_range(&r, Some((4, 5, 9)))); // < min
        assert!(!version_in_range(&r, Some((5, 0, 1)))); // > max
    }

    /// 提取失败的 UA 对带区间的规则一律不命中；不限版本的规则不受影响。
    #[test]
    fn version_extraction_failure_never_hits_ranged_rules() {
        assert!(!version_in_range(&rule("4.0", ""), None));
        assert!(version_in_range(&rule("", ""), None));
    }
}
