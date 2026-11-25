//! 经济口径回归：赢局净收益不得为负（资深玩家一眼抓的结算 bug）。
use crate::games::doudizhu::DOU_DIZHU_RAKE;
use crate::games::doudizhu_settle::{
    doudizhu_expected_value, settle_payout, Role,
};

#[test]
fn win_never_loses_money() {
    let stake = 100i64;
    for mult in [1i64, 2, 3, 8] {
        for role in [Role::Farmer, Role::Landlord] {
            let payout = settle_payout(role, stake, mult, DOU_DIZHU_RAKE);
            assert!(payout >= stake, "赢局回款不得低于本金: {payout}");
        }
    }
}

#[test]
fn ev_stays_below_one() {
    assert!(doudizhu_expected_value(DOU_DIZHU_RAKE) < 1.0);
    assert!(doudizhu_expected_value(0) < 1.0);
    assert!(doudizhu_expected_value(500) < 1.0);
}
