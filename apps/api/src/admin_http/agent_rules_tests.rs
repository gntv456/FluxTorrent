//! 0351 单元测试：正则预校验（原有）+ 版本区间/优先级校验。
//! 自 agent_rules.rs 拆出（300 行门禁）；被测函数在 agent_rule_check.rs。

use crate::admin_http::agent_rule_check::{
    check_pattern, check_version_priority, ver_tuple, AgentRuleReq,
};

fn req(vmin: &str, vmax: &str, prio: i32) -> AgentRuleReq {
    AgentRuleReq {
        mode: "deny".into(),
        pattern: "Transmission/3".into(),
        note: None,
        version_min: vmin.into(),
        version_max: vmax.into(),
        priority: prio,
    }
}

#[test]
fn literal_substring_still_compiles() {
    // 0036 注释举的例子：子串写法本身就是合法正则
    assert!(check_pattern("Transmission/3").is_ok());
    assert!(check_pattern("^qBittorrent/4\\.6").is_ok());
}

#[test]
fn invalid_regex_is_rejected_at_the_write_door() {
    for bad in ["(", "a{2,1}", "[z-a]", "*x", "(?P<n>x", "x)"] {
        assert!(
            check_pattern(bad).is_err(),
            "{bad} 应在写入口就被拒，而不是加载时静默跳过"
        );
    }
}

/// 写入口只判语法，判不出**意图**——这条断言钉住这个边界，免得以后
/// 有人以为「存进去了就一定按字面生效」。`uTorrent/3.5.5 (build)` 是
/// 合法正则（括号成了捕获组），它匹配的是 `uTorrent/3.5.5 build`，
/// 而真实 UA 里那对括号是字面字符 ⇒ 规则永远不命中，却不报错。
/// 想按字面匹配必须自己转义：`uTorrent/3\.5\.5 \(build\)`。
#[test]
fn legal_but_literal_looking_parens_are_accepted_not_escaped() {
    assert!(check_pattern("uTorrent/3.5.5 (build)").is_ok());
    assert!(check_pattern(r"uTorrent/3\.5\.5 \(build\)").is_ok());
    let loose = regex::Regex::new("uTorrent/3.5.5 (build)").unwrap();
    let strict = regex::Regex::new(r"uTorrent/3\.5\.5 \(build\)").unwrap();
    // 拿站长填的那串字面本身当 UA：未转义时连自己都匹配不到
    let ua = "uTorrent/3.5.5 (build)";
    assert!(!loose.is_match(ua), "未转义的括号组匹配不到字面 UA");
    assert!(strict.is_match(ua), "转义后才按字面匹配");
}

#[test]
fn version_shape_rejects_non_dotted_numeric() {
    // 合法：空串 / 1-3 段点分数字
    for ok in ["", "4", "4.6", "5.0.1"] {
        assert!(
            check_version_priority(&req(ok, "", 0)).is_ok(),
            "{ok} 应为合法 version_min"
        );
    }
    // 非法：通配/前缀/四段/负数/空段
    for bad in ["4.x", "v4.6", "4.6.0.1", "-1", "4..6", "4."] {
        assert!(
            check_version_priority(&req(bad, "", 0)).is_err(),
            "{bad} 应在写入口被拒"
        );
    }
}

/// 4.6 vs 4.10 按数字段比较：字符串序会判 4.6 > 4.10（'6'>'1'），
/// 这条测试钉住区间自洽校验用的是数字语义（tracker 侧同口径，见
/// agent_ver.rs 的对应测试）。
#[test]
fn version_range_compare_is_numeric_not_lexicographic() {
    assert!(ver_tuple("4.6") < ver_tuple("4.10"));
    // 4.6 <= 4.10 数字序成立 ⇒ 合法
    assert!(check_version_priority(&req("4.6", "4.10", 0)).is_ok());
    // 反过来 min=4.10 / max=4.6 必须被拒
    assert!(check_version_priority(&req("4.10", "4.6", 0)).is_err());
}

#[test]
fn priority_must_be_within_0_to_1000() {
    assert!(check_version_priority(&req("", "", 0)).is_ok());
    assert!(check_version_priority(&req("", "", 1000)).is_ok());
    assert!(check_version_priority(&req("", "", -1)).is_err());
    assert!(check_version_priority(&req("", "", 1001)).is_err());
}
