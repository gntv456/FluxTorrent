//! 即时分享率闸门（2026-10-08 站长拍板；报告 §十-③）。
//!
//! 拦的是**下载**（announce 的 `left > 0`），做种永不拦——拦做种是反效果
//! （把想补比率的人赶出 swarm，比率只会更差）。
//!
//! 与已经活着的 `ratio_watch` 异步链（`worker/jobs/audit.rs:123`：跌破阈值 →
//! 警告 + N 天观察期 → 到期仍跌破才 `download_enabled=FALSE`）是**两层不同的东西**：
//! 那条是延迟处置，本闸门是当场准入。两者不重复罚：观察期内的账号已经
//! 在被处置，闸门放行。
//!
//! 门槛来源 = `max(等级 user_classes.min_ratio, 站点 ratiolimit)`，
//! 再被 `ratio_gate_max` 截断。这两处现网值都不能直接信：本机实测
//! `ratiolimit = 6`、17 个等级的 `min_ratio` 全是 0 —— 照 6 直接开 block
//! 就是把全站下载锁死。所以截断是**默认开的**（1.0），要真用高门槛得
//! 显式上调 `ratio_gate_max`；被截断时打一条 warn 指名道姓，
//! 而不是安静地按一个荒谬值把人挡在门外。

/// passkey 缓存里存的东西：一次 SQL 拿全闸门要用的量
/// （announce 热路径 ⇒ 不允许「再查一次」）。
///
/// 为什么是 struct 而不是十元组：`ledger_guard` 那个 10 元组就是靠注释维系
/// 顺序，本轮的 P0 恰恰出在「读的人与写的人对同一组位置理解不同」。
#[derive(Clone, Copy, Debug)]
pub(crate) struct Pku {
    pub(crate) id: i64,
    pub(crate) download_enabled: bool,
    pub(crate) suspended: bool,
    pub(crate) class_id: i32,
    pub(crate) uploaded: i64,
    pub(crate) downloaded: i64,
    /// 注册至今的天数；-1 = 时间戳缺失
    pub(crate) age_days: i64,
    /// `ratio_watch_until > now()`：已在分享率观察期内
    pub(crate) in_watch: bool,
    /// `user_classes.min_ratio`（0 = 该等级没要求）
    pub(crate) class_min_ratio: f64,
    /// `user_classes.min_age_days`
    pub(crate) class_age_days: i64,
}

impl Pku {
    pub(crate) fn staff(&self) -> bool {
        self.class_id >= 90
    }
    /// 闸门输入（等级两值不进缓存键，随人现取）
    pub(crate) fn who(&self) -> Who {
        Who {
            uploaded: self.uploaded,
            downloaded: self.downloaded,
            age_days: self.age_days,
            in_watch: self.in_watch,
            staff: self.staff(),
        }
    }
}

/// 档位：0=off 1=warn 2=block
pub(crate) const OFF: i8 = 0;
pub(crate) const WARN: i8 = 1;
pub(crate) const BLOCK: i8 = 2;

/// 闸门的输入（全部来自 passkey 缓存那一条 SQL，热路径零额外查询）。
#[derive(Clone, Copy, Debug)]
pub(crate) struct Gate {
    /// 档位（off/warn/block）
    pub(crate) mode: i8,
    /// 等级门槛（`user_classes.min_ratio`，0 = 该等级没要求）
    pub(crate) class_min: f64,
    /// 站点门槛（`site_settings.ratiolimit`，0 = 未设）
    pub(crate) site_min: f64,
    /// 门槛上界（安全轨，`ratio_gate_max`）
    pub(crate) ceiling: f64,
    /// 新人宽限天数（站点档 `ratio_gate_grace_days`）
    pub(crate) grace_days: i64,
    /// 该等级的注册天数要求（`user_classes.min_age_days`，与上面取较大者）
    pub(crate) class_age_days: i64,
}

