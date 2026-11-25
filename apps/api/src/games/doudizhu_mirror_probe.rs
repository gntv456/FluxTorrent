//! PvP 镜像局结算一致性：三行按各自 landlord 座位查 call_base 都一致。
//! 物理玩家 P 在行 idx 的座位 = (P + 3 - idx) % 3；
//! 建行时 state.turn = (3 - idx) % 3（物理发牌序 0 先叫）。
#[test]
fn mirror_call_base_consistent_across_rows() {
    for idx in 0..3usize {
        let seat_of = |p: usize| (p + 3 - idx) % 3;
        let start = (3 - idx) % 3;
        let mut bids: Vec<(u8, i64)> = Vec::new();
        let mut cur = start as u8;
        for phys in 0..3 {
            let score = phys as i64 + 1; // P0=1, P1=2, P2=3 当地主
            bids.push((cur, score));
            cur = (cur + 1) % 3;
        }
        let landlord_seat = seat_of(2) as u8;
        let call_base = bids
            .iter()
            .find(|(s, _)| *s == landlord_seat)
            .map(|x| x.1)
            .unwrap();
        assert_eq!(call_base, 3, "行 {idx} 按 landlord 座位查到的叫分底应为 3");
    }
}
