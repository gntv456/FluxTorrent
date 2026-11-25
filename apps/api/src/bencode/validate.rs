//! .torrent 的入库前结构校验（0288 从 torrent.rs 拆出，300 行门禁）。
//!
//! 为什么不做进 `parse_torrent`：那个函数还服务历史种子的再分发与站外导入，
//! 收紧它会让存量种子当场不可下载。校验只挂在**发种入口**上。

use super::encode::encode;
use super::hex;
use super::parse;
use super::torrent::ParsedTorrent;
use sha1::{Digest, Sha1};

/// 单个种子允许的最大文件数（超大合集的真实上限，同时挡住「一个种塞几万条路径」的写库放大）。
pub const MAX_TORRENT_FILES: i64 = 10_000;

/// 站外取源键：这些键让内容不经 tracker 直接到手，比率/H&R/免费时段全部失真。
const OFF_TRACKER_KEYS: [&[u8]; 3] = [b"url-list", b"httpseeds", b"dht_nodes"];

/// **发种入库前**的最小结构校验。
///
/// 刻意不塞进 `parse_torrent`：那个函数还服务历史种子的再分发与站外导入，
/// 收紧它会让存量种子当场不可下载（P0-6 实测：无 `pieces`、`pieces` 长度非 20 倍数、
/// `piece length=0`、总大小 0、空标题、`../../` 路径穿越、带 HTTP seed 外链，
/// 七种畸形 .torrent 全部 200 入库 —— 这类种子在客户端必然 hash 校验失败，
/// 变成 0 做种死种，工单全回到审核员面前）。
pub fn validate_for_upload(parsed: &ParsedTorrent) -> Result<(), String> {
    if parsed.name.trim().is_empty() {
        return Err(
            "种子名称为空：.torrent 的 info.name 与表单名称至少要有一个".into(),
        );
    }
    if parsed.size <= 0 {
        return Err("种子总大小为 0：文件清单里至少有一个非空文件".into());
    }
    if parsed.piece_length <= 0 {
        return Err("info.piece length 必须大于 0".into());
    }
    if parsed.pieces_len == 0 {
        return Err(
            ".torrent 缺少 info.pieces 字段（客户端无法校验分片）".into()
        );
    }
    if parsed.pieces_len % 20 != 0 {
        return Err(format!(
            "info.pieces 长度 {} 不是 20 的整数倍（分片哈希表被截断或损坏）",
            parsed.pieces_len
        ));
    }
    // BEP3 恒等式：分片数 = ceil(总大小 / piece length)。不满足的文件与哈希表对不上，
    // 任何客户端都会在校验阶段放弃这颗种。
    let pieces_expected =
        (parsed.size + parsed.piece_length - 1) / parsed.piece_length;
    let pieces_actual = (parsed.pieces_len / 20) as i64;
    if pieces_expected != pieces_actual {
        return Err(format!(
            "分片数与文件大小不符：按 {} 字节 / {} 字节片应为 {} 片，实际 {} 片",
            parsed.size, parsed.piece_length, pieces_expected, pieces_actual
        ));
    }
    if parsed.numfiles < 1 {
        return Err("文件清单为空".into());
    }
    if parsed.numfiles > MAX_TORRENT_FILES {
        return Err(format!(
            "文件数 {} 超过上限 {}",
            parsed.numfiles, MAX_TORRENT_FILES
        ));
    }
    for (path, len) in &parsed.files {
        if *len < 0 {
            return Err(format!("文件 {path} 的大小为负数"));
        }
        for seg in path.split('/') {
            if seg.is_empty() || seg == "." || seg == ".." {
                return Err(format!(
                    "文件路径不合法（含空段或 . / ..）：{path}"
                ));
            }
        }
        if path.contains('\\') || path.contains(':') || path.starts_with('/') {
            return Err(format!(
                "文件路径含穿越/绝对路径字符（\\ : 或开头 /）：{path}"
            ));
        }
    }
    // P-300（10-14 实测）：info 里没有 private=1 的种，客户端照常启用
    // DHT/PEX/LSD，内容对非会员敞开，而本站的访问与计量全在 announce 上。
    // 只在上传边界拦：改存量种的 info 会改 info_hash（swarm 断线），
    // 那属站方拍板项，不在这里顺手做。
    let root = super::parse::parse(&parsed.raw).ok().map(|(r, _)| r);
    let priv_flag = root
        .as_ref()
        .and_then(|b| b.get(b"info"))
        .and_then(|i| i.get(b"private"))
        .and_then(|v| v.as_int())
        .unwrap_or(0);
    if priv_flag != 1 {
        return Err("种子未标记私有：.torrent 的 info.private 必须为 1。\
私有站的访问与计量都在 announce 上，非私有种会被客户端拿去走 DHT/PEX \
分发。请用客户端另存为「私有种子」后重新发布"
            .into());
    }
    Ok(())
}

