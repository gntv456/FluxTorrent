//! Bencode 编解码与 .torrent 处理（M04/M05）。
//! 服务端权威解析（§M04：前端 WASM 仅预览）；info_hash = SHA1(bencoded info dict)。

use sha1::{Digest, Sha1};

#[derive(Debug, Clone, PartialEq)]
pub enum Bencode {
    Int(i64),
    Bytes(Vec<u8>),
    List(Vec<Bencode>),
    Dict(Vec<(Vec<u8>, Bencode)>),
}

impl Bencode {
    pub fn get(&self, key: &[u8]) -> Option<&Bencode> {
        match self {
            Bencode::Dict(pairs) => pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Bencode::Bytes(b) => Some(b),
            _ => None,
        }
    }
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Bencode::Int(i) => Some(*i),
            _ => None,
        }
    }
}

pub fn parse(input: &[u8]) -> Result<(Bencode, usize), String> {
    if input.is_empty() {
        return Err("empty input".into());
    }
    let (val, consumed) = parse_one(input, 0)?;
    Ok((val, consumed))
}

fn parse_one(buf: &[u8], pos: usize) -> Result<(Bencode, usize), String> {
    let rest = &buf.get(pos..).ok_or("eof")?;
    match rest.first() {
        Some(b'i') => {
            let end = rest
                .iter()
                .position(|&b| b == b'e')
                .ok_or("unterminated int")?;
            let s = std::str::from_utf8(&rest[1..end]).map_err(|_| "int utf8")?;
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
            let colon = rest.iter().position(|&c| c == b':').ok_or("bad string")?;
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

pub fn encode(val: &Bencode, out: &mut Vec<u8>) {
    match val {
        Bencode::Int(i) => {
            out.extend_from_slice(format!("i{}e", i).as_bytes());
        }
        Bencode::Bytes(b) => {
            out.extend_from_slice(format!("{}:", b.len()).as_bytes());
            out.extend_from_slice(b);
        }
        Bencode::List(items) => {
            out.push(b'l');
            for it in items {
                encode(it, out);
            }
            out.push(b'e');
        }
        Bencode::Dict(pairs) => {
            out.push(b'd');
            // 字典键须按字节序（BEP3）
            let mut sorted: Vec<_> = pairs.iter().collect();
            sorted.sort_by(|a, b| a.0.cmp(&b.0));
            for (k, v) in sorted {
                encode(&Bencode::Bytes(k.clone()), out);
                encode(v, out);
            }
            out.push(b'e');
        }
    }
}

/// SHA1(bencoded info) —— info_hash
pub fn info_hash(info: &Bencode) -> [u8; 20] {
    let mut buf = Vec::new();
    encode(info, &mut buf);
    let mut h = Sha1::new();
    h.update(&buf);
    h.finalize().into()
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

#[allow(dead_code)]
pub struct ParsedTorrent {
    pub info_hash_hex: String,
    /// 跨站辅种二级指纹：SHA1(info.pieces 原始字节)（YemaPT piecesHash 口径）。
    /// info_hash 会因子典外字段变化（重打包/source）失效；pieces_hash 只由分片内容决定，
    /// 是辅种 / 转种生态识别「同内容」的更鲁棒指纹。缺 pieces 字段时为空串。
    pub pieces_hash_hex: String,
    /// BEP3 原始字节口径 info_hash（不重排序）：客户端 announce 哈希与此一致。
    /// info_hash_hex 为规范化重编码口径（历史存量）；双口径见 worker 匹配逻辑。
    pub raw_info_hash_hex: String,
    pub name: String,
    pub size: i64,
    pub numfiles: i64,
    pub piece_length: i64,
    /// 文件清单（多文件种按 files 列表展开为 path.join("/")；单文件种为 name 一行）。
    /// 上传时写 files 表供「文件列表 / 按文件名搜索」使用（修复前该表从不写入）
    pub files: Vec<(String, i64)>,
    /// 发布时去掉私有种子的 announce 列表，由服务端重新注入
    pub raw: Vec<u8>,
}

/// 解析 .torrent 字节流（服务端权威校验：M04 验收「畸形/私种拒收」）
pub fn parse_torrent(bytes: &[u8]) -> Result<ParsedTorrent, String> {
    let (root, _) = parse(bytes)?;
    let info = root.get(b"info").ok_or("missing info dict")?;
    let name = std::str::from_utf8(info.get(b"name").and_then(|v| v.as_bytes()).unwrap_or(b""))
        .unwrap_or("")
        .to_string();
    let piece_length = info
        .get(b"piece length")
        .and_then(|v| v.as_int())
        .unwrap_or(0);

    // 计算总大小、文件数与文件清单（files 表数据源）
    let (size, files) = if let Some(files) = info.get(b"files").and_then(|v| match v {
        Bencode::List(l) => Some(l),
        _ => None,
    }) {
        let mut total = 0i64;
        let mut list = Vec::with_capacity(files.len());
        for f in files {
            let length = f.get(b"length").and_then(|v| v.as_int()).unwrap_or(0);
            total += length;
            // path 是 UTF-8 字节段列表（BEP3）：逐段解码（非法字节有损替换）后拼接
            let path = f
                .get(b"path")
                .and_then(|v| match v {
                    Bencode::List(segs) => Some(
                        segs.iter()
                            .filter_map(|s| s.as_bytes())
                            .map(|b| String::from_utf8_lossy(b).into_owned())
                            .collect::<Vec<_>>(),
                    ),
                    _ => None,
                })
                .unwrap_or_default()
                .join("/");
            list.push((path, length));
        }
        (total, list)
    } else {
        let len = info.get(b"length").and_then(|v| v.as_int()).unwrap_or(0);
        (len, vec![(name.clone(), len)])
    };
    let numfiles = files.len() as i64;

    let ih = info_hash(info);
    // 审计修复（P1）：BEP3 客户端按 info 字典「原始字节」计算 announce 哈希，
    // 规范化重编码在键序非排序的种子上与客户端口径不一致 → worker 静默丢计费。
    // 这里重扫原始字节定位 info 区间（d...e 配对），对原始字节做 SHA1。
    let raw_info_hash_hex = raw_info_span(bytes)
        .map(|span| {
            let mut h = Sha1::new();
            h.update(&bytes[span.0..span.1]);
            hex(h.finalize().as_slice())
        })
        .unwrap_or_else(|| hex(&ih));
    // pieces_hash = SHA1(info.pieces 原始字节)：只依赖文件分片内容，不含 announce/private/source
    // 等字典外字段，跨站重打包后仍稳定 —— 辅种/转种工具按它匹配「同内容」（YemaPT 口径）。
    let pieces_hash_hex = info
        .get(b"pieces")
        .and_then(|v| v.as_bytes())
        .map(|pieces| {
            let mut h = Sha1::new();
            h.update(pieces);
            hex(h.finalize().as_slice())
        })
        .unwrap_or_default();
    Ok(ParsedTorrent {
        info_hash_hex: hex(&ih),
        raw_info_hash_hex,
        pieces_hash_hex,
        name,
        size,
        numfiles,
        piece_length,
        files,
        raw: bytes.to_vec(),
    })
}

/// 生成下载用 .torrent：重新注入本站 announce（含 passkey，M05）。
/// announce_fallbacks 非空时按 BEP12 追加 announce-list（单 tier：首选地址 + 回退地址，
/// 如 https 首选 + http 回退），客户端汇报失败时自动降级。
pub fn build_download_torrent(
    raw: &[u8],
    announce_url: &str,
    announce_fallbacks: &[String],
) -> Result<Vec<u8>, String> {
    let (root, _) = parse(raw)?;
    let mut pairs = match &root {
        Bencode::Dict(p) => p.clone(),
        _ => return Err("root not dict".into()),
    };
    // 移除旧 announce 列表并写入本站
    pairs.retain(|(k, _)| k != b"announce-list" && k != b"announce");
    pairs.push((
        b"announce".to_vec(),
        Bencode::Bytes(announce_url.as_bytes().to_vec()),
    ));
    let mut tier: Vec<Bencode> = Vec::with_capacity(1 + announce_fallbacks.len());
    tier.push(Bencode::Bytes(announce_url.as_bytes().to_vec()));
    for u in announce_fallbacks {
        if !u.is_empty() && u != announce_url {
            tier.push(Bencode::Bytes(u.as_bytes().to_vec()));
        }
    }
    if tier.len() > 1 {
        pairs.push((
            b"announce-list".to_vec(),
            Bencode::List(vec![Bencode::List(tier)]),
        ));
    }
    pairs.push((b"private".to_vec(), Bencode::Int(1)));
    let rebuilt = Bencode::Dict(pairs);
    let mut out = Vec::new();
    encode(&rebuilt, &mut out);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_torrent() -> Vec<u8> {
        let t = b"d4:infod6:lengthi1024e4:name8:test.bin12:piece lengthi16384eee";
        t.to_vec()
    }

    #[test]
    fn parse_roundtrip() {
        let bytes = make_torrent();
        let pt = parse_torrent(&bytes).unwrap();
        assert_eq!(pt.name, "test.bin");
        assert_eq!(pt.size, 1024);
        assert_eq!(pt.numfiles, 1);
        assert_eq!(pt.info_hash_hex.len(), 40);
    }

    #[test]
    fn reject_garbage() {
        assert!(parse_torrent(b"not a torrent").is_err());
        assert!(parse_torrent(b"d4:infod").is_err());
    }

    #[test]
    fn download_rebuild_injects_announce() {
        let bytes = make_torrent();
        let out = build_download_torrent(
            &bytes,
            "http://tracker.flux.local/announce?passkey=abc",
            &[],
        )
        .unwrap();
        let (root, _) = parse(&out).unwrap();
        let ann = std::str::from_utf8(root.get(b"announce").unwrap().as_bytes().unwrap()).unwrap();
        assert!(ann.contains("passkey=abc"));
        assert!(
            root.get(b"announce-list").is_none(),
            "无回退时不应有 announce-list"
        );
        // info dict 未被改动 → info_hash 不变
        let (_, re_parsed) = parse(&out).unwrap();
        let _ = re_parsed;
        let pt2 = parse_torrent(&out).unwrap();
        let pt1 = parse_torrent(&bytes).unwrap();
        assert_eq!(pt1.info_hash_hex, pt2.info_hash_hex);
    }

    #[test]
    fn download_rebuild_announce_list_https_first() {
        let bytes = make_torrent();
        let out = build_download_torrent(
            &bytes,
            "https://tracker.flux.local/announce/PK",
            &["http://tracker.flux.local/announce/PK".to_string()],
        )
        .unwrap();
        let (root, _) = parse(&out).unwrap();
        // BEP12：单 tier [https 首选, http 回退]
        let list = root.get(b"announce-list").expect("announce-list 缺失");
        let tier = match list {
            Bencode::List(tiers) => match &tiers[0] {
                Bencode::List(t) => t,
                _ => panic!("tier 结构错误"),
            },
            _ => panic!("announce-list 结构错误"),
        };
        let first = std::str::from_utf8(tier[0].as_bytes().unwrap()).unwrap();
        let second = std::str::from_utf8(tier[1].as_bytes().unwrap()).unwrap();
        assert_eq!(first, "https://tracker.flux.local/announce/PK");
        assert_eq!(second, "http://tracker.flux.local/announce/PK");
        // info_hash 稳定
        assert_eq!(
            parse_torrent(&bytes).unwrap().info_hash_hex,
            parse_torrent(&out).unwrap().info_hash_hex
        );
    }

    #[test]
    fn download_rebuild_dedup_fallback() {
        let bytes = make_torrent();
        // 回退与首选相同 → 不生成 announce-list
        let out = build_download_torrent(
            &bytes,
            "http://t.example/announce/PK",
            &["http://t.example/announce/PK".to_string()],
        )
        .unwrap();
        let (root, _) = parse(&out).unwrap();
        assert!(root.get(b"announce-list").is_none());
    }
}

/// 定位 .torrent 字节中顶层 info 字典的原始区间 [start, end)。
/// 只处理顶层键（不递归进嵌套 dict/list 的 e），失败返回 None（调用方退规范化口径）。
fn raw_info_span(bytes: &[u8]) -> Option<(usize, usize)> {
    // 顶层必须以 'd' 开头
    if bytes.first() != Some(&b'd') {
        return None;
    }
    let mut pos = 1usize;
    loop {
        // 读键：长度前缀:bytes
        let (key_start, key_end) = match read_string(bytes, pos)? {
            (s, e) => (s, e),
        };
        let key = &bytes[key_start..key_end];
        pos = key_end;
        if pos >= bytes.len() {
            return None;
        }
        if key == b"info" && bytes[pos] == b'd' {
            // 深度配对扫描到匹配的 e
            let start = pos;
            let end = scan_dict_end(bytes, pos)?;
            return Some((start, end));
        }
        // 跳过该键的值（任意 bencode 值）
        pos = skip_value(bytes, pos)?;
        if pos < bytes.len() && bytes[pos] == b'e' {
            return None; // 顶层结束仍未遇 info
        }
    }
}

fn read_string(bytes: &[u8], pos: usize) -> Option<(usize, usize)> {
    let colon = bytes[pos..].iter().position(|&b| b == b':')? + pos;
    let len: usize = std::str::from_utf8(&bytes[pos..colon]).ok()?.parse().ok()?;
    let s = colon + 1;
    let e = s.checked_add(len)?;
    if e > bytes.len() {
        return None;
    }
    Some((s, e))
}

fn scan_dict_end(bytes: &[u8], start: usize) -> Option<usize> {
    // start 指向 'd'
    let mut pos = start + 1;
    loop {
        if pos >= bytes.len() {
            return None;
        }
        match bytes[pos] {
            b'e' => return Some(pos + 1),
            b'd' | b'l' => {
                pos = skip_value(bytes, pos)?;
            }
            b'i' => {
                pos = bytes[pos..].iter().position(|&b| b == b'e')? + pos + 1;
            }
            b'0'..=b'9' => {
                let (_, e) = read_string(bytes, pos)?;
                pos = e;
            }
            _ => return None,
        }
    }
}

fn skip_value(bytes: &[u8], pos: usize) -> Option<usize> {
    match *bytes.get(pos)? {
        b'd' | b'l' => scan_dict_end(bytes, pos),
        b'i' => Some(bytes[pos..].iter().position(|&b| b == b'e')? + pos + 1),
        b'0'..=b'9' => {
            let (_, e) = read_string(bytes, pos)?;
            Some(e)
        }
        _ => None,
    }
}
