//! EAC / CUETools 日志解析与定分（0312；判分表 2026-10-09 按真实日志
//! 字面量重写）。
//!
//! EAC 日志是**逐轨证据链**：每轨一段，段内有 Test CRC / Copy CRC /
//! Peak level / Track quality / Accurately ripped / Copy OK。所以解析按
//! 「Track 行开段、段内置旗标」走一遍即可，不需要完整语法。
//!
//! 字面量口径（每条判据都对应真实 EAC 日志里的一行，虚构标记已清除）：
//! - 逐轨音质行是 `Track quality 100.0 %`——旧的裸 `Quality` 匹配不到
//!   任何真实日志行；
//! - 抓轨中断的致命标志是 `Copy aborted`（旧的「含 cancel 即命中」会
//!   误伤 `Cancellation` 之类无关词）；
//! - AR 三态：`Accurately ripped (confidence N)`（过）/
//!   `Cannot be verified as accurate`（比对失败，−30 档）/
//!   整份无任何 AR/CTDB 记录（未验证，−20 档）；
//! - `AR v3` / `RT:` / `CRT` / clipping / TRIM 等标记在真实日志里不存在，
//!   一度写进识别表的这些串已删。
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
    ar_failed: bool,
    copy_ok: bool,
    copy_nok: bool,
}

