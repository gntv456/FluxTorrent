//! 斗地主对局状态机：叫分 → 出牌 → 结算的**纯状态推进**层。
//!
//! 状态整体序列化进 `arcade_doudizhu_games.state` jsonb（服务端权威），
//! 本层只做「读快照 → 校验动作 → 写新快照」，不碰 DB 不拿时钟；
//! `turn_deadline` 由 HTTP 层按设置键算好传入。
//!
//! 座位 0 = 玩家，1/2 = AI。轮转 0→1→2→0。地主确定后 farmers() 给出
//! 玩家阵营；出完手牌即终局，春天判定看农民是否一张未出。

use super::doudizhu::{beats, detect, Combo, ComboKind, Rank};
use super::doudizhu_settle::Role;
use serde::{Deserialize, Serialize};

/// 阶段。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    /// 叫分中（按座轮流报 0..3 分）。
    Bidding,
    /// 出牌中。
    Playing,
    /// 已终局（结算数据回填 DB 列，快照保留 events）。
    Done,
}

/// 一次出牌/叫分记录（复盘用，前端直接重放）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub seat: u8,
    /// bid=叫分（score），play=出牌（cards），pass=过。
    pub action: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score: Option<i64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cards: Vec<u8>,
}

/// 完整牌局快照（DB jsonb）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    pub hands: [Vec<u8>; 3],
    pub kitty: Vec<u8>,
    pub phase: Phase,
    /// 当前轮到哪个座位（叫分阶段=下一个叫分者；出牌阶段=下一个出牌者）。
    pub turn: u8,
    /// 叫分记录 (seat, score)；全 0 流局重开（HTTP 层处理）。
    pub bids: Vec<(u8, i64)>,
    /// 地主座位（叫分完成后落定）。
    pub landlord: Option<u8>,
    /// 桌面最后一手（None = 两家都要不起/新领出）。
    pub last: Option<Combo>,
    /// 最后一手是谁出的。
    pub last_seat: Option<u8>,
    /// 连续过牌计数（=2 时桌面清空）。
    pub passes: u8,
    /// 累计炸弹/王炸数。
    pub bombs: u32,
    /// 出牌流水（终局判春天/反春）。
    pub events: Vec<Event>,
    /// 终局结果（seat → 剩余张数），结算由 HTTP 层取数。
    pub finished: Option<u8>,
    /// 癞子牌座位表：`wildcards[seat]` = 该手牌中的癞子编码集合。
    /// 缺省空 = 经典模式（0521 存量快照无此字段，serde default 兼容）。
    #[serde(default)]
    pub wildcards: WildMap,
}

/// 癞子映射：seat → 该座手牌中被指定为癞子的牌编码列表。
/// 经典模式恒空；癞子模式在发牌时按规则选定（见 `doudizhu::pick_wildcards`）。
pub type WildMap = [Vec<u8>; 3];

impl GameState {
    /// 开局快照：三家手牌（降序）+ 底牌，玩家先叫分。
    pub fn new(hands: [Vec<u8>; 3], kitty: Vec<u8>) -> Self {
        GameState {
            hands,
            kitty,
            phase: Phase::Bidding,
            turn: 0,
            bids: Vec::new(),
            landlord: None,
            last: None,
            last_seat: None,
            passes: 0,
            bombs: 0,
            events: Vec::new(),
            finished: None,
            wildcards: [Vec::new(), Vec::new(), Vec::new()],
        }
    }

    /// 玩家（seat 0）的角色；未定地主返回 None。
    pub fn player_role(&self) -> Option<Role> {
        self.landlord.map(
            |s| {
                if s == 0 {
                    Role::Landlord
                } else {
                    Role::Farmer
                }
            },
        )
    }

    /// 两家农民座位。
    pub fn farmers(&self) -> [u8; 2] {
        let l = self.landlord.unwrap_or(0);
        [(l + 1) % 3, (l + 2) % 3]
    }

