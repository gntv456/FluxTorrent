//! seedbox 纯函数层的单测（边界判据必须承重——CIDR 放错等于开门）。

use super::*;
use std::net::Ipv4Addr;

/// 合法 v4 段通过。
#[test]
fn accepts_reasonable_v4() {
    assert!(normalize_cidr("1.2.3.0/24").is_ok());
    assert!(normalize_cidr("10.0.5.0/24").is_ok()); // 机房私网，正常用法
    assert!(normalize_cidr("1.2.3.4/32").is_ok());
}

/// 前缀过宽必须拒——这是最重要的一条：/8 等于把大半个互联网纳入豁免。
#[test]
fn rejects_too_wide_v4() {
    for bad in ["1.2.3.0/23", "1.2.0.0/16", "10.0.0.0/8", "0.0.0.0/0"] {
        assert!(normalize_cidr(bad).is_err(), "{bad} 应被拒（过宽）");
    }
}

/// IPv6 卡 /48。
#[test]
fn v6_prefix_floor_is_48() {
    assert!(normalize_cidr("2001:db8::/48").is_ok());
    assert!(normalize_cidr("2001:db8::/32").is_err());
}

/// 裸 IP（无前缀）必须拒——否则「1.2.3.4」会被当成 /32 还是全网，
/// 两种解释都危险，干脆要求写全。
#[test]
fn rejects_bare_ip() {
    assert!(normalize_cidr("1.2.3.4").is_err());
    assert!(normalize_cidr("").is_err());
    assert!(normalize_cidr("   ").is_err());
}

/// 前缀超界（>32 / >128）。
#[test]
fn rejects_prefix_out_of_range() {
    assert!(normalize_cidr("1.2.3.0/33").is_err());
    assert!(normalize_cidr("2001:db8::/129").is_err());
}

/// 一行多段（冒进解析）必须拒。
#[test]
fn rejects_multiple_segments_in_one_string() {
    assert!(normalize_cidr("1.2.3.0/24 5.6.7.0/24").is_err());
}

/// 批量：去重 + 排序，保证同输入同存储形态（submit 幂等的基础）。
#[test]
fn batch_normalizes_dedups_and_sorts() {
    let got = normalize_claims(&[
        "5.6.7.0/24".into(),
        "1.2.3.0/24".into(),
        "1.2.3.0/24".into(), // 重复
    ])
    .expect("应通过");
    assert_eq!(got, vec!["1.2.3.0/24", "5.6.7.0/24"]);
}

/// 批量上限。
#[test]
fn batch_respects_max() {
    let many: Vec<String> = (0..30).map(|i| format!("10.{i}.0.0/24")).collect();
    assert!(normalize_claims(&many).is_err());
}

/// 空批次必须拒（否则等于「申报一个空集」拿不到任何放宽，语义模糊）。
#[test]
fn rejects_empty_batch() {
    assert!(normalize_claims(&[]).is_err());
}

/// ip_in_claims：段内命中、段外不命中。
#[test]
fn ip_in_claims_masks_correctly() {
    let claims = vec!["1.2.3.0/24".to_string()];
    let inside: IpAddr = "1.2.3.77".parse().unwrap();
    let outside: IpAddr = "1.2.4.77".parse().unwrap();
    assert!(ip_in_claims(inside, &claims));
    assert!(!ip_in_claims(outside, &claims));
}

/// v4/v6 混合不得互相命中。
#[test]
fn v4_and_v6_do_not_cross_match() {
    let claims = vec!["1.2.3.0/24".to_string()];
    let v6: IpAddr = "2001:db8::1".parse().unwrap();
    assert!(!ip_in_claims(v6, &claims));
}

/// 默认标志零影响——老用户不该被特殊对待。
#[test]
fn default_flags_are_neutral() {
    let f = Flags::default();
    assert!(!f.is_seedbox);
    assert!(!f.hnr_exempt);
    assert!(!f.speed_relax);
    assert_eq!(f.bonus_factor, 1.0);
}

/// 安全边界：detected 只能给 speed_relax，不得给 hnr_exempt / 倍率。
#[test]
fn detected_cannot_grant_strong_exemptions() {
    let f = Flags::from_profile(false, true, true, 2.0, false);
    assert!(f.is_seedbox);
    assert!(!f.user_declared);
    assert!(f.speed_relax, "系统识别应给速度放宽");
    assert!(!f.hnr_exempt, "H&R 豁免必须人工审核背书");
    assert_eq!(f.bonus_factor, 1.0, "系统识别不得给倍率加成");
}

/// approved 全给。
#[test]
fn approved_grants_all() {
    let f = Flags::from_profile(true, false, true, 1.5, true);
    assert!(f.is_seedbox);
    assert!(f.user_declared);
    assert!(f.hnr_exempt);
    assert_eq!(f.bonus_factor, 1.5);
    assert!(f.speed_relax);
}

/// 倍率不得被配置成 <1（那会变成惩罚，违反「只放宽」边界）。
#[test]
fn bonus_factor_cannot_go_below_one() {
    let f = Flags::from_profile(true, false, false, 0.1, false);
    assert_eq!(f.bonus_factor, 1.0);
}

/// 非盒子一律回落默认（哪怕 profile 行里残留了脏值）。
#[test]
fn non_seedbox_ignores_dirty_profile() {
    let f = Flags::from_profile(false, false, true, 9.9, true);
    assert_eq!(f, Flags::default());
}
