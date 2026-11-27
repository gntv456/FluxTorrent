//! BT 协议握手探测（2026-10-07 P0-1 治本，自 probes.rs 邻近域新建）。
//!
//! 问题：原 connectable 探测只做 **TCP 三手**——只要目标端口有进程在 listen
//! 就判定「可达」。于是 `nc -l -p 6881` 这类裸监听即可伪装成做种客户端，
//! TCP 层完全无法分辨真假。
//!
//! 手段：对抽中的 peer 发一条标准 BT handshake（BEP3，68 字节固定长度），
//! 并要求对方回一条同样 68 字节的 handshake 响应，且响应里的 info_hash
//! 与被探测的种子一致。真做种客户端（qBittorrent / libtorrent / Transmission
//! / Deluge…）必然应答；裸 TCP 监听不会，且伪造响应需要知道 info_hash
//! 与协议格式——成本远高于收益。
//!
//! 判定结果与 TCP 层同义复用 connectable 三态：
//!   CONN_OK    —— TCP 通 **且** BT 握手应答且 info_hash 匹配（真做种）
//!   CONN_DEAD  —— 握手失败/超时/信息不匹配（不可用，视为不可信）
//!   CONN_UNTESTED —— 本轮未探测

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// BEP3 握手的协议串（19 字节，不含长度前缀）。长度前缀 19 单独写在
/// build_handshake 的首字节——写死 19 避免把 "\x13BitTorrent protocol"
/// 整段当数据再算一次长度（那会得到 1+20 的越界握手）。
pub(crate) const PSTR: &[u8] = b"BitTorrent protocol";
pub(crate) const PSTR_LEN: usize = 19;
pub(crate) const RESERVED: [u8; 8] = [0u8; 8];
/// 标准客户端握手长度：1+19 +8+20 +20 = 68
pub(crate) const HANDSHAKE_LEN: usize = 68;

/// tracker 探测的 peer_id：**每次握手随机生成**（开源反作弊威胁模型收口，
/// 2026-10-08）。旧版固定 "FLUXPROBE000"——注释自辩「便于站方识别」，但开源后
/// 同样便于作弊者识别：一条 peer_id 前缀规则即可对探针连接优待（回真握手/
/// 真 bitfield/真 piece），对普通 leecher 不供数据。随机化后探针与正常
/// leecher 在协议层不可区分。种子由调用方传入（probe_loop 的 xorshift 流）。
pub(crate) fn probe_peer_id(seed: &mut u64) -> [u8; 20] {
    let mut id = [0u8; 20];
    for chunk in id.chunks_mut(8) {
        let r = crate::http_track::probe_cfg::cheap_rand(seed).to_be_bytes();
        chunk.copy_from_slice(&r[r.len() - chunk.len()..]);
    }
    // 保持 Azureus 风格的可见前缀不必要——不可识别正是目的
    id
}

/// 构造 68 字节 handshake。
/// `info_hash` 必须是 20 字节原始值（不是 hex 字符串）。
pub(crate) fn build_handshake(
    info_hash: &[u8; 20],
    peer_id: &[u8; 20],
) -> [u8; HANDSHAKE_LEN] {
    let mut h = [0u8; HANDSHAKE_LEN];
    let mut i = 0;
    h[i] = PSTR_LEN as u8; // pstrlen = 19
    i += 1;
    h[i..i + PSTR_LEN].copy_from_slice(PSTR);
    i += PSTR_LEN;
    h[i..i + 8].copy_from_slice(&RESERVED);
    i += 8;
    h[i..i + 20].copy_from_slice(info_hash);
    i += 20;
    h[i..i + 20].copy_from_slice(peer_id);
    h
}

/// 纯 TCP 可达性（不含协议验证）。
///
/// 存在的意义（2026-10-08）：把「端口根本不通」与「端口开着但不响应明文
/// BT」区分开。前者是实锤不可信（DEAD）；后者只是**无法验证**（SUSPECT），
/// 典型是仅加密连接的客户端。两者若混为一谈，通用站会误伤这类用户。
pub(crate) async fn probe_tcp_reachable(
    ip: &str,
    port: u16,
    timeout: std::time::Duration,
) -> bool {
    matches!(
        tokio::time::timeout(timeout, TcpStream::connect((ip, port))).await,
        Ok(Ok(_))
    )
}

