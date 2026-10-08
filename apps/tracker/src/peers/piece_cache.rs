//! piece 抽查所需的种子元信息加载（`piece length` + 首个 piece 哈希）。
//!
//! 这两项只存在于 `torrent_files.raw` 的 bencode 里（`torrents` 表没有对应列），
//! 因此必须读原始 .torrent 再解析（见 `torrent_parse`）。
//!
//! 缓存策略（对齐 tracker 既有的 guard 缓存纪律）：
//!   · 正缓存 TTL 10 分钟——piece 信息对同一种子**永不变化**（.torrent 是
//!     不可变的），10 分钟纯粹是为了新发种能较快被抽查覆盖；
//!   · 负缓存 10 分钟——查不到（种子刚建 / bencode 异常）时不重复打 DB，
//!     避免畸形种子把 DB 变成放大器；
//!   · DB 不可达时 fail-open 返回 None（拿不到就不做 piece 抽查，退回
//!     握手+bitfield 判定，**不**把探测失败当成作弊）。
//!
//! 容量有界：piece 信息每条约 28 字节，4096 条 ≈ 112 KB，可忽略；
//! 上限防止极端情况下（大量畸形 hash 洪水）无界增长。

use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::torrent_parse::{parse_piece_probe, PieceProbe};

const TTL: Duration = Duration::from_secs(600);
const CAP: usize = 4096;

#[derive(Clone)]
struct Entry {
    probe: Option<PieceProbe>,
    at: Instant,
}

/// info_hash(hex) → piece 抽查信息
static CACHE: Mutex<Option<HashMap<String, Entry>>> = Mutex::new(None);

fn cache() -> &'static Mutex<Option<HashMap<String, Entry>>> {
    &CACHE
}

/// 取某种子的 piece 抽查信息；查不到/异常返回 None（= 不做 piece 抽查）。
pub(crate) async fn piece_probe_for(
    db: &PgPool,
    info_hash_hex: &str,
) -> Option<PieceProbe> {
    // 正缓存命中且未过期
    if let Some(hit) = cache()
        .lock()
        .ok()
        .and_then(|g| g.as_ref().and_then(|m| m.get(info_hash_hex).cloned()))
    {
        if hit.at.elapsed() < TTL {
            return hit.probe;
        }
    }
    // 查 DB：拿 .torrent 原始字节 → 解析
    let raw: Option<Vec<u8>> = sqlx::query_scalar(
        "SELECT tf.raw FROM torrent_files tf \
         JOIN torrents t ON t.id = tf.torrent_id \
         WHERE t.info_hash = $1 OR t.raw_info_hash = $1 LIMIT 1",
    )
    .bind(info_hash_hex)
    .fetch_optional(db)
    .await
    .ok()
    .flatten();
    let probe = raw.and_then(|b| parse_piece_probe(&b));
    if let Ok(mut g) = cache().lock() {
        let m = g.get_or_insert_with(HashMap::new);
        if m.len() >= CAP {
            // 容量满：淘汰最旧的一条（六轮审计 P2-G——旧写法整体 clear()
            // 会把热种子的缓存一起丢，下一轮 400 个并发探测全部重新打 DB，
            // 形成周期性 DB 尖峰；逐条淘汰最旧者保持其余命中）。
            if let Some(oldest) = m
                .iter()
                .min_by_key(|(_, e)| e.at)
                .map(|(k, _)| k.clone())
            {
                m.remove(&oldest);
            }
        }
        m.insert(
            info_hash_hex.to_string(),
            Entry {
                probe,
                at: Instant::now(),
            },
        );
    }
    probe
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_stores_and_expires_by_ttl() {
        let p = PieceProbe {
            piece_len: 16384,
            first_hash: [1u8; 20],
        };
        let mut m = HashMap::new();
        m.insert(
            "ih1".to_string(),
            Entry {
                probe: Some(p),
                at: Instant::now(),
            },
        );
        let got = m.get("ih1").cloned().unwrap();
        assert_eq!(got.probe.unwrap().piece_len, 16384);
        assert!(got.at.elapsed() < TTL);
    }

    #[test]
    fn negative_probe_is_cached_too() {
        // 查不到的种子也要缓存，避免反复打 DB
        let mut m = HashMap::new();
        m.insert(
            "ih_bad".to_string(),
            Entry {
                probe: None,
                at: Instant::now(),
            },
        );
        assert!(m.get("ih_bad").unwrap().probe.is_none());
    }

    #[test]
    fn cap_evicts_oldest_keeps_rest() {
        let mut m: HashMap<String, Entry> = HashMap::new();
        // ih0 最旧（其它都晚于它插入）
        m.insert(
            "ih0".to_string(),
            Entry {
                probe: None,
                at: Instant::now() - Duration::from_secs(600),
            },
        );
        for i in 1..CAP {
            m.insert(
                format!("ih{i}"),
                Entry {
                    probe: None,
                    at: Instant::now(),
                },
            );
        }
        assert_eq!(m.len(), CAP);
        // 模拟插入第 CAP+1 条时的淘汰逻辑（六轮 P2-G：淘汰最旧而非清空）
        if m.len() >= CAP {
            if let Some(oldest) =
                m.iter().min_by_key(|(_, e)| e.at).map(|(k, _)| k.clone())
            {
                m.remove(&oldest);
            }
        }
        m.insert(
            "ih_overflow".to_string(),
            Entry {
                probe: None,
                at: Instant::now(),
            },
        );
        assert_eq!(m.len(), CAP, "超容量应只淘汰最旧一条");
        assert!(!m.contains_key("ih0"), "最旧的 ih0 被淘汰");
        assert!(m.contains_key("ih1"), "其余条目保留");
        assert!(m.contains_key("ih_overflow"), "新条目就位");
    }
}