/// 站外取源键拒收（与 `build_download_torrent` 的下载侧剥离配对）：
/// 允许这类种子上架，等于让下载者从站外拿数据、绕过本站计量与会员边界。
pub fn reject_off_tracker_sources(bytes: &[u8]) -> Result<(), String> {
    let (root, _) = parse(bytes)?;
    for key in OFF_TRACKER_KEYS {
        if root.get(key).is_some() {
            return Err(format!(
                concat!(
                    ".torrent 含 {} 字段：本站禁止不经 tracker 的取源通道",
                    "（HTTP seed / DHT 提示），请重新打包后发布"
                ),
                String::from_utf8_lossy(key)
            ));
        }
    }
    Ok(())
}

/// 站点 source 旗（NP takeupload 口径）：上传时往 info 字典写入
/// `source = "[<值>]"` 并重打包。效果与主流 NexusPHP/UNIT3D 站一致——
/// 每站 hash 唯一，转载种在下载器里各占一行、各自向本站 tracker 独立
/// 汇报（上传/下载/H&R 归因干净，不存在混 tracker 双计数）。
///
/// 三 hash 口径：info_hash / raw_info_hash 随重打包改变并入库（tracker
/// 白名单、announce、piece cache 全按新 hash 服务）；pieces_hash 只由
/// info.pieces 原始字节决定，source 不在 pieces 里 ⇒ 跨站「同内容」
/// 指纹不变，IYUU/辅种匹配、重复发布策略（dup_policy）不受影响。
///
/// 刻意**不加** UNIT3D 的 `entropy` 随机字段：同内容重复上传 hash 仍相同，
/// info_hash 判重/幂等闸照常工作（U3D 靠人工审核兜判重，代价不可取）。
///
/// `raw` 整体替换为打旗后的字节：下游 INSERT / torrent_files 存档 /
/// 下载侧 `build_download_torrent` 全链自动一致，下载时 info 区间
/// 原样透传 ⇒ 客户端拿到的 hash 与上传入库严格相等。
pub fn stamp_source(
    mut parsed: ParsedTorrent,
    source: &str,
) -> Result<ParsedTorrent, String> {
    let (root, _) = parse(&parsed.raw)?;
    let mut pairs = match root {
        super::Bencode::Dict(p) => p,
        _ => return Err("root not dict".into()),
    };
    let info = pairs
        .iter()
        .find(|(k, _)| k == b"info")
        .map(|(_, v)| v)
        .ok_or("missing info dict")?
        .clone();
    let mut info_pairs = match info {
        super::Bencode::Dict(p) => p,
        _ => return Err("info not dict".into()),
    };
    let flag = format!("[{source}]").into_bytes();
    info_pairs.retain(|(k, _)| k != b"source");
    info_pairs.push((b"source".to_vec(), super::Bencode::Bytes(flag)));
    let new_info = super::Bencode::Dict(info_pairs);
    // 双 hash 重算：encode() 按 BEP3 键序产出规范化口径。stamp 后的 raw
    // 本身就是 encode 产物（键已序），原始字节口径与规范化口径恒等——
    // 老种子两侧分叉（键序非标准）的问题对新上传种不再存在。
    let mut info_buf = Vec::new();
    encode(&new_info, &mut info_buf);
    let mut h = Sha1::new();
    h.update(&info_buf);
    let ih = h.finalize();
    parsed.info_hash_hex = hex(&ih);
    parsed.raw_info_hash_hex = hex(&ih);
    // pieces_hash 不重算：source 键不影响 info.pieces 字节
    // 根字典重组：其余顶层键原样保留（announce-list 清洗是下载侧
    // build_download_torrent 的职责，这里不做越权处理）
    pairs.retain(|(k, _)| k != b"info");
    pairs.push((b"info".to_vec(), new_info));
    let new_root = super::Bencode::Dict(pairs);
    let mut out = Vec::new();
    encode(&new_root, &mut out);
    parsed.raw = out;
    Ok(parsed)
}
