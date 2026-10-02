//! bencode 编码与字节工具：announce/scrape 响应、hex、percent-decode。

use super::model::{CompactPeer, CompactPeer6};

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// peer_id 的 hex 字符串 → 原始 20 字节（非 compact 响应用）。
/// peer_id 规范上就是 20 字节，但不合规客户端会发短/长 id ——
/// 短了右侧补 0、长了截断，绝不 panic（tracker 不能被单个脏 peer 打挂）。
pub fn peer_id_bytes(hex_str: &str) -> [u8; 20] {
    let mut out = [0u8; 20];
    let n = hex_str.len().min(40) / 2;
    for i in 0..n {
        if let Ok(b) = u8::from_str_radix(&hex_str[i * 2..i * 2 + 2], 16) {
            out[i] = b;
        }
    }
    out
}

/// BEP3 bencode announce 响应。
///
/// `compact=true`（默认，BEP23 之后的事实标准）：v4 6 字节/peer，
/// BEP-7 v6 18 字节/peer 进 `peers6`。
/// `compact=false`（老客户端兜底）：`peers` 为 peer 字典列表
/// （`d2:ip…7:peer id…4:port…ee`），此时不发 `peers6` —— 不支持 compact 的
/// 客户端本来也读不懂 BEP7 字段。
///
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
    compact: bool,
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
    // 字典键序按字节序（BEP3：sorted as raw strings）：
    // complete < downloaded < incomplete < interval < min interval < peers < peers6。
    // 0267 修正：此前写成 complete/incomplete/downloaded，不合规
    // （宽容的客户端能忍，严格实现的校验器会拒收）。
    let mut out = format!(
        "d8:completei{complete}e10:downloadedi{downloaded}e10:incompletei{incomplete}e\
         8:intervali{interval}e12:min intervali{min_interval}e5:peers"
    )
    .into_bytes();
    if compact {
        out.extend_from_slice(format!("{}:", peers_bytes.len()).as_bytes());
        out.extend_from_slice(&peers_bytes);
        if !peers6_bytes.is_empty() {
            out.extend_from_slice(
                format!("6:peers6{}:", peers6_bytes.len()).as_bytes(),
            );
            out.extend_from_slice(&peers6_bytes);
        }
    } else {
        // 非 compact：list of dicts。条目内键序同样是字节序（ip < peer id < port）。
        // 只发 v4：BEP3 的非 compact 字典要求带 peer id，而 v6 peer 我们只存
        // ip/port（compact 路径不需要 id）—— 与其塞一个假 peer id 骗客户端，
        // 不如只发能诚实描述的那部分（不支持 compact 的客户端本就是 IPv4 时代产物）。
        out.push(b'l');
        for p in peers {
            let ip = std::net::Ipv4Addr::from(p.ip).to_string();
            out.extend_from_slice(
                format!("d2:ip{}:{}7:peer id20:", ip.len(), ip).as_bytes(),
            );
            out.extend_from_slice(&p.peer_id);
            out.extend_from_slice(format!("4:porti{}ee", p.port).as_bytes());
        }
        out.push(b'e');
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
