# FluxTorrent 开放 API 接入指南

> 面向第三方工具作者（ptool / PT-Plugin-Plus / PT-depiler / MoviePilot / Prowlarr /
> Jackett / Sonarr·Radarr / cross-seed / autobrr / RSS 客户端 / 移动壳）。
> 站点为自研 Rust 架构，对外暴露 **NexusPHP 字段口径兼容层**、**开放 API Token**、
> **Torznab 出口** 与 **RSS** 四条通道。
> 完整机器可读文档：`GET /api/v1/openapi.json`；架构自描述：`GET /api/v1/compat/meta`。
>
> **兼容承诺**：旧端点保留 ≥2 个版本周期；破坏性变更提前一个版本在 `compat/meta`
> 与本页公告（YemaPT 硬删接口为反面教材）。

## 1. 凭据

| 凭据 | 获取 | 用途 | 有效期 |
|---|---|---|---|
| **API Token**（`fxo_...`） | 网页端「我的 → API Token」自助签发 | 兼容层 / 开放 API / Torznab 全部端点；请求头 `Authorization: Token fxo_...`，**或** 查询参数 `?apikey=` | 180 天，每人 ≤3 枚，可吊销；`POST /me/tokens/refresh` 滚动续期 |
| **passkey**（32 位） | 个人页查看；兼容层 `user.json` 亦可读到 | `download.php` 下载、`getrss.php`/`/rss/{passkey}` 订阅、tracker announce、`userdetails.php` | 长期（可在个人页重置） |
| **临时下载凭证**（`fxk_...`） | `POST /downloads/keys` 用 API Token 换 | 单种子下载，免 Authorization | **30 分钟** |

限流：Token 默认 60 req/min（签发时可调 1-600）；passkey 下载 30 次/min；
凭证下载 20 次/min；凭证签发 10 次/min。

**限流可观测**：所有 Token 鉴权响应都带
`X-RateLimit-Limit` / `X-RateLimit-Remaining` / `X-RateLimit-Reset`（窗口剩余秒）；
超限返回 `429` + `code:1015` + `Retry-After: 60`。客户端请据此退避，不要盲目重试。

## 2. 用户信息

```
GET /api/v1/plugins/ptppUserInfo          # PT-Plugin-Plus 字段口径聚合端点（推荐）
Authorization: Token fxo_...
→ data: { id, name, bonus, uploaded, downloaded, seeding, leeching,
          seedingSize, invites, levelName, joinTime, messageCount, passkey, isLogged }
```

`passkey` 于 0267 加入：插件拼 `download.php?id=$id$&passkey=$passkey$` 时必须有值，
否则下载链会拼成空 passkey 而 401。语义与 NP 的 userdetails 页面内含本人 passkey 一致。

```
GET /api/v1/compat/nexusphp/user.json     # NP userdetails 口径
→ data: { id, username, uploaded, downloaded, seedbonus,
          class, class_name, ratio, ratio_infinite, ratio_display, passkey }
```

- `class` 为数字等级 id，`class_name` 为可读等级名（0267 新增）。
- 分享率恒非负：`downloaded=0 且 uploaded>0` 时 `ratio=0` 且 `ratio_infinite=true`、
  `ratio_display="∞"`（旧实现回 `-1.0`，工具会当负数直接显示）。

```
GET /api/v1/compat/nexusphp/userdetails.php?passkey={passkey}   # NP 别名（同上形状）
```

## 3. 种子列表与搜索

```
GET /api/v1/compat/nexusphp/torrents.json?page=1&pagesize=50&keyword=高等数学&category=3
→ data: { page, page_size, total_estimate, has_more, items: [
    { id, name, small_descr, seeders, leechers, completed, size(字节),
      added(Unix秒),
      category, category_name, category_np, category_newznab,
      medium, medium_name, promotion, promotion_name,
      info_hash, pieces_hash, free, official, sticky } ] }
```

- `pagesize` ≤50；`has_more=false` 即最后一页（无精确总数——主动牺牲 count 换性能）。
- **存活口径（0267 变更）**：默认返回**全部**过审种（含零做种新种）——
  新种在有人做种前也必须能被搜到，否则冷启动期工具搜索恒空。
  只要活种：`alive=1`；只要断种：`alive=2`；旧习惯 `include_dead=0` 等价 `alive=1`。
- **分类**：`category` 仍是站内 id（语义未变）；新增 `category_name`（可读名）、
  `category_np`（NexusPHP 4xx 号）、`category_newznab`（Newznab 标准号），
  工具无需自备映射表。
- **指纹**：`info_hash` / `pieces_hash` 于 0267 加入，供辅种/查重工具精确匹配。
- 详情：`GET /api/v1/compat/nexusphp/torrent/{id}.json`（含 descr/group_id/download 模板与双指纹）。

## 4. 下载（三种方式按需选择）

1. **passkey 直下**（NP 工具习惯）：`GET /api/v1/compat/nexusphp/download.php?id={id}&passkey={passkey}`
   → `.torrent`（内嵌个人 tracker 地址，免 cookie）。
2. **临时凭证**（推荐，泄露窗口 30 分钟）：
   `POST /api/v1/downloads/keys {"torrent_id": 123}` → `{key, download_url}`
   然后 `GET /api/v1/downloads/123?token=fxk_...`（免 Authorization）。
3. 网页会话下载：Bearer JWT（仅本站前端使用，第三方勿依赖）。

## 5. RSS

```
GET /api/v1/rss/{passkey}?categories=1,2&mediums=3&official=1&search=关键字&showrows=50
```

