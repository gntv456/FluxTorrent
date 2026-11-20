//! 任务商店兑换的纯逻辑单测（0115）：门槛比较、魔力取整、
//! 库存售罄判据——这些是「错一次就白扣钱/超发」的关键口径。

fn price_i64(cost: f64) -> i64 {
    cost.round() as i64
}
fn sold_out(stock: i32, used: i32) -> bool {
    stock > 0 && used >= stock
}
fn gate_ok(done: i64, need: i32) -> bool {
    done >= i64::from(need)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_cost_rounds_to_spark_integer() {
        // 货架 cost 存 NUMERIC(1.0)，魔力是整数——1.0/1.4/1.5 都按四舍五入
        assert_eq!(price_i64(1.0), 1);
        assert_eq!(price_i64(1.4), 1);
        assert_eq!(price_i64(1.5), 2);
        assert_eq!(price_i64(0.0), 0);
    }

    #[test]
    fn stock_zero_means_unlimited() {
        assert!(!sold_out(0, 1_000_000));
        assert!(!sold_out(74, 73));
        assert!(sold_out(74, 74));
        assert!(sold_out(1, 2));
    }

    #[test]
    fn tier_gate_uses_done_count_not_global_stats() {
        assert!(gate_ok(12, 12));
        assert!(gate_ok(15, 12));
        assert!(!gate_ok(11, 12));
    }
}
