//! BT 握手/piece 探测测试（2026-11-28 从 bt_probe.rs 拆出守行数门禁）。
//! mock peer 基建为握手与 piece 两域共享，集成测试整体随迁本件；
//! 被测原语经 bt_probe 模块 pub(crate) 引用。
use super::super::bt_probe::{
    bitfield_has_piece, build_handshake, hex_to_20, probe_bt_handshake_seeded,
    probe_peer_id, probe_piece_hash, BITFIELD_ID, HANDSHAKE_LEN, INTERESTED_ID,
    MAX_PIECE_LEN, PIECE_ID, PSTR, PSTR_LEN, REQUEST_ID, RESERVED, UNCHOKE_ID,
};
use std::time::Duration;

#[test]
fn handshake_is_68_bytes_with_correct_layout() {
    let ih = [0x5Au8; 20];
    let mut seed = 42u64;
    let pid = probe_peer_id(&mut seed);
    let hs = build_handshake(&ih, &pid);
    assert_eq!(hs.len(), 68);
    assert_eq!(hs[0], 19); // pstrlen
    assert_eq!(&hs[1..20], PSTR); // "BitTorrent protocol"（19 字节）
    assert_eq!(&hs[20..28], &RESERVED); // 8 字节 reserved
    assert_eq!(&hs[28..48], &ih); // info_hash
    assert_eq!(&hs[48..68], &pid); // peer_id 20 字节
}

#[test]
fn probe_peer_id_is_random_and_not_identifiable() {
    // 随机化后：同一种子可复现（确定性好测），不同种子必不同，
    // 且不含可识别的固定前缀（开源威胁模型：探针不可被指纹）
    let a = probe_peer_id(&mut 1u64);
    let b = probe_peer_id(&mut 2u64);
    assert_eq!(a.len(), 20);
    assert_ne!(a, b, "不同种子的 peer_id 必须不同");
    assert_ne!(&a[..12], b"FLUXPROBE000");
    let mut seed = 7u64;
    assert_eq!(probe_peer_id(&mut seed), probe_peer_id(&mut 7u64));
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
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
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
    let mut sd = 42u64;
    let ok = probe_bt_handshake_seeded(
        &ip,
        port,
        &hex_of(&ih),
        Duration::from_secs(3),
        &mut sd,
    )
    .await;
    assert!(ok, "握手正确且有 bitfield 数据的真做种应判为可达");
}

#[tokio::test]
async fn probe_rejects_handshake_without_data() {
    // 关键用例：握手过了但拿不出 bitfield = 幽灵做种 → 拒绝
    let ih = [0x42u8; 20];
    let (ip, port) = spawn_mock(Mock::HandshakeNoData).await;
    let mut sd = 43u64;
    let ok = probe_bt_handshake_seeded(
        &ip,
        port,
        &hex_of(&ih),
        Duration::from_millis(1200),
        &mut sd,
    )
    .await;
    assert!(!ok, "只回握手但无 bitfield 数据的幽灵必须被拒");
}

#[tokio::test]
async fn probe_rejects_bare_listener() {
    // 裸 TCP 监听：accept 后不回任何 BT 数据
    let ih = [0x42u8; 20];
    let (ip, port) = spawn_mock(Mock::Silent).await;
    let mut sd = 44u64;
    let ok = probe_bt_handshake_seeded(
        &ip,
        port,
        &hex_of(&ih),
        Duration::from_millis(800),
        &mut sd,
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
    let mut sd = 45u64;
    let ok = probe_bt_handshake_seeded(
        &ip,
        port,
        &hex_of(&ih),
        Duration::from_secs(3),
        &mut sd,
    )
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
    let mut sd = 46u64;
    let ok = probe_bt_handshake_seeded(
        &ip,
        port,
        &hex_of(&ih),
        Duration::from_secs(3),
        &mut sd,
    )
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
/// mock 会**回读 request 的 index 并按请求的 piece 回包**——随机抽查
/// 位置下，请求与响应同 index 才是真实客户端行为。
async fn spawn_piece_mock(
    block: Vec<u8>,
    declare_piece0: bool,
) -> (String, u16) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
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
        // ③ 读 interested（完整帧 5 字节：<4B len=1><id=2>）→ ④ 回 bitfield。
        //    旧版只读 4 字节，interested 的 id 字节滞留流里，后续 request
        //    帧错位——旧版 mock 回写写死的 index=0 恰好掩盖了它。
        let mut interested = [0u8; 5];
        if sock.read_exact(&mut interested).await.is_err() {
            return;
        }
        let bf_bit = if declare_piece0 { 0xFFu8 } else { 0x00u8 };
        // 声明「全部 piece 都有」（多 piece 位图，随机 index 必命中）；
        // 不声明则全零。len 字段 = id(1) + 位图(4) = 5
        let bf = [0u8, 0, 0, 5, BITFIELD_ID, bf_bit, bf_bit, bf_bit, bf_bit];
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
        // ⑦ 回 piece（len=9+block, id=7, **index 回读请求值**, begin=0）
        let req_index = u32::from_be_bytes(req[5..9].try_into().unwrap());
        let mut msg = Vec::with_capacity(9 + block.len());
        msg.extend_from_slice(&((9 + block.len()) as u32).to_be_bytes());
        msg.push(PIECE_ID);
        msg.extend_from_slice(&req_index.to_be_bytes()); // index 同请求
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
        hashes: vec![sha1_of(&block); 32],
    };
    let (ip, port) = spawn_piece_mock(block, true).await;
    let mut sd = 100u64;
    let got = probe_piece_hash(
        &ip,
        port,
        &hex_of(&ih),
        &info,
        Duration::from_secs(3),
        &mut sd,
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
        // 期望的（不同的）哈希：全部 piece 都不匹配
        hashes: vec![[0x00u8; 20]; 32],
    };
    let (ip, port) = spawn_piece_mock(block, true).await;
    let mut sd = 100u64;
    let got = probe_piece_hash(
        &ip,
        port,
        &hex_of(&ih),
        &info,
        Duration::from_secs(3),
        &mut sd,
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
        hashes: vec![[0x00u8; 20]; 32],
    };
    let (ip, port) = spawn_piece_mock(vec![0u8; 16], false).await;
    let mut sd = 101u64;
    let got = probe_piece_hash(
        &ip,
        port,
        &hex_of(&ih),
        &info,
        Duration::from_millis(1200),
        &mut sd,
    )
    .await;
    assert_eq!(got, None, "未声明 piece 0 时应返回 None 而非判伪造");
}
