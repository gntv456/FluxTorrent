//! WHipper（ruby CD ripper）日志（0312）。
//!
//! 识别得出来，但**本版不出分**。原因不是偷懒：WHipper 的日志是它自己的
//! 键名体系（逐轨校验值写作 AU / AR，而不是 EAC 的 Test CRC / Copy CRC），
//! 而 `eac::check` 的扣分表是按 EAC 键名写的。把两套键名混为一谈的后果是
//! **确定性的假红**——合法的 WHipper 抓轨会因为「没有 Test CRC」被扣成
//! 97 分，而 `logcheck_policy=require` 的无损站会因此拒掉好种。
//!
//! 所以这里显式回「无法定分」，与「认不出引擎」区分开：
//! - `unknown` = 上传的不是日志；
//! - `unsupported_engine` = 是日志，本站打分器尚未覆盖，需人工/后续批次处理。
//!
//! 补齐条件（下一批）：拿真实 WHipper 日志固化键名表 + 一份样本进测试，
//! 再把逐轨校验/是否两次读取映射到 `mod.rs` 的扣分表上。

use serde_json::json;

use super::{Engine, Issue, LogCheck};

pub(super) fn check(text: &str) -> LogCheck {
    let tracks = text
        .lines()
        .filter(|l| {
            super::eac::track_number(&l.trim().to_ascii_lowercase()).is_some()
        })
        .count() as i32;
    LogCheck {
        engine: Engine::Whipper,
        score: None,
        tracks,
        issues: vec![Issue::new(
            "unsupported_engine",
            "识别为 WHipper 日志，但本站打分器尚未覆盖其校验键名，暂不出分\
             （不是抓轨不合格）",
        )],
        facts: json!({ "track_lines": tracks }),
    }
}
