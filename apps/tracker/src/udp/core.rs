//! UdpTracker 核心：UDP 会话表、run 循环、connect 包处理与 action 分发。

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

use actix_web::web;
use tokio::net::UdpSocket;

use super::pkt::{
    rand_u64, ANNOUNCE_ACTION, CONNECT_ACTION, CONN_CAP, CONN_TTL,
    ERROR_ACTION, PROTOCOL_ID, SCRAPE_ACTION,
};

pub(super) struct ConnEntry {
    pub(super) id: u64,
    pub(super) at: Instant,
}

/// UDP 会话表：((ip, port) → connection_id)。DashMap 足够（无锁读）。
type ConnTable = dashmap::DashMap<(String, u16), ConnEntry>;

pub struct UdpTracker {
    pub(super) state: web::Data<crate::TrackerState>,
    pub(super) conns: ConnTable,
}

impl UdpTracker {
    pub fn new(state: web::Data<crate::TrackerState>) -> Self {
        Self {
            state,
            conns: ConnTable::new(),
        }
    }

    pub async fn run(self: Arc<Self>, bind: &str) -> anyhow::Result<()> {
        let sock = Arc::new(UdpSocket::bind(bind).await?);
        tracing::info!("flux-tracker UDP listening on {bind}");
        let mut buf = vec![0u8; 2048];
        loop {
            let (n, peer) = match sock.recv_from(&mut buf).await {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!(%e, "udp recv error");
                    continue;
                }
            };
            // 拷贝一份再交给任务：buf 会被下一个包复用
            let pkt = buf[..n].to_vec();
            let sock2 = sock.clone();
            let this = self.clone();
            tokio::spawn(async move {
                if let Some(resp) = this.handle(&pkt, peer).await {
                    let _ = sock2.send_to(&resp, peer).await;
                }
            });
        }
    }

    pub(super) fn err_pkt(transaction_id: u32, msg: &str) -> Vec<u8> {
        let mut out = Vec::with_capacity(16 + msg.len());
        out.extend_from_slice(&ERROR_ACTION.to_be_bytes());
        out.extend_from_slice(&transaction_id.to_be_bytes());
        out.extend_from_slice(msg.as_bytes());
        out
    }

    pub(super) fn check_conn(&self, peer: &SocketAddr, id: u64) -> bool {
        match self.conns.get(&(peer.ip().to_string(), peer.port())) {
            Some(c) => c.id == id && c.at.elapsed() < CONN_TTL,
            None => false,
        }
    }

    async fn handle(&self, pkt: &[u8], peer: SocketAddr) -> Option<Vec<u8>> {
        if pkt.len() < 16 {
            return None; // 连最小包长都不足：丢弃（不回包，防反射放大）
        }
        let action = u32::from_be_bytes(pkt[8..12].try_into().ok()?);
        let transaction_id = u32::from_be_bytes(pkt[12..16].try_into().ok()?);
        // BEP15：仅 connect 的头 8 字节是协议魔数；announce/scrape 头 8 字节是
        // connection_id（由 connect 会话校验覆盖），不能按魔数过滤。
        if action == CONNECT_ACTION {
            let proto = u64::from_be_bytes(pkt[0..8].try_into().ok()?);
            if proto != PROTOCOL_ID {
                return None; // 协议号不符：丢弃
            }
            return Some(self.connect(pkt, peer, transaction_id));
        }
        match action {
            ANNOUNCE_ACTION => Some(
                super::announce::announce(self, pkt, peer, transaction_id)
                    .await,
            ),
            SCRAPE_ACTION => Some(
                super::scrape::scrape(self, pkt, peer, transaction_id).await,
            ),
            _ => Some(Self::err_pkt(transaction_id, "未知 action")),
        }
    }

    fn connect(
        &self,
        pkt: &[u8],
        peer: SocketAddr,
        transaction_id: u32,
    ) -> Vec<u8> {
        if pkt.len() != 16 {
            return Self::err_pkt(transaction_id, "connect 包长无效");
        }
        // 会话表容量防护：满了先清过期再整体清（正常站点量级远达不到）
        if self.conns.len() >= CONN_CAP {
            self.conns.retain(|_, v| v.at.elapsed() < CONN_TTL);
            if self.conns.len() >= CONN_CAP {
                self.conns.clear();
            }
        }
        let conn_id = rand_u64();
        self.conns.insert(
            (peer.ip().to_string(), peer.port()),
            ConnEntry {
                id: conn_id,
                at: Instant::now(),
            },
        );
        let mut out = Vec::with_capacity(16);
        out.extend_from_slice(&CONNECT_ACTION.to_be_bytes());
        out.extend_from_slice(&transaction_id.to_be_bytes());
        out.extend_from_slice(&conn_id.to_be_bytes());
        out
    }
}