#[derive(Default)]
struct Ledger {
    tracks: Vec<Track>,
    ctdb: bool,
    ar_verified: bool,
    /// 段外出现的 AR 失败行（老版 EAC 把 AR 汇总写在文件尾部独立区）
    orphan_ar_fails: u32,
    no_errors: bool,
    footer: bool,
    aborted: bool,
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
            } else if low.starts_with("track quality") {
                t.quality = true;
            } else if low.contains("accurately ripped") {
                t.accurate_rip = true;
            } else if low.contains("cannot be verified as accurate") {
                // AR 比对失败：库里同轨的校验值与本轨对不上——比「没有
                // 比对记录」重一档的证据，不能混进 unverified
                t.ar_failed = true;
            } else if low.starts_with("copy nok")
                || low.contains("does not match")
            {
                t.copy_nok = true;
            } else if low.starts_with("copy ok") {
                t.copy_ok = true;
            }
        } else if low.contains("cannot be verified as accurate") {
            l.orphan_ar_fails += 1;
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
        if low.contains("copy aborted") {
            l.aborted = true;
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

/// 扣分档（2026-10-09 起与业界口径同量级；此前一律 −1 不可比）。
/// 每档对应的日志行见各判据注释；多项命中累计，下限 0。
const DED_AR_FAILED: i16 = 30;
const DED_UNVERIFIED: i16 = 20;
const DED_SINGLE_PASS: i16 = 20;
const DED_THIN: i16 = 10;
const DED_TRUNCATED: i16 = 10;

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
    if !mismatched.is_empty() || l.aborted {
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
        if l.aborted {
            hard.issues.push(Issue::new(
                "aborted",
                "日志中出现 Copy aborted——读取被中断，本次抓轨不成立",
            ));
        }
        return hard;
    }

    // AR 比对失败（Cannot be verified as accurate）：−30 档
    let mut ded = 0i16;
    let ar_failed: Vec<u32> = l
        .tracks
        .iter()
        .filter(|t| t.ar_failed)
        .map(|t| t.number)
        .collect();
    if !ar_failed.is_empty() || l.orphan_ar_fails > 0 {
        ded += DED_AR_FAILED;
        if !ar_failed.is_empty() {
            issues.push(Issue::new(
                "ar_failed",
                format!(
                    "第 {} 轨 AccurateRip 比对失败（Cannot be verified as \
                     accurate），校验值与数据库不符",
                    list_nums(&ar_failed)
                ),
            ));
        } else {
            issues.push(Issue::new(
                "ar_failed",
                format!(
                    "AR 汇总区有 {} 行比对失败（Cannot be verified as \
                     accurate）",
                    l.orphan_ar_fails
                ),
            ));
        }
    }

    // 未验证（整份无 AR/CTDB 过线记录）：−20 档
    let verified =
        l.ctdb || l.ar_verified || l.tracks.iter().any(|t| t.accurate_rip);
    if !verified {
        ded += DED_UNVERIFIED;
        issues.push(Issue::new(
            "unverified",
            "没有 AccurateRip / CUETools 比对记录，抓轨结果未经第二方核对",
        ));
    }

    // 非 Test & Copy（无 Test CRC）：−20 档
    let untested: Vec<u32> = l
        .tracks
        .iter()
        .filter(|t| t.test_crc.is_none())
        .map(|t| t.number)
        .collect();
    if !untested.is_empty() {
        ded += DED_SINGLE_PASS;
        issues.push(Issue::new(
            "single_pass",
            format!(
                "第 {} 轨没有 Test CRC，即未做「比对二次读取」（非安全模式\
                 两次校验）",
                list_nums(&untested)
            ),
        ));
    }

    // 逐轨证据链缺环（Peak level / Track quality）：−10 档
    let thin: Vec<u32> = l
        .tracks
        .iter()
        .filter(|t| !t.peak || !t.quality)
        .map(|t| t.number)
        .collect();
    if !thin.is_empty() {
        ded += DED_THIN;
        issues.push(Issue::new(
            "missing_evidence",
            format!(
                "第 {} 轨缺少 Peak level 或 Track quality 记录，逐轨证据链\
                 不完整",
                list_nums(&thin)
            ),
        ));
    }

    // 无收尾行（可能截断）：−10 档
    if !l.footer {
        ded += DED_TRUNCATED;
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

#[cfg(test)]
mod tests {
    use super::*;

    /// EAC 1.x 真实形态的最小完整日志（所有证据齐 + AR 全过）
    fn perfect_log() -> String {
        let mut s = String::from(
            "Exact Audio Copy V1.3 from 23. August 2016\n\
             \n\
             EAC extraction logfile from 5. February 2023, 14:30\n\
             \n\
             Used drive  : HL-DT-ST BD-RE WH14NS40\n\
             Read mode               : Secure\n\
             \n",
        );
        for n in 1..=2 {
            s.push_str(&format!(
                "Track  {n}\n\
                 Filename E:\\rip\\0{n}.flac\n\
                 Peak level 99.9 %\n\
                 Track quality 100.0 %\n\
                 Test CRC 2AFB9E3F\n\
                 Copy CRC 2AFB9E3F\n\
                 Accurately ripped (confidence 12) [2AFB9E3F]\n\
                 Copy OK\n\
                 \n"
            ));
        }
        s.push_str("No errors occurred\n\nEnd of status report\n");
        s
    }

    /// 换掉一行的工具：判据定位用行前缀
    fn replace_line(log: &str, prefix: &str, with: &str) -> String {
        log.lines()
            .map(|l| {
                if l.trim().starts_with(prefix) {
                    with
                } else {
                    l
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn perfect_log_scores_100() {
        let c = check(&perfect_log(), Engine::Eac);
        assert_eq!(c.score, Some(100), "issues: {:?}", c.issues);
        assert!(c.issues.is_empty());
        assert_eq!(c.tracks, 2);
    }

    #[test]
    fn ar_fail_line_deducts_30() {
        let log = replace_line(
            &perfect_log(),
            "Accurately ripped",
            "Cannot be verified as accurate (confidence 1) [00000000]",
        );
        let c = check(&log, Engine::Eac);
        // AR 失败的轨不再计 verified：−30（比对失败）−20（无过线记录）
        assert_eq!(c.score, Some(50), "issues: {:?}", c.issues);
        assert!(c.issues.iter().any(|i| i.code == "ar_failed"));
    }

    #[test]
    fn unverified_disc_deducts_20() {
        // AR 区段被裁（既无逐轨 AR 也无 CTDB）→ 100−20
        let log: String = perfect_log()
            .lines()
            .filter(|l| !l.trim().starts_with("Accurately ripped"))
            .collect::<Vec<_>>()
            .join("\n");
        let c = check(&log, Engine::Eac);
        assert_eq!(c.score, Some(80), "issues: {:?}", c.issues);
        assert!(c.issues.iter().any(|i| i.code == "unverified"));
    }

    #[test]
    fn single_pass_deducts_20() {
        let log = replace_line(
            &perfect_log(),
            "Test CRC",
            "  (无 Test CRC 行——单次 Copy)",
        );
        let log = log.replace("  (无 Test CRC 行——单次 Copy)\n", "");
        let c = check(&log, Engine::Eac);
        assert_eq!(c.score, Some(80), "issues: {:?}", c.issues);
        assert!(c.issues.iter().any(|i| i.code == "single_pass"));
    }

    #[test]
    fn bare_quality_marker_is_not_evidence() {
        // 旧判分表把裸 `Quality` 当证据；真实 EAC 写的是 `Track quality`
        let log =
            replace_line(&perfect_log(), "Track quality", "Quality 100 %");
        let c = check(&log, Engine::Eac);
        assert_eq!(c.score, Some(90), "issues: {:?}", c.issues);
        assert!(c.issues.iter().any(|i| i.code == "missing_evidence"));
    }

    #[test]
    fn copy_aborted_is_zero() {
        let log = replace_line(&perfect_log(), "Copy OK", "Copy aborted");
        let c = check(&log, Engine::Eac);
        assert_eq!(c.score, Some(0));
        assert!(c.issues.iter().any(|i| i.code == "aborted"));
    }

    #[test]
    fn crc_mismatch_is_zero() {
        let log = replace_line(&perfect_log(), "Copy CRC", "Copy CRC 00000001");
        let c = check(&log, Engine::Eac);
        assert_eq!(c.score, Some(0));
        assert!(c.issues.iter().any(|i| i.code == "crc_mismatch"));
    }

    #[test]
    fn truncated_footer_deducts_10() {
        let log: String = perfect_log()
            .lines()
            .filter(|l| !l.contains("End of status report"))
            .collect::<Vec<_>>()
            .join("\n");
        let c = check(&log, Engine::Eac);
        assert_eq!(c.score, Some(90), "issues: {:?}", c.issues);
        assert!(c.issues.iter().any(|i| i.code == "truncated"));
    }

    #[test]
    fn ctdb_counts_as_verified() {
        // 无逐轨 AR，但带 CUETools DB 校验段 → 已验证，不扣 unverified
        let mut log: String = perfect_log()
            .lines()
            .filter(|l| !l.trim().starts_with("Accurately ripped"))
            .collect::<Vec<_>>()
            .join("\n");
        log.push_str("\nCUETools DB Plugin: V2.1.6\nCTDB TOCID: xyz\n");
        let c = check(&log, Engine::Eac);
        assert_eq!(c.score, Some(100), "issues: {:?}", c.issues);
    }

    #[test]
    fn no_tracks_yields_null_score() {
        let c = check("这不是一份日志\n只是几行中文", Engine::Eac);
        assert_eq!(c.score, None);
        assert!(c.issues.iter().any(|i| i.code == "no_tracks"));
    }

    #[test]
    fn orphan_ar_fail_counted() {
        // 老版 EAC：AR 汇总区在文件尾（不在轨段内）
        let mut log: String = perfect_log()
            .lines()
            .filter(|l| !l.trim().starts_with("Accurately ripped"))
            .collect::<Vec<_>>()
            .join("\n");
        log.push_str("\nCannot be verified as accurate (confidence 0)\n");
        let c = check(&log, Engine::Eac);
        assert_eq!(c.score, Some(50), "issues: {:?}", c.issues);
        assert!(c.issues.iter().any(|i| i.code == "ar_failed"));
        assert!(c.issues.iter().any(|i| i.code == "unverified"));
    }
}
