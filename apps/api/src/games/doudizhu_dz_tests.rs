//! 斗地主 DouZero 移植件单测：move_gen/selector 编码对拍锁定。
//! 关键口径全部来自上游 kwai/DouZero（Apache-2.0），此处用固定牌面
//! 断言防漂移——尤其 17（点数 2）列位、顺子不含 2、响应含 pass。

use super::doudizhu_dz_detector::move_type;
use super::doudizhu_dz_encode::{cards2array_ref, dz_rank};
use super::doudizhu_dz_move_gen::{
    MoveGen, T_BOMB, T_KING_BOMB, T_SERIAL_SINGLE, T_WRONG,
};
use super::doudizhu_dz_selector::{beats_move, legal_responses};

fn dz(ranks: &[i16]) -> Vec<i16> {
    ranks.to_vec()
}

#[test]
fn dz_rank_mapping_matches_upstream() {
    // 3→3 … A(本仓11)→14，2(本仓12)→17，小王→20，大王→30
    assert_eq!(dz_rank(0), 3); // 本仓 rank0 = 点3
    assert_eq!(dz_rank(44), 14); // 本仓 rank11 = A
    assert_eq!(dz_rank(48), 17); // 本仓 rank12 = 2
    assert_eq!(dz_rank(52), 20); // 小王
    assert_eq!(dz_rank(53), 30); // 大王
}

#[test]
fn dz_cards2array_col_layout() {
    // 上游 matrix[4,13] flatten('F')：同 rank 4 计数位连续，17→col12(48..51)
    let a = cards2array_ref(&dz(&[3, 3, 4]));
    assert!(a[0] == 1.0 && a[1] == 1.0 && a[2] == 0.0); // rank3 两张 → 0,1
    assert!(a[4] == 1.0 && a[5] == 0.0); // rank4 一张 → 4
    let b = cards2array_ref(&dz(&[17]));
    assert!(b[48] == 1.0 && b[49] == 0.0); // 点数2 → col12
    let c = cards2array_ref(&dz(&[20, 30]));
    assert!(c[52] == 1.0 && c[53] == 1.0); // 双王
}

#[test]
fn dz_move_type_basics() {
    assert_eq!(move_type(&[]).0, 0); // pass
    assert_eq!(move_type(&dz(&[9])).0, 1); // 单张
    assert_eq!(move_type(&dz(&[9, 9])).0, 2); // 对子
    assert_eq!(move_type(&dz(&[9, 9, 9, 9])).0, T_BOMB);
    assert_eq!(move_type(&dz(&[20, 30])).0, T_KING_BOMB);
    // 顺子 3-7
    let (ty, major, len) = move_type(&dz(&[3, 4, 5, 6, 7]));
    assert_eq!((ty, major, len), (T_SERIAL_SINGLE, 3, 5));
    // 顺子含 2（17）非法 → T_WRONG（上游 TYPE_15_WRONG）
    assert_eq!(move_type(&dz(&[10, 11, 12, 13, 17])).0, T_WRONG);
    // 3带1：上游取 move[1] 当主 rank
    assert_eq!(move_type(&dz(&[9, 9, 9, 3])).0, 6);
    // 3带2
    assert_eq!(move_type(&dz(&[9, 9, 9, 3, 3])).0, 7);
}

#[test]
fn dz_gen_serial_excludes_rank2() {
    // 手牌含 2（17）：任何顺子/连对/飞机不得含 2
    let hand = dz(&[10, 11, 12, 13, 14, 17]);
    let mg = MoveGen::new(&hand);
    for mv in mg.gen_serial_single(0) {
        assert!(!mv.contains(&17), "顺子含2: {mv:?}");
        assert!(mv.len() >= 5);
    }
    // 10-A 正好一条顺子
    assert!(mg
        .gen_serial_single(0)
        .iter()
        .any(|mv| mv == &&[10, 11, 12, 13, 14][..]));
}

