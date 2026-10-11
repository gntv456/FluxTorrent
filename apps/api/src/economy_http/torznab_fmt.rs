//! Torznab 出文本的纯格式化件（300 行门禁从 economy_http/recon.rs
//! 拆出）：feed 外壳、`torznab:attr` 单项、XML 转义。

/// Torznab feed 外壳（caps 之外的 search 响应仍是 RSS 2.0 + torznab 命名空间）。
pub(super) fn feed_xml(items: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<rss version=\"2.0\" xmlns:torznab=\"http://torznab.com/schemas/2015/feed\">\n\
<channel>\n\
  <title>FluxTorrent</title>\n\
  <description>FluxTorrent Torznab feed</description>\n\
{items}</channel>\n\
</rss>\n"
    )
}

/// 单条 `torznab:attr`。属性值是客户端解析的唯一入口 ——
/// 旧实现把 seeders/peers/size 写成裸标签，Sonarr/Radarr 一律读成 0。
pub(super) fn tattr(name: &str, value: &str) -> String {
    format!(
        "    <torznab:attr name=\"{name}\" value=\"{}\" />\n",
        xml_escape(value)
    )
}

pub(super) fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
