//! pieces_hash 回填（0069）。
//! 从 jobs.rs 按域拆出。

use sqlx::PgPool;

/// 存量种子 pieces_hash 回填（0069）：扫描 pieces_hash IS NULL 的种子，读 raw 计算 SHA1(info.pieces)。
/// 每轮 ≤50 条（避免一次长事务拖垮 worker），全部处理完后自然空转。worker 内置极简
/// bencode 解析（只依赖「定位 info 字典 → 取 pieces 字节串」，不重复造完整编解码）。
pub async fn backfill_pieces_hash(db: &PgPool) -> anyhow::Result<u64> {
    let rows: Vec<(i64, Vec<u8>)> = sqlx::query_as(
        "SELECT t.id, f.raw FROM torrents t \
         JOIN torrent_files f ON f.torrent_id = t.id \
         WHERE t.pieces_hash IS NULL LIMIT 50",
    )
    .fetch_all(db)
    .await?;
    let mut n = 0u64;
    for (id, raw) in rows {
        if let Some(hash) = extract_pieces_hash(&raw) {
            sqlx::query(
                "UPDATE torrents SET pieces_hash = $1 WHERE id \
                 = $2 AND pieces_hash IS NULL",
            )
            .bind(hash)
            .bind(id)
            .execute(db)
            .await?;
            n += 1;
        } else {
            // 缺 pieces 字段的畸形种：写空串占位，避免每轮重复扫描
            sqlx::query(
                "UPDATE torrents SET pieces_hash = '' WHERE id \
                 = $1 AND pieces_hash IS NULL",
            )
            .bind(id)
            .execute(db)
            .await?;
        }
    }
    Ok(n)
}

/// 极简 bencode 遍历：返回 SHA1(info.pieces 原始字节) 的 hex。与 api 侧 bencode.rs 口径一致。
fn extract_pieces_hash(raw: &[u8]) -> Option<String> {
    use sha1::{Digest, Sha1};
    fn parse(buf: &[u8], pos: usize) -> Option<(BVal, usize)> {
        let rest = buf.get(pos..)?;
        match rest.first()? {
            b'i' => {
                let end = rest.iter().position(|&b| b == b'e')?;
                let n: i64 =
                    std::str::from_utf8(&rest[1..end]).ok()?.parse().ok()?;
                Some((BVal::Int(n), pos + end + 1))
            }
            b'l' => {
                let mut p = pos + 1;
                let mut items = Vec::new();
                while *buf.get(p)? != b'e' {
                    let (v, np) = parse(buf, p)?;
                    items.push(v);
                    p = np;
                }
                Some((BVal::List(items), p + 1))
            }
            b'd' => {
                let mut p = pos + 1;
                let mut pairs = Vec::new();
                while *buf.get(p)? != b'e' {
                    let (k, np) = parse(buf, p)?;
                    let BVal::Bytes(kb) = k else { return None };
                    let (v, np2) = parse(buf, np)?;
                    pairs.push((kb, v));
                    p = np2;
                }
                Some((BVal::Dict(pairs), p + 1))
            }
            b if b.is_ascii_digit() => {
                let colon = rest.iter().position(|&c| c == b':')?;
                let len: usize =
                    std::str::from_utf8(&rest[..colon]).ok()?.parse().ok()?;
                let start = pos + colon + 1;
                let end = start + len;
                if end > buf.len() {
                    return None;
                }
                Some((BVal::Bytes(buf[start..end].to_vec()), end))
            }
            _ => None,
        }
    }
    #[allow(dead_code)]
    enum BVal {
        Int(i64),
        Bytes(Vec<u8>),
        List(Vec<BVal>),
        Dict(Vec<(Vec<u8>, BVal)>),
    }
    let (root, _) = parse(raw, 0)?;
    let BVal::Dict(pairs) = root else { return None };
    let info = pairs.into_iter().find(|(k, _)| k == b"info")?.1;
    let BVal::Dict(info_pairs) = info else {
        return None;
    };
    let pieces = info_pairs.into_iter().find(|(k, _)| k == b"pieces")?.1;
    let BVal::Bytes(pieces) = pieces else {
        return None;
    };
    if pieces.is_empty() {
        return None;
    }
    let mut h = Sha1::new();
    h.update(&pieces);
    Some(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}