#[test]
fn dz_gen_3_1_wing_excludes_body_rank() {
    // 三带一翅膀不得与机身同 rank（第 4 张同 rank 不存在——炸弹是独立型）
    let hand = dz(&[9, 9, 9, 3]);
    let mg = MoveGen::new(&hand);
    let moves = mg.gen_3_1();
    // 上游 append t+i 不排序：[9,9,9,3]
    assert!(moves.iter().any(|mv| mv == &&[9, 9, 9, 3][..]));
}

#[test]
fn dz_beats_same_type_only() {
    assert!(beats_move(&dz(&[5]), &dz(&[4])));
    assert!(!beats_move(&dz(&[3]), &dz(&[4])));
    assert!(!beats_move(&dz(&[5, 6, 7, 8, 9]), &dz(&[4]))); // 异型不压
                                                            // 炸弹压一切非炸、大炸压小炸
    assert!(beats_move(&dz(&[9, 9, 9, 9]), &dz(&[4])));
    assert!(!beats_move(&dz(&[3, 3, 3, 3]), &dz(&[9, 9, 9, 9])));
    assert!(beats_move(&dz(&[20, 30]), &dz(&[9, 9, 9, 9])));
    assert!(!beats_move(&dz(&[9, 9, 9, 9]), &dz(&[20, 30])));
}

#[test]
fn dz_responses_include_pass_and_bombs() {
    // 响应单张 9：更大单张 + 炸弹 + pass
    let hand = dz(&[3, 10, 9, 9, 9, 9]);
    let resp = legal_responses(&hand, &dz(&[9]));
    assert!(resp.contains(&vec![]), "响应必含 pass");
    assert!(resp.contains(&vec![10]));
    assert!(!resp.contains(&vec![3]));
    assert!(resp.contains(&vec![9, 9, 9, 9]), "炸弹兜底必在");
    // 响应王炸：只剩 pass
    let resp2 = legal_responses(&hand, &dz(&[20, 30]));
    assert_eq!(resp2, vec![Vec::<i16>::new()]);
}

#[test]
fn dz_lead_has_no_pass() {
    // 领出（rival 空）：全型枚举，绝无 pass
    let hand = dz(&[3, 4, 5, 6, 7, 8]);
    let resp = legal_responses(&hand, &[]);
    assert!(!resp.contains(&vec![]));
    assert!(resp.iter().any(|mv| mv.len() == 6)); // 六连顺
}

#[test]
fn dz_serial_triple_with_king_wings() {
    // 飞机 333444 带两单（含王）——翅膀按 rank 组合
    let hand = dz(&[3, 3, 3, 4, 4, 4, 20, 30]);
    let resp = legal_responses(&hand, &dz(&[5, 5, 5, 8, 8, 8]));
    // 上游口径：333444(4) 压不过 555888(8) → 同型候选全滤，剩王炸兜底+pass
    assert_eq!(resp, vec![vec![20, 30], Vec::<i16>::new()]);
    // 反向：rival 333444(纯连三TYPE_10) → 上游只给纯连三候选+pass（不带翅，
    // 响应同型口径），666777 压过 333444
    let hand2 = dz(&[6, 6, 6, 7, 7, 7, 3, 5]);
    let resp2 = legal_responses(&hand2, &dz(&[3, 3, 3, 4, 4, 4]));
    assert_eq!(resp2, vec![vec![6, 6, 6, 7, 7, 7], Vec::<i16>::new()]);
    // 领出口径：rival 空 → 全型枚举，带翅飞机 666777+35 必须在集
    let lead = legal_responses(&hand2, &[]);
    assert!(
        lead.iter().any(|mv| mv.len() == 8
            && mv.windows(6).any(|w| w == [6, 6, 6, 7, 7, 7])),
        "领出应含带翅飞机: {lead:?}"
    );
}

