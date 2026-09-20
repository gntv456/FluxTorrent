//! Bencode 解析：字节流 → `Bencode` 值树（服务端权威，§M04）。

use super::Bencode;

pub fn parse(input: &[u8]) -> Result<(Bencode, usize), String> {
    if input.is_empty() {
        return Err("empty input".into());
    }
    let (val, consumed) = parse_one(input, 0)?;
    Ok((val, consumed))
}

pub(super) fn parse_one(
    buf: &[u8],
    pos: usize,
) -> Result<(Bencode, usize), String> {
    let rest = &buf.get(pos..).ok_or("eof")?;
    match rest.first() {
        Some(b'i') => {
            let end = rest
                .iter()
                .position(|&b| b == b'e')
                .ok_or("unterminated int")?;
            let s =
                std::str::from_utf8(&rest[1..end]).map_err(|_| "int utf8")?;
            let n: i64 = s.parse().map_err(|_| "int parse")?;
            Ok((Bencode::Int(n), pos + end + 1))
        }
        Some(b'l') => {
            let mut p = pos + 1;
            let mut items = Vec::new();
            while *buf.get(p).ok_or("eof")? != b'e' {
                let (v, np) = parse_one(buf, p)?;
                items.push(v);
                p = np;
            }
            Ok((Bencode::List(items), p + 1))
        }
        Some(b'd') => {
            let mut p = pos + 1;
            let mut pairs = Vec::new();
            while *buf.get(p).ok_or("eof")? != b'e' {
                let (k, np) = parse_one(buf, p)?;
                let key = match k {
                    Bencode::Bytes(b) => b,
                    _ => return Err("dict key must be bytes".into()),
                };
                let (v, np2) = parse_one(buf, np)?;
                pairs.push((key, v));
                p = np2;
            }
            Ok((Bencode::Dict(pairs), p + 1))
        }
        Some(b) if b.is_ascii_digit() => {
            let colon =
                rest.iter().position(|&c| c == b':').ok_or("bad string")?;
            let len: usize = std::str::from_utf8(&rest[..colon])
                .map_err(|_| "len utf8")?
                .parse()
                .map_err(|_| "len parse")?;
            let start = pos + colon + 1;
            let end = start + len;
            if end > buf.len() {
                return Err("string overrun".into());
            }
            Ok((Bencode::Bytes(buf[start..end].to_vec()), end))
        }
        _ => Err("unexpected token".into()),
    }
}