/// BT 握手可达性探测。
///
/// 步骤：TCP 连接 → 发 handshake → 读 68 字节响应 → 校验响应是合法
/// handshake 且其 info_hash 与目标种子一致。任一步失败/超时 → 不可用。
///
/// 分两阶段（2026-10-07 增补 bitfield 抽查）：
///   ① 握手：发 68B handshake，要求对端应答且 info_hash 匹配；
///   ② 有数据：发 interested 消息，诱使对端回 bitfield（BEP3 msg id 5）。
///      **声称做种（left=0）却没有 bitfield 数据 = 幽灵**：真做种者必有
///      至少一个 piece。只回握手不供数据的伪服务端过不了这一关——
///      这正是「握手通过但其实没数据」的补洞。
/// 两阶段都在同一 timeout 预算内。
pub(crate) async fn probe_bt_handshake(
    ip: &str,
    port: u16,
    info_hash_hex: &str,
    timeout: std::time::Duration,
) -> bool {
    probe_bt_handshake_seeded(ip, port, info_hash_hex, timeout, &mut 0).await
}

/// 带随机种子的握手探测（probe_loop 传入自己的 xorshift 流，探针身份随机）。
pub(crate) async fn probe_bt_handshake_seeded(
    ip: &str,
    port: u16,
    info_hash_hex: &str,
    timeout: std::time::Duration,
    seed: &mut u64,
) -> bool {
    // hex → 20 字节；非法直接判不可信（宁可保守）
    let Ok(ih) = hex_to_20(info_hash_hex) else {
        return false;
    };
    let pid = probe_peer_id(seed);
    let hs = build_handshake(&ih, &pid);
    let attempt = tokio::time::timeout(timeout, async {
        let mut stream = TcpStream::connect((ip, port)).await.ok()?;
        // ① 握手：发 handshake → 读 68B 响应
        stream.write_all(&hs).await.ok()?;
        stream.flush().await.ok()?;
        let mut buf = [0u8; HANDSHAKE_LEN];
        stream.read_exact(&mut buf).await.ok()?;
        // 校验响应：pstrlen(19) + info_hash 匹配
        if buf[0] as usize != PSTR_LEN {
            return None;
        }
        let info_off = 1 + PSTR_LEN + 8;
        if buf[info_off..info_off + 20] != ih {
            return None;
        }
        // ② 发 interested（len=1, id=2），诱使对端回 bitfield
        stream
            .write_all(&[0u8, 0, 0, 1, INTERESTED_ID])
            .await
            .ok()?;
        stream.flush().await.ok()?;
        // ③ 读一条 peer 消息，取出 bitfield（msg id 5）判定是否有数据
        //    消息帧：<4B len><1B id><payload>；只读到第一条含 bitfield 的为止。
        let mut has_piece = false;
        for _ in 0..4 {
            let mut lenb = [0u8; 4];
            if stream.read_exact(&mut lenb).await.is_err() {
                break;
            }
            let len = u32::from_be_bytes(lenb) as usize;
            if len == 0 || len > 1 << 20 {
                break; // 非法/超界长度，放弃
            }
            let mut body = vec![0u8; len];
            if stream.read_exact(&mut body).await.is_err() {
                break;
            }
            if body[0] == BITFIELD_ID {
                // bitfield payload：body[1..] 为位图。至少有一个 1 位即有数据。
                // 末字节末尾多余的空闲位不算（掩码裁剪）。
                if bitfield_has_piece(&body[1..]) {
                    has_piece = true;
                }
                break;
            }
            // 其它消息（keep-alive/choked/port…）继续等 bitfield
        }
        Some(has_piece)
    })
    .await;
    matches!(attempt, Ok(Some(true)))
}

