//! Bencode 编解码与 .torrent 处理（M04/M05）。
//! 服务端权威解析（§M04：前端 WASM 仅预览）；info_hash = SHA1(bencoded info dict)。

mod encode;
mod parse;
mod torrent;

#[cfg(test)]
mod tests;

// 门禁拆分：`parse`/`encode`/`ParsedTorrent` 等仅在模块内与单测使用，
// bin crate 的 pub use 重导出会触发 unused_imports（原文件为 pub fn + dead_code 静默）。
#[allow(unused_imports)]
pub use encode::{encode, info_hash};
#[allow(unused_imports)]
pub use parse::parse;
#[allow(unused_imports)]
pub use torrent::{build_download_torrent, parse_torrent, ParsedTorrent};

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
            Bencode::Dict(pairs) => {
                pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v)
            }
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

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}