/// 判定所需的人侧数据（来自 `Pku`）。
#[derive(Clone, Copy, Debug)]
pub(crate) struct Who {
    pub(crate) uploaded: i64,
    pub(crate) downloaded: i64,
    /// 注册至今的天数；负数 = 未知（不据此豁免谁，交给 age 判据自己保守）
    pub(crate) age_days: i64,
    /// `ratio_watch_until > now()`：已在观察期内
    pub(crate) in_watch: bool,
    pub(crate) staff: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Decision {
    /// 放行（含「本来就不该拦」的四种豁免）
    Allow,
    /// 低比率但只留痕（warn 档）
    Warn,
    /// 低比率且拒
    Block,
}

/// 实际执行门槛（含安全轨截断）。返回 `(门槛, 是否被截断)`。
pub(crate) fn threshold(g: &Gate) -> (f64, bool) {
    let raw = if g.class_min > g.site_min {
        g.class_min
    } else {
        g.site_min
    };
    if raw <= 0.0 {
        return (0.0, false);
    }
    let ceil = if g.ceiling > 0.0 { g.ceiling } else { 1.0 };
    (raw.min(ceil), raw > ceil)
}

/// 该不该拦。豁免顺序有讲究：先「档位与身份」，再「无数据可依」，
/// 最后才是比数字——数字比较放最后，前面任一条成立就不该出现
/// 「新注册账号被自己的 0 比率挡住」这种误伤。
pub(crate) fn decide(g: &Gate, w: &Who) -> Decision {
    if g.mode == OFF || w.staff {
        return Decision::Allow;
    }
    let (th, clamped) = threshold(g);
    if clamped {
        // 只在真的有人会被这个荒谬值挡住时说话，且随刷新最多 60s 一条
        tracing::warn!(
            class_min = g.class_min,
            site_min = g.site_min,
            ceiling = g.ceiling,
            "分享率门槛超过 ratio_gate_max，已按上界执行（要真用高门槛请显式上调上界）"
        );
    }
    if th <= 0.0 {
        return Decision::Allow;
    }
    // 没下过东西 ⇒ 比率无从计算，不是「比率为 0」
    if w.downloaded <= 0 {
        return Decision::Allow;
    }
    let grace = g.grace_days.max(g.class_age_days);
    if grace > 0 && w.age_days >= 0 && w.age_days < grace {
        return Decision::Allow;
    }
    // 观察期已经给了处置与期限，闸门不再重复出手
    if w.in_watch {
        return Decision::Allow;
    }
    let ratio = w.uploaded as f64 / w.downloaded as f64;
    // 整数无关的浮点比较：等于门槛算达标（>=），与「正好踩线必须被拒」
    // 的闸门口径相反 —— 这里是准入门槛，踩线放行才是本意
    if ratio < th {
        if g.mode == BLOCK {
            return Decision::Block;
        }
        return Decision::Warn;
    }
    Decision::Allow
}

/// 供 refresh_guard 用：把 site_settings 的三个值折成档位常量。
pub(crate) fn mode_of(text: &str) -> i8 {
    match text.trim() {
        "off" => OFF,
        "block" => BLOCK,
        _ => WARN,
    }
}

/// 读站点三档（缺行/读失败 ⇒ 保留旧值，由调用方决定）
pub(crate) fn cfg_statics() -> &'static std::sync::RwLock<Gate> {
    static V: std::sync::OnceLock<std::sync::RwLock<Gate>> =
        std::sync::OnceLock::new();
    V.get_or_init(|| {
        std::sync::RwLock::new(Gate {
            mode: WARN,
            class_min: 0.0,
            site_min: 0.0,
            ceiling: 1.0,
            grace_days: 7,
            class_age_days: 0,
        })
    })
}

/// 站点级两值刷新（class_min/class_age_days 随每个人来，不放这里）
pub(crate) fn refresh_site(
    site_min: f64,
    ceiling: f64,
    grace_days: i64,
    mode: i8,
) {
    if let Ok(mut w) = cfg_statics().write() {
        w.site_min = site_min.max(0.0);
        w.ceiling = if ceiling > 0.0 { ceiling } else { 1.0 };
        w.grace_days = grace_days.max(0);
        w.mode = mode;
    }
}

/// announce 现场用：等级两值随人，其余取站点档
pub(crate) fn gate_for(class_min: f64, class_age_days: i64) -> Gate {
    let base = match cfg_statics().read() {
        Ok(r) => *r,
        Err(e) => *e.into_inner(),
    };
    Gate {
        class_min,
        class_age_days,
        ..base
    }
}

#[cfg(test)]
mod tests;
