//! Bencode 编码与 info_hash：`Bencode` 值树 → 字节流（BEP3 字典键字节序）。

use sha1::{Digest, Sha1};

use super::Bencode;

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
