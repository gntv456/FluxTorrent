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

    /// UDP 收包入口（0227 G31-E1）：TRACKER_UDP_WORKERS=N（缺省 1）时以
    /// SO_REUSEPORT 起 N 个 socket，内核对包做负载均衡——十万级并发 announce
    /// 的标准解。会话表 conns 在 Arc<Self> 内共享，多循环无一致性开销。
    /// N=1 走原 tokio bind 路径，行为不变。
    pub async fn run(self: Arc<Self>, bind: &str) -> anyhow::Result<()> {
        let mut workers = std::env::var("TRACKER_UDP_WORKERS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(1)
            .clamp(1, 16);
        // SO_REUSEPORT 仅 Unix：Windows 上第二个 socket bind 10048 直接退出
        #[cfg(not(unix))]
        if workers > 1 {
            tracing::warn!(
                "TRACKER_UDP_WORKERS>1 仅支持 Unix（SO_REUSEPORT）；本平台回退 1"
            );
            workers = 1;
        }
        if workers > 1 {
            use socket2::{Domain, Protocol, Socket, Type};
            let addr: std::net::SocketAddr = bind.parse()?;
            let is_v6 = addr.is_ipv6();
            for i in 0..workers {
                let sock = Socket::new(
                    if is_v6 { Domain::IPV6 } else { Domain::IPV4 },
                    Type::DGRAM,
                    Some(Protocol::UDP),
                )?;
                #[cfg(unix)]
                sock.set_reuse_port(true)?;
                if is_v6 {
                    sock.set_only_v6(false)?;
                }
                sock.bind(&addr.into())?;
                sock.set_nonblocking(true)?;
                let std_sock: std::net::UdpSocket = sock.into();
                std_sock.set_nonblocking(true)?;
                let sock = Arc::new(UdpSocket::from_std(std_sock)?);
                let this = self.clone();
                tracing::info!(worker = i + 1, workers, "UDP worker bound");
                tokio::spawn(async move { this.recv_loop(sock).await });
            }
            // 永不返回（各循环独立运行）
            std::future::pending::<()>().await;
        }
        let sock = Arc::new(UdpSocket::bind(bind).await?);
        tracing::info!("flux-tracker UDP listening on {bind}");
        self.recv_loop(sock).await;
        Ok(())
    }

    /// 单 socket 收包循环（原 run 循环体）
    async fn recv_loop(self: Arc<Self>, sock: Arc<UdpSocket>) {
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
            // connect 洪水防护（审计 10-06 第 8 条）：伪造 connect 会向会话表
            // 灌条目。进程内本地窗口（不落 Redis）每 IP 600/min——正常客户端
            // 60s 一次 connect，NAT 大出口仍有余量；超限静默丢弃（不回包，
            // 不给放大面）。
            if self
                .state
                .local
                .over(&format!("udp:connect:{}", peer.ip()), 600)
            {
                return None;
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
        // 会话表容量防护：满了先清过期；仍满（伪造 connect 洪水）淘汰最旧的
        // 25%——整体 clear 会把全部在线会话一并重置，攻击者可周期性反复触发
        // （审计 10-06 第 8 条）。排序 O(n log n) 只在极端路径发生。
        if self.conns.len() >= CONN_CAP {
            self.conns.retain(|_, v| v.at.elapsed() < CONN_TTL);
            if self.conns.len() >= CONN_CAP {
                let mut ages: Vec<_> = self
                    .conns
                    .iter()
                    .map(|e| (e.key().clone(), e.at))
                    .collect();
                ages.sort_unstable_by_key(|(_, at)| *at);
                let cut = ages.len() / 4;
                for (k, _) in ages.into_iter().take(cut) {
                    self.conns.remove(&k);
                }
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