#[test]
fn dz_encoder_dimensions() {
    // landlord 373 / farmer 484 维——上游 x 段序
    use super::doudizhu_dz_encode::X_DIM_FARMER;
    use super::doudizhu_dz_encode::X_DIM_LANDLORD;
    assert_eq!(X_DIM_LANDLORD, 373);
    assert_eq!(X_DIM_FARMER, 484);
}

#[test]
fn dz_debug2() {
    let hand = dz(&[3, 3, 3, 4, 4, 4, 20, 30]);
    let mg = MoveGen::new(&hand);
    println!("s31(2): {:?}", mg.gen_serial_3_1(2));
    println!("bombs: {:?}", mg.gen_bombs());
    println!("king: {:?}", mg.gen_king_bomb());
    let resp = legal_responses(&hand, &dz(&[5, 5, 5, 8, 8, 8]));
    println!("resp: {:?}", resp);
}

/// 真模型端到端：三座位建议都能出（或 pass），且状态机全接。
/// #[ignore] 门控：CI 无模型/无 ort 编译也能绿，本地带 --ignored 实测。
#[test]
#[ignore]
fn dz_real_model_end_to_end() {
    use super::doudizhu_dz::dz_choose_safe;
    use super::doudizhu_game::{GameState, Phase};
    // 固定牌组三手（同 sim_tests 的确定性切法）
    let (hands, kitty) = {
        let mut deck: Vec<u8> = (0..54u8).collect();
        let mut x = 7u32;
        for i in (1..deck.len()).rev() {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            let j = (x as usize) % (i + 1);
            deck.swap(i, j);
        }
        let mut hs = [[0u8; 17]; 3];
        for s in 0..3 {
            let mut cards = deck[s * 17..(s + 1) * 17].to_vec();
            cards.sort_by_key(|&c| {
                std::cmp::Reverse(super::doudizhu::Rank::of(c))
            });
            hs[s].copy_from_slice(&cards);
        }
        let mut k = [0u8; 3];
        k.copy_from_slice(&deck[51..54]);
        (hs, k)
    };
    let mut g = GameState::new(
        [hands[0].to_vec(), hands[1].to_vec(), hands[2].to_vec()],
        kitty.to_vec(),
    );
    // 叫分定地主（seat0 建议 3 直接过）
    while g.phase == Phase::Bidding {
        let seat = g.turn;
        let bid = GameState::suggested_bid(&g.hands[seat as usize]).min(3);
        g.bid(seat, bid).unwrap();
    }
    while g.phase == Phase::Grab {
        let seat = g.turn;
        let take = GameState::suggested_bid(&g.hands[seat as usize]) >= 2;
        g.grab(seat, take).unwrap();
    }
    if g.phase == Phase::Discard {
        let seat = g.turn;
        g.discard(seat, &[]).unwrap();
    }
    while g.phase == Phase::Double {
        let seat = g.turn;
        g.double(seat, 0).unwrap();
    }
    // 模型必须可用（本测试带模型跑）
    assert!(
        super::doudizhu_dz_nn::available(),
        "模型不可用——检查 models/doudizhu/ 落盘"
    );
    // 全 AI 对局：每手都用模型建议
    let mut steps = 0;
    while g.phase == Phase::Playing {
        steps += 1;
        assert!(steps < 600, "对局卡死");
        let seat = g.turn;
        let cards = dz_choose_safe(&g, seat, false).unwrap_or_default();
        if cards.is_empty() && g.last.is_none() {
            // 领出必须出牌：模型给了 pass 是缺陷，兜底最小单张
            let min = *g.hands[seat as usize].iter().min().unwrap();
            g.play(seat, &[min]).unwrap();
        } else if g.play(seat, &cards).is_err() && !cards.is_empty() {
            g.play(seat, &[]).ok();
        }
    }
    assert!(g.finished.is_some(), "终局未到");
    println!("dz end-to-end steps={steps}");
}
