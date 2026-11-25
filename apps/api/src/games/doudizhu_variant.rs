//! 斗地主对局变体：癞子开局（从 `doudizhu_game.rs` 拆出贴 300 行门禁）。
//!
//! `new_wild` 在经典开局之上按「底牌最后一张的点数」定癞子点——该点数
//! 的 4 张牌全部成为万能牌（不可当王），记进 `GameState.wildcards`。
//! 癞子合法性判定在 `doudizhu_wild::wild_beats`（枚举替身面）。

use super::doudizhu_game::GameState;

/// 癞子模式开局：发牌后按底牌定癞子。
pub fn new_wild(hands: [Vec<u8>; 3], kitty: Vec<u8>) -> GameState {
    let mut g = GameState::new(hands, kitty);
    g.wildcards = super::doudizhu_wild::pick_wildcards(&g.hands, &g.kitty);
    g
}

/// 春天判定（从 GameState::is_spring 下沉）。
pub fn is_spring(g: &GameState) -> bool {
    let Some(winner) = g.finished else {
        return false;
    };
    let Some(landlord) = g.landlord else {
        return false;
    };
    if winner == landlord {
        // 地主赢：农民全程没出牌 = 春天
        g.farmers().iter().all(|&f| {
            g.events
                .iter()
                .filter(|e| e.seat == f && e.action == "play")
                .count()
                == 0
        })
    } else {
        // 反春：农民赢且地主全程只出过 1 手（首出那手）。
        // 旧口径 <=1 把「地主 0 手」也算反春，不合经典规则；
        // 地主 0 手在真实对局中不可能（地主首出必出牌），收紧为 ==1。
        g.events
            .iter()
            .filter(|e| e.seat == landlord && e.action == "play")
            .count()
            == 1
    }
}

/// 建议叫分：王+2 总量粗评牌力（纯启发式，0..3）。
/// 大王+2 个 2 以上叫 3；单王或 2 个 2 叫 2；有 2/王叫 1；否则不叫。
pub fn suggested_bid(hand: &[u8]) -> i64 {
    let mut pts = 0i64;
    for &c in hand {
        let r = super::doudizhu::Rank::of(c).0;
        if r == 13 || r == 14 {
            pts += 2; // 王
        } else if r == 12 {
            pts += 1; // 2
        }
    }
    if pts >= 5 {
        3
    } else if pts >= 2 {
        2
    } else if pts >= 1 {
        1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(rank: u8, suit: usize) -> u8 {
        rank * 4 + suit as u8
    }

    #[test]
    fn new_wild_sets_wildcards_by_kitty_last() {
        // 底牌最后一张 = rank 7 → 三座的 rank7 牌全是癞子
        let hands = [
            vec![c(7, 0), c(3, 1)],
            vec![c(7, 1), c(4, 2)],
            vec![c(9, 0)],
        ];
        let kitty = vec![c(2, 2), c(5, 1), c(7, 3)];
        let g = new_wild(hands, kitty);
        assert_eq!(g.wildcards[0], vec![c(7, 0)]);
        assert_eq!(g.wildcards[1], vec![c(7, 1)]);
        assert!(g.wildcards[2].is_empty());
    }

    #[test]
    fn classic_game_has_empty_wildcards() {
        let g = GameState::new(
            [vec![c(3, 0)], vec![c(4, 0)], vec![c(5, 0)]],
            vec![c(6, 0), c(6, 1), c(6, 2)],
        );
        assert!(g.wildcards.iter().all(|w| w.is_empty()));
    }
    #[test]
    fn landlord_kitty_wildcards_registered() {
        // 癞子点 = 7（kitty 末张），底牌含一张 7：地主拿底后那张 7 必须
        // 进 wildcards（资深玩家实测：底牌癞子曾当普通牌用被拒）。
        let hands = [vec![c(7, 0)], vec![c(3, 1)], vec![c(4, 2)]];
        let kitty = vec![c(2, 2), c(5, 1), c(7, 3)];
        let mut g = new_wild(hands, kitty);
        assert_eq!(g.wildcards[0], vec![c(7, 0)]);
        assert!(g.wildcards[2].is_empty());
        // 玩家叫 3 当地主 → 拿 3 张底牌（含 7）
        g.bid(0, 3).unwrap();
        assert_eq!(g.landlord, Some(0));
        assert!(
            g.wildcards[0].contains(&c(7, 3)),
            "底牌中的癞子必须登记进地主 wildcards"
        );
    }
}
