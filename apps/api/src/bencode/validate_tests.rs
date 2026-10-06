//! `validate_for_upload` / `reject_off_tracker_sources` 的单测
//! （0288 从 tests.rs 拆出，300 行门禁）。

use super::*;

// ---- 发种入库前的结构校验（实测审计 P0-6：畸形 .torrent 曾全部 200 入库）----

/// 合法的最小种子：单文件、pieces 与大小匹配（1 MiB / 16 KiB = 64 片）
fn valid_torrent(name: &[u8], length: i64, piece_len: i64) -> Vec<u8> {
    // 测试夹具自己不许除零：piece length=0 的用例只给一片哈希，
    // 让生产代码在「piece length 必须大于 0」这一步判死。
    let pieces = if piece_len > 0 {
        (length + piece_len - 1) / piece_len
    } else {
        1
    };
    let info = Bencode::Dict(vec![
        (b"length".to_vec(), Bencode::Int(length)),
        (b"name".to_vec(), Bencode::Bytes(name.to_vec())),
        (b"piece length".to_vec(), Bencode::Int(piece_len)),
        (
            b"pieces".to_vec(),
            Bencode::Bytes(vec![3u8; (pieces * 20) as usize]),
        ),
    ]);
    let mut out = Vec::new();
    encode(
        &Bencode::Dict(vec![
            (b"info".to_vec(), info),
            (b"creation date".to_vec(), Bencode::Int(1_700_000_000)),
        ]),
        &mut out,
    );
    out
}

fn validate(bytes: &[u8]) -> Result<(), String> {
    parse_torrent(bytes).and_then(|p| validate_for_upload(&p))
}

#[test]
fn validate_accepts_well_formed_torrent() {
    assert!(validate(&valid_torrent(b"good.bin", 1_048_576, 16_384)).is_ok());
}

#[test]
fn validate_rejects_missing_pieces() {
    let info = Bencode::Dict(vec![
        (b"length".to_vec(), Bencode::Int(1024)),
        (b"name".to_vec(), Bencode::Bytes(b"no-pieces.bin".to_vec())),
        (b"piece length".to_vec(), Bencode::Int(16_384)),
    ]);
    let mut out = Vec::new();
    encode(&Bencode::Dict(vec![(b"info".to_vec(), info)]), &mut out);
    let err = validate(&out).unwrap_err();
    assert!(err.contains("pieces"), "错误要说清缺什么: {err}");
}

#[test]
fn validate_rejects_pieces_not_multiple_of_20() {
    let info = Bencode::Dict(vec![
        (b"length".to_vec(), Bencode::Int(1024)),
        (b"name".to_vec(), Bencode::Bytes(b"bad-pieces.bin".to_vec())),
        (b"piece length".to_vec(), Bencode::Int(16_384)),
        (b"pieces".to_vec(), Bencode::Bytes(vec![7u8; 25])),
    ]);
    let mut out = Vec::new();
    encode(&Bencode::Dict(vec![(b"info".to_vec(), info)]), &mut out);
    assert!(validate(&out).unwrap_err().contains("20"));
}

#[test]
fn validate_rejects_zero_piece_length() {
    let err = validate(&valid_torrent(b"zero-pl.bin", 1024, 0)).unwrap_err();
    assert!(err.contains("piece length"), "错误文案要指到字段: {err}");
}

#[test]
fn validate_rejects_zero_total_size() {
    let err = validate(&valid_torrent(b"empty.bin", 0, 16_384)).unwrap_err();
    assert!(err.contains("大小"), "应说明是大小问题: {err}");
}

#[test]
fn validate_rejects_empty_name() {
    let err = validate(&valid_torrent(b"", 1024, 16_384)).unwrap_err();
    assert!(err.contains("名称"), "应说明是名称问题: {err}");
}

#[test]
fn validate_rejects_piece_count_mismatch() {
    // 声明 1 MiB / 16 KiB 片 = 64 片，却只给 3 片的哈希表：客户端必然校验失败
    let info = Bencode::Dict(vec![
        (b"length".to_vec(), Bencode::Int(1_048_576)),
        (b"name".to_vec(), Bencode::Bytes(b"mismatch.bin".to_vec())),
        (b"piece length".to_vec(), Bencode::Int(16_384)),
        (b"pieces".to_vec(), Bencode::Bytes(vec![9u8; 60])),
    ]);
    let mut out = Vec::new();
    encode(&Bencode::Dict(vec![(b"info".to_vec(), info)]), &mut out);
    let err = validate(&out).unwrap_err();
    assert!(err.contains("分片数"), "应给出期望/实际片数: {err}");
}