/// interested 消息 id（BEP3）
pub(crate) const INTERESTED_ID: u8 = 2;
/// bitfield 消息 id（BEP3）
pub(crate) const BITFIELD_ID: u8 = 5;
/// unchoke 消息 id（BEP3）——请求 piece 前必须先被 unchoke
pub(crate) const UNCHOKE_ID: u8 = 1;
/// request 消息 id（BEP3）
pub(crate) const REQUEST_ID: u8 = 6;
/// piece 消息 id（BEP3）
pub(crate) const PIECE_ID: u8 = 7;

/// 单个 piece 的最大字节数。标准客户端 piece_length 上限常见 4 MiB；
/// 超过此值直接判失败，防止畸形 length 让我们分配巨量内存。
pub(crate) const MAX_PIECE_LEN: u32 = 8 * 1024 * 1024;

/// piece 级抽查：请求**随机一个** piece 并比对 SHA-1。
///
/// 这是「声称有数据」与「真有数据」的分界：bitfield 可以随便声明，
/// 但 piece 内容必须哈希正确才算数（真做种客户端从磁盘读真实数据，
/// 临时伪造的假数据过不了 SHA-1）。
///
/// 流程：握手 → interested → 等 bitfield（确认对方有该 piece）→ 等 unchoke
/// → 发 request(index=随机, begin=0, len=piece_len) → 收 piece → 比对 SHA-1。
///
/// 返回值语义：
///   `Some(true)`  = piece 哈希匹配（确认真有数据）
///   `Some(false)` = 拿到了 piece 但哈希不匹配（**伪造数据，实锤作弊**）
///   `None`        = 没能完成校验（对方不给数据/超时/畸形）——调用方应
///                   保守处理：**不要**据此判为作弊，只是不算通过抽查。
///
/// 抽查位置随机（2026-10-08 开源反作弊收口）：旧版恒查 piece 0——作弊者
/// 只需持有真实 piece 0（从任意真种子副本读一次）即可过检，其余全假。
/// 随机选 index 后，针对性预备任意单块的成本与全量做种相同。种子由
/// `piece_probe_for` 侧的哈希数组提供（`hashes[idx]`），请求与校验同源。
/// 每次仍只查一块：验证一个即可判定真伪，避免 tracker 出站带宽爆炸。
pub(crate) async fn probe_piece_hash(
    ip: &str,
    port: u16,
    info_hash_hex: &str,
    info: &super::torrent_parse::PieceProbe,
    timeout: std::time::Duration,
    seed: &mut u64,
) -> Option<bool> {
    if info.piece_len == 0 || info.piece_len > MAX_PIECE_LEN {
        return None;
    }
    if info.hashes.is_empty() {
        return None;
    }
    let ih = hex_to_20(info_hash_hex).ok()?;
    let pid = probe_peer_id(seed);
    let hs = build_handshake(&ih, &pid);
    let piece_len = info.piece_len;
    // 随机选 piece index（开源威胁模型：位置不可预测）
    let index = (crate::http_track::probe_cfg::cheap_rand(seed)
        % info.hashes.len() as u64) as usize;
    let expect_hash = info.hashes[index];
    let index_u32 = index as u32;
    let attempt = tokio::time::timeout(timeout, async {
        use tokio::io::AsyncReadExt as _;
        let mut stream = TcpStream::connect((ip, port)).await.ok()?;
        // ① 握手
        stream.write_all(&hs).await.ok()?;
        stream.flush().await.ok()?;
        let mut buf = [0u8; HANDSHAKE_LEN];
        stream.read_exact(&mut buf).await.ok()?;
        if buf[0] as usize != PSTR_LEN {
            return None;
        }
        if buf[1 + PSTR_LEN + 8..1 + PSTR_LEN + 28] != ih {
            return None;
        }
        // ② interested
        stream
            .write_all(&[0u8, 0, 0, 1, INTERESTED_ID])
            .await
            .ok()?;
        stream.flush().await.ok()?;

        let mut unchoked = false;
        // ③ 等 bitfield（确认对方有目标 piece）+ unchoke
        //    注意 `read_peer_msg` 返回的 body 已**剥离 id 字节**，对 bitfield
        //    而言 body[0] 起就是位图（piece i 的位在 byte[i/8] 的
        //    (0x80 >> i%8)）。
        for _ in 0..8 {
            let Some((id, body)) = read_peer_msg(&mut stream).await else {
                break;
            };
            match id {
                BITFIELD_ID => {
                    // 必须声明有目标 piece，否则抽查无从进行
                    let byte = (index / 8) as usize;
                    if body.len() <= byte
                        || body[byte] & (0x80u8 >> (index % 8)) == 0
                    {
                        return None;
                    }
                }
                UNCHOKE_ID => unchoked = true,
                _ => {}
            }
            if unchoked {
                break;
            }
        }
        if !unchoked {
            return None; // 未被 unchoke（正常客户端会 unchoke；不上传则不该算作弊）
        }
        // ④ request 目标 piece（随机 index）
        let mut req = Vec::with_capacity(17);
        req.extend_from_slice(&13u32.to_be_bytes()); // len = 1+4+4+4
        req.push(REQUEST_ID);
        req.extend_from_slice(&index_u32.to_be_bytes()); // index
        req.extend_from_slice(&0u32.to_be_bytes()); // begin
        req.extend_from_slice(&piece_len.to_be_bytes()); // length
        stream.write_all(&req).await.ok()?;
        stream.flush().await.ok()?;

        // ⑤ 收 piece（可能夹杂其它消息，最多等 8 条）
        for _ in 0..8 {
            let Some((id, body)) = read_peer_msg(&mut stream).await else {
                break;
            };
            if id != PIECE_ID {
                continue;
            }
            // piece payload（body 已剥离 id）：index(4) + begin(4) + block
            if body.len() < 8 {
                return None;
            }
            let idx = u32::from_be_bytes(body[0..4].try_into().ok()?);
            let begin = u32::from_be_bytes(body[4..8].try_into().ok()?);
            if idx != index_u32 || begin != 0 {
                continue; // 不是我们要的那块，继续等
            }
            let block = &body[8..];
            // 末块可能短于 piece_len；哈希按实际收到的全部字节算
            use sha1::{Digest, Sha1};
            let mut h = Sha1::new();
            h.update(block);
            let digest = h.finalize();
            return Some(digest.as_slice() == expect_hash);
        }
        None
    })
    .await;
    attempt.ok().flatten()
}

