//! 兼容层共享工具：SHA3-256 哈希十六进制输出（passkey / 凭证哈希与限流桶）。

use sha3::{Digest, Sha3_256};

pub(super) fn sha3_hex(bytes: &[u8]) -> String {
    let mut h = Sha3_256::new();
    h.update(bytes);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}
