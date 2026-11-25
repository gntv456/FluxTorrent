//! 固定抽签的抽选纯函数（对标 tangpt fixed-lottery 的 HMAC bottom-k）。
//!
//! 【为什么不用 0126 的 `ORDER BY random()`】论坛抽奖赌的是「分池」，结果
//! 无需复算；固定抽签发的是真奖励（邀请/VIP/勋章），「公平」是唯一的信任
//! 基础——排序键必须可复算。协议与 [`crate::domain::lottery_fair`] 同构：
//!
//! 1. 开奖瞬间生成 32 字节 CSPRNG `seed`（[`super::lottery_fair::new_seed`]）。
//! 2. 每个候选人排序键 = `HMAC-SHA256(key = seed, msg = "{cid}:{uid}")`。
//!    cid 进 msg：同 seed 在不同活动给出完全不同的次序；uid 进 msg：每人
//!    一密钥，排序不退化为入参顺序。
//! 3. 键升序取前 N（bottom-k 不放回抽样，天然「每人限中一次」）。
//! 4. seed、候选快照哈希（[`snapshot_hash`]）落 `draw_meta`；任何人拿到
//!    seed + 报名名单即可离线重放比对中奖位次，无须库权限。
//!
//! seed 在候选集冻结后才生成——管理员唯一可操作的是开奖时机，而键是新鲜
//! 随机量的函数，时机不影响均匀性。
//!
//! 本模块纯函数（FRAME §7 第 6 条）：无 IO、无 actix、无 sqlx。

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

/// 单个候选人的抽选材料：uid + 排序键。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub uid: i64,
    pub key: [u8; 32],
}

/// 一场抽选的完整结果：按排序键升序的全量次序。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub ordered: Vec<Candidate>,
}

/// 候选人排序键：`HMAC-SHA256(key = seed, msg = "{campaign_id}:{uid}")`。
pub fn candidate_key(seed_hex: &str, campaign_id: i64, uid: i64) -> [u8; 32] {
    let Ok(mut mac) = HmacSha256::new_from_slice(seed_hex.as_bytes()) else {
        // HMAC 接受任意长度 key；真失败说明实现坏了。
        return [0u8; 32];
    };
    mac.update(campaign_id.to_string().as_bytes());
    mac.update(b":");
    mac.update(uid.to_string().as_bytes());
    let out = mac.finalize().into_bytes();
    let mut k = [0u8; 32];
    k.copy_from_slice(&out);
    k
}

/// 全量次序：按排序键升序；键撞时 uid 升序兜底（HMAC 撞键可忽略，
/// 但兜底保证全序——复算必须确定）。
pub fn selection_of(
    seed_hex: &str,
    campaign_id: i64,
    uids: &[i64],
) -> Selection {
    let mut ordered: Vec<Candidate> = uids
        .iter()
        .map(|&uid| Candidate {
            uid,
            key: candidate_key(seed_hex, campaign_id, uid),
        })
        .collect();
    ordered.sort_by(|a, b| a.key.cmp(&b.key).then(a.uid.cmp(&b.uid)));
    Selection { ordered }
}

/// 候选快照哈希：uid 升序逐个拼接的 SHA-256。
///
/// 与 tangpt 插件同口径（`hash_update(uid ‖ "\n")`）；进 draw_meta 后，
/// 事后改报名表会让快照对不上——「无法暗改名单」的证据。
pub fn snapshot_hash(uids: &[i64]) -> String {
    let mut sorted = uids.to_vec();
    sorted.sort_unstable();
    let mut h = Sha256::new();
    for uid in sorted {
        h.update(uid.to_string().as_bytes());
        h.update(b"\n");
    }
    hex_encode(&h.finalize())
}

/// 槽位分配：把全量次序前 `slots` 位按奖项顺序展开。
///
/// `prizes` 按 (sort_order, prize_id) 已排；候选人少于总名额时按实际人数
/// 出奖，剩余名额空缺（不回填、不跨奖项挪用）。返回
/// `(prize_id, slot_no, draw_position, uid)` 列表。
pub fn assign_slots(
    sel: &Selection,
    prizes: &[(i64, i32)],
) -> Vec<(i64, i32, i64, i64)> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    for &(prize_id, qty) in prizes {
        for slot_no in 1..=qty.max(0) {
            let Some(c) = sel.ordered.get(pos) else {
                return out; // 候选耗尽：剩余名额全部空缺
            };
            pos += 1;
            out.push((prize_id, slot_no, pos as i64, c.uid));
        }
    }
    out
}

fn hex_encode(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

#[cfg(test)]
mod tests;
