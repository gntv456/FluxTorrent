//! .torrent 处理：服务端权威解析（M04）+ 下载重建（M05）+ 原始字节区间扫描。

use sha1::{Digest, Sha1};

use super::encode::encode;
use super::parse::parse;
use super::{hex, Bencode};

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
    let name = std::str::from_utf8(
        info.get(b"name").and_then(|v| v.as_bytes()).unwrap_or(b""),
    )
    .unwrap_or("")
    .to_string();
    let piece_length = info
        .get(b"piece length")
        .and_then(|v| v.as_int())
        .unwrap_or(0);

    // 计算总大小、文件数与文件清单（files 表数据源）
    let (size, files) = if let Some(files) =
        info.get(b"files").and_then(|v| match v {
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

    let ih = super::encode::info_hash(info);
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
    let mut tier: Vec<Bencode> =
        Vec::with_capacity(1 + announce_fallbacks.len());
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
    let len: usize =
        std::str::from_utf8(&bytes[pos..colon]).ok()?.parse().ok()?;
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
