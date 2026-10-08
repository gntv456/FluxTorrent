//! 日志字节 → UTF-8 文本（0312）。
//!
//! 为什么不能直接 `String::from_utf8_lossy`：EAC 按**系统区域设置**写日志，
//! 中文圈的抓轨日志大量是 GBK/GB18030，日区是 Shift_JIS，港台是 Big5。
//! lossy 会把轨道名变成一串 U+FFFD —— 分数还能算（判据都是 ASCII），但
//! 「查看日志」就成了乱码砖，审核员无法据此复核。
//!
//! UTF-8 优先严格解码；失败再按候选码页逐个试，取**替换字符最少**的那个。
//! 全都不理想时回落 lossy UTF-8：宁可局部乱码，不可整体丢日志。

/// 非 UTF-8 时的候选码页（按中文无损区的出现频率排序）。
const FALLBACKS: [&str; 4] = ["gb18030", "big5", "shift_jis", "windows-1252"];

/// 解码为 UTF-8。BOM（含 UTF-16）先剥/先判，否则会污染首行引擎识别。
pub fn to_text(bytes: &[u8]) -> String {
    if let Some(s) = decode_utf16(bytes) {
        return s;
    }
    let body = strip_bom(bytes);
    if let Ok(s) = std::str::from_utf8(body) {
        return s.to_string();
    }
    let mut best: Option<(usize, String)> = None;
    for label in FALLBACKS {
        let Some(enc) = encoding_rs::Encoding::for_label(label.as_bytes())
        else {
            continue;
        };
        let (cow, _, _) = enc.decode(body);
        let bad = cow.matches('\u{FFFD}').count();
        if best.as_ref().is_none_or(|(b, _)| bad < *b) {
            best = Some((bad, cow.into_owned()));
        }
        if bad == 0 {
            break;
        }
    }
    best.map(|(_, s)| s)
        .unwrap_or_else(|| String::from_utf8_lossy(body).into_owned())
}

fn decode_utf16(bytes: &[u8]) -> Option<String> {
    let enc = if bytes.starts_with(&[0xFF, 0xFE]) {
        encoding_rs::UTF_16LE
    } else if bytes.starts_with(&[0xFE, 0xFF]) {
        encoding_rs::UTF_16BE
    } else {
        return None;
    };
    let (cow, _, _) = enc.decode(bytes);
    // 解码结果自带 U+FEFF：留着它首行匹配会差一个字符
    Some(cow.strip_prefix('\u{FEFF}').unwrap_or(&cow).to_string())
}

fn strip_bom(bytes: &[u8]) -> &[u8] {
    bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_passes_through() {
        assert_eq!(to_text("Peak level 99.4%".as_bytes()), "Peak level 99.4%");
    }

    #[test]
    fn utf8_bom_stripped() {
        let raw = "\u{FEFF}EAC extraction logfile".as_bytes();
        assert_eq!(to_text(raw), "EAC extraction logfile");
    }

    #[test]
    fn gbk_chinese_track_names_survive() {
        // 「01 - 周杰伦 稻香.flac」的 GB18030 字节
        let gbk = encoding_rs::GB18030
            .encode("Track  1\r\n  Filename C:\\Rip\\01 - 周杰伦 稻香.flac")
            .0;
        let text = to_text(&gbk);
        assert!(text.contains("周杰伦"), "GBK 未解出中文：{text}");
        assert!(!text.contains('\u{FFFD}'));
    }

    #[test]
    fn utf16le_with_bom_decodes() {
        let mut raw = vec![0xFF, 0xFE];
        for u in "EAC extraction logfile".encode_utf16() {
            raw.extend_from_slice(&u.to_le_bytes());
        }
        assert_eq!(to_text(&raw), "EAC extraction logfile");
    }

    #[test]
    fn garbage_does_not_panic() {
        let weird = [0xC3u8, 0x28, 0xA0, 0x00, 0xFF, 0xFE, 0xFD];
        let s = to_text(&weird);
        assert!(!s.is_empty());
    }
}
