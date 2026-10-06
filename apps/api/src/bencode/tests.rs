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

#[test]
fn download_rebuild_strips_offtracker_sources() {
    let info = Bencode::Dict(vec![
        (b"length".to_vec(), Bencode::Int(1024)),
        (b"name".to_vec(), Bencode::Bytes(b"test.bin".to_vec())),
        (b"piece length".to_vec(), Bencode::Int(16384)),
        (b"pieces".to_vec(), Bencode::Bytes(vec![7u8; 20])),
    ]);
    let root = Bencode::Dict(vec![
        (
            b"announce".to_vec(),
            Bencode::Bytes(b"http://old.example/announce".to_vec()),
        ),
        (
            b"url-list".to_vec(),
            Bencode::Bytes(b"http://evil.example/seed.bin".to_vec()),
        ),
        (
            b"httpseeds".to_vec(),
            Bencode::List(vec![Bencode::Bytes(b"http://evil/hs".to_vec())]),
        ),
        (b"dht_nodes".to_vec(), Bencode::List(vec![])),
        (b"info".to_vec(), info),
    ]);
    let mut raw = Vec::new();
    encode(&root, &mut raw);
    let out = build_download_torrent(&raw, "http://t.example/announce/PK", &[])
        .unwrap();
    let (parsed, _) = parse(&out).unwrap();
    // 这三个键都让内容绕过 tracker 直接到手（计量/风控全部失真）
    assert!(parsed.get(b"url-list").is_none(), "url-list 未剥离");
    assert!(parsed.get(b"httpseeds").is_none(), "httpseeds 未剥离");
    assert!(parsed.get(b"dht_nodes").is_none(), "dht_nodes 未剥离");
    assert!(parsed.get(b"private").is_some(), "private 标记丢失");
    assert_eq!(
        parse_torrent(&raw).unwrap().info_hash_hex,
        parse_torrent(&out).unwrap().info_hash_hex,
        "剥离不得改变 info_hash"
    );
}

#[test]
fn download_rebuild_preserves_non_canonical_info_bytes() {
    // 键序非 BEP3 排序的上传（重打包/跨站复种常见）：重编码会改字节序，
    // 下载种的 info_hash 与入库时的 raw 口径分叉 —— M05 承诺「info 不动」，
    // 此前只对规范输入成立（审计四轮实测复现）。
    let bytes =
        b"d4:infod12:piece lengthi16384e4:name8:test.bin6:lengthi1024eee"
            .to_vec();
    let pt_in = parse_torrent(&bytes).unwrap();
    assert_ne!(
        pt_in.raw_info_hash_hex, pt_in.info_hash_hex,
        "构造输入必须为非规范键序（raw≠canonical），否则测不到修复路径"
    );
    let out =
        build_download_torrent(&bytes, "http://t.example/announce/PK", &[])
            .unwrap();
    let pt_out = parse_torrent(&out).unwrap();
    assert_eq!(
        pt_out.raw_info_hash_hex, pt_in.raw_info_hash_hex,
        "info 原始字节必须逐字节保留"
    );
    // 规范化口径亦不变（info 内容未动，仅键序）
    assert_eq!(pt_out.info_hash_hex, pt_in.info_hash_hex);
}

#[test]
fn parser_rejects_deep_nesting_and_len_overflow() {
    // 深度：1 MiB 的 `l` 串（远小于 4 MiB 上传上限）曾能打爆请求线程栈
    let deep = vec![b'l'; 200_000];
    assert!(parse(&deep).is_err(), "深层嵌套必须被拒绝而不是栈溢出");
    // 长度回绕：长度前缀本身写成 usize::MAX 时，release 下曾算出 end < start
    // 而切片 panic；现在必须是错误
    assert!(parse(b"18446744073709551615:x").is_err());
    assert!(
        parse(b"99999999999999999999:x").is_err(),
        "超出 usize 的长度前缀"
    );
}

#[test]
fn download_rebuild_does_not_duplicate_private_key() {
    let mut raw: Vec<u8> = Vec::new();
    let info = Bencode::Dict(vec![
        (b"length".to_vec(), Bencode::Int(1024)),
        (b"name".to_vec(), Bencode::Bytes(b"t.bin".to_vec())),
        (b"piece length".to_vec(), Bencode::Int(16384)),
        (b"pieces".to_vec(), Bencode::Bytes(vec![3u8; 20])),
    ]);
    let root = Bencode::Dict(vec![
        (b"private".to_vec(), Bencode::Int(0)),
        (
            b"announce".to_vec(),
            Bencode::Bytes(b"http://old/a".to_vec()),
        ),
        (b"info".to_vec(), info),
    ]);
    encode(&root, &mut raw);
    let out =
        build_download_torrent(&raw, "http://t/announce/PK", &[]).unwrap();
    let (parsed, _) = parse(&out).unwrap();
    let pairs = match &parsed {
        Bencode::Dict(p) => p,
        _ => panic!("root dict"),
    };
    let n = pairs.iter().filter(|(k, _)| k == b"private").count();
    assert_eq!(n, 1, "根级 private 必须唯一，重复键会让严格模式客户端拒读");
    assert_eq!(
        parse_torrent(&raw).unwrap().info_hash_hex,
        parse_torrent(&out).unwrap().info_hash_hex
    );
}
