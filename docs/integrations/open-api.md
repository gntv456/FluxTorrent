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
| **RSS/下载 Token**（`fxr_...`） | 网页端「获取 RSS」页或 `POST /rss/tokens` 创建（会话登录），明文仅创建时可见 | `GET /rss/{token}` 订阅、`getrss.php?token=` 别名、`download.php?id=&token=` 直下、`userdetails.php?token=`；撤销即整条订阅链失效 | 长期（每人 ≤5 枚，可吊销/轮换 `POST /rss/tokens/{id}/rotate`） |
| **临时下载凭证**（`fxk_...`） | `POST /downloads/keys` 用 API Token 换 | 单种子下载，免 Authorization | **30 分钟** |

> **passkey（32 位）已不作为 API 凭据**（v0.5.11 收口）：它只用于 tracker
> announce。RSS / download.php / getrss.php / userdetails.php 的旧参数名
> `passkey=` 仍可传，但**只接受 `fxr_` token 值**——传原始 32 位 passkey
> 一律 401/404。老脚本把参数值从 passkey 换成 fxr_ token 即可零改造迁移。

限流：Token 默认 60 req/min（签发时可调 1-600）；token 直下 30 次/min；
凭证下载 20 次/min；凭证签发 10 次/min。

**限流可观测**：所有 Token 鉴权响应都带
`X-RateLimit-Limit` / `X-RateLimit-Remaining` / `X-RateLimit-Reset`（窗口剩余秒）；
超限返回 `429` + `code:1015` + `Retry-After: 60`。客户端请据此退避，不要盲目重试。

## 2. 用户信息

```
GET /api/v1/plugins/ptppUserInfo          # PT-Plugin-Plus 字段口径聚合端点（推荐）
Authorization: Token fxo_...
→ data: { id, name, bonus, seedingPoints, bonusPerHour, uploaded, downloaded, seeding, leeching,
          seedingSize, uploads, invites, levelName, joinTime, messageCount, isLogged }
```

`bonusPerHour`（时魔）与 `uploads`（发布数）于 0.5.19 加入：插件用户卡的
「时魔」「发布数」两列此前恒为空，现与页面「我的魔力」/档案页同一口径。
`seedingPoints`（做种积分，1 积分 = 1 小时做种，`sum(snatches.seeded_seconds)/3600`）
于 0.5.19-dev.20 加入：PT-depiler 用户卡「积分」列的做种积分一行
（IPtppUserInfo.seedingPoints / NP 口径 seedingBonus）此前恒空。
**passkey 字段已移除**（收口批）：下载改走 fxr_ token 直下或 fxk_ 临时凭证（§4）。

```
GET /api/v1/compat/nexusphp/user.json     # NP userdetails 口径

→ data: { id, username, uploaded, downloaded, seedbonus, seedingPoints, seed_points, bonusPerHour, uploads,
          class, class_name, ratio, ratio_infinite, ratio_display }
```

- `class` 为数字等级 id，`class_name` 为可读等级名（0267 新增）。
- 分享率恒非负：`downloaded=0 且 uploaded>0` 时 `ratio=0` 且 `ratio_infinite=true`、
  `ratio_display="∞"`（旧实现回 `-1.0`，工具会当负数直接显示）。