    /// 叫分：score 0..3；3 即刻定地主；全部叫完取最高分者。
    /// 返回 Err(()) = 非法（分数超档、重复座位、不是该座叫分）。
    pub fn bid(&mut self, seat: u8, score: i64) -> Result<(), &'static str> {
        if self.phase != Phase::Bidding {
            return Err("当前不在叫分阶段");
        }
        if !(0..=3).contains(&score) {
            return Err("叫分必须是 0..3");
        }
        if seat != self.turn {
            return Err("还没轮到该座叫分");
        }
        self.bids.push((seat, score));
        self.events.push(Event {
            seat,
            action: "bid".into(),
            score: Some(score),
            cards: Vec::new(),
        });
        // 有人叫 3：立刻定地主
        if score == 3 {
            self.landlord = Some(seat);
            self.enter_playing(seat);
            return Ok(());
        }
        if self.bids.len() == 3 {
            // 全 0 = 流局；否则最高分者为地主（同分先叫先得——
            // max_by_key 取最后最大者，这里手动按序取首个最大）
            let mut best: Option<(u8, i64)> = None;
            for &(seat, s) in &self.bids {
                if s > 0 && best.is_none_or(|(_, bs)| s > bs) {
                    best = Some((seat, s));
                }
            }
            let Some((seat, _)) = best else {
                self.phase = Phase::Done;
                self.finished = Some(u8::MAX); // 约定：MAX = 流局重开
                return Ok(());
            };
            self.landlord = Some(seat);
            self.enter_playing(seat);
            return Ok(());
        }
        self.turn = (self.turn + 1) % 3;
        Ok(())
    }

    fn enter_playing(&mut self, landlord: u8) {
        self.phase = Phase::Playing;
        // 地主拿底牌
        let kitty = std::mem::take(&mut self.kitty);
        let hand = &mut self.hands[landlord as usize];
        hand.extend(&kitty);
        hand.sort_by_key(|c| std::cmp::Reverse(Rank::of(*c)));
        self.turn = landlord;
        // 癞子桌：底牌中的癞子点牌随底牌归地主，必须补进地主的
        // wildcards——否则地主手里的底牌癞子走不了枚举判定面（资深
        // 玩家实测：地主拿 3 张底牌癞子却当普通牌用）。
        let wr = self
            .wildcards
            .iter()
            .flatten()
            .next()
            .copied()
            .map(|c| Rank::of(c).0);
        if let Some(wr) = wr {
            for c in hand.iter() {
                let e = *c;
                if Rank::of(e).0 == wr
                    && !self.wildcards[landlord as usize].contains(&e)
                {
                    self.wildcards[landlord as usize].push(e);
                }
            }
        }
    }

    /// 玩家出牌：`cards` 为空 = 过牌（领出者不许过）。
    pub fn play(&mut self, seat: u8, cards: &[u8]) -> Result<(), &'static str> {
        if self.phase != Phase::Playing {
            return Err("当前不在出牌阶段");
        }
        if seat != self.turn {
            return Err("还没轮到该座出牌");
        }
        if cards.is_empty() {
            if self.last.is_none() {
                return Err("领出不能过牌");
            }
            self.passes += 1;
            self.events.push(Event {
                seat,
                action: "pass".into(),
                score: None,
                cards: Vec::new(),
            });
            if self.passes >= 2 {
                // 两家都过：桌面清空，**最后出牌者**（本轮赢家）重新领出。
                // 此前写 (最后过牌者+1)%3——AI1 出牌、玩家+AI2 都过时会
                // 轮到 AI2 领出而非 AI1，托管代打可因此死循环（玩家永远
                // 压不过同一张领出牌）。
                let winner = self.last_seat;
                self.last = None;
                self.last_seat = None;
                self.passes = 0;
                self.turn = winner.unwrap_or((seat + 1) % 3);
            } else {
                self.turn = (self.turn + 1) % 3;
            }
            return Ok(());
        }
        // 手牌校验 + 牌型校验 + 压制校验
        let hand = &self.hands[seat as usize];
        let mut known: Vec<u8> = hand.clone();
        for &c in cards {
            let pos = known.iter().position(|&k| k == c);
            let Some(pos) = pos else {
                return Err("手里没有这张牌");
            };
            known.remove(pos);
        }
        // 牌型判定：癞子座走枚举面（wildcards 非空），经典座走 detect。
        // prev 在两种判定里语义一致：别家出的牌需要被压过。
        let prev = match (&self.last, self.last_seat) {
            (Some(c), Some(s)) if s != seat => Some(c),
            _ => None, // 自己上轮的牌继续领出，可任意出
        };
        let is_wild_seat = !self.wildcards[seat as usize].is_empty();
        let combo = if is_wild_seat {
            let wilds_in_sel: Vec<u8> = cards
                .iter()
                .copied()
                .filter(|c| self.wildcards[seat as usize].contains(c))
                .collect();
            super::doudizhu_wild::wild_beats(cards, &wilds_in_sel, prev)
                .ok_or("不是合法牌型或压不过上一手")?
        } else {
            let c = detect(cards).ok_or("不是合法牌型")?;
            if !beats(&c, prev) {
                return Err("压不过上一手");
            }
            c
        };
        if combo.kind == ComboKind::Bomb || combo.kind == ComboKind::Rocket {
            self.bombs += if combo.kind == ComboKind::Rocket {
                2
            } else {
                1
            };
        }
        // 收回手牌
        for &c in cards {
            let pos = self.hands[seat as usize]
                .iter()
                .position(|&k| k == c)
                .expect("上面已校验");
            self.hands[seat as usize].remove(pos);
        }
        self.events.push(Event {
            seat,
            action: "play".into(),
            score: None,
            cards: cards.to_vec(),
        });
        self.last = Some(combo);
        self.last_seat = Some(seat);
        self.passes = 0;
        if self.hands[seat as usize].is_empty() {
            self.phase = Phase::Done;
            self.finished = Some(seat);
            return Ok(());
        }
        self.turn = (self.turn + 1) % 3;
        Ok(())
    }

    /// 春天：地主赢且两农民一张未出；反春：农民赢且地主只出过底牌
    /// 那一手。终局前返回 false。实现下沉
    /// `doudizhu_variant::is_spring`（主文件贴 300 门禁）。
    pub fn is_spring(&self) -> bool {
        super::doudizhu_variant::is_spring(self)
    }

    /// 建议叫分：王+2+炸弹总量粗评牌力（纯启发式，0..3）。
    /// 实现下沉 `doudizhu_variant::suggested_bid`（主文件贴 300 门禁）。
    pub fn suggested_bid(hand: &[u8]) -> i64 {
        super::doudizhu_variant::suggested_bid(hand)
    }
}
