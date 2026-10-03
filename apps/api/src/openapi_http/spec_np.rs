//! OpenAPI：兼容层（NexusPHP 口径）/ Torznab / RSS 路径表。
//! 组装入口在 spec.rs::build()；schema 在 spec_schemas.rs。

use serde_json::{json, Value};

use super::spec_util::{op, qp};

/// 兼容层 + 别名 + PT-Plugin-Plus + 下载凭证
pub(super) fn paths_compat() -> Value {
    json!({
        "/compat/nexusphp/user.json": { "get": op(
            "NP userdetails 口径账号信息", "兼容层(NP)",
            Some("含 passkey（工具用它拼 download.php 与 tracker 汇报）。\
    分享率恒非负，downloaded=0 时 ratio_infinite=true。"),
            vec![], "NpUser", "") },
        "/compat/nexusphp/torrents.json": { "get": op(
            "NP 口径种子列表（高级筛选）", "兼容层(NP)",
            Some("category 仍是站内 id；新增 category_name/category_np/\
    category_newznab、info_hash/pieces_hash。默认含零做种新种。"),
            vec![qp("page", "1-400", "integer"),
                 qp("pagesize", "≤50，默认 30", "integer"),
                 qp("keyword", "关键字（标题）", "string"),
                 qp("category", "站内分类 id", "integer"),
                 qp("search_area", "0=标题 1=简介 3=发布者 4=IMDb", "integer"),
                 qp("imdb", "IMDb id（等效 search_area=4）", "string"),
                 qp("alive", "0=全部(默认) 1=活种 2=断种", "integer"),
                 qp("promo", "free/x2/half/any/none，逗号多选", "string"),
                 qp("size_min", "字节数或 5GB/500MB/2TB", "string"),
                 qp("size_max", "同上", "string"),
                 qp("min_seeders", "做种数下界（含）", "integer"),
                 qp("max_seeders", "做种数上界（含）", "integer"),
                 qp("date_from", "YYYY-MM-DD（含当天）", "string"),
                 qp("date_to", "YYYY-MM-DD（含当天）", "string"),
                 qp("owner", "发布者用户名（模糊）", "string"),
                 qp("official", "1 = 仅官种", "string"),
                 qp("sort", "created/seeders/size/completed（可加 _asc）", "string"),
                 qp("tags", "tag_dict.id 逗号串", "string"),
                 qp("tag_mode", "any（默认）/ all", "string")],
            "NpTorrentList", "items + has_more 分页语义") },
        "/compat/nexusphp/torrent/{id}.json": { "get": op(
            "NP 口径种子详情", "兼容层(NP)", Some("含双指纹与 download 模板。"),
            vec![], "NpTorrentDetail", "") },
        "/compat/nexusphp/download.php": { "get": op(
            "NP 形状下载（passkey 鉴权，免 cookie）", "兼容层(NP)",
            Some("返回 .torrent，内嵌本人 announce；每 passkey 30 次/分钟。"),
            vec![qp("id", "种子 id（必填）", "integer"),
                 qp("passkey", "32 位 passkey（必填）", "string")],
            "-", "application/x-bittorrent") },
        "/compat/nexusphp/userdetails.php": { "get": op(
            "别名：userdetails.php?passkey= → 账号 JSON", "兼容层(NP)",
            Some("passkey 即身份（与 tracker/下载同源凭证）；30 次/分钟。"),
            vec![qp("passkey", "32 位 passkey（必填）", "string")],
            "NpUser", "与 user.json 同形状") },
        "/compat/nexusphp/getrss.php": { "get": op(
            "别名：getrss.php → 302 到 /rss/{passkey}", "兼容层(NP)",
            Some("其余筛选参数原样透传；限流在目标端点计。"),
            vec![qp("passkey", "32 位 passkey（必填）", "string")],
            "-", "302 重定向") },
        "/compat/nexusphp/takelogin.php": { "post": op(
            "别名：takelogin.php → 307 到 /auth/login", "兼容层(NP)",
            Some("保留 POST body；登录端点同时接受表单与 JSON。"),
            vec![], "-", "307 重定向") },
        "/compat/nexusphp/details.php": { "get": op(
            "别名：details.php → 302 到站点详情页", "兼容层(NP)",
            Some("不伪造 HTML：解析型工具请改用 /open/torrents/{id}。"),
            vec![qp("id", "种子 id（必填）", "integer")],
            "-", "302 重定向") },
        "/plugins/ptppUserInfo": { "get": op(
            "PT-Plugin-Plus 字段口径聚合端点", "兼容层(NP)",
            Some("插件用户卡的全部字段，含 passkey（拼下载链必须）。"),
            vec![], "PtppUser", "") },
        "/downloads/keys": { "post": op(
            "签发 30 分钟临时下载凭证", "兼容层(NP)",
            Some("把「下载」从长期 Token/passkey 里解耦，泄露窗口 30 分钟；\
    每用户 10 次/分钟。"),
            vec![], "DownloadKey", "key + download_url") },
        "/downloads/{torrent_id}": { "get": op(
            "凭证换 .torrent（免 Authorization）", "兼容层(NP)",
            Some("凭证与用户+种子绑定；每凭证 20 次/分钟。"),
            vec![qp("token", "fxk_ 凭证（必填）", "string")],
            "-", "application/x-bittorrent") },
    })
}

/// Torznab + RSS
pub(super) fn paths_media() -> Value {
    json!({
        "/torznab": { "get": op("Torznab caps", "Torznab",
            Some("规范的 <caps> 根节点 + <searching> 能力声明 + 分类映射；\
    Prowlarr/Jackett 直接可加。"),
            vec![], "-", "application/xml") },
        "/torznab/search": { "get": op("Torznab search", "Torznab",
            Some("支持 t=search|tvsearch|movie（season/ep、imdbid/tmdbid）\
    与 cat=Newznab 分类号；item 带完整 torznab:attr（含免费倍率）。"),
            vec![qp("apikey", "Token（或用 Authorization 头）", "string"),
                 qp("t", "search/tvsearch/movie", "string"),
                 qp("q", "关键字", "string"),
                 qp("season", "季（tvsearch）", "integer"),
                 qp("ep", "集（tvsearch）", "integer"),
                 qp("imdbid", "IMDb id（movie/search）", "string"),
                 qp("cat", "Newznab 分类号，逗号分隔", "string"),
                 qp("limit", "1-100，默认 50", "integer"),
                 qp("offset", "offset+limit ≤ 1000", "integer")],
            "-", "application/xml（RSS 2.0 + torznab 命名空间）") },
        "/rss/{passkey}": { "get": op("个性化 RSS 订阅", "RSS",
            Some("item 含 <enclosure> 下载直链（qB RSS/autobrr/Flexget 直接用）；\
    passkey 在路径里，30 次/分钟。"),
            vec![json!({ "name": "passkey", "in": "path",
                   "required": true, "schema": { "type": "string" } }),
                 qp("categories", "分类 id 逗号多选", "string"),
                 qp("mediums", "媒介 id 逗号多选", "string"),
                 qp("search", "关键字", "string"),
                 qp("official", "1 = 仅官种", "string"),
                 qp("paid", "1 = 仅免费（free/x2free）", "integer"),
                 qp("showrows", "1-200，默认 50", "integer"),
                 qp("linktype", "page = link 指详情页且标题仅名称", "string")],
            "-", "application/rss+xml") },
    })
}
