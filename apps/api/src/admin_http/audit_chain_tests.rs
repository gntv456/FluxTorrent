//! 0357 单元测试：chain-verify 首行对照物选择（有锚/无锚两分支）。
//! 端点 SQL 本身要活库才能跑，这里钉住「对照物选择」这一最小决策面的
//! Rust 镜像；SQL 与本函数的一致性由 `first_row_expected_prev` 的两个
//! 调用点（CASE WHEN uses_anchor THEN anchor_hash ELSE genesis END）共用
//! 同一语义保证。

/// 最小存活 id 行的 prev_hash 应对照什么：
/// - 有锚（且锚 cutoff_id < min_alive_id，即锚确实切在存活首行之前）：
///   对照锚的 cutoff_self_hash（归档断点衔接）；
/// - 无锚：对照全零创世（0266 原口径，从未归档过的库维持现状）。
/// 锚 cutoff_id ≥ min_alive_id（误配/回拨）视为无效锚，退回创世——
/// 拿一个切在链中间的 hash 当首行前驱必然假报断链。
pub(crate) fn first_row_expected_prev(
    has_alive_rows: bool,
    anchor: Option<(i64, Vec<u8>)>, // (cutoff_id, cutoff_self_hash)
    min_alive_id: i64,
) -> PrevRef {
    if !has_alive_rows {
        return PrevRef::EmptyTable;
    }
    match anchor {
        Some((cutoff_id, hash)) if cutoff_id < min_alive_id => {
            PrevRef::Anchor(hash)
        }
        _ => PrevRef::Genesis,
    }
}

/// 首行 prev_hash 的对照物（端点 SQL 里 CASE 表达式的语义枚举）。
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PrevRef {
    /// 全零 32 字节创世（0266）
    Genesis,
    /// 最近锚点的 cutoff_self_hash（0357 归档断点）
    Anchor(Vec<u8>),
    /// 活表为空：无首行，谈不上对照（total=0 → first_broken_id=null）
    EmptyTable,
}

#[test]
fn no_anchor_falls_back_to_genesis() {
    // 从未归档：锚表为空 → 全零创世（0266 口径不变）
    assert_eq!(first_row_expected_prev(true, None, 42), PrevRef::Genesis);
}

#[test]
fn anchor_below_min_alive_uses_anchor_hash() {
    // 归档过：cutoff_id=100 < 存活首行 101 → 对照锚 hash
    let hash = vec![0xAB; 32];
    assert_eq!(
        first_row_expected_prev(true, Some((100, hash.clone())), 101),
        PrevRef::Anchor(hash)
    );
}

#[test]
fn anchor_not_below_min_alive_is_ignored() {
    // 锚没切在存活首行之前（等于/晚于 min id）→ 无效锚，退创世
    assert_eq!(
        first_row_expected_prev(true, Some((101, vec![1; 32])), 101),
        PrevRef::Genesis
    );
    assert_eq!(
        first_row_expected_prev(true, Some((999, vec![1; 32])), 101),
        PrevRef::Genesis
    );
}

#[test]
fn empty_table_has_no_first_row() {
    // 活表空（全部归档完）：无对照物；端点 total=0、first_broken_id=null
    assert_eq!(
        first_row_expected_prev(false, Some((100, vec![1; 32])), 0),
        PrevRef::EmptyTable
    );
}
