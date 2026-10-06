//! Bencode 解析：字节流 → `Bencode` 值树（服务端权威，§M04）。
//!
//! 0289 加固：解析器原本对**嵌套深度**与**长度算术**都没设防——
//! ① `l`/`d` 无深度上限，1 MiB 的 `llll…`（远小于 4 MiB 上传上限）即可打爆
//!    请求线程栈；`Bencode` 树的递归 `Drop` 在错误路径上同样会溢出。
//! ② `start + len` 无溢出保护，`18446744073709551615:x` 在 release 下回绕成
//!    `end < start`，切片直接 panic。
//! 两者都是「一个会员上传一个文件就能弄挂一个 worker 线程」的量级。

use super::Bencode;

/// 嵌套深度上限。真实 .torrent 最深约 4~6 层（root/info/files/path），
/// 留到 32 已远超任何合法文件，纯为拒绝构造性嵌套。
const MAX_DEPTH: usize = 32;

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
    parse_at(buf, pos, 0)
}

fn parse_at(
    buf: &[u8],
    pos: usize,
    depth: usize,
) -> Result<(Bencode, usize), String> {
    if depth >= MAX_DEPTH {
        return Err("bencode nesting too deep".into());
    }
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
                let (v, np) = parse_at(buf, p, depth + 1)?;
                items.push(v);
                p = np;
            }
            Ok((Bencode::List(items), p + 1))
        }
        Some(b'd') => {
            let mut p = pos + 1;
            let mut pairs = Vec::new();
            while *buf.get(p).ok_or("eof")? != b'e' {
                let (k, np) = parse_at(buf, p, depth + 1)?;
                let key = match k {
                    Bencode::Bytes(b) => b,
                    _ => return Err("dict key must be bytes".into()),
                };
                let (v, np2) = parse_at(buf, np, depth + 1)?;
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
            // 回绕防护：合法长度必然落在缓冲区之内，用 checked_add 把
            // 「超大 len」变成错误而不是 panic
            let end = start
                .checked_add(len)
                .filter(|e| *e <= buf.len())
                .ok_or("string overrun")?;
            Ok((Bencode::Bytes(buf[start..end].to_vec()), end))
        }
        _ => Err("unexpected token".into()),
    }
}