```
GET /api/v1/compat/nexusphp/userdetails.php?token={fxr_token}   # NP 别名（同上形状）
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

- `pagesize` ≤50（`page` 1-400）；`has_more=false` 即最后一页（无精确总数——
  主动牺牲 count 换性能，别拿 `total_estimate` 算总页数）。全量同步的姿势：
  循环 page=1,2,3… 直到 `has_more=false`，配 `sort=created_desc` 保证遍历稳定。
- **翻页口径速查**：兼容层 `page/pagesize`（深翻页可用）；Torznab
  `limit`(1-100)/`offset`（**offset+limit ≤ 1000**，更深按 Torznab 惯例拒绝，
  要拉深换兼容层）；增量同步用 `/open/announces` 游标（`since_id`→
  `next_since_id`，只拉新种不用翻页）。
- **存活口径（0267 变更）**：默认返回**全部**过审种（含零做种新种）——
  新种在有人做种前也必须能被搜到，否则冷启动期工具搜索恒空。
  只要活种：`alive=1`；只要断种：`alive=2`；旧习惯 `include_dead=0` 等价 `alive=1`。
- **分类**：`category` 仍是站内 id（语义未变）；新增 `category_name`（可读名）、
  `category_np`（NexusPHP 4xx 号）、`category_newznab`（Newznab 标准号），
  工具无需自备映射表。
- **指纹**：`info_hash` / `pieces_hash` 于 0267 加入，供辅种/查重工具精确匹配。
- **高级筛选（0268）**：`promo`(free/x2/half/any/none，逗号多选)、`size_min`/`size_max`
  （字节数或 `5GB`/`500MB`）、`min_seeders`/`max_seeders`、`date_from`/`date_to`
  （`YYYY-MM-DD`）、`owner`、`official`、`sort`(created/seeders/size/completed，可加 `_asc`)、
  `tags`+`tag_mode`(any/all)。解析口径与站内搜索**同一套归一器**，不会各说各话。
- 详情：`GET /api/v1/compat/nexusphp/torrent/{id}.json`（含 descr/group_id/download 模板与双指纹）。
  要文件清单/MediaInfo/多维属性请用 `/open/torrents/{id}`（见 §8）。

## 4. 下载（三种方式按需选择）

1. **token 直下**（NP 工具习惯）：`GET /api/v1/compat/nexusphp/download.php?id={id}&token={fxr_token}`
   → `.torrent`（内嵌个人 tracker 地址，免 cookie）。旧参数名 `passkey=` 仍可传
   token 值，但**原始 32 位 passkey 一律 401**；也可用 `apikey={fxo_}`（开放 API
   Token，走 scope 闸）。
2. **临时凭证**（推荐，泄露窗口 30 分钟）：
   `POST /api/v1/downloads/keys {"torrent_id": 123}` → `{key, download_url}`
   然后 `GET /api/v1/downloads/123?token=fxk_...`（免 Authorization）。
3. 网页会话下载：Bearer JWT（仅本站前端使用，第三方勿依赖）。

## 5. RSS

```
GET /api/v1/rss/{fxr_token}?categories=1,2&mediums=3&official=1&search=关键字&showrows=50
```

- **鉴权是 `fxr_` RSS Token**（见 §1 凭据表），不是 passkey——passkey 形状
  （无前缀 32 位）在入口即被拒（404）。token 在网页「获取 RSS」页创建。
- **item 含 `<enclosure>` 直链**（0267 修复）：`url` 指向
  `download.php?id=…&token=…`（跟随订阅用的同一枚 fxr_ token，撤销它即
  整条订阅链失效），`type=application/x-bittorrent`。
  此前只有 `<link>` 指向网页、没有 enclosure，qBittorrent RSS 自动下载 /
  autobrr / Flexget 拿到的是 HTML 页面 → 整条刷流链路不可用。
- 默认 `<link>` 也指向下载直链（NP 口径）；`linktype=page` 时 `<link>` 指详情页
  且标题仅名称。**注意 `linktype` 管的是标题格式，不是链接类型。**
- 筛选参数：`categories`（逗号多选）/ `mediums` / `official` / `search` /
  `showrows`(1-200) / `paid`(1=仅免费) / `sec_{kind}` 六类型维度。
- 论坛 RSS：`GET /api/v1/rss/forum/{fxr_token}?forums=1,2`
- **NP 别名**：`GET /api/v1/compat/nexusphp/getrss.php?token={fxr_token}` → 302 到上面的地址
  （其余参数原样透传），供硬编码 `/getrss.php` 的老脚本零改造接入；旧字段名
  `passkey=` 仍可传 token 值，但不接受真实 passkey。

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
- enclosure/link 中的下载直链沿用**调用方自己的 API Token**（`fxo_`，
  `download.php?id=…&token=fxo_…`），不换出 tracker passkey。

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

## 8. 分类字典 · 深详情 · 我的数据（0268 新增）

```
GET /api/v1/open/categories
→ data: { categories: [ { id, name, legacy_id, newznab_id } ], media: [ { id, name } ] }
```

做分类下拉/筛选 UI 不再需要自备映射表。

```
GET /api/v1/open/torrents/{id}?with_nfo=1
→ data: { …种子主信息…, descr, numfiles, files:[{index,path,size}],
          mediainfo, sections, hr_policy, price, thanks_count,
          downloadvolumefactor, uploadvolumefactor, info_hash, pieces_hash }
```

深详情 = 站内详情页同一批数据；`with_nfo=1` 才带 NFO 全文（默认不带，省流量）。

```
GET /api/v1/open/me/overview      # 上传量/做种/未读/H&R 一条拿齐
GET /api/v1/open/me/seeding       # 我正在做种（含 my_uploaded / 进度）
GET /api/v1/open/me/history       # 我的下载/抓取历史
GET /api/v1/open/me/hr?status=open  # H&R 违约（含 shortfall_seconds）
GET /api/v1/open/me/messages?unread=1  # 站内信
```

全部只读、只暴露 Token 主人自己的数据（uid 一律取自 Token，不收路径参数），
`read` scope 即可。此前移动壳要爬 HTML 的四件事现在都有 JSON。

> **机器可读文档**：`GET /api/v1/openapi.json` 自 0268 起覆盖全部对外路径
> （开放数据 / 发种 / 我的数据 / 兼容层 / Torznab / RSS / Token 管理，共 20+ 路径），
> 含组件 schema —— 先拉 spec 再写适配器即可，不必读源码。

## 9. 发种（Token 化）

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

## 10. NP 别名路径

| 别名 | 行为 |
|---|---|
| `GET /compat/nexusphp/getrss.php?token=` | 302 → `/rss/{fxr_token}`（参数透传；旧字段名 `passkey=` 只接受 token 值） |
| `POST /compat/nexusphp/takelogin.php`（表单） | 307 → `/auth/login`（保留 body） |
| `GET /compat/nexusphp/userdetails.php?token=` | 200 JSON（userdetails 口径，不含 passkey） |
| `GET /compat/nexusphp/details.php?id=` | 302 → 站点详情页 |

登录端点（`POST /api/v1/auth/login`）**同时接受 JSON 与
`application/x-www-form-urlencoded`** —— 按 NP 习惯用表单登录的工具可直接对接。

## 11. PT-Plugin-Plus 收录

站方已备好收录材料（`.research/ptpp-config-template/`：`config.json` +
`fluxtorrent.js` 解析脚本），字段口径与 `ptppUserInfo` 一致；下载链的
`passkey=` 参数由用户在插件 PassKey 配置位填 `fxr_` RSS Token（服务端
只认 token 值，原始 passkey 一律 401）。

---
*维护：FluxTorrent 团队 · 2026-10-02 · 变更公告同步 `compat/meta.endpoints` 与 `capabilities`*
