//! 引擎识别（0312）。
//!
//! 判据只用**版本/抬头行**，不用「有没有 Track 行」这类通用词——曲目列表、
//! 转换日志、甚至歌词文本里都可能出现 `Track 01`，把它当 EAC 日志打分
//! 就是给非日志内容发徽标。
//!
//! 优先级有讲究：EAC 日志尾部常追加 CUETools DB 区块（插件写回同一份日志），
//! 所以 EAC 先判，否则带 CTDB 校验的**最好**那批日志会被认成 CUETools。

use super::Engine;

/// 识别只看头部（版本行总在前面）。8 KiB 足够覆盖 drive 信息一整段。
const HEAD_BYTES: usize = 8 * 1024;

pub(super) fn detect(text: &str) -> Engine {
    let mut head = String::with_capacity(text.len().min(HEAD_BYTES) + 8);
    for ch in text.chars() {
        if head.len() >= HEAD_BYTES {
            break;
        }
        head.push(ch);
    }
    let head = head.to_ascii_lowercase();
    let whole = text.to_ascii_lowercase();

    if any(&head, EAC_HEAD) || any(&whole, EAC_TAIL) {
        return Engine::Eac;
    }
    if any(&head, WHIPPER_HEAD) {
        return Engine::Whipper;
    }
    // 独立 CUETools 运行日志（没有 EAC 抬头，只有 CUETools 自己的版本行）
    if any(&head, CUETOOLS_HEAD) {
        return Engine::CueTools;
    }
    Engine::Unknown
}

const EAC_HEAD: &[&str] = &[
    "exact audio copy",
    "eac extraction logfile",
    "extraction logfile from",
];
/// EAC 收尾标志：抬头被编辑器截掉/用户只贴了后半段时仍能认出来。
const EAC_TAIL: &[&str] = &["end of status report", "eac extraction log"];
const WHIPPER_HEAD: &[&str] = &["whipper", "ripper version"];
const CUETOOLS_HEAD: &[&str] =
    &["cuetools", "accuraterip", "cuetools db plugin"];

fn any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| haystack.contains(n))
}
