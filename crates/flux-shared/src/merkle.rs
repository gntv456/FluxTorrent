//! 每日结算凭证：Merkle 树的唯一实现（worker 封存 / api 出证明共用）。
//!
//! 叶子与内部节点用不同前缀哈希（0x00 / 0x01），避免第二原像；
//! 奇数层复制末节点，证明里的 position 决定拼接顺序。
//! 本模块只证明「记录未被事后改动」，不判断金额规则对错。

use sha2::{Digest, Sha256};

pub const ALGORITHM: &str = "sha256-merkle-v1";
/// 不写链、不锚定外部账本；公开树根即可。
pub const ANCHORING: &str = "none";
pub const TIMEZONE: &str = "Asia/Shanghai";

/// 一层证明：兄弟哈希 + 它在当前节点左边还是右边。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ProofStep {
    pub position: &'static str,
    pub hash: String,
}

/// 一条流水的规范化字段（叶子原文即由这些字段拼成）。
pub struct LedgerLeaf<'a> {
    pub day: &'a str,
    pub id: i64,
    pub user_id: i64,
    pub amount: i64,
    pub kind: &'a str,
    pub ref_type: Option<&'a str>,
    pub ref_id: Option<i64>,
    pub balance_after: Option<i64>,
    pub created_at: &'a str,
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

