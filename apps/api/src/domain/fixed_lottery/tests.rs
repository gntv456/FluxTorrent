use super::{assign_slots, candidate_key, selection_of, snapshot_hash};

#[test]
fn key_is_deterministic_and_per_uid() {
    let a = candidate_key("seed1", 7, 42);
    let b = candidate_key("seed1", 7, 42);
    assert_eq!(a, b, "同 seed 同人键必须确定");
    assert_ne!(a, candidate_key("seed1", 7, 43), "不同人键不同");
    assert_ne!(a, candidate_key("seed2", 7, 42), "换 seed 键全变");
    assert_ne!(a, candidate_key("seed1", 8, 42), "换活动键全变");
}

#[test]
fn selection_orders_by_key_ascending() {
    let uids: Vec<i64> = (1..=64).collect();
    let sel = selection_of("abc", 1, &uids);
    assert_eq!(sel.ordered.len(), 64);
    for w in sel.ordered.windows(2) {
        assert!(w[0].key <= w[1].key, "键必须升序");
    }
    // 全序：键撞时 uid 兜底，两次计算结果逐位一致（复算前提）
    let again = selection_of("abc", 1, &uids);
    assert_eq!(sel.ordered, again.ordered);
}

#[test]
fn selection_is_permutation_of_candidates() {
    let uids: Vec<i64> = (100..=200).collect();
    let sel = selection_of("xyz", 3, &uids);
    let mut got: Vec<i64> = sel.ordered.iter().map(|c| c.uid).collect();
    got.sort_unstable();
    assert_eq!(got, uids, "次序是候选全集的重排，不丢人不多人");
}

#[test]
fn bottom_k_matches_bruteforce_on_tiny_input() {
    // 2 人取 1 名：排序键小的必中——与暴力重算一致
    let uids = vec![1i64, 2];
    let sel = selection_of("k", 1, &uids);
    let winner = sel.ordered[0].uid;
    let brute_k1 = candidate_key("k", 1, 1);
    let brute_k2 = candidate_key("k", 1, 2);
    let expect = if brute_k1 <= brute_k2 { 1 } else { 2 };
    assert_eq!(winner, expect);
}

#[test]
fn snapshot_hash_sorted_and_stable() {
    let h1 = snapshot_hash(&[3, 1, 2]);
    let h2 = snapshot_hash(&[1, 2, 3]);
    assert_eq!(h1, h2, "快照哈希与入参顺序无关（升序归一）");
    assert_ne!(h1, snapshot_hash(&[1, 2, 4]), "换人哈希即变");
}

#[test]
fn empty_snapshot_is_sha256_of_nothing() {
    assert_eq!(
        snapshot_hash(&[]),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn slots_walk_prizes_in_order() {
    let uids: Vec<i64> = (1..=10).map(|i| i * 7).collect();
    let sel = selection_of("s", 9, &uids);
    let prizes = vec![(11i64, 2i32), (12, 1), (13, 3)];
    let slots = assign_slots(&sel, &prizes);
    assert_eq!(slots.len(), 6, "2+1+3 共 6 个名额");
    // 位次全局递增；槽号在奖项内从 1 起
    assert_eq!(slots[0].2, 1);
    assert_eq!(slots[5].2, 6);
    assert_eq!(slots[2].0, 12);
    assert_eq!(slots[2].1, 1);
    assert_eq!(slots[3].0, 13);
    assert_eq!(slots[5].1, 3);
    // 中奖人两两不同（bottom-k 不放回）
    let mut us: Vec<i64> = slots.iter().map(|s| s.3).collect();
    us.sort_unstable();
    us.dedup();
    assert_eq!(us.len(), 6);
}

#[test]
fn slots_vacate_when_candidates_fewer() {
    let sel = selection_of("s", 1, &[5]);
    let prizes = vec![(1i64, 3i32)];
    let slots = assign_slots(&sel, &prizes);
    assert_eq!(slots.len(), 1, "1 个候选人只占 1 个名额，余下空缺");
}

#[test]
fn slots_empty_when_no_candidates() {
    let sel = selection_of("s", 1, &[]);
    let slots = assign_slots(&sel, &[(1, 5)]);
    assert!(slots.is_empty(), "0 人报名 = 流拍，无中奖行");
}

#[test]
fn uniformity_smoke_no_dominant_position() {
    // 4 人取 1，多 seed 冒烟：无单人垄断首位（严格均匀分布判定交给
    // 外部统计工具；这里只防实现性偏置，如把 uids 顺序直接当次序）
    let uids = vec![1i64, 2, 3, 4];
    let mut first_counts = [0usize; 4];
    for r in 0..200u64 {
        let seed = format!("seed{r}");
        let sel = selection_of(&seed, 1, &uids);
        let idx = sel.ordered[0].uid as usize - 1;
        first_counts[idx] += 1;
    }
    assert!(
        first_counts.iter().all(|&c| c > 0),
        "首位出现次数 {first_counts:?} 偏置——实现性偏置"
    );
}
