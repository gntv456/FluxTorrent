# FluxTorrent 开放 API 接入指南

> 面向第三方工具作者（ptool / PT-Plugin-Plus / PT-depiler / MoviePilot / RSS 客户端 / 移动壳）。
> 站点为自研 Rust 架构，但对外暴露 **NexusPHP 字段口径的兼容层**与**开放 API Token** 两套通道。
> 完整机器可读文档：`GET /api/v1/openapi.json`；架构自描述：`GET /api/v1/compat/meta`。
>
> **兼容承诺**：旧端点保留 ≥2 个版本周期；破坏性变更提前一个版本在 `compat/meta` 与本页公告（YemaPT 硬删接口为反面教材）。

## 1. 凭据

| 凭据 | 获取 | 用途 | 有效期 |
|---|---|---|---|
| **API Token**（`fxo_...`） | 网页端「我的 → API Token」自助签发 | 兼容层/开放 API 全部 JSON 端点，请求头 `Authorization: Token fxo_...` | 180 天，每人 ≤3 枚，可随时吊销 |
| **passkey**（32 位） | 个人页查看 | `download.php` 形状下载、tracker announce、RSS | 长期（可在个人页重置） |
| **临时下载凭证**（`fxk_...`） | `POST /downloads/keys` 用 API Token 换 | 单种子下载，免 Authorization | **30 分钟** |

限流：Token 默认 60 req/min（签发时可调 1-600）；passkey 下载 30 次/min；凭证下载 20 次/min。超限返回 code 1015。

## 2. 用户信息

```
GET /api/v1/plugins/ptppUserInfo          # PT-Plugin-Plus 字段口径聚合端点（推荐）
Authorization: Token fxo_...
→ data: { id, name, bonus, uploaded, downloaded, seeding, leeching,
          seedingSize, invites, levelName, joinTime, messageCount, isLogged }
```

```
GET /api/v1/compat/nexusphp/user.json     # NP userdetails 口径（含 passkey/ratio/class）
```

## 3. 种子列表与搜索

```
GET /api/v1/compat/nexusphp/torrents.json?page=1&pagesize=50&keyword=高等数学&category=3
→ data: { page, page_size, total_estimate, has_more, items: [
    { id, name, small_descr, seeders, leechers, completed, size(字节),
      added(Unix秒), category, medium, promotion, free, official, sticky } ] }
```

- `pagesize` ≤50；`has_more=false` 即最后一页（无精确总数——主动牺牲 count 换性能，YemaPT 口径）。
- 详情：`GET /api/v1/compat/nexusphp/torrent/{id}.json`（含 descr/group_id/download 模板）。

## 4. 下载（三种方式按需选择）

1. **passkey 直下**（NP 工具习惯）：`GET /api/v1/compat/nexusphp/download.php?id={id}&passkey={passkey}` → `.torrent`（内嵌个人 tracker 地址）。
2. **临时凭证**（推荐，泄露窗口 30 分钟）：
   `POST /api/v1/downloads/keys {"torrent_id": 123}` → `{key, download_url}`
   然后 `GET /api/v1/downloads/123?token=fxk_...`（免 Authorization）。
3. 网页会话下载：Bearer JWT（仅本站前端使用，第三方勿依赖）。

## 5. 辅种（cross-seed / IYUU 类工具）

- 本站种子带 **pieces_hash**（`info.pieces` 的 SHA-1，跨站重打包场景比 info_hash 更鲁棒）。
- 批量反查：`GET /api/v1/open/recent` 拿最新 50 条；列表端点逐页拉全量。
- 上传幂等建议：先按 pieces_hash 查重再决定重试（对齐 YemaPT 文档惯例；上传 API 计划中，见 §7）。

## 6. RSS

`GET /rss/{passkey}?...`（分类/媒介/官种/付费/关键字筛选，与站内「获取 RSS」页生成器同参数）。

## 7. 路线图（未上线，勿依赖）

- `POST` 上传端点（multipart + 幂等建议）
- Torznab 出口（Prowlarr/Jackett/cross-seed 生态）
- msg/notice 端点（App 化预留）

## 8. PT-Plugin-Plus 收录

站方已备好收录材料（`config.json` + `fluxtorrent.js` 解析脚本模板），字段口径与朱雀 `ptppUserInfo` 模式一致。工具用户可引用本页向 [pt-plugins/PT-Plugin-Plus](https://github.com/pt-plugins/PT-Plugin-Plus) 提交站点收录 PR。

---
*维护：FluxTorrent 团队 · 2026-09-14 · 变更公告将同步 `compat/meta.endpoints`*
