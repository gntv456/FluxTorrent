use super::*;

/// 端到端：开售 → 售票 → 开奖 → 验算通过。
#[test]
fn roundtrip_verifies() {
    let (seed, c) = commit_new_round();
    assert_eq!(c.commit, commit_of(&seed));

    // 逐张链式建票（不能用迭代器：第 i 张要读第 i-1 张的结果）
    let mut ts: Vec<String> = Vec::new();
    for i in 0..10u32 {
        let prev = ts.last().cloned().unwrap_or_else(genesis_root);
        ts.push(ticket_hash(&seed, &prev, i));
    }
    let root = ts.last().unwrap().clone();

    let proof = build_proof("42", &seed, &root, &ts, 3);
    let v = verify(&c.commit, &proof, &ts, 3).expect("应验算通过");
    assert_eq!(v.participants, 10);
    assert_eq!(v.winners, 3);
    assert_eq!(proof.winner_ticket_hashes.len(), 3);
}

/// 伪造 seed（改一个字节）→ commit 对不上 → 拒。
#[test]
fn forged_seed_rejected() {
    let (seed, c) = commit_new_round();
    let mut bad = seed.clone();
    bad.push('0');
    let ts = vec![ticket_hash(&seed, &genesis_root(), 0)];
    let root = ts[0].clone();
    let mut proof = build_proof("1", &seed, &root, &ts, 1);
    proof.seed = bad;
    assert!(verify(&c.commit, &proof, &ts, 1).is_err());
}

/// 中途加一张票（票据集变了）→ tickets_root 对不上 → 拒。
#[test]
fn tampered_ticket_rejected() {
    let (seed, c) = commit_new_round();
    let mut ts = vec![ticket_hash(&seed, &genesis_root(), 0)];
    ts.push(ticket_hash(&seed, &ts[0].clone(), 1));
    let root = ts[1].clone();
    let proof = build_proof("1", &seed, &root, &ts, 1);
    // 验证时少给一张（模拟悄悄塞一个人）
    assert!(verify(&c.commit, &proof, &ts[..1], 1).is_err());
}

/// 排序键必须与入参顺序无关（洗牌是真的）。
#[test]
fn order_is_permuted_not_input_order() {
    let seed = new_seed();
    let ts: Vec<String> = (0..20)
        .map(|i| ticket_hash(&seed, &genesis_root(), i))
        .collect();
    let order = draw_order(&seed, &ts, 20);
    let seq: Vec<String> = order.iter().map(|(h, _)| (*h).clone()).collect();
    assert_ne!(seq, ts, "排序不应等于入参顺序");
    // 但必须是一个排列（不丢不重）
    let mut a = seq.clone();
    let mut b = ts.clone();
    a.sort();
    b.sort();
    assert_eq!(a, b);
}
