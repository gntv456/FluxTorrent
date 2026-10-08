//! EAC / CUETools 日志解析与定分（0312）。
//!
//! EAC 日志是**逐轨证据链**：每轨一段，段内有 Test CRC / Copy CRC /
//! Peak level / Quality / Accurately ripped / Copy OK。所以解析按
//! 「Track 行开段、段内置旗标」走一遍即可，不需要完整语法。
//!
//! 定分口径见 `mod.rs` 顶部表格；这里只保证**每条扣分都能指回日志里的一行**。

use serde_json::json;

use super::{Engine, Issue, LogCheck};

/// 一轨的证据旗标。
#[derive(Debug, Default)]
struct Track {
    number: u32,
    test_crc: Option<String>,
    copy_crc: Option<String>,
    peak: bool,
    quality: bool,
    accurate_rip: bool,
    copy_ok: bool,
    copy_nok: bool,
}

#[derive(Default)]
struct Ledger {
    tracks: Vec<Track>,
    ctdb: bool,
    ar_verified: bool,
    no_errors: bool,
    footer: bool,
    cancelled: bool,
    drive: Option<String>,
    read_mode: Option<String>,
    version: Option<String>,
}

pub(super) fn check(text: &str, engine: Engine) -> LogCheck {
    let mut l = Ledger::default();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let low = line.to_ascii_lowercase();
        if let Some(num) = track_number(&low) {
            l.tracks.push(Track {
                number: num,
                ..Default::default()
            });
            continue;
        }
        let cur = l.tracks.last_mut();
        // 段内旗标只在已有轨段时生效：抬头区的全局词另算
        if let Some(t) = cur {
            if low.starts_with("test crc") {
                t.test_crc = hex_after(&low);
            } else if low.starts_with("copy crc") {
                t.copy_crc = hex_after(&low);
            } else if low.starts_with("peak level") {
                t.peak = true;
            } else if low.starts_with("quality") {
                t.quality = true;
            } else if low.contains("accurately ripped")
                || low.contains("ar v")
                || low.contains("ar(v")
            {
                t.accurate_rip = true;
            } else if low.starts_with("copy nok")
                || low.contains("does not match")
            {
                t.copy_nok = true;
            } else if low.starts_with("copy ok") {
                t.copy_ok = true;
            }
        }
        if low.contains("ctdb tocid") || low.contains("cuetools db plugin") {
            l.ctdb = true;
        }
        if low.contains("all tracks accurately ripped") {
            l.ar_verified = true;
        }
        if low.starts_with("no errors occurred") {
            l.no_errors = true;
        }
        if low.contains("end of status report") {
            l.footer = true;
        }
        if low.contains("cancel") {
            l.cancelled = true;
        }
        if low.starts_with("used drive") {
            l.drive = Some(line.to_string());
        }
        if low.starts_with("read mode") {
            l.read_mode = Some(line.to_string());
        }
        if l.version.is_none() && looks_like_version_line(line) {
            l.version = Some(line.to_string());
        }
    }
    score(engine, l)
}