- **item 含 `<enclosure>` 直链**（0267 修复）：`url` 指向
  `download.php?id=…&passkey=…`，`type=application/x-bittorrent`。
  此前只有 `<link>` 指向网页、没有 enclosure，qBittorrent RSS 自动下载 /
  autobrr / Flexget 拿到的是 HTML 页面 → 整条刷流链路不可用。
- 默认 `<link>` 也指向下载直链（NP 口径）；`linktype=page` 时 `<link>` 指详情页
  且标题仅名称。**注意 `linktype` 管的是标题格式，不是链接类型。**
- 筛选参数：`categories`（逗号多选）/ `mediums` / `official` / `search` /
  `showrows`(1-200) / `paid`(1=仅免费) / `sec_{kind}` 六类型维度。
- 论坛 RSS：`GET /api/v1/rss/forum/{passkey}?forums=1,2`
- **NP 别名**：`GET /api/v1/compat/nexusphp/getrss.php?passkey={passkey}` → 302 到上面的地址
  （其余参数原样透传），供硬编码 `/getrss.php?passkey=` 的老脚本零改造接入。

## 6. Torznab（Prowlarr / Jackett / Sonarr / Radarr / cross-seed）

```
GET /api/v1/torznab                          # caps（无需鉴权）
GET /api/v1/torznab/search?apikey={token}&t=search&q=关键字&limit=50&offset=0
```

- caps 为规范的 `<caps>` 根节点，声明 `<searching>`
  的 `search` / `tv-search`（`q,season,ep`）/ `movie-search`（`q,imdbid`），
  分类按站点 `categories.newznab_id` 实际映射输出。
- 每条 item 带完整 `torznab:attr`：
  `category` / `size` / `seeders` / `leechers` / `peers` / `grabs` / `infohash` /
  `downloadvolumefactor` / `uploadvolumefactor`，促销种额外带 `tags`。
- `t=movie` 支持 `imdbid` / `tmdbid`；`t=tvsearch` 支持 `season`/`ep`
  （按标题 `SxxExx` 收窄）；`cat` 传 Newznab 分类号（命不中时返回空 feed，不静默忽略）。
- 存活口径同兼容层：**默认含零做种新种**。
- enclosure/link 中已替换为**调用方本人的真实 passkey**。

## 7. 增量新种流（autobrr / 自建推送器）

```
GET /api/v1/open/announces?since_id=0&limit=100
→ data: { items: [ TorrentSummary ], next_since_id, count }
```

返回 `id > since_id` 的过审种子（升序）。保存 `next_since_id` 作为下次游标即可做
「只推新种」的轮询，无需反复拉全量列表。无新种时返回空数组（正常态，不是 404）。

```
GET /api/v1/open/recent?limit=50        # 最新 N 枚（TorrentSummary[]）
```

`TorrentSummary` = `{ id, name, description, size, published_at, seeders, leechers,
category, category_name, category_np, category_newznab, info_hash, pieces_hash }`。

> 0267 修正：`/open/recent` 此前在默认 `guest_policy=all_private` 下**恒返空数组**。
> 该端点要求有效 API Token（调用者本就是本站注册成员），游客策略管的是未登录可见度，
> 不应拦已认证成员 —— 现改为对 Token 持有者正常返回。

## 8. 发种（Token 化）

```
POST /api/v1/open/torrents?category_id=1&name=...&small_descr=...
Authorization: Token fxo_...
Content-Type: multipart/form-data

file=<.torrent 字节，≤4MiB>       # 必填
nfo=<NFO 文本，≤1MiB>             # 可选
```

元数据走查询参数（与网页发种同一套字段）：`category_id`（必填）、`name`、
`small_descr`、`descr`、`anonymous`、`price`、`sections`（JSON 串）、`group_id` 等。

**幂等**：按 `info_hash` / `raw_info_hash` 判重，已存在时返回 `200` 且
`duplicate=true` 并附上既有 `id` —— 工具重试同一 `.torrent` 不会失败。
新建成功时返回 `{ id, approval_status, auto_approved, duplicate:false,
info_hash, pieces_hash, group_suggest }`。

权限与网页发种同源（`torrent.upload`），过审/免审/被拒禁发/扣费规则完全一致。

## 9. NP 别名路径

| 别名 | 行为 |
|---|---|
| `GET /compat/nexusphp/getrss.php?passkey=` | 302 → `/rss/{passkey}`（参数透传） |
| `POST /compat/nexusphp/takelogin.php`（表单） | 307 → `/auth/login`（保留 body） |
| `GET /compat/nexusphp/userdetails.php?passkey=` | 200 JSON（userdetails 口径） |
| `GET /compat/nexusphp/details.php?id=` | 302 → 站点详情页 |

登录端点（`POST /api/v1/auth/login`）**同时接受 JSON 与
`application/x-www-form-urlencoded`** —— 按 NP 习惯用表单登录的工具可直接对接。

## 10. PT-Plugin-Plus 收录

站方已备好收录材料（`.research/ptpp-config-template/`：`config.json` +
`fluxtorrent.js` 解析脚本），字段口径与 `ptppUserInfo` 一致，
并已包含 `passkey` 提取字段（缺它下载链会拼成空 passkey）。
工具用户可引用本页向 [pt-plugins/PT-Plugin-Plus](https://github.com/pt-plugins/PT-Plugin-Plus)
提交站点收录 PR；PT-depiler 同理。

---
*维护：FluxTorrent 团队 · 2026-10-02 · 变更公告同步 `compat/meta.endpoints` 与 `capabilities`*
