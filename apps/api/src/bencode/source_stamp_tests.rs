//! source 旗（NP takeupload 口径）测试。自 validate_tests.rs 拆出守
//! 300 行门禁；`valid_torrent` 夹具复用原文件（同 crate 内 pub(crate)）。

use super::encode::encode;
use super::hex;
use super::parse;
use super::torrent::{build_download_torrent, parse_torrent};
use super::validate::{stamp_source, validate_for_upload};
use super::validate_tests::valid_torrent;
use super::Bencode;

// ---- 站点 source 旗（NP takeupload 口径）：上传打旗后 hash 唯一 ----

#[test]
fn stamp_source_changes_hash_and_bytes_but_not_pieces() {
    let bytes = valid_torrent(b"s.bin", 1_048_576, 16_384);
    let p = parse_torrent(&bytes).unwrap();
    let (ih_before, pieces_before) =
        (p.info_hash_hex.clone(), p.pieces_hash_hex.clone());
    let raw_before = p.raw.clone();

    let stamped = stamp_source(p, "tracker.example 测试站").unwrap();

    // hash 变：与上传原始种（其他站/源站副本）彻底区分
    assert_ne!(
        stamped.info_hash_hex, ih_before,
        "source 旗必须改变 info_hash"
    );
    // pieces_hash 不变：同内容跨站指纹保留（IYUU/辅种/dup_policy 依据）
    assert_eq!(
        stamped.pieces_hash_hex, pieces_before,
        "source 键不影响 info.pieces，pieces_hash 必须稳定"
    );
    // 字节变了且 info 里真的有 source
    assert_ne!(stamped.raw, raw_before);
    let (root, _) = parse(&stamped.raw).unwrap();
    let src = root
        .get(b"info")
        .and_then(|i| i.get(b"source"))
        .and_then(|v| v.as_bytes())
        .unwrap();
    assert_eq!(src, "[tracker.example 测试站]".as_bytes());
    // raw_info_hash 与规范化口径恒等（encode 键序产物）
    assert_eq!(stamped.info_hash_hex.len(), 40);
    assert_eq!(stamped.raw_info_hash_hex, stamped.info_hash_hex);
    // 打旗后的种子能原样通过上传校验（private 保留）
    validate_for_upload(&stamped).unwrap();
    // 下载侧重建（换 announce 不碰 info）后 hash 与入库一致——
    // 「上传打旗 → 客户端拿到同 hash」闭环
    let dl = build_download_torrent(&stamped.raw, "http://t/announce/k", &[])
        .unwrap();
    let p2 = parse_torrent(&dl).unwrap();
    assert_eq!(p2.info_hash_hex, stamped.info_hash_hex);
}

#[test]
fn stamp_source_is_idempotent_per_content() {
    // 同内容重复上传（不带 entropy）：hash 相同 ⇒ 判重/幂等闸照常工作
    let a = stamp_source(
        parse_torrent(&valid_torrent(b"m.bin", 1_048_576, 16_384)).unwrap(),
        "t.example 站",
    )
    .unwrap();
    let b = stamp_source(
        parse_torrent(&valid_torrent(b"m.bin", 1_048_576, 16_384)).unwrap(),
        "t.example 站",
    )
    .unwrap();
    assert_eq!(a.info_hash_hex, b.info_hash_hex);
}

#[test]
fn stamp_source_overrides_existing_source_flag() {
    // 转载种自带他站 source（如 "[school] 学校"）：覆盖为本站旗，不残留
    let bytes = valid_torrent(b"m.bin", 1_048_576, 16_384);
    let (root, _) = parse(&bytes).unwrap();
    let mut pairs = match root {
        Bencode::Dict(p) => p,
        _ => unreachable!(),
    };
    let idx = pairs.iter().position(|(k, _)| k == b"info").unwrap();
    if let Bencode::Dict(info) = &mut pairs[idx].1 {
        info.push((b"source".to_vec(), Bencode::Bytes(b"[school] x".to_vec())));
    }
    let mut out = Vec::new();
    encode(&Bencode::Dict(pairs), &mut out);
    let stamped =
        stamp_source(parse_torrent(&out).unwrap(), "t.example 站").unwrap();
    let (root2, _) = parse(&stamped.raw).unwrap();
    let src = root2
        .get(b"info")
        .and_then(|i| i.get(b"source"))
        .and_then(|v| v.as_bytes())
        .unwrap();
    assert_eq!(src, "[t.example 站]".as_bytes(), "他站 source 必须被覆盖");
    // info 字典里只有一个 source 键
    let n = root2
        .get(b"info")
        .map(|i| match i {
            Bencode::Dict(pp) => {
                pp.iter().filter(|(k, _)| k == &b"source").count()
            }
            _ => 0,
        })
        .unwrap_or(0);
    assert_eq!(n, 1, "不得出现重复 source 键（严格客户端 bdecode 失败）");
}
