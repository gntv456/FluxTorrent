//! CUE 表合规判定（G3 后半，Gazelle「Perfect FLAC」口径的另一半）。
//!
//! RED/What.CD 的完美无损 = `HasLog + LogScore 100 + HasCue`。日志分已由
//! `eac` / `whipper` 判分器覆盖，本模块补 **HasCue 的合规性**：判的不是
//! 「有没有 .cue 文件」，而是「这张 CUE 表能不能被正确还原成音轨布局」——
//! 缺 `FILE`、某轨缺 `INDEX 01`、轨号不连续，都是不可用的表。
//!
//! 判据只认**逐行可复现的结构事实**，不做主观判断；问题清单里的每一句
//! 都能指回 CUE 里的一行。

/// CUE 校验结果。
#[derive(Debug, Clone, PartialEq)]
pub struct CueCheck {
    /// 全部结构判据通过
    pub valid: bool,
    /// TRACK 条数
    pub tracks: usize,
    /// 是否有 FILE 声明
    pub has_file: bool,
    /// 缺 INDEX 01 的轨号（按 CUE 里的编号）
    pub missing_index: Vec<usize>,
    /// 问题清单（给人读的一句话）
    pub issues: Vec<String>,
}

/// 解析 CUE 文本并判定合规性。
pub fn parse(text: &str) -> CueCheck {
    let mut has_file = false;
    let mut tracks: usize = 0;
    let mut numbering_ok = true;
    let mut missing_index: Vec<usize> = Vec::new();
    let mut cur_no: usize = 0;
    let mut cur_has_index = false;

    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let mut it = line.split_whitespace();
        let Some(tok) = it.next() else { continue };
        match tok.to_ascii_uppercase().as_str() {
            "FILE" => has_file = true,
            "TRACK" => {
                // 收尾上一轨
                if cur_no > 0 && !cur_has_index {
                    missing_index.push(cur_no);
                }
                cur_has_index = false;
                let no: usize =
                    it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
                tracks += 1;
                if no != tracks {
                    numbering_ok = false;
                }
                cur_no = if no > 0 { no } else { tracks };
                // 第三个 token 是 TRACK 类型（AUDIO 等），不参与判定
            }
            "INDEX" => {
                if it.next() == Some("01") {
                    cur_has_index = true;
                }
            }
            _ => {}
        }
    }
    if cur_no > 0 && !cur_has_index {
        missing_index.push(cur_no);
    }

    let mut issues: Vec<String> = Vec::new();
    if !has_file {
        issues.push("缺少 FILE 声明".to_string());
    }
    if tracks == 0 {
        issues.push("缺少 TRACK 记录".to_string());
    }
    if !numbering_ok {
        issues.push("TRACK 编号不连续（应从 1 起递增）".to_string());
    }
    for n in &missing_index {
        issues.push(format!("TRACK {n} 缺 INDEX 01"));
    }
    let valid =
        has_file && tracks > 0 && numbering_ok && missing_index.is_empty();
    CueCheck {
        valid,
        tracks,
        has_file,
        missing_index,
        issues,
    }
}

#[cfg(test)]
mod tests {
    use super::parse;

    const GOOD: &str = "PERFORMER \"X\"\nTITLE \"Y\"\n\
FILE \"a.flac\" WAVE\n  TRACK 01 AUDIO\n    INDEX 01 00:00:00\n\
  TRACK 02 AUDIO\n    INDEX 01 03:20:10\n";

    #[test]
    fn good_cue_is_valid() {
        let c = parse(GOOD);
        assert!(c.valid, "{:?}", c.issues);
        assert_eq!(c.tracks, 2);
        assert!(c.has_file);
    }

    #[test]
    fn no_file_declaration_is_invalid() {
        let c = parse("TRACK 01 AUDIO\n  INDEX 01 00:00:00\n");
        assert!(!c.valid);
        assert!(c.issues.iter().any(|i| i.contains("FILE")));
    }

    #[test]
    fn missing_index_is_reported() {
        let c = parse(
            "FILE \"a.flac\" WAVE\nTRACK 01 AUDIO\n  INDEX 01 00:00:00\n\
TRACK 02 AUDIO\n",
        );
        assert!(!c.valid);
        assert_eq!(c.missing_index, vec![2]);
    }

    #[test]
    fn non_sequential_track_is_flagged() {
        let c = parse(
            "FILE \"a.flac\" WAVE\nTRACK 02 AUDIO\n  INDEX 01 00:00:00\n",
        );
        assert!(!c.valid);
        assert!(c.issues.iter().any(|i| i.contains("不连续")));
    }

    #[test]
    fn empty_text_is_invalid() {
        let c = parse("");
        assert!(!c.valid);
        assert_eq!(c.tracks, 0);
    }

    #[test]
    fn lowercase_keywords_are_tolerated() {
        let c = parse(
            "file \"a.flac\" wave\ntrack 01 audio\n  index 01 00:00:00\n",
        );
        assert!(c.valid, "{:?}", c.issues);
    }
}