/// 读一条 peer message：`<4B len><1B id><payload(len-1 字节)>`，
/// 返回 `(id, payload)`。keep-alive（len=0）返回 `(0, vec![])`。
async fn read_peer_msg(stream: &mut TcpStream) -> Option<(u8, Vec<u8>)> {
    use tokio::io::AsyncReadExt as _;
    let mut lenb = [0u8; 4];
    stream.read_exact(&mut lenb).await.ok()?;
    let len = u32::from_be_bytes(lenb) as usize;
    if len == 0 {
        return Some((0, Vec::new())); // keep-alive
    }
    if len > MAX_PIECE_LEN as usize + 64 {
        return None; // 超界，畸形
    }
    let mut body = vec![0u8; len];
    stream.read_exact(&mut body).await.ok()?;
    let id = body[0];
    Some((id, body[1..].to_vec()))
}

/// bitfield 位图里是否至少有一个「我有」的 piece。
pub(crate) fn bitfield_has_piece(bits: &[u8]) -> bool {
    // 空闲尾位忽略：BitTorrent 规定最后一个字节未使用位为 0，
    // 正常客户端不会置位；若真有非零尾位也不采信（保守）。
    bits.iter().any(|b| *b != 0)
}

/// 40 字符 hex → 20 字节
pub(crate) fn hex_to_20(s: &str) -> Result<[u8; 20], ()> {
    let b = s.as_bytes();
    if b.len() != 40 {
        return Err(());
    }
    let mut out = [0u8; 20];
    for i in 0..20 {
        let hi = (b[i * 2] as char).to_digit(16).ok_or(())?;
        let lo = (b[i * 2 + 1] as char).to_digit(16).ok_or(())?;
        out[i] = ((hi << 4) | lo) as u8;
    }
    Ok(out)
}

#[cfg(test)]
#[path = "bt_probe_tests.rs"]
mod tests;
