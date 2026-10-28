//! 从归档的 `.torrent`（`torrent_files.raw`）里取**文件名列表**。
//!
//! 为什么放这里：api 侧 `bencode.rs` 是 api crate 的私有模块，worker 用不到；
//! 而 auto_review 在 worker 里跑、需要「info 字典 → 文件名」。只实现够用的子集
//! （bytes/int/list/dict），其余一律判坏返回空。
//!
//! 返回空 = **不知道**，不是「没有文件」：auto_review 的视频率/伪媒体规则会把
//! 空清单当未知而不是证据（见 `jobs/auto_review.rs` 的 `!file_names.is_empty()`）。

enum Val<'a> {
    Bytes(&'a [u8]),
    Int(i64),
    List(Vec<Val<'a>>),
    Dict(Vec<(&'a [u8], Val<'a>)>),
}

fn parse<'a>(buf: &'a [u8], pos: &mut usize) -> Option<Val<'a>> {
    let mut p = *pos;
    let c = *buf.get(p)?;
    let v = match c {
        b'i' => {
            p += 1;
            let end = p + buf[p..].iter().position(|&b| b == b'e')?;
            let n: i64 =
                std::str::from_utf8(&buf[p..end]).ok()?.parse().ok()?;
            p = end + 1;
            Val::Int(n)
        }
        b'l' => {
            p += 1;
            let mut v = Vec::new();
            while *buf.get(p)? != b'e' {
                v.push(parse(buf, &mut p)?);
            }
            p += 1;
            Val::List(v)
        }
        b'd' => {
            p += 1;
            let mut v = Vec::new();
            while *buf.get(p)? != b'e' {
                let Val::Bytes(k) = parse(buf, &mut p)? else {
                    return None;
                };
                v.push((k, parse(buf, &mut p)?));
            }
            p += 1;
            Val::Dict(v)
        }
        b'0'..=b'9' => {
            let colon = p + buf[p..].iter().position(|&b| b == b':')?;
            let len: usize =
                std::str::from_utf8(&buf[p..colon]).ok()?.parse().ok()?;
            let (start, end) = (colon + 1, colon + 1 + len);
            if end > buf.len() {
                return None;
            }
            p = end;
            Val::Bytes(&buf[start..end])
        }
        _ => return None,
    };
    *pos = p;
    Some(v)
}

fn field<'a, 'b>(d: &'b Val<'a>, key: &[u8]) -> Option<&'b Val<'a>> {
    let Val::Dict(m) = d else { return None };
    m.iter().find(|(k, _)| *k == key).map(|(_, v)| v)
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

/// 文件名列表：多文件取 `info.files[].path` 用 `/` 连接；单文件取 `info.name`。
/// 上限 200 条（auto_review 只做启发式判定，不需要全量）。
pub fn file_names(raw: &[u8]) -> Vec<String> {
    let mut pos = 0;
    let Some(root) = parse(raw, &mut pos) else {
        return Vec::new();
    };
    let Some(info) = field(&root, b"info") else {
        return Vec::new();
    };
    let mut out: Vec<String> = Vec::new();
    if let Some(Val::List(files)) = field(info, b"files") {
        for f in files {
            let Some(Val::List(path)) = field(f, b"path") else {
                continue;
            };
            let parts: Vec<String> = path
                .iter()
                .filter_map(|p| match p {
                    Val::Bytes(b) => Some(text(b)),
                    _ => None,
                })
                .collect();
            if !parts.is_empty() {
                out.push(parts.join("/"));
            }
            if out.len() >= 200 {
                break;
            }
        }
    } else if let Some(Val::Bytes(n)) = field(info, b"name") {
        out.push(text(n));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::file_names;

    #[test]
    fn single_file_uses_name() {
        let raw = b"d4:infod4:name5:a.mkvee";
        assert_eq!(file_names(raw), vec!["a.mkv".to_string()]);
    }

    #[test]
    fn multi_file_joins_path() {
        // info.files = [ { path: ["dir", "b.mkv"] } ]
        let raw = b"d4:infod5:filesld4:pathl3:dir5:b.mkveeeee";
        assert_eq!(file_names(raw), vec!["dir/b.mkv".to_string()]);
    }

    #[test]
    fn malformed_is_empty_not_fake() {
        assert!(file_names(b"").is_empty());
        assert!(file_names(b"i12e").is_empty());
        assert!(file_names(b"d4:infod4:name5:abc").is_empty());
    }
}
