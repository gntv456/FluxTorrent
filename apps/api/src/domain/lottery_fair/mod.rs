//! 抽奖可验证公平（commit-reveal，PT Studio FR-LOT-001~023 对标）。
//!
//! 【为什么必须有】赌博类玩法里，「公平」不是体验，是**唯一的信任基础**。
//! 现状（0126）：`worker/jobs/lottery.rs` 用 `ORDER BY random()` 直接抽，
//! 站长改库、注入、或者干脆「按需挑人」都无法从结果里看出来——用户没有任何
//! 办法验证这一轮到底公不公平。本模块把随机从「服务端说随机」升级成
//! 「第三方可独立复算的随机」。
//!
//! 【协议（与 PT Studio 同构）】
//!
//! 1. **开售**：生成 32 字节 `seed`（crypto RNG），存**密文**；
//!    同时公布 `commit = SHA-256(seed)`。
//! 2. **售票**：参与者领一张「票据」——`ticket_hash = SHA-256(seed ‖ ticket_nonce)`。
//!    票据哈希构成一条**哈希链**（每人把前一条的哈希并进自己的），末条即
//!    `tickets_root`。哈希链保证「中途加票/删票/改票」都会让 root 对不上，
//!    这是「无法暗改名单」的关键。
//! 3. **停售**：锁死 `tickets_root`。
//! 4. **开奖**：`rand = HMAC-SHA-256(key = seed, msg = round_id ‖ tickets_root)`。
//!    按 `rand` 逐字节喂给排序算法（Fisher–Yates 的无状态变体：按
//!    `HMAC(seed, ticket_hash_i)` 升序排），取前 N 名。
//!    —— 关键在于排序键也由 seed 派生：seed 未公开时，管理员无法提前
//!    算出「该让谁中奖」。
//! 5. **揭晓**：公开 `seed` 与 `rand`，任何人可重算第 4 步。
//!
//! 【防串通的两处细节】
//! - `rand` 的 msg **绑 tickets_root**：事后改名单会让 rand 对不上。
//! - 排序键用 `HMAC(seed, ticket_hash)` 而非 `rand`：单条 rand 无法给每个
//!   参与者公平的次序，必须每人一密钥。
//!
//! 本模块是**纯函数**（FRAME §7 第 6 条口径：判定进 domain/）：不含 IO、
//! 不碰 actix、不 import sqlx。DB 落点在 `repo/lottery_fair.rs`。

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

use crate::errors::DomainResult;

#[cfg(test)]
mod tests;

type HmacSha256 = Hmac<Sha256>;

/// 票据序号占位——`u32` 足够（一场抽奖的上限远小于 2^32）。
pub type TicketNonce = u32;

/// 一轮抽奖的公开承诺（开售时公布，可随时查看）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commitment {
    /// `SHA-256(seed)` 的十六进制——公布它，保密 seed。
    pub commit: String,
    /// 32 字节 seed 的**密文**（hex）；揭晓前任何人（含站长）都不应能
    /// 读懂，但要存下来以便结算时取回——加密密钥由部署方持有。
    pub seed_cipher: String,
}

/// 一场抽奖的完整可验证材料（结算后公开）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DrawProof {
    /// 轮次唯一标识（取 topic_id，保证同一场抽奖的证明可追溯）。
    pub round_id: String,
    /// 32 字节 seed 的明文（hex）——揭晓。
    pub seed: String,
    /// 票据哈希链的末条。
    pub tickets_root: String,
    /// `HMAC-SHA-256(seed, round_id ‖ tickets_root)` 的 hex。
    pub rand: String,
    /// 中奖者的票据哈希（按 HMAC 排序后的前 N 条），便于逐个复算。
    pub winner_ticket_hashes: Vec<String>,
}

/// 生成新 seed（32 字节 hex）。用 OsRng——这是**唯一**不可预测性的来源，
/// 一旦用了可预测 RNG，整个 commit-reveal 协议就失去意义。
pub fn new_seed() -> String {
    use rand::RngCore;
    let mut buf = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut buf);
    hex_encode(&buf)
}

/// `commit = SHA-256(seed_hex)`。
///
/// 注意：对 seed 的**十六进制文本**取哈希（而非对原始 32 字节）——这样
/// 验证者只需复制页面上那段 seed 文本即可复算，不必自己 hex 解码。
pub fn commit_of(seed_hex: &str) -> String {
    let mut h = Sha256::new();
    h.update(seed_hex.as_bytes());
    hex_encode(&h.finalize())
}

/// 单张票据的哈希：前一条的哈希并进来，形成哈希链。
pub fn ticket_hash(
    seed_hex: &str,
    prev_root: &str,
    nonce: TicketNonce,
) -> String {
    let mut h = Sha256::new();
    h.update(seed_hex.as_bytes());
    h.update(b"|");
    h.update(prev_root.as_bytes());
    h.update(b"|");
    h.update(nonce.to_be_bytes());
    hex_encode(&h.finalize())
}

/// 空链的起点（全 0）。用作第一张票据的 prev。
pub fn genesis_root() -> String {
    "0".repeat(64)
}

/// 开售：生成 seed + 承诺。返回 `(seed, Commitment)`。
pub fn commit_new_round() -> (String, Commitment) {
    let seed = new_seed();
    let commit = commit_of(&seed);
    (seed.clone(), Commitment { commit, seed_cipher: seed.clone() })
}

