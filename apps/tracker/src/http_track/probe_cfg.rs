//! 探测调度配置（0309，开源反作弊威胁模型收口）。
//!
//! 开源 = 探测算法人人可读。对策是「算法公开、密钥私有」：
//! 探测时机抖动（`probe_jitter_secs`）与 piece 抽查比例
//! （`probe_piece_ratio`）都放 site_settings，每站私有——
//! 作弊者读上游源码只能知道「有这两道检测」，无法知道
//! 任何一个站实际何时探、抽多少人。
//!
//! 配置随 guard 刷新（60s 节流 + flux:guard:ver 立即生效），
//! 与 ratio_gate 同一纪律：**查询失败保留旧值**，只有
//! 「行确实不存在」才用出厂缺省。

pub(crate) struct ProbeCfg {
    /// 探测循环随机启动延迟上界（秒），0 = 不抖动
    pub(crate) jitter_secs: u32,
    /// piece 抽查比例（0.0-1.0，对已通过握手+bitfield 的 peer）
    pub(crate) piece_ratio: f64,
    /// TCP/BT 握手探测超时（秒）
    pub(crate) tcp_bt_timeout_secs: u64,
    /// piece 抽查超时（秒，含握手+bitfield+unchoke+传输）
    pub(crate) piece_timeout_secs: u64,
}

pub(crate) fn cfg_statics() -> &'static std::sync::RwLock<ProbeCfg> {
    static V: std::sync::OnceLock<std::sync::RwLock<ProbeCfg>> =
        std::sync::OnceLock::new();
    V.get_or_init(|| {
        std::sync::RwLock::new(ProbeCfg {
            jitter_secs: 90,
            piece_ratio: 0.25,
            tcp_bt_timeout_secs: 3,
            piece_timeout_secs: 8,
        })
    })
}

/// 站点级刷新（guard_refresh 在读到设定后调用）
pub(crate) fn refresh(
    jitter_secs: u32,
    piece_ratio: f64,
    tcp_bt_timeout_secs: u64,
    piece_timeout_secs: u64,
) {
    if let Ok(mut w) = cfg_statics().write() {
        w.jitter_secs = jitter_secs.min(600);
        w.piece_ratio = if piece_ratio.is_finite() {
            piece_ratio.clamp(0.0, 1.0)
        } else {
            0.25
        };
        w.tcp_bt_timeout_secs = tcp_bt_timeout_secs.clamp(1, 30);
        w.piece_timeout_secs = piece_timeout_secs.clamp(2, 60);
    }
}

/// 探测循环现场读当前配置（读锁失败也能拿到底线值）
pub(crate) fn current() -> ProbeCfg {
    match cfg_statics().read() {
        Ok(r) => ProbeCfg {
            jitter_secs: r.jitter_secs,
            piece_ratio: r.piece_ratio,
            tcp_bt_timeout_secs: r.tcp_bt_timeout_secs,
            piece_timeout_secs: r.piece_timeout_secs,
        },
        Err(e) => {
            let g = e.into_inner();
            ProbeCfg {
                jitter_secs: g.jitter_secs,
                piece_ratio: g.piece_ratio,
                tcp_bt_timeout_secs: g.tcp_bt_timeout_secs,
                piece_timeout_secs: g.piece_timeout_secs,
            }
        }
    }
}

/// 简易无依赖随机（xorshift64*）：探测抖动/抽查不需要密码学强度，
/// 只需要「不可从源码预测具体值」——种子来自进程启动时刻，每次重启不同。
/// 不引 rand crate：tracker 的依赖面就是攻击面，能不加就不加。
pub(crate) fn cheap_rand(seed: &mut u64) -> u64 {
    let mut x = *seed | 1; // 0 会卡死
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *seed = x;
    x.wrapping_mul(0x2545F4914F6CDD1D)
}

/// 每轮探测的抖动延迟（0 ~ jitter_secs 秒）
pub(crate) fn jitter_duration(seed: &mut u64) -> std::time::Duration {
    let cfg = current();
    if cfg.jitter_secs == 0 {
        return std::time::Duration::from_secs(0);
    }
    let r = cheap_rand(seed) % cfg.jitter_secs as u64;
    std::time::Duration::from_secs(r)
}

/// 按 `ratio` 从候选索引里随机抽 k 个（k = floor(len*ratio)，至少保 1 个
/// 当 ratio>0）。返回被抽中的索引集合——与位置无关，前段后段等概率。
pub(crate) fn pick_piece_indices(
    total: usize,
    ratio: f64,
    seed: &mut u64,
) -> std::collections::HashSet<usize> {
    use std::collections::HashSet;
    let mut out = HashSet::new();
    if total == 0 || ratio <= 0.0 {
        return out;
    }
    let mut k = (total as f64 * ratio).floor() as usize;
    if k == 0 {
        k = 1;
    }
    if k >= total {
        return (0..total).collect();
    }
    // 部分 Fisher-Yates：前 k 位逐位与未选区间随机交换，O(total)
    let mut idx: Vec<usize> = (0..total).collect();
    for i in 0..k {
        let j = i + (cheap_rand(seed) as usize % (total - i));
        idx.swap(i, j);
        out.insert(idx[i]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jitter_respects_zero_upper_bound() {
        refresh(0, 0.25, 3, 8);
        let mut seed = 42u64;
        assert_eq!(jitter_duration(&mut seed).as_secs(), 0);
    }

    #[test]
    fn jitter_stays_within_bound() {
        refresh(90, 0.25, 3, 8);
        let mut seed = 7u64;
        for _ in 0..64 {
            assert!(jitter_duration(&mut seed).as_secs() <= 90);
        }
        // 恢复出厂，避免影响其它用例（statics 是进程级）
        refresh(90, 0.25, 3, 8);
    }

    #[test]
    fn piece_indices_respect_ratio_bounds() {
        let mut seed = 1u64;
        // ratio=0 → 空
        assert!(pick_piece_indices(10, 0.0, &mut seed).is_empty());
        // ratio=1 → 全选
        assert_eq!(pick_piece_indices(4, 1.0, &mut seed).len(), 4);
        // 0<ratio 且 floor=0 → 至少 1 个
        assert_eq!(pick_piece_indices(3, 0.1, &mut seed).len(), 1);
        // 正常比例
        assert_eq!(pick_piece_indices(100, 0.25, &mut seed).len(), 25);
        // total=0 安全
        assert!(pick_piece_indices(0, 0.5, &mut seed).is_empty());
    }

    #[test]
    fn piece_indices_cover_all_positions_over_rounds() {
        // 随机化不许把人钉死在同一位置：多轮后每个位置都应被抽到过
        let mut seed = 99u64;
        let mut seen = std::collections::HashSet::new();
        for _ in 0..200 {
            for i in pick_piece_indices(20, 0.2, &mut seed) {
                seen.insert(i);
            }
        }
        assert_eq!(seen.len(), 20, "200 轮后 20 个位置应全部被抽到过");
    }

    #[test]
    fn refresh_clamps_out_of_range() {
        refresh(99_999, 5.0, 99, 99);
        let c = current();
        assert_eq!(c.jitter_secs, 600);
        assert_eq!(c.tcp_bt_timeout_secs, 30, "超时应被钳到 30");
        assert_eq!(c.piece_timeout_secs, 60, "piece 超时应被钳到 60");
        assert!((c.piece_ratio - 1.0).abs() < f64::EPSILON);
        // NaN 比例回落 0.25
        refresh(90, f64::NAN, 3, 8);
        assert!((current().piece_ratio - 0.25).abs() < f64::EPSILON);
    }
}