fn unhex(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(s.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some(out)
}

fn hashed(prefix: u8, parts: &[&[u8]]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update([prefix]);
    for p in parts {
        h.update(p);
    }
    h.finalize().into()
}

pub fn leaf_hash(preimage: &str) -> String {
    hex(&hashed(0, &[preimage.as_bytes()]))
}

/// 没有流水的日子也要有确定值，才能把「空日」也封存下来。
pub fn empty_root() -> String {
    leaf_hash("flux-ledger-empty-v1")
}

fn levels(leaves: &[String]) -> Option<Vec<Vec<[u8; 32]>>> {
    if leaves.is_empty() {
        return Some(Vec::new());
    }
    let mut level: Vec<[u8; 32]> =
        leaves.iter().map(|s| unhex(s)).collect::<Option<_>>()?;
    let mut out = Vec::new();
    loop {
        // 证明也要落在补齐后的层上：只在哈希时补末节点会让
        // 「最后一个叶子」的兄弟下标越界。
        let mut padded = level.clone();
        if padded.len() % 2 == 1 && padded.len() > 1 {
            let last = *padded.last()?;
            padded.push(last);
        }
        out.push(padded.clone());
        if padded.len() <= 1 {
            break;
        }
        level = padded
            .chunks(2)
            .map(|c| hashed(1, &[&c[0][..], &c[1][..]]))
            .collect();
    }
    Some(out)
}

pub fn root(leaves: &[String]) -> Option<String> {
    if leaves.is_empty() {
        return Some(empty_root());
    }
    let ls = levels(leaves)?;
    Some(hex(ls.last()?.first()?))
}

pub fn proof(leaves: &[String], index: usize) -> Option<Vec<ProofStep>> {
    let ls = levels(leaves)?;
    if index >= leaves.len() {
        return None;
    }
    let mut idx = index;
    let mut out = Vec::with_capacity(ls.len().saturating_sub(1));
    for level in ls.iter().take(ls.len().saturating_sub(1)) {
        let sibling = level.get(idx ^ 1)?;
        out.push(ProofStep {
            position: if idx % 2 == 0 { "right" } else { "left" },
            hash: hex(sibling),
        });
        idx /= 2;
    }
    Some(out)
}

pub fn verify(
    preimage: &str,
    leaf: &str,
    proof: &[ProofStep],
    root: &str,
) -> bool {
    if leaf_hash(preimage) != leaf {
        return false;
    }
    let mut cur = match unhex(leaf) {
        Some(h) => h,
        None => return false,
    };
    for step in proof {
        let sibling = match unhex(&step.hash) {
            Some(h) => h,
            None => return false,
        };
        cur = match step.position {
            "left" => hashed(1, &[&sibling[..], &cur[..]]),
            "right" => hashed(1, &[&cur[..], &sibling[..]]),
            _ => return false,
        };
    }
    hex(&cur) == root
}

fn opt<T: ToString>(v: Option<T>) -> String {
    v.map(|x| x.to_string()).unwrap_or_else(|| "-".into())
}

/// 叶子原文。字段顺序/分隔符/空值写法一经上线不得改动——
/// 改了会让所有历史证明同时失效。前端只对服务端下发的原文做哈希，
/// 不重新拼串。
pub fn ledger_preimage(l: &LedgerLeaf<'_>) -> String {
    format!(
        "flux-ledger-v1|{}|{}|{}|{}|{}:{}|{}|{}|{}|{}",
        l.day,
        l.id,
        l.user_id,
        l.amount,
        l.kind.len(),
        l.kind,
        l.ref_type.unwrap_or("-"),
        opt(l.ref_id),
        opt(l.balance_after),
        l.created_at,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaves(n: usize) -> (Vec<String>, Vec<String>) {
        let pre: Vec<String> = (0..n).map(|i| format!("p{i}")).collect();
        let hs = pre.iter().map(|p| leaf_hash(p)).collect();
        (pre, hs)
    }

    #[test]
    fn every_leaf_proves_against_root_for_odd_and_even() {
        for n in 1..=9 {
            let (pre, hs) = leaves(n);
            let r = root(&hs).unwrap();
            for (i, p) in pre.iter().enumerate() {
                let pf = proof(&hs, i).unwrap();
                assert!(verify(p, &hs[i], &pf, &r), "n={n} i={i}");
            }
        }
    }

    #[test]
    fn tampered_record_or_proof_fails() {
        let (pre, hs) = leaves(3);
        let r = root(&hs).unwrap();
        let pf = proof(&hs, 1).unwrap();
        assert!(!verify("tampered", &hs[1], &pf, &r));
        let mut bad = pf.clone();
        bad[0].hash = leaf_hash("x");
        assert!(!verify(&pre[1], &hs[1], &bad, &r));
        assert!(!verify(&pre[1], &hs[1], &pf, &leaf_hash("y")));
    }

    #[test]
    fn single_leaf_root_is_the_leaf() {
        let (pre, hs) = leaves(1);
        assert_eq!(root(&hs).unwrap(), hs[0]);
        assert!(proof(&hs, 0).unwrap().is_empty());
        assert!(verify(&pre[0], &hs[0], &[], &hs[0]));
    }

    #[test]
    fn empty_day_has_stable_root() {
        assert_eq!(root(&[]).unwrap(), empty_root());
        assert_ne!(empty_root(), leaf_hash(""));
        assert!(proof(&[], 0).is_none());
    }

    #[test]
    fn preimage_is_stable() {
        let l = LedgerLeaf {
            day: "2026-10-20",
            id: 7,
            user_id: 3,
            amount: -2,
            kind: "shop",
            ref_type: None,
            ref_id: Some(9),
            balance_after: None,
            created_at: "2026-10-20T01:00:00.000000Z",
        };
        assert_eq!(
            ledger_preimage(&l),
            "flux-ledger-v1|2026-10-20|7|3|-2|4:shop|-|9|-|\
             2026-10-20T01:00:00.000000Z"
        );
    }

    /// 跨语言锁定：前端 `lib/merkle.ts` 的用例用同一组向量。
    /// 任何一侧改了前缀/拼接顺序都会在这里或 TS 用例里红。
    #[test]
    fn reference_vector_matches_frontend() {
        let pre = [
            "flux-ledger-v1|2026-10-20|1|7|5|3:foo|-|-|100|\
             2026-10-20T01:00:00.000000Z",
            "flux-ledger-v1|2026-10-20|2|7|-2|3:bar|task|9|98|\
             2026-10-20T02:00:00.000000Z",
            "flux-ledger-v1|2026-10-20|3|8|1|4:seed|-|-|50|\
             2026-10-20T03:00:00.000000Z",
        ];
        let leaf = [
            "0bb45b817cc68f86ea9bb5c95ff7c2bcd1b005b589b4d8df4b97bdff6fd25d23",
            "8bac5a46c36a43e168b46f60e9281cd287124dd4d144ae3eeabb3ca5d93dc874",
            "10e8c966acb183c33ee8f5c22213b1ab05646573dfa750597fc8ff94e2c7fe9a",
        ];
        for (p, h) in pre.iter().zip(leaf.iter()) {
            assert_eq!(leaf_hash(p), *h);
        }
        let hs: Vec<String> = leaf.iter().map(|s| s.to_string()).collect();
        let r = root(&hs).unwrap();
        assert_eq!(
            r,
            "1fa855f2ba392f4d5a4cea81cb83b963777ffe23d2277ff60a00287b259d0b48"
        );
        let pf = proof(&hs, 2).unwrap();
        assert_eq!(pf[0].position, "right");
        assert_eq!(pf[1].position, "left");
        assert!(verify(pre[2], &hs[2], &pf, &r));
    }
}
