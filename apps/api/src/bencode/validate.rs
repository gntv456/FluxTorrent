//! .torrent 的入库前结构校验（0288 从 torrent.rs 拆出，300 行门禁）。
//!
//! 为什么不做进 `parse_torrent`：那个函数还服务历史种子的再分发与站外导入，
//! 收紧它会让存量种子当场不可下载。校验只挂在**发种入口**上。

use super::parse;
use super::torrent::ParsedTorrent;

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
