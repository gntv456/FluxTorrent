//! 上传盖 source 旗（NP takeupload 口径，2026-11-25 站长拍板）。
//! 自 `publish_http/upload.rs` 拆出守 300 行门禁（基线 509 顶到 541）。
//!
//! 口径：入库前往 info 写 `source = "[tracker主机] 站名"` 并重算 hash。
//! 每站 hash 唯一，转载种在下载器里各占一行、只向本站 tracker 汇报
//! （PT 单站计量口径，混 tracker 双计数不存在）；pieces_hash 不变 ⇒
//! 辅种/判重指纹不受影响。判重必须在盖戳**之前**按用户上传的原始
//! hash 完成——盖戳后 parsed 全是新 hash 新字节，INSERT / outbox /
//! torrent_files / 下载侧自动一致。

use crate::bencode::ParsedTorrent;
use crate::errors::DomainError;
use crate::state::AppState;

/// 算出盖戳后的 parsed：tracker 主机 + 站名取配置，缺省回退
/// PUBLIC_TRACKER_URL / 主机自身。
pub(super) async fn stamped(
    state: &AppState,
    parsed: ParsedTorrent,
) -> Result<ParsedTorrent, DomainError> {
    let tracker_base =
        crate::repo::settings::read_base(&state.repo, "announce_url")
            .await
            .unwrap_or_else(|| {
                std::env::var("PUBLIC_TRACKER_URL")
                    .unwrap_or_else(|_| "http://127.0.0.1:7070".into())
            });
    // 主机段：scheme://host[:port]/… → host[:port]（source 旗与 passkey
    // 无关，只要能识别「来自哪个站」即可）
    let host = tracker_base
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or(&tracker_base)
        .to_string();
    let site = crate::repo::settings::read_base(&state.repo, "site_name")
        .await
        .unwrap_or_else(|| host.clone());
    crate::bencode::stamp_source(parsed, &format!("{host} {site}"))
        .map_err(DomainError::TorrentInvalid)
}
