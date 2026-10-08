//! 抓轨日志识别与打分（0312：音乐站 Logchecker）。
//!
//! 输入是发种时随 `.torrent` 一起提交的抓轨日志正文（EAC / CUETools /
//! WHipper / XLD / dbPowerAmp / APE…），输出「引擎 + 日志分 + 问题清单」。
//!
//! 打分口径是**本项目自己公开的规则**，不是任何站点私有 Logchecker 的复刻：
//! 每条扣分都对应日志里一行可复现的证据，上传者能在详情页看到分是怎么掉的。
//! 基线 100 分，按下列上限逐条扣减（多项命中取累计，最低 0 分）。
//! 2026-10-09 起扣分档与业界口径同量级（此前一律 −1 不可比）：
//!
//! | 证据（真实日志行） | 扣分 | 为什么 |
//! |---|---|---|
//! | 认不出引擎 / 日志结构不完整 | 不出分（NULL） | 无法判断 ≠ 判断为差 |
//! | `Cannot be verified as accurate`（AR 比对失败） | −30 | 校验值与数据库不符，比「没比对」重 |
//! | 整份无 AccurateRip / CUETools 比对记录 | −20 | 抓轨结果未经第二方核对 |
//! | 非「Test & Copy」单次拷贝（无 Test CRC） | −20 | 同盘重读才能暴露读错 |
//! | 有轨道缺 Peak level / Track quality 记录 | −10 | 逐轨证据链断了一环 |
//! | 日志无收尾行（End of status report） | −10 | 可能被截断 |
//! | 出现 CRC 不符 / `Copy aborted` | 直接 0 | 日志自证这次抓轨不成立 |
//!
//! 与站点准入的关系在 `publish_http::upload_logcheck`：`logcheck_policy`
//! 决定「不查 / 只标注 / 达线才让发」，多碟取 **MIN** 作为种子日志分
//! （一张碟 99 分，整包就不是 100 分的发行）。

mod decode;
mod detect;
mod eac;
mod whipper;

pub use decode::to_text;

use serde::Serialize;

/// 日志引擎。`Unknown` 也必须落库：「用户上传了个不像日志的东西」与
/// 「用户没上传」是两件事，前者要让审核员看得见。
///
/// 只收录**已用真实日志样本核对过标记串**的引擎。XLD / Max / dbPowerAmp /
/// APE 的头部特征行尚未核对，未核对就写进识别表 = 恒不命中的死代码，
/// 宁可让日志落进 `unknown` 由人工判定，增补时这里是唯一改动的枚举。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Engine {
    Eac,
    CueTools,
    Whipper,
    Unknown,
}

impl Engine {
    pub fn code(self) -> &'static str {
        match self {
            Engine::Eac => "eac",
            Engine::CueTools => "cuetools",
            Engine::Whipper => "whipper",
            Engine::Unknown => "unknown",
        }
    }
}

/// 一条问题：`code` 给机器（筛选/统计），`msg` 给人（详情页直接展示）。
#[derive(Debug, Clone, Serialize)]
pub struct Issue {
    pub code: &'static str,
    pub msg: String,
}

impl Issue {
    pub fn new(code: &'static str, msg: impl Into<String>) -> Self {
        Self {
            code,
            msg: msg.into(),
        }
    }
}

/// 单份日志的检查结果。
#[derive(Debug, Clone, Serialize)]
pub struct LogCheck {
    pub engine: Engine,
    /// 0–100；`None` = 无法定分（认不出引擎或日志残缺）
    pub score: Option<i16>,
    /// 日志里记录的轨道数（0 = 没解析出来）
    pub tracks: i32,
    pub issues: Vec<Issue>,
    /// 解析出的事实（drive / read mode / AR 置信度 / CTDB TOCID…），
    /// 供审核台与后续校验复用；不做展示契约
    pub facts: serde_json::Value,
}

impl LogCheck {
    fn unknown(msg: impl Into<String>) -> Self {
        Self {
            engine: Engine::Unknown,
            score: None,
            tracks: 0,
            issues: vec![Issue::new("unrecognized", msg)],
            facts: serde_json::Value::Null,
        }
    }

    /// 基线扣分：`score` 下限 0，问题列表原样带出。
    fn scored(
        engine: Engine,
        deduction: i16,
        tracks: i32,
        issues: Vec<Issue>,
        facts: serde_json::Value,
    ) -> Self {
        Self {
            engine,
            score: Some((100 - deduction).clamp(0, 100)),
            tracks,
            issues,
            facts,
        }
    }
}

/// 检查一份日志正文（调用方已按码页解码成 UTF-8）。
pub fn check_log(text: &str) -> LogCheck {
    match detect::detect(text) {
        Engine::Unknown => LogCheck::unknown(
            "无法从内容识别出抓轨工具（不是 EAC / CUETools / WHipper 日志）",
        ),
        engine @ (Engine::Eac | Engine::CueTools) => eac::check(text, engine),
        Engine::Whipper => whipper::check(text),
    }
}

/// 多碟合成种子级日志分：取 MIN（任一碟未达线即整包未达线）。
/// 全部无法定分时回 `None`（区别于 0 分）。
pub fn overall(scores: &[Option<i16>]) -> Option<i16> {
    scores.iter().flatten().min().copied()
}