fn score(engine: Engine, l: Ledger) -> LogCheck {
    let tracks = l.tracks.len() as i32;
    let ar_tracks = l.tracks.iter().filter(|t| t.accurate_rip).count();
    let tested = l.tracks.iter().filter(|t| t.test_crc.is_some()).count();
    let facts = json!({
        "version": l.version,
        "drive": l.drive,
        "read_mode": l.read_mode,
        "ctdb": l.ctdb,
        "no_errors": l.no_errors,
        "footer": l.footer,
        "ar_tracks": ar_tracks,
        "tested_tracks": tested,
    });
    let mut issues = Vec::new();

    // 一轨都没解析出来：要么不是日志，要么被截断。不出分（≠ 0 分），
    // 让「看不懂」与「不合格」在审核台上是两个不同的状态。
    if tracks == 0 {
        issues.push(Issue::new(
            "no_tracks",
            "日志里解析不出任何轨道段（可能不是抓轨日志，或只截了半份）",
        ));
        return LogCheck {
            engine,
            score: None,
            tracks: 0,
            issues,
            facts,
        };
    }

    let mismatched: Vec<u32> = l
        .tracks
        .iter()
        .filter(|t| {
            t.copy_nok
                || matches!((&t.test_crc, &t.copy_crc),
                    (Some(a), Some(b)) if a != b)
        })
        .map(|t| t.number)
        .collect();
    if !mismatched.is_empty() || l.cancelled {
        // 日志自证这次抓轨不成立：分数直接归零，不参与「差一档」的扣分
        let mut hard = LogCheck::scored(engine, 100, tracks, vec![], facts);
        if !mismatched.is_empty() {
            hard.issues.push(Issue::new(
                "crc_mismatch",
                format!(
                    "第 {} 轨 Test/Copy 校验不一致或标注 NOK",
                    list_nums(&mismatched)
                ),
            ));
        }
        if l.cancelled {
            hard.issues
                .push(Issue::new("cancelled", "日志中出现被取消的读取记录"));
        }
        return hard;
    }

    let verified =
        l.ctdb || l.ar_verified || l.tracks.iter().any(|t| t.accurate_rip);
    let mut ded = 0i16;
    if !verified {
        ded += 1;
        issues.push(Issue::new(
            "unverified",
            "没有 AccurateRip / CUETools 比对记录，抓轨结果未经第二方核对",
        ));
    }
    let untested: Vec<u32> = l
        .tracks
        .iter()
        .filter(|t| t.test_crc.is_none())
        .map(|t| t.number)
        .collect();
    if !untested.is_empty() {
        ded += 1;
        issues.push(Issue::new(
            "single_pass",
            format!(
                "第 {} 轨没有 Test CRC，即未做「比对二次读取」（非安全模式两次校验）",
                list_nums(&untested)
            ),
        ));
    }
    let thin: Vec<u32> = l
        .tracks
        .iter()
        .filter(|t| !t.peak || !t.quality)
        .map(|t| t.number)
        .collect();
    if !thin.is_empty() {
        ded += 1;
        issues.push(Issue::new(
            "missing_evidence",
            format!(
                "第 {} 轨缺少 Peak level 或 Quality 记录，逐轨证据链不完整",
                list_nums(&thin)
            ),
        ));
    }
    if !l.footer {
        ded += 1;
        issues.push(Issue::new(
            "truncated",
            "日志没有收尾行（End of status report），可能被截断",
        ));
    }
    LogCheck::scored(engine, ded, tracks, issues, facts)
}

/// `track 1` / `track  7` / `track 01:` → 轨道号；其它一律 None。
/// （whipper.rs 也用它数轨，判据只留一份）
pub(super) fn track_number(low: &str) -> Option<u32> {
    let rest = low.strip_prefix("track")?.trim_start();
    // whipper 用 `Track 01:`，EAC 用 `Track  1`；都只吃前置数字
    let digits: String =
        rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    // 数字后面必须到行尾或只有分隔符，否则是 `Track 01 - 歌名.flac` 这类
    // **文件名行**（曲目列表也能匹配上）
    let tail = rest[digits.len()..].trim_start();
    if !(tail.is_empty() || tail.chars().all(|c| ":;-—_".contains(c))) {
        return None;
    }
    digits.parse().ok()
}

/// 取标记词后面的第一个 8 位十六进制串（EAC 的 CRC 是 8 hex）。
fn hex_after(low: &str) -> Option<String> {
    low.split_whitespace()
        .find(|w| w.len() == 8 && w.chars().all(|c| c.is_ascii_hexdigit()))
        .map(str::to_string)
}

fn looks_like_version_line(line: &str) -> bool {
    let low = line.to_ascii_lowercase();
    low.contains("exact audio copy")
        || (low.contains("cuetools") && low.contains("v"))
}

fn list_nums(v: &[u32]) -> String {
    let s: Vec<String> = v.iter().take(6).map(|n| n.to_string()).collect();
    if v.len() > 6 {
        format!("{}…（共 {} 轨）", s.join("、"), v.len())
    } else {
        s.join("、")
    }
}