#[test]
fn validate_rejects_traversal_and_absolute_paths() {
    for path in [
        "../../etc/passwd".to_string(),
        "/etc/passwd".to_string(),
        "..\\win.ini".to_string(),
        "ok/..\\evil".to_string(),
    ] {
        let info = Bencode::Dict(vec![
            (b"name".to_vec(), Bencode::Bytes(b"multi".to_vec())),
            (b"piece length".to_vec(), Bencode::Int(16_384)),
            (b"pieces".to_vec(), Bencode::Bytes(vec![5u8; 20])),
            (
                b"files".to_vec(),
                Bencode::List(vec![Bencode::Dict(vec![
                    (b"length".to_vec(), Bencode::Int(1024)),
                    (
                        b"path".to_vec(),
                        Bencode::List(vec![Bencode::Bytes(
                            path.clone().into_bytes(),
                        )]),
                    ),
                ])]),
            ),
        ]);
        let mut out = Vec::new();
        encode(&Bencode::Dict(vec![(b"info".to_vec(), info)]), &mut out);
        let err = validate(&out).unwrap_err();
        assert!(err.contains("路径"), "{path} 应被拒: {err}");
    }
}

#[test]
fn validate_rejects_absurd_file_count() {
    let files = Bencode::List(
        (0..(MAX_TORRENT_FILES + 1) as usize)
            .map(|_| {
                Bencode::Dict(vec![
                    (b"length".to_vec(), Bencode::Int(1)),
                    (
                        b"path".to_vec(),
                        Bencode::List(vec![Bencode::Bytes(b"f.bin".to_vec())]),
                    ),
                ])
            })
            .collect(),
    );
    let info = Bencode::Dict(vec![
        (b"name".to_vec(), Bencode::Bytes(b"too-many".to_vec())),
        (b"piece length".to_vec(), Bencode::Int(16_384)),
        (b"pieces".to_vec(), Bencode::Bytes(vec![1u8; 20])),
        (b"files".to_vec(), files),
    ]);
    let mut out = Vec::new();
    encode(&Bencode::Dict(vec![(b"info".to_vec(), info)]), &mut out);
    assert!(validate(&out).unwrap_err().contains("文件数"));
}

#[test]
fn off_tracker_sources_are_rejected_at_upload() {
    let info = Bencode::Dict(vec![
        (b"length".to_vec(), Bencode::Int(1024)),
        (b"name".to_vec(), Bencode::Bytes(b"seeded.bin".to_vec())),
        (b"piece length".to_vec(), Bencode::Int(16_384)),
        (b"pieces".to_vec(), Bencode::Bytes(vec![4u8; 20])),
    ]);
    for key in [
        b"url-list".to_vec(),
        b"httpseeds".to_vec(),
        b"dht_nodes".to_vec(),
    ] {
        let mut out = Vec::new();
        encode(
            &Bencode::Dict(vec![
                (b"info".to_vec(), info.clone()),
                (
                    key.clone(),
                    Bencode::Bytes(b"http://evil.example/x".to_vec()),
                ),
            ]),
            &mut out,
        );
        let err = reject_off_tracker_sources(&out).unwrap_err();
        assert!(
            err.contains("tracker"),
            "{} 应被拒并说明原因: {err}",
            String::from_utf8_lossy(&key)
        );
    }
    // 干净种子放行
    let mut clean = Vec::new();
    encode(&Bencode::Dict(vec![(b"info".to_vec(), info)]), &mut clean);
    assert!(reject_off_tracker_sources(&clean).is_ok());
}

#[test]
fn parse_exposes_pieces_length_for_validation() {
    let bytes = valid_torrent(b"pl.bin", 1_048_576, 16_384);
    let p = parse_torrent(&bytes).unwrap();
    assert_eq!(p.pieces_len, 64 * 20, "64 片的哈希表应有 1280 字节");
}
