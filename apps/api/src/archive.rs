//! 极简 zip 打包（stored / 无压缩，零依赖）。
//!
//! 用途：种子列表「批量下载」把 N 个 .torrent 打成一个 zip。
//! .torrent 本身只有几十 KB，不需要 deflate —— 用 stored 模式可以让实现
//! 只有「local header + 数据 + central directory + EOCD」四段，
//! 不必为单个功能给 api crate 引入 zip/flate 依赖。
//!
//! 正确性：`crc32` 与结构由单测锁；并已用 Python `zipfile` 交叉校验过
//! （生成的 zip 能被标准库正常列出与读取）。

const SIG_LOCAL: u32 = 0x0403_4b50;
const SIG_CENTRAL: u32 = 0x0201_4b50;
const SIG_EOCD: u32 = 0x0605_4b50;

/// CRC-32（IEEE，反射多项式 0xEDB88320）——zip 每个条目都需要
fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

/// 文件名清洗：剥掉目录部分与控制字符，防 zip-slip（`../../x` 之类）
pub fn safe_name(raw: &str) -> String {
    let base = raw.rsplit(['/', '\\']).next().unwrap_or(raw);
    let cleaned: String = base
        .chars()
        .filter(|c| !c.is_control() && *c != '\0')
        .collect();
    let trimmed = cleaned.trim().trim_matches('.');
    if trimmed.is_empty() {
        "file".to_string()
    } else {
        trimmed.chars().take(80).collect()
    }
}

/// 打包为 zip（stored）。`files` 顺序即条目顺序。
pub fn zip_stored(files: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    let mut central: Vec<u8> = Vec::new();
    for (name, data) in files {
        let name_bytes = name.as_bytes();
        let crc = crc32(data);
        let offset = out.len() as u32;
        let size = data.len() as u32;
        // ---- local file header ----
        out.extend_from_slice(&SIG_LOCAL.to_le_bytes());
        out.extend_from_slice(&20u16.to_le_bytes()); // version needed
                                                     // flags bit11 = 文件名 UTF-8
        out.extend_from_slice(&0x0800u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // method = stored
        out.extend_from_slice(&0u16.to_le_bytes()); // mtime
        out.extend_from_slice(&0x21u16.to_le_bytes()); // mdate = 1980-01-01
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // extra len
        out.extend_from_slice(name_bytes);
        out.extend_from_slice(data);
        // ---- central directory entry ----
        central.extend_from_slice(&SIG_CENTRAL.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes()); // version made by
        central.extend_from_slice(&20u16.to_le_bytes()); // version needed
        central.extend_from_slice(&0x0800u16.to_le_bytes()); // flags（UTF-8 名）
        central.extend_from_slice(&0u16.to_le_bytes()); // method
        central.extend_from_slice(&0u16.to_le_bytes()); // mtime
        central.extend_from_slice(&0x21u16.to_le_bytes()); // mdate
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes()); // extra
        central.extend_from_slice(&0u16.to_le_bytes()); // comment
        central.extend_from_slice(&0u16.to_le_bytes()); // disk start
        central.extend_from_slice(&0u16.to_le_bytes()); // internal attrs
        central.extend_from_slice(&0u32.to_le_bytes()); // external attrs
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name_bytes);
    }
    let cd_offset = out.len() as u32;
    let cd_size = central.len() as u32;
    out.extend_from_slice(&central);
    // ---- end of central directory ----
    let n = files.len() as u16;
    out.extend_from_slice(&SIG_EOCD.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // this disk
    out.extend_from_slice(&0u16.to_le_bytes()); // cd disk
    out.extend_from_slice(&n.to_le_bytes());
    out.extend_from_slice(&n.to_le_bytes());
    out.extend_from_slice(&cd_size.to_le_bytes());
    out.extend_from_slice(&cd_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // comment len
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// CRC-32 标准向量（IEEE 校验串）
    #[test]
    fn crc32_matches_reference() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b""), 0);
        assert_eq!(crc32(b"a"), 0xE8B7_BE43);
    }

    /// zip 结构：头尾签名、条目数、EOCD 里的偏移可反查 central directory
    #[test]
    fn zip_structure_self_consistent() {
        let files = vec![
            ("a.torrent".to_string(), b"hello".to_vec()),
            ("b.torrent".to_string(), b"world!".to_vec()),
        ];
        let z = zip_stored(&files);
        assert_eq!(&z[0..4], &SIG_LOCAL.to_le_bytes());
        assert_eq!(&z[z.len() - 22..z.len() - 18], &SIG_EOCD.to_le_bytes());
        // EOCD 尾部字段：条目数 / cd 大小 / cd 偏移
        let tail = &z[z.len() - 22..];
        let entries = u16::from_le_bytes([tail[10], tail[11]]);
        let cd_tail = &tail[12..20];
        let cd_size = u32::from_le_bytes(cd_tail[0..4].try_into().unwrap());
        let cd_off = u32::from_le_bytes(cd_tail[4..8].try_into().unwrap());

        assert_eq!(entries, 2);
        assert_eq!(cd_off + cd_size, (z.len() - 22) as u32);
        assert_eq!(
            &z[cd_off as usize..cd_off as usize + 4],
            &SIG_CENTRAL.to_le_bytes()
        );
        // 数据确实写进了 local header 之后
        assert!(z.windows(5).any(|w| w == b"hello"));
        assert!(z.windows(6).any(|w| w == b"world!"));
    }

    /// 文件名清洗：剥路径、去控制字符、兜底非空
    #[test]
    fn safe_name_blocks_traversal() {
        assert_eq!(safe_name("../../etc/passwd"), "passwd");
        assert_eq!(safe_name("dir\\evil.torrent"), "evil.torrent");
        assert_eq!(safe_name(".."), "file");
        assert_eq!(safe_name(""), "file");
        assert!(!safe_name("a\u{7}b").contains('\u{7}'));
    }
}
