//! UDP tracker（BEP15 精简实现：connect / announce / scrape）。
//!
//! 设计取舍（与 HTTP announce 同一资源池、同一套防护）：
//! - 复用 TrackerState：passkey 缓存、ip_bans、agent_rules、限流、peer 表、
//!   Redis 事件流（计费走 emit_event，worker 零改动）。
//! - passkey 携带方式：BEP15 无 path，passkey 放 announce 的 tracker_id 字段
//!   （连入后原样回显；qbittorrent/Transmission 均支持在 URL ?passkey= 之外
//!   通过 tracker id 传凭证——本站 announce URL 形如
//!   udp://host:port/{passkey}，客户端会把 path 段填进 tracker_id）。
//! - connection_id 校验放宽为「按 (ip, port) 会话」：避免实现 BEP42 的
//!   IP 混淆 HLS（单机部署下伪造源 IP 的 UDP 反射风险由 connection_id
//!   一次性握手 + 60s 窗口缓解；生产可再启用 BEP42）。
//! - announce 复用 HTTP 版全部防护链（熔断/ip_bans/限流/passkey/agent_rules）。
//!
//! 按域拆分：pkt（包头/常量与 passkey 提取、PRNG）、core（会话表与
//! connect/run 包处理）、announce/scrape（两个业务 action 的处理）。

mod announce;
mod core;
mod pkt;
mod scrape;

pub use core::UdpTracker;

#[cfg(test)]
mod tests;
