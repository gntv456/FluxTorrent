//! bencode 编码与字节工具：announce/scrape 响应、hex、percent-decode。

use super::model::{CompactPeer, CompactPeer6};

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// BEP3 bencode announce 响应（compact 模式：v4 6 字节/peer，BEP-7 v6 18 字节/peer 进 peers6）。
/// interval / min interval 为 BEP3 强制字段：告知客户端汇报间隔，
/// 缺失时部分客户端会按自身默认值频繁重发 —— 这是高频 announce 的诱因之一。
/// 返回原始字节 —— peer 列表是二进制，绝不可经 String/UTF-8 转换（会损坏数据）。
pub fn bencode_announce(
    complete: i64,
    incomplete: i64,
    downloaded: i64,
    peers: &[CompactPeer],
    peers6: &[CompactPeer6],
    interval: i64,
    min_interval: i64,
) -> Vec<u8> {
    let mut peers_bytes = Vec::with_capacity(peers.len() * 6);
    for p in peers {
        peers_bytes.extend_from_slice(&p.ip);
        peers_bytes.extend_from_slice(&p.port.to_be_bytes());
    }
    let mut peers6_bytes = Vec::with_capacity(peers6.len() * 18);
    for p in peers6 {
        peers6_bytes.extend_from_slice(&p.ip);
        peers6_bytes.extend_from_slice(&p.port.to_be_bytes());
    }
    // 字典键序按字节序（BEP3）：… peers < peers6（前缀短者在前）
    let mut out = format!(
        "d8:completei{complete}e10:incompletei{incomplete}e10:downloadedi{downloaded}e\
         8:intervali{interval}e12:min intervali{min_interval}e5:peers{}:",
        peers_bytes.len()
    )
    .into_bytes();
    out.extend_from_slice(&peers_bytes);
    if !peers6_bytes.is_empty() {
        out.extend_from_slice(
            format!("6:peers6{}:", peers6_bytes.len()).as_bytes(),
        );
        out.extend_from_slice(&peers6_bytes);
    }
    out.extend_from_slice(b"e");
    out
}

/// BEP3 scrape 响应：files 字典键为 20 字节原始 info_hash。
pub fn bencode_scrape(files: &[(Vec<u8>, usize, usize)]) -> Vec<u8> {
    let mut out = b"d5:filesd".to_vec();
    for (ih, seeders, leechers) in files {
        out.extend_from_slice(b"20:");
        out.extend_from_slice(ih);
        out.extend_from_slice(
            format!("d8:completei{seeders}e10:incompletei{leechers}e10:downloadedi0ee").as_bytes(),
        );
    }
    out.extend_from_slice(b"e");
    out
}

/// 手工 percent-decode：BT 客户端的 info_hash/peer_id 是任意字节的 URL 编码（含 %00-%FF），
/// 标准库 Query 反序列化走 UTF-8 会失败或损坏 —— 必须在字节层解码。
pub fn percent_decode(raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == b'%' && i + 2 < raw.len() {
            let hi = (raw[i + 1] as char).to_digit(16);
            let lo = (raw[i + 2] as char).to_digit(16);
            if let (Some(h), Some(l)) = (hi, lo) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(raw[i]);
        i += 1;
    }
    out
}
