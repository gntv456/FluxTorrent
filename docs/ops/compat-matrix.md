# 生态工具兼容矩阵

> E4 实证（2026-09-28，本地 docker 栈 v0.2.0+master 0237）：对生态内热度最高的
> 三类工具做协议级冒烟。结论：**全绿**。收录/README 宣传可直接引用本页。
>
> 相关：[ecosystem.md](ecosystem.md)（收录视角）· [openapi](../customize/README.md)（Token 发放）

## 冒烟环境与前提

- 本地栈：api 8080 / tracker HTTP 7070 / web 3000；库内 1 个过审活种（e4-compat-probe）
- 开放 API Token：`POST /api/v1/me/tokens`（`fxo_` 前缀，明文仅返回一次，
  每人 3 枚上限；鉴权头 `Authorization: Token <fxo_...>` 或 `?apikey=`）
- 兼容端点鉴权统一走开放 Token；RSS/下载/tracker 走 passkey（user.json 可自助读到本人 passkey）

## 矩阵

| 工具 | 依赖链路 | 端点 | 状态 | 证据 |
|---|---|---|---|---|
| **PT-Plugin-Plus**（浏览器插件，2.6k★ 生态） | 用户卡片聚合 | `GET /plugins/ptppUserInfo`（Token） | ✅ | code=0，13 字段（id/levelName/bonus/uploaded/downloaded/leeching/impressions…），`isLogged=true` |
| **cross-seed**（自动辅种，1.5k★） | RSS 拉种列表 + download.php 抓种 + announce 校验 | `GET /rss/{passkey}`；`GET /compat/nexusphp/download.php?id&passkey`；`GET :7070/announce/{passkey}` | ✅ | RSS 200 含 `<item>` 条目；下载返回 bencode 且 announce 注入本人 passkey（1376B）；announce 回包 bencode 正常 |
| **pt_mate / NP 系客户端**（移动端，502★） | NP 口径 JSON | `GET /compat/nexusphp/user.json`；`GET /compat/nexusphp/torrents.json?page=1` | ✅ | user.json 含 uploaded/downloaded/seedbonus/class/ratio/passkey；列表 items + has_more 分页语义 |
| Torznab 客户端（Jackett/Prowlarr） | apikey 查询参数 | 同上（`?apikey=` 兼容） | ✅（同链路） | require_token 原生支持 query apikey |
| 开放 API（自研工具） | Bearer/Token | `/open/*`（60 req/min 按 token 分桶限流） | ✅ | 0202 起在位，见开放 API 接入指南 |

## 已知口径（非缺陷）

- **活种视图**：`torrents.json` 与站内列表同口径——默认只出 `seeders > 0` 的过审种。
  新发种子在有人做种前不出现在兼容列表（与 NP torrents.php 行为一致）。冒烟时
  给测试种补 seeder 后复验通过。
- **深翻页**：兼容层用 `list_torrents_noclamp`（第 2 页起不钳 50），pagesize ≤50。
- **token 上限**：每人 3 枚有效开放 Token，测试轮换需先 `POST /me/tokens/revoke`。
- **鉴权头形态**：开放 API 是 `Authorization: Token fxo_...`（不是 Bearer）；
  会话 JWT 才是 Bearer。工具配置时别混。

## 复测方法

```bash
# 造一个活种（或用现成），然后按矩阵逐端点打：
curl -H "Authorization: Token $FXO" http://127.0.0.1:8080/api/v1/plugins/ptppUserInfo
curl -H "Authorization: Token $FXO" http://127.0.0.1:8080/api/v1/compat/nexusphp/torrents.json?page=1
curl "http://127.0.0.1:8080/api/v1/compat/nexusphp/download.php?id=<tid>&passkey=<pk>" -o t.torrent
curl "http://127.0.0.1:8080/api/v1/rss/<pk>"
```
