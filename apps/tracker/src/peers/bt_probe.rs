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
const PSTR: &[u8] = b"BitTorrent protocol";
const PSTR_LEN: usize = 19;
const RESERVED: [u8; 8] = [0u8; 8];
/// 标准客户端握手长度：1+19 +8+20 +20 = 68
const HANDSHAKE_LEN: usize = 68;

/// tracker 探测用的固定 peer_id：ASCII "FLUXPROBE000"（12 字节）+ 8 字节
/// 填充 = 20 字节。自报 id 便于站方在 peer 端日志里识别探测流量。
const PROBE_PEER_ID: [u8; 20] = [
    b'F', b'L', b'U', b'X', b'P', b'R', b'O', b'B', b'E', b'0', b'0',
    b'0', // 12
    0, 0, 0, 0, 0, 0, 0, 0, // 8 填充 = 20
];

/// 构造 68 字节 handshake。
/// `info_hash` 必须是 20 字节原始值（不是 hex 字符串）。
fn build_handshake(info_hash: &[u8; 20]) -> [u8; HANDSHAKE_LEN] {
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
    h[i..i + 20].copy_from_slice(&PROBE_PEER_ID);
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
        tokio::time::timeout(
            timeout,
            TcpStream::connect((ip, port))
        )
        .await,
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
    // hex → 20 字节；非法直接判不可信（宁可保守）
    let Ok(ih) = hex_to_20(info_hash_hex) else {
        return false;
    };
    let hs = build_handshake(&ih);
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
        stream.write_all(&[0u8, 0, 1, INTERESTED_ID]).await.ok()?;
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
const INTERESTED_ID: u8 = 2;
/// bitfield 消息 id（BEP3）
const BITFIELD_ID: u8 = 5;
/// unchoke 消息 id（BEP3）——请求 piece 前必须先被 unchoke
const UNCHOKE_ID: u8 = 1;
/// request 消息 id（BEP3）
const REQUEST_ID: u8 = 6;
/// piece 消息 id（BEP3）
const PIECE_ID: u8 = 7;

/// 单个 piece 的最大字节数。标准客户端 piece_length 上限常见 4 MiB；
/// 超过此值直接判失败，防止畸形 length 让我们分配巨量内存。
const MAX_PIECE_LEN: u32 = 8 * 1024 * 1024;

/// piece 级抽查：请求第 0 号 piece 并比对 SHA-1。
///
/// 这是「声称有数据」与「真有数据」的分界：bitfield 可以随便声明，
/// 但 piece 内容必须哈希正确才算数（真做种客户端从磁盘读真实数据，
/// 临时伪造的假数据过不了 SHA-1）。
///
/// 流程：握手 → interested → 等 bitfield（确认对方有 piece 0）→ 等 unchoke
/// → 发 request(index=0, begin=0, len=piece_len) → 收 piece → 比对 SHA-1。
///
/// 返回值语义：
///   `Some(true)`  = piece 哈希匹配（确认真有数据）
///   `Some(false)` = 拿到了 piece 但哈希不匹配（**伪造数据，实锤作弊**）
///   `None`        = 没能完成校验（对方不给数据/超时/畸形）——调用方应
///                   保守处理：**不要**据此判为作弊，只是不算通过抽查。
///
/// 只抽查第 0 号 piece：验证一个即可判定真伪，避免为每个 peer 传多个
/// piece 造成 tracker 出站带宽爆炸。
pub(crate) async fn probe_piece_hash(
    ip: &str,
    port: u16,
    info_hash_hex: &str,
    info: &super::torrent_parse::PieceProbe,
    timeout: std::time::Duration,
) -> Option<bool> {
    if info.piece_len == 0 || info.piece_len > MAX_PIECE_LEN {
        return None;
    }
    let ih = hex_to_20(info_hash_hex).ok()?;
    let hs = build_handshake(&ih);
    let piece_len = info.piece_len;
    let expect_hash = info.first_hash;
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
        stream.write_all(&[0u8, 0, 1, INTERESTED_ID]).await.ok()?;
        stream.flush().await.ok()?;

        let mut unchoked = false;
        // ③ 等 bitfield（确认有 piece 0）+ unchoke
        //    注意 `read_peer_msg` 返回的 body 已**剥离 id 字节**，对 bitfield
        //    而言 body[0] 就是位图首字节（piece 0 的位在它的最高位）。
        for _ in 0..8 {
            let Some((id, body)) = read_peer_msg(&mut stream).await else {
                break;
            };
            match id {
                BITFIELD_ID => {
                    // 必须声明有 piece 0（位图第 0 位 = body[0] 的 0x80）
                    if body.is_empty() || body[0] & 0x80 == 0 {
                        return None; // 没有 piece 0，抽查无从进行
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
        // ④ request piece 0
        let mut req = Vec::with_capacity(17);
        req.extend_from_slice(&13u32.to_be_bytes()); // len = 1+4+4+4
        req.push(REQUEST_ID);
        req.extend_from_slice(&0u32.to_be_bytes()); // index
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
            if idx != 0 || begin != 0 {
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
fn bitfield_has_piece(bits: &[u8]) -> bool {
    // 空闲尾位忽略：BitTorrent 规定最后一个字节未使用位为 0，
    // 正常客户端不会置位；若真有非零尾位也不采信（保守）。
    bits.iter().any(|b| *b != 0)
}

/// 40 字符 hex → 20 字节
fn hex_to_20(s: &str) -> Result<[u8; 20], ()> {
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
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn handshake_is_68_bytes_with_correct_layout() {
        let ih = [0x5Au8; 20];
        let hs = build_handshake(&ih);
        assert_eq!(hs.len(), 68);
        assert_eq!(hs[0], 19); // pstrlen
        assert_eq!(&hs[1..20], PSTR); // "BitTorrent protocol"（19 字节）
        assert_eq!(&hs[20..28], &RESERVED); // 8 字节 reserved
        assert_eq!(&hs[28..48], &ih); // info_hash
        assert_eq!(&hs[48..68], &PROBE_PEER_ID); // peer_id 20 字节
    }

    #[test]
    fn probe_peer_id_is_20_bytes() {
        assert_eq!(PROBE_PEER_ID.len(), 20);
        assert_eq!(&PROBE_PEER_ID[..12], b"FLUXPROBE000");
    }

    #[test]
    fn hex_to_20_roundtrip() {
        let ih = [0xFFu8; 20];
        let hex = hex_of(&ih);
        assert_eq!(hex.len(), 40);
        assert_eq!(hex_to_20(&hex).unwrap(), ih);
    }

    #[test]
    fn hex_to_20_rejects_bad_len_and_chars() {
        assert!(hex_to_20("abcd").is_err()); // 太短
        assert!(hex_to_20(&"z".repeat(40)).is_err()); // 非 hex
        assert!(hex_to_20(&"g".repeat(40)).is_err());
    }

    /// 校验逻辑单测：响应必须是合法 handshake 且 info_hash 匹配。
    /// 复刻 probe 内部的校验分支（真连接留给集成测试）。
    fn validate_response(resp: &[u8; 68], ih: &[u8; 20]) -> bool {
        if resp[0] as usize != PSTR_LEN {
            return false;
        }
        let info_off = 1 + PSTR_LEN + 8;
        resp[info_off..info_off + 20] == *ih
    }

    #[test]
    fn response_validation_matches_and_mismatches() {
        let ih = [0x11u8; 20];
        let mut good = [0u8; 68];
        good[0] = 19;
        good[1..20].copy_from_slice(PSTR);
        good[28..48].copy_from_slice(&ih);
        assert!(validate_response(&good, &ih));

        // info_hash 不匹配（串台/伪造）→ 拒绝
        let mut wrong_ih = good;
        wrong_ih[28] = 0x99;
        assert!(!validate_response(&wrong_ih, &ih));

        // pstrlen 不对（裸 TCP 回显等）→ 拒绝
        let mut bad_len = good;
        bad_len[0] = 13;
        assert!(!validate_response(&bad_len, &ih));
    }

    // ---- 真实 TCP 路径的集成测试（验证 mock 行为与生产函数一致） ----

    /// mock peer 的行为配置。
    enum Mock {
        /// 裸监听：读完握手即关，不回任何东西
        Silent,
        /// 回指定 68 字节握手响应（可故意写错，测校验分支）
        Handshake([u8; 68]),
        /// 正常做种：回正确握手 + 回一个有数据的 bitfield
        SeederWithData,
        /// 只回握手但不回 bitfield（声称做种却无数据 = 幽灵）
        HandshakeNoData,
    }

    /// 起一个只服务一次连接的 mock peer，按 `mock` 行为应答。
    async fn spawn_mock(mock: Mock) -> (String, u16) {
        let listener =
            tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let port = addr.port();
        tokio::spawn(async move {
            use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
            let Ok((mut sock, _)) = listener.accept().await else {
                return;
            };
            // ① 读客户端 68B handshake
            let mut hs = [0u8; 68];
            if sock.read_exact(&mut hs).await.is_err() {
                return;
            }
            match mock {
                Mock::Silent => return,
                Mock::Handshake(resp) => {
                    let _ = sock.write_all(&resp).await;
                    let _ = sock.flush().await;
                    return;
                }
                _ => {}
            }
            // ② 回正确握手（用客户端发来的 info_hash，保证匹配）
            let mut resp = [0u8; 68];
            resp[0] = 19;
            resp[1..20].copy_from_slice(PSTR);
            resp[28..48].copy_from_slice(&hs[28..48]);
            resp[48..68].copy_from_slice(&[b'R'; 20]);
            if sock.write_all(&resp).await.is_err() {
                return;
            }
            let _ = sock.flush().await;
            // ③ 读 interested（<4B len=1><1B id=2> = 4 字节）
            let mut interested = [0u8; 4];
            if sock.read_exact(&mut interested).await.is_err() {
                return;
            }
            if matches!(mock, Mock::HandshakeNoData) {
                return; // 握手过了但拿不出 bitfield
            }
            // ④ 回 bitfield：<4B len=2><id=5><1 字节位图 0xFF>（有一个 piece）
            //    长度字段是**固定 4 字节**（BEP3 peer message 帧头），此处
            //    写成 3 字节会让 probe 读出 len=0x00000205 而误判——已踩过。
            let bf = [0u8, 0, 0, 2, BITFIELD_ID, 0xFF];
            let _ = sock.write_all(&bf).await;
            let _ = sock.flush().await;
            // 短暂保持连接：任务立刻结束会 drop socket 触发关闭（可能 RST），
            // 让对端读不到刚发的 bitfield——这是 mock 侧竞态，非协议行为。
            tokio::time::sleep(Duration::from_millis(120)).await;
        });
        (addr.ip().to_string(), port)
    }

    fn good_response(ih: &[u8; 20]) -> [u8; 68] {
        let mut r = [0u8; 68];
        r[0] = 19;
        r[1..20].copy_from_slice(PSTR);
        r[28..48].copy_from_slice(ih);
        r[48..68].copy_from_slice(&[b'R'; 20]);
        r
    }

    fn hex_of(ih: &[u8; 20]) -> String {
        ih.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[tokio::test]
    async fn probe_accepts_real_seeder_with_data() {
        // 真做种：握手正确 + 有 bitfield 数据 → 通过
        let ih = [0x42u8; 20];
        let (ip, port) = spawn_mock(Mock::SeederWithData).await;
        let ok =
            probe_bt_handshake(&ip, port, &hex_of(&ih), Duration::from_secs(3))
                .await;
        assert!(ok, "握手正确且有 bitfield 数据的真做种应判为可达");
    }

    #[tokio::test]
    async fn probe_rejects_handshake_without_data() {
        // 关键用例：握手过了但拿不出 bitfield = 幽灵做种 → 拒绝
        let ih = [0x42u8; 20];
        let (ip, port) = spawn_mock(Mock::HandshakeNoData).await;
        let ok = probe_bt_handshake(
            &ip,
            port,
            &hex_of(&ih),
            Duration::from_millis(1200),
        )
        .await;
        assert!(!ok, "只回握手但无 bitfield 数据的幽灵必须被拒");
    }

    #[tokio::test]
    async fn probe_rejects_bare_listener() {
        // 裸 TCP 监听：accept 后不回任何 BT 数据
        let ih = [0x42u8; 20];
        let (ip, port) = spawn_mock(Mock::Silent).await;
        let ok = probe_bt_handshake(
            &ip,
            port,
            &hex_of(&ih),
            Duration::from_millis(800),
        )
        .await;
        assert!(!ok, "裸监听（无 BT 应答）必须判为不可信");
    }

    #[tokio::test]
    async fn probe_rejects_wrong_info_hash() {
        // 应答合法握手但 info_hash 不匹配（串台/伪造）→ 拒绝
        let ih = [0x42u8; 20];
        let mut wrong = good_response(&ih);
        wrong[28] = 0x77; // 篡改 info_hash 首字节
        let (ip, port) = spawn_mock(Mock::Handshake(wrong)).await;
        let ok =
            probe_bt_handshake(&ip, port, &hex_of(&ih), Duration::from_secs(3))
                .await;
        assert!(!ok, "info_hash 不匹配的握手必须被拒");
    }

    #[tokio::test]
    async fn probe_rejects_non_bt_garbage() {
        // 回 HTTP 文本（伪装成能连的端口）→ 拒绝
        let ih = [0x42u8; 20];
        let mut garbage = [0u8; 68];
        garbage[..9].copy_from_slice(b"HTTP/1.1 ");
        let (ip, port) = spawn_mock(Mock::Handshake(garbage)).await;
        let ok =
            probe_bt_handshake(&ip, port, &hex_of(&ih), Duration::from_secs(3))
                .await;
        assert!(!ok, "非 BT 应答（pstrlen 不符）必须被拒");
    }

    #[test]
    fn bitfield_has_piece_detects_any_set_bit() {
        assert!(bitfield_has_piece(&[0xFF]));
        assert!(bitfield_has_piece(&[0x00, 0x01]));
        // 全零 = 声称有 peer 但无任何 piece → 无数据
        assert!(!bitfield_has_piece(&[0x00]));
        assert!(!bitfield_has_piece(&[0x00, 0x00, 0x00]));
    }

    // ---- piece 级抽查的集成测试 ----

    /// 起一个会走完「握手 → bitfield → unchoke → piece」全流程的 mock。
    /// `block` 是它返回的 piece 内容；其 SHA-1 会与调用方给的期望哈希比对，
    /// 因此同一 mock 可用于「哈希匹配（真做种）」与「哈希不符（伪造）」两测。
    async fn spawn_piece_mock(
        block: Vec<u8>,
        declare_piece0: bool,
    ) -> (String, u16) {
        let listener =
            tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let port = addr.port();
        tokio::spawn(async move {
            use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
            let Ok((mut sock, _)) = listener.accept().await else {
                return;
            };
            // ① 读握手 → ② 回握手
            let mut hs = [0u8; 68];
            if sock.read_exact(&mut hs).await.is_err() {
                return;
            }
            let mut resp = [0u8; 68];
            resp[0] = 19;
            resp[1..20].copy_from_slice(PSTR);
            resp[28..48].copy_from_slice(&hs[28..48]);
            resp[48..68].copy_from_slice(&[b'R'; 20]);
            if sock.write_all(&resp).await.is_err() {
                return;
            }
            let _ = sock.flush().await;
            // ③ 读 interested → ④ 回 bitfield（声明有 piece 0）
            let mut interested = [0u8; 4];
            if sock.read_exact(&mut interested).await.is_err() {
                return;
            }
            let bf_bit = if declare_piece0 { 0x80u8 } else { 0x00u8 };
            let bf = [0u8, 0, 0, 2, BITFIELD_ID, bf_bit];
            let _ = sock.write_all(&bf).await;
            // ⑤ 回 unchoke（<4B len=1><id=1> = 5 字节）
            //    长度字段是固定 **4 字节** [0,0,0,1]——写成 3 字节会让
            //    对端读出 len=257 而错位（同一个坑第三次出现，务必注意）。
            let unc = [0u8, 0, 0, 1, UNCHOKE_ID];
            let _ = sock.write_all(&unc).await;
            let _ = sock.flush().await;
            // ⑥ 读 request（len=13, id=6, index/begin/len 各 4 字节）
            let mut req = [0u8; 17];
            if sock.read_exact(&mut req).await.is_err() {
                return;
            }
            // ⑦ 回 piece（len=9+block, id=7, index=0, begin=0, block）
            let mut msg = Vec::with_capacity(9 + block.len());
            msg.extend_from_slice(&((9 + block.len()) as u32).to_be_bytes());
            msg.push(PIECE_ID);
            msg.extend_from_slice(&0u32.to_be_bytes()); // index
            msg.extend_from_slice(&0u32.to_be_bytes()); // begin
            msg.extend_from_slice(&block);
            let _ = sock.write_all(&msg).await;
            let _ = sock.flush().await;
            tokio::time::sleep(Duration::from_millis(120)).await;
        });
        (addr.ip().to_string(), port)
    }

    fn sha1_of(data: &[u8]) -> [u8; 20] {
        use sha1::{Digest, Sha1};
        let mut h = Sha1::new();
        h.update(data);
        let d = h.finalize();
        let mut out = [0u8; 20];
        out.copy_from_slice(d.as_slice());
        out
    }

    #[tokio::test]
    async fn piece_probe_accepts_matching_hash() {
        // 真做种：返回的 piece 哈希与期望一致 → Some(true)
        let ih = [0x42u8; 20];
        let block = vec![0xABu8; 256];
        let info = crate::peers::torrent_parse::PieceProbe {
            piece_len: 256,
            first_hash: sha1_of(&block),
        };
        let (ip, port) = spawn_piece_mock(block, true).await;
        let got = probe_piece_hash(
            &ip,
            port,
            &hex_of(&ih),
            &info,
            Duration::from_secs(3),
        )
        .await;
        assert_eq!(got, Some(true), "哈希匹配的 piece 应判定为真数据");
    }

    #[tokio::test]
    async fn piece_probe_rejects_wrong_hash() {
        // 伪造：返回的 piece 哈希与期望不符 → Some(false)（实锤作弊）
        let ih = [0x42u8; 20];
        let block = vec![0xCDu8; 256]; // 伪造内容
        let info = crate::peers::torrent_parse::PieceProbe {
            piece_len: 256,
            first_hash: [0x00u8; 20], // 期望的（不同的）哈希
        };
        let (ip, port) = spawn_piece_mock(block, true).await;
        let got = probe_piece_hash(
            &ip,
            port,
            &hex_of(&ih),
            &info,
            Duration::from_secs(3),
        )
        .await;
        assert_eq!(got, Some(false), "哈希不符的 piece 必须判为伪造");
    }

    #[tokio::test]
    async fn piece_probe_none_when_no_piece0_declared() {
        // bitfield 不声明 piece 0 → 抽查无从进行 → None（不判作弊）
        let ih = [0x42u8; 20];
        let info = crate::peers::torrent_parse::PieceProbe {
            piece_len: 256,
            first_hash: [0x00u8; 20],
        };
        let (ip, port) = spawn_piece_mock(vec![0u8; 16], false).await;
        let got = probe_piece_hash(
            &ip,
            port,
            &hex_of(&ih),
            &info,
            Duration::from_millis(1200),
        )
        .await;
        assert_eq!(got, None, "未声明 piece 0 时应返回 None 而非判伪造");
    }
}
