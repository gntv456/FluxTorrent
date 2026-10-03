//! OpenAPI 文档组装（0268）。
//!
//! 此前 `/openapi.json` 只覆盖 4 个 `/open/*` 路径 —— 兼容层、Torznab、RSS
//! 全不在机器可读 spec 里。开发者按惯例先拉 spec 再写适配器，看到半张图
//! 会误判「这站没这些能力」，然后被迫去读源码。spec 也是契约的一部分。
//!
//! 拆分：路径表在 spec.rs（开放数据/我的数据/凭据）与 spec_np.rs
//! （兼容层/Torznab/RSS），schema 在 spec_schemas.rs，组装器在 spec_util.rs。

use serde_json::{json, Map, Value};

use super::spec_np;
use super::spec_util::{op, qp};

fn paths_open() -> Value {
    json!({
        "/open/recent": { "get": op("最新种子（id 倒序）", "开放数据", None,
            vec![qp("limit", "1-200，默认 50", "integer")],
            "TorrentSummaryList", "Envelope<TorrentSummary[]>") },
        "/open/announces": { "get": op("增量新种流（只推新种轮询）", "开放数据",
            Some("返回 id > since_id 的过审种子（升序）。保存响应里的 \
    next_since_id 作下次游标。"),
            vec![qp("since_id", "上次游标，缺省 0", "integer"),
                 qp("since", "发布时间下界（Unix 秒）", "integer"),
                 qp("limit", "1-200，默认 100", "integer")],
            "AnnouncePage", "items + next_since_id + count") },
        "/open/categories": { "get": op("分类与媒介字典", "开放数据",
            Some("每条分类同时给站内 id / NexusPHP 4xx 号 / Newznab 标准号，\
    工具无需自备映射表。"),
            vec![],
            "CategoryDict", "categories + media") },
        "/open/torrents": { "post": op("Token 化发种（需 upload scope）", "发种",
            Some("multipart/form-data：file=<.torrent>（必填，≤4MiB）、\
    nfo=<文本>（≤1MiB）；元数据走查询参数。按 info_hash 幂等：\
    已存在返回 200 且 duplicate=true。"),
            vec![qp("category_id", "站内分类 id（必填）", "integer"),
                 qp("name", "标题（缺省用 .torrent 内名）", "string"),
                 qp("small_descr", "副标题", "string"),
                 qp("anonymous", "1 = 匿名发布", "string"),
                 qp("price", "付费下载定价，0=免费", "integer")],
            "UploadResult", "id/approval_status/duplicate/info_hash/pieces_hash") },
        "/open/torrents/{id}": { "get": op("种子深详情", "开放数据",
            Some("文件清单 / MediaInfo / 多维属性 / 促销倍率 / H&R 策略，\
    与站内详情页同一批数据。"),
            vec![qp("with_nfo", "1 = 附 NFO 全文（默认不带）", "string")],
            "TorrentDetail", "含 files[]") },
    })
}

fn paths_me() -> Value {
    json!({
        "/open/me/overview": { "get": op("我的面板聚合", "我的数据",
            Some("上传量/做种/未读/H&R 一条拿齐；自动化工具轮询这条即可感知账号变化。"),
            vec![], "MeOverview", "细项见下方四个端点") },
        "/open/me/seeding": { "get": op("我正在做种的清单", "我的数据", None,
            vec![qp("limit", "1-100，默认 50", "integer")],
            "SnatchList", "含 my_uploaded 与进度") },
        "/open/me/history": { "get": op("我的下载/抓取历史", "我的数据", None,
            vec![qp("limit", "1-100，默认 50", "integer")],
            "SnatchList", "按最近活跃排序") },
        "/open/me/hr": { "get": op("H&R 违约清单", "我的数据", None,
            vec![qp("status", "open（默认）/ all", "string"),
                 qp("limit", "1-100，默认 50", "integer")],
            "HrList", "含 shortfall_seconds") },
        "/open/me/messages": { "get": op("站内信（本人）", "我的数据", None,
            vec![qp("unread", "1 = 仅未读", "string"),
                 qp("limit", "1-100，默认 50", "integer")],
            "MessageList", "") },
    })
}

fn paths_token() -> Value {
    json!({
        "/me/tokens": {
            "get": op("列出我的 Token", "凭据", None, vec![], "TokenList", ""),
            "post": op("签发 Token（明文仅返回一次）", "凭据",
                Some("180 天、每人 ≤3 枚；scopes 白名单 read/upload，\
    read 恒隐含 —— 发种必须显式 upload。"),
                vec![qp("name", "Token 名称 1-50", "string"),
                     qp("scopes", "read / upload", "string"),
                     qp("rate_per_min", "1-600，默认 60", "integer")],
                "TokenIssued", ""),
        },
        "/me/tokens/revoke": { "post": op("吊销 Token", "凭据", None,
            vec![], "Generic", "") },
        "/me/tokens/refresh": { "post": op("滚动续期 180 天", "凭据",
            None, vec![], "Generic", "") },
    })
}

pub(super) fn build() -> Value {
    let mut paths = Map::new();
    for part in [
        paths_open(),
        paths_me(),
        paths_token(),
        spec_np::paths_compat(),
        spec_np::paths_media(),
    ] {
        if let Value::Object(m) = part {
            for (k, v) in m {
                paths.insert(k, v);
            }
        }
    }
    json!({
        "openapi": "3.1.0",
        "info": {
            "title": "FluxTorrent Open API",
            "version": "1.2.0",
            "description": "第三方接口全集：开放数据 / 兼容层（NexusPHP 口径）\
    / Torznab / RSS / Token 管理。鉴权三选一：Authorization: Token <fxo_>（推荐）、\
    ?apikey=<fxo_>（Torznab 客户端习惯）、passkey（下载与 RSS，URL 内传递）。\
    限流响应带 X-RateLimit-Limit/Remaining/Reset；429 带 Retry-After: 60。",
        },
        "servers": [{ "url": "/api/v1" }],
        "tags": [
            { "name": "开放数据" }, { "name": "发种" }, { "name": "我的数据" },
            { "name": "凭据" }, { "name": "兼容层(NP)" }, { "name": "Torznab" },
            { "name": "RSS" },
        ],
        "components": super::spec_schemas::components(),
        "security": [{ "apiToken": [] }],
        "paths": Value::Object(paths),
        "x-compat-notes": "兼容层/别名/torznab/rss 的字段口径另见 \
    GET /compat/meta（架构自描述）与 _doc/开放API接入指南.md。",
    })
}
