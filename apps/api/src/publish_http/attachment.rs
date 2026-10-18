//! 下载口的文件名头（P-299）。
//!
//! 此前 `.torrent` 响应只带 `Content-Type`，浏览器按 URL 末段存盘，
//! 文件名一律是 `download` —— 而批量收种口（`torrent_http/batch.rs`）
//! 早就是 `{种名}-{id}.torrent` 的口径，单击与批量两条路存下来的
//! 文件不同名，用户只能靠内容辨认。这里补齐单击口。
//!
//! 种名大量是中文，`filename="..."` 只容 ASCII，故按 RFC 6266 同时
//! 下发 `filename*`（RFC 5987，UTF-8 百分号编码）与 ASCII 回退。

/// RFC 5987 的 `value-chars`（其余字节一律百分号编码）。
fn rfc5987(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.as_bytes() {
        let keep = matches!(
            b,
            b'!' | b'#' | b'$' | b'&' | b'\'' | b'*' | b'+' | b'-'
                | b'.' | b'0'..=b'9' | b'A'..=b'Z' | b'^' | b'_' | b'`'
                | b'a'..=b'z' | b'|' | b'~'
        );
        if keep {
            out.push(*b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// ASCII 回退名：引号、分号、控制字符一律换成下划线——种名由发帖人
/// 提供，含 `"` 时能改写这一行的结构（参数被截断或伪造）。
fn ascii_fallback(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.as_bytes() {
        let keep = b.is_ascii_alphanumeric()
            || matches!(
                b,
                b'.' | b'_' | b'-' | b' ' | b'(' | b')' | b'[' | b']'
            );
        out.push(if keep { *b as char } else { '_' });
    }
    if out.chars().all(|c| c == '_' || c == ' ') {
        return "torrent.torrent".to_string();
    }
    out
}

/// `Content-Disposition` 的值（附件下载 + 双口径文件名）。
pub(crate) fn content_disposition(name: &str) -> String {
    format!(
        "attachment; filename=\"{}\"; filename*=UTF-8''{}",
        ascii_fallback(name),
        rfc5987(name)
    )
}

#[cfg(test)]
mod tests {
    use super::content_disposition;

    /// 取出 `filename="..."` 里的回退段
    fn fallback(h: &str) -> String {
        h.split("filename=\"")
            .nth(1)
            .unwrap_or_default()
            .split('"')
            .next()
            .unwrap_or_default()
            .to_string()
    }

    #[test]
    fn cjk_name_goes_to_filename_star() {
        let h = content_disposition("测试种-42.torrent");
        // 测 = E6 B5 8B，试 = E8 AF 95，种 = E7 A7 8D
        assert!(
            h.contains("filename*=UTF-8''%E6%B5%8B%E8%AF%95%E7%A7%8D"),
            "{h}"
        );
        let fb = fallback(&h);
        // 回退段不得留中文（老客户端按字节读会乱码）
        assert!(fb.is_ascii(), "{fb}");
        assert!(fb.ends_with("-42.torrent"), "{fb}");
    }

    #[test]
    fn ascii_name_survives_unchanged() {
        let h = content_disposition("Show.S01E01-7.torrent");
        assert!(h.contains("filename=\"Show.S01E01-7.torrent\""), "{h}");
        assert!(h.contains("filename*=UTF-8''Show.S01E01-7.torrent"), "{h}");
    }

    #[test]
    fn quote_cannot_rewrite_the_header() {
        let h = content_disposition("a\";evil=x\r\nX-Injected: 1.torrent");
        assert_eq!(h.matches('"').count(), 2, "{h}");
        assert!(!h.contains('\r') && !h.contains('\n'), "{h}");
        // 危险字符（引号/分号/CR/LF）在 filename* 里必须是转义形态，
        // 字母本身留着无害——存盘名仍是同一串
        assert!(h.contains("a%22%3Bevil%3Dx%0D%0AX"), "{h}");
        assert!(!h.contains("evil="), "{h}");
    }

    #[test]
    fn all_symbol_name_falls_back_to_a_real_file() {
        let h = content_disposition("！！！");
        assert_eq!(fallback(&h), "torrent.torrent", "{h}");
    }
}
