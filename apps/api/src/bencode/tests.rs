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
    let ann =
        std::str::from_utf8(root.get(b"announce").unwrap().as_bytes().unwrap())
            .unwrap();
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