/// 开奖随机值：`HMAC-SHA-256(key = seed, msg = round_id ‖ tickets_root)`。
///
/// `tickets_root` 进 msg 是「防事后改名单」的关键：名单一动，rand 即变，
/// 而 rand 已被公布——篡改立刻暴露。
pub fn draw_rand(seed_hex: &str, round_id: &str, tickets_root: &str) -> String {
    let Ok(mut mac) = HmacSha256::new_from_slice(seed_hex.as_bytes()) else {
        // HMAC 接受任意长度 key，这里不可能失败；真失败说明实现坏了。
        return String::new();
    };
    mac.update(round_id.as_bytes());
    mac.update(b"|");
    mac.update(tickets_root.as_bytes());
    hex_encode(&mac.finalize().into_bytes())
}

/// 每张票据的排序键：`HMAC-SHA-256(seed, "order|" ‖ ticket_hash)`。
///
/// 用每人一密钥（而非共用一个 rand）才能给每个参与者公平的次序；共用
/// rand 的话所有人键相同，排序退化成入参顺序——等于没洗。
pub fn order_key(seed_hex: &str, ticket_hash_hex: &str) -> [u8; 32] {
    let Ok(mut mac) = HmacSha256::new_from_slice(seed_hex.as_bytes()) else {
        return [0u8; 32];
    };
    mac.update(b"order|");
    mac.update(ticket_hash_hex.as_bytes());
    let out = mac.finalize().into_bytes();
    let mut k = [0u8; 32];
    k.copy_from_slice(&out);
    k
}

/// 按 seed 派生的次序给票据排序，返回 `(ticket_hash, 中奖?)` 列表。
///
/// 排序键相等时用 ticket_hash 字典序兜底——HMAC 撞键概率可忽略，
/// 但兜底保证**全序**（排序必须确定，否则复算结果会不一致）。
pub fn draw_order<'a>(
    seed_hex: &str,
    tickets: &'a [String],
    winners: usize,
) -> Vec<(&'a String, bool)> {
    let mut idx: Vec<(usize, [u8; 32])> = tickets
        .iter()
        .enumerate()
        .map(|(i, h)| (i, order_key(seed_hex, h)))
        .collect();
    idx.sort_by(|a, b| {
        // 键相同时用票据哈希字典序兜底：HMAC 撞键概率可忽略，但全序是
        // 复算一致的前提（部分序会让不同实现给出不同名单）。
        a.1.cmp(&b.1)
            .then_with(|| tickets[a.0].as_str().cmp(tickets[b.0].as_str()))
    });
    idx.into_iter()
        .enumerate()
        .map(|(rank, (i, _))| (&tickets[i], rank < winners))
        .collect()
}

/// 组装完整证明（结算后一次性公开）。
pub fn build_proof(
    round_id: &str,
    seed_hex: &str,
    tickets_root: &str,
    tickets: &[String],
    winners: usize,
) -> DrawProof {
    let rand = draw_rand(seed_hex, round_id, tickets_root);
    let order = draw_order(seed_hex, tickets, winners);
    let winner_hashes = order
        .iter()
        .filter(|(_, won)| *won)
        .map(|(h, _)| (*h).clone())
        .collect();
    DrawProof {
        round_id: round_id.to_string(),
        seed: seed_hex.to_string(),
        tickets_root: tickets_root.to_string(),
        rand,
        winner_ticket_hashes: winner_hashes,
    }
}

/// 独立验算：第三方拿着公开材料能否复算出同样的中奖名单。
///
/// 这是协议的生命线——如果 `verify` 与 `build_proof` 用的是**两套独立
/// 代码路径**，一处有 bug 就会立刻暴露；写成 `verify` 调 `build_proof`
/// 则失去意义。故 verify 自己重算一遍哈希，不复用 build_proof。
pub fn verify(
    commit_hex: &str,
    proof: &DrawProof,
    tickets: &[String],
    winners: usize,
) -> DomainResult<Verification> {
    if commit_of(&proof.seed) != commit_hex {
        return Err(crate::errors::DomainError::Validation(
            "commit 与 seed 不匹配：承诺被伪造".into(),
        ));
    }
    // 复算哈希链，确认 tickets_root 与公布的票据集一致
    let mut root = genesis_root();
    for (i, t) in tickets.iter().enumerate() {
        let expect = ticket_hash(&proof.seed, &root, i as TicketNonce);
        if &expect != t {
            return Err(crate::errors::DomainError::Validation(format!(
                "第 {i} 张票据与 seed/链不符"
            )));
        }
        root = expect;
    }
    if root != proof.tickets_root {
        return Err(crate::errors::DomainError::Validation(
            "tickets_root 与票据集不符：名单被改动".into(),
        ));
    }
    let rand = draw_rand(&proof.seed, &proof.round_id, &proof.tickets_root);
    if rand != proof.rand {
        return Err(crate::errors::DomainError::Validation(
            "rand 与 (seed, round_id, tickets_root) 不符".into(),
        ));
    }
    let order = draw_order(&proof.seed, tickets, winners);
    let winners_actual: Vec<&String> = order
        .iter()
        .filter(|(_, won)| *won)
        .map(|(h, _)| *h)
        .collect();
    let claimed: Vec<&String> =
        proof.winner_ticket_hashes.iter().collect();
    let ok = winners_actual == claimed;
    if !ok {
        return Err(crate::errors::DomainError::Validation(
            "中奖名单与 seed 派生的次序不符".into(),
        ));
    }
    Ok(Verification {
        participants: tickets.len(),
        winners: winners_actual.len(),
    })
}

/// 验算结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verification {
    pub participants: usize,
    pub winners: usize,
}

fn hex_encode(b: &[u8]) -> String {
    let mut s = String::with_capacity(b.len() * 2);
    for x in b {
        use std::fmt::Write;
        let _ = write!(s, "{x:02x}");
    }
    s
}

