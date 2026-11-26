//! gzip 豁免薄闸（2026-11-26 加速批）。
//!
//! main() 挂了 `Compress::default()` 后，browse 列表/公告/字典等大 JSON
//! 按客户端 Accept-Encoding 协商压缩（actix-web 默认 feature 自带
//! compress-gzip，零依赖变更）。但 actix 内置谓词只豁免 image/video，
//! 仍有两类响应不该压：
//! 1. `.torrent`（bencode 已是紧凑形态，再压白烧 CPU 不省字节）——
//!    网页单击口 download-link、批量口 batch-download、compat 的
//!    downloads/keys 与 nexusphp/download.php 共四口；
//! 2. SSE 未读流（me/messages/stream）——压缩缓冲会让 EventSource
//!    的逐帧推送整段卡住直到连接结束。
//!
//! 本件按前缀命中摘掉请求的 Accept-Encoding 头，下游 Compress 协商成
//! identity 原样透传。挂载顺序：wrap 注册序 = 执行逆序，故本 mw 在
//! Compress 之后注册、先于其执行，拦得到请求头。附件图/视频本就被
//! actix 内置谓词豁免；RSS 的 XML 值得压，不豁免。

/// 前缀与各口 utoipa 路径逐字核对过（download-link 用子串会被
/// `/torrents/{id}/download-link` 命中，前缀同样命中；batch-download 同理）。
const GZIP_EXEMPT_PREFIXES: &[&str] = &[
    "/api/v1/me/messages/stream",
    "/api/v1/torrents/batch-download",
    "/api/v1/downloads/",
    "/api/v1/compat/nexusphp/download.php",
    // /metrics 文本：Prometheus 拉取端，文本小，压了反而增加解析面
    "/metrics",
    // 网页单击下载口（路径参数在尾部，前缀比对到 /torrents/ 段即可：
    // 精确前缀 + contains 双闸避免误吞列表口）
    "/api/v1/torrents/",
];

/// download-link 是 `/torrents/{id}/download-link`，前缀闸没法精确到它，
/// 用子串兜底（列表/详情口都不含该字面量，无误伤面）。
const GZIP_EXEMPT_CONTAINS: &[&str] = &["/download-link"];

/// 薄闸入口：见模块注释。
pub async fn gzip_exempt_mw(
    mut req: actix_web::dev::ServiceRequest,
    next: actix_web::middleware::Next<impl actix_web::body::MessageBody>,
) -> actix_web::Result<
    actix_web::dev::ServiceResponse<impl actix_web::body::MessageBody>,
> {
    let p = req.path();
    let hit = GZIP_EXEMPT_PREFIXES.iter().any(|pre| p.starts_with(pre))
        || GZIP_EXEMPT_CONTAINS.iter().any(|sub| p.contains(sub));
    if hit {
        req.headers_mut()
            .remove(actix_web::http::header::ACCEPT_ENCODING);
    }
    next.call(req).await
}
