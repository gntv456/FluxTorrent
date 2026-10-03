//! OpenAPI 组件（0268）：安全方案与数据形状。
//! 单独成文件是因为它比路径表还长（300 行门禁）。

use serde_json::{json, Value};

/// 共享组件：安全方案 + 数据形状
pub(super) fn components() -> Value {
    let summary = |extra: Value| {
        json!({ "allOf": [
            { "$ref": "#/components/schemas/TorrentSummary" }, extra ] })
    };
    json!({
        "securitySchemes": {
            "apiToken": { "type": "apiKey", "in": "header",
                "name": "Authorization",
                "description": "值形如 `Token fxo_xxx`；兼容层与开放接口通用" },
            "apikeyQuery": { "type": "apiKey", "in": "query", "name": "apikey",
                "description": "同一枚 Token，Torznab 客户端习惯" },
            "passkeyQuery": { "type": "apiKey", "in": "query", "name": "passkey",
                "description": "下载/别名端点用；RSS 走路径段 /rss/{passkey}" },
        },
        "schemas": {
            "Envelope": { "type": "object", "properties": {
                "code": { "type": "integer" },
                "message": { "type": "string" },
                "data": {},
                "request_id": { "type": "string", "format": "uuid" } } },
            "TorrentSummary": { "type": "object", "properties": {
                "id": { "type": "integer", "format": "int64" },
                "name": { "type": "string" },
                "description": { "type": "string", "nullable": true },
                "size": { "type": "integer", "format": "int64" },
                "published_at": { "type": "integer",
                    "description": "Unix 秒" },
                "seeders": { "type": "integer" },
                "leechers": { "type": "integer" },
                "category": { "type": "integer" },
                "category_name": { "type": "string" },
                "category_np": { "type": "integer" },
                "category_newznab": { "type": "integer" },
                "info_hash": { "type": "string" },
                "pieces_hash": { "type": "string", "nullable": true } } },
            "TorrentSummaryList": { "type": "array", "items": {
                "$ref": "#/components/schemas/TorrentSummary" } },
            "AnnouncePage": { "type": "object", "properties": {
                "items": { "type": "array", "items": {
                    "$ref": "#/components/schemas/TorrentSummary" } },
                "next_since_id": { "type": "integer" },
                "count": { "type": "integer" } } },
            "CategoryDict": { "type": "object", "properties": {
                "categories": { "type": "array", "items": { "type": "object",
                    "properties": {
                        "id": { "type": "integer" },
                        "name": { "type": "string" },
                        "legacy_id": { "type": "integer" },
                        "newznab_id": { "type": "integer" } } } },
                "media": { "type": "array", "items": { "type": "object",
                    "properties": { "id": { "type": "integer" },
                        "name": { "type": "string" } } } } } },
            "NpTorrentItem": summary(json!({
                "small_descr": { "type": "string", "nullable": true },
                "completed": { "type": "integer" },
                "added": { "type": "integer" },
                "medium": { "type": "integer", "nullable": true },
                "medium_name": { "type": "string", "nullable": true },
                "promotion": { "type": "string", "nullable": true },
                "promotion_name": { "type": "string", "nullable": true },
                "free": { "type": "boolean" },
                "official": { "type": "boolean" },
                "sticky": { "type": "boolean" } })),
            "NpTorrentList": { "type": "object", "properties": {
                "page": { "type": "integer" },
                "page_size": { "type": "integer" },
                "total_estimate": { "type": "integer" },
                "has_more": { "type": "boolean" },
                "items": { "type": "array", "items": {
                    "$ref": "#/components/schemas/NpTorrentItem" } } } },
            "NpUser": { "type": "object", "properties": {
                "id": { "type": "integer" }, "username": { "type": "string" },
                "uploaded": { "type": "integer", "format": "int64" },
                "downloaded": { "type": "integer", "format": "int64" },
                "seedbonus": { "type": "integer" },
                "class": { "type": "integer" },
                "class_name": { "type": "string" },
                "ratio": { "type": "number" },
                "ratio_infinite": { "type": "boolean" },
                "ratio_display": { "type": "string" },
                "passkey": { "type": "string" } } },
            "NpTorrentDetail": { "type": "object", "properties": {
                "id": { "type": "integer" }, "name": { "type": "string" },
                "descr": { "type": "string", "nullable": true },
                "size": { "type": "integer", "format": "int64" },
                "seeders": { "type": "integer" },
                "leechers": { "type": "integer" },
                "completed": { "type": "integer" },
                "category": { "type": "integer" },
                "info_hash": { "type": "string" },
                "pieces_hash": { "type": "string", "nullable": true },
                "group_id": { "type": "integer", "nullable": true },
                "added": { "type": "integer" },
                "download": { "type": "string" } } },
            "PtppUser": { "type": "object", "properties": {
                "id": { "type": "integer" }, "name": { "type": "string" },
                "bonus": { "type": "integer" },
                "uploaded": { "type": "integer", "format": "int64" },
                "downloaded": { "type": "integer", "format": "int64" },
                "seeding": { "type": "integer" },
                "leeching": { "type": "integer" },
                "seedingSize": { "type": "integer", "format": "int64" },
                "invites": { "type": "integer" },
                "levelName": { "type": "string" },
                "joinTime": { "type": "string" },
                "messageCount": { "type": "integer" },
                "passkey": { "type": "string" },
                "isLogged": { "type": "boolean" } } },
            "DownloadKey": { "type": "object", "properties": {
                "key": { "type": "string" },
                "expires_at": { "type": "string" },
                "ttl_minutes": { "type": "integer" },
                "download_url": { "type": "string" } } },
            "UploadResult": { "type": "object", "properties": {
                "id": { "type": "integer", "nullable": true },
                "duplicate": { "type": "boolean" },
                "approval_status": { "type": "integer" },
                "auto_approved": { "type": "boolean" },
                "info_hash": { "type": "string" },
                "pieces_hash": { "type": "string" } } },
            "TorrentDetail": { "type": "object", "properties": {
                "id": { "type": "integer" }, "name": { "type": "string" },
                "descr": { "type": "string", "nullable": true },
                "numfiles": { "type": "integer" },
                "size": { "type": "integer", "format": "int64" },
                "seeders": { "type": "integer" },
                "leechers": { "type": "integer" },
                "files": { "type": "array", "items": { "type": "object",
                    "properties": { "index": { "type": "integer" },
                        "path": { "type": "string" },
                        "size": { "type": "integer" } } } },
                "mediainfo": { "type": "string", "nullable": true },
                "nfo": { "type": "string", "nullable": true },
                "sections": {},
                "hr_policy": { "type": "string", "nullable": true },
                "downloadvolumefactor": { "type": "number" },
                "uploadvolumefactor": { "type": "number" } } },
            "MeOverview": { "type": "object", "properties": {
                "uploaded": { "type": "integer", "format": "int64" },
                "downloaded": { "type": "integer", "format": "int64" },
                "ratio": { "type": "number" },
                "bonus": { "type": "integer" },
                "seeding": { "type": "integer" },
                "leeching": { "type": "integer" },
                "unread_messages": { "type": "integer" },
                "hr_open": { "type": "integer" },
                "class_id": { "type": "integer" } } },
            "SnatchList": { "type": "object", "properties": {
                "items": { "type": "array", "items": { "type": "object",
                    "properties": {
                        "torrent_id": { "type": "integer" },
                        "name": { "type": "string" },
                        "size": { "type": "integer", "format": "int64" },
                        "seeders": { "type": "integer" },
                        "leechers": { "type": "integer" },
                        "my_uploaded": { "type": "integer", "format": "int64" },
                        "my_downloaded": { "type": "integer", "format": "int64" },
                        "progress_percent": { "type": "integer" },
                        "seeding": { "type": "boolean" },
                        "leeching": { "type": "boolean" } } } },
                "count": { "type": "integer" } } },
            "HrList": { "type": "object", "properties": {
                "items": { "type": "array", "items": { "type": "object",
                    "properties": {
                        "torrent_id": { "type": "integer" },
                        "name": { "type": "string" },
                        "seeded_seconds": { "type": "integer" },
                        "required_seconds": { "type": "integer" },
                        "shortfall_seconds": { "type": "integer" },
                        "detected_at": { "type": "integer" },
                        "resolved_at": { "type": "integer", "nullable": true },
                        "open": { "type": "boolean" } } } },
                "count": { "type": "integer" } } },
            "MessageList": { "type": "object", "properties": {
                "items": { "type": "array", "items": { "type": "object",
                    "properties": {
                        "id": { "type": "integer" },
                        "from": { "type": "string" },
                        "subject": { "type": "string" },
                        "body": { "type": "string" },
                        "created_at": { "type": "integer" },
                        "read_at": { "type": "integer", "nullable": true },
                        "unread": { "type": "boolean" } } } },
                "count": { "type": "integer" } } },
            "TokenList": { "type": "array", "items": { "type": "object",
                "properties": {
                    "id": { "type": "integer" }, "name": { "type": "string" },
                    "scopes": { "type": "array", "items": { "type": "string" } },
                    "rate_per_min": { "type": "integer" },
                    "last_used_at": { "type": "string", "nullable": true },
                    "expires_at": { "type": "string", "nullable": true },
                    "revoked_at": { "type": "string", "nullable": true } } } },
            "TokenIssued": { "type": "object", "properties": {
                "id": { "type": "integer" }, "name": { "type": "string" },
                "token": { "type": "string" },
                "scopes": { "type": "array", "items": { "type": "string" } },
                "hint": { "type": "string" } } },
            "Generic": { "type": "object" },
        }
    })
}
