# 生态工具兼容矩阵

> 对生态内热度最高的几类工具做**真机**验证。
> 0267（2026-10-02）修订：此前的「全绿」结论**过于乐观**——三条只验了「端点可达」，
> 没验「工具的消费路径」。修订后按第三方工具的真实用法逐项复测。
>
> 验证方式：`scripts/redteam_compat_0267.py`（48 项断言，本地 docker 栈实测）。
> 相关：[ecosystem.md](ecosystem.md)（收录视角）· `_doc/开放API接入指南.md`（字段与用法）

## 矩阵（0267 复测后）

| 工具 | 依赖链路 | 端点 | 状态 | 说明 |
|---|---|---|---|---|
| **PT-Plugin-Plus**（用户信息卡 + 搜索 + 下载） | 聚合端点 / 兼容列表 / passkey 下载 | `GET /plugins/ptppUserInfo`（Token）；`torrents.json`；`download.php` | ✅ | `passkey` 已加入聚合响应（此前缺失 → 插件下载链拼空 passkey 必 401）；收录模板已修 |
| **cross-seed**（Torznab 模式） | caps + search + download | `/torznab`、`/torznab/search`、`download.php` | ✅ | caps 合规（`<caps>` + `<searching>`）、item 带 `torznab:attr`（含 `infohash`） |
| **cross-seed**（RSS 模式） | RSS + download | `/rss/{passkey}` | ✅（0267 修复） | 此前 **❌**：item 无 `<enclosure>` 且 `<link>` 指向网页，工具拿到的是 HTML |
| **Prowlarr / Jackett**（索引器） | caps 解析 + Torznab 搜索 | `/torznab`、`/torznab/search?apikey=` | ✅（0267 修复） | 此前 **❌**：caps 根节点写成 `<torznab:search>`，Prowlarr 解析失败无法添加 |
| **Sonarr / Radarr** | tv-search / movie-search + 免费/双倍语义 | 同上 | ✅（0267 修复） | 此前 **❌**：item 缺 `torznab:attr`（读到 0 做种）、分类硬编码 8000、`t=tvsearch/movie` 与 season/imdbid 被静默忽略 |
| **pt_mate / NP 系移动端** | NP 口径 JSON | `user.json` / `torrents.json` | ✅ | 新增 `class_name` / `ratio_display`；`category` 语义未变（兼容） |
| **qBittorrent RSS 自动下载 / Flexget** | RSS enclosure | `/rss/{passkey}` | ✅（0267 修复） | 此前 **❌**（同 cross-seed RSS） |
| **autobrr**（实时抓新种） | IRC announce | — | ⚠️ 不适用 | 本站不提供 IRC；改用 `GET /open/announces?since_id=` 增量轮询 |
| **PT-depiler / MoviePilot**（自动发种） | Token 化发种 | `POST /open/torrents` | ✅（0267 新增） | 需 `upload` scope；按 `info_hash` 幂等；权限/过审/扣费与网页发种同源 |
| **老 NP 脚本**（硬编码路径） | 别名薄壳 | `getrss.php` / `takelogin.php` / `userdetails.php` / `details.php` | ✅（0267 新增） | 302/307 转发，不复制业务逻辑 |
| 自研工具 | 开放 API | `/openapi.json`、`/open/recent`、`/open/announces` | ✅ | 响应带 `X-RateLimit-*`；429 带 `Retry-After: 60` |
| **移动壳 / 全功能客户端** | 分类字典 / 深详情 / 我的数据 | `/open/categories`、`/open/torrents/{id}`、`/open/me/*` | ✅（0268 新增） | 此前 **❌**：只有账号汇总，做种/下载史/H&R/站内信要爬 HTML |
| **开发者（DX）** | 机器可读文档 | `/openapi.json`（27 路径，含组件 schema） | ✅（0268 修复） | 此前 **⚠️** 只覆盖 4 个 `/open/*` 路径，compat/Torznab/RSS 全不在 spec 里 |
| 兼容层高级筛选 | promo/size/seeders/date/sort/tags | `torrents.json` | ✅（0268 新增） | 与站内搜索**同一套归一器**（norm_promo/parse_size…），口径不漂移 |

## 已知口径（非缺陷）

- **对外列表默认含零做种新种**（0267 变更）：新种在有人做种前也必须能被工具搜到，
  否则冷启动期工具侧永远「零结果」、被误判成「对接坏了」。只要活种用 `alive=1`。
  ⚠️ 这意味着**成员可以用 Token 批量导出目录**（与站内浏览页可见度一致，受限流封顶）。
- **tracker 拒绝未注册的 info_hash**：探针/自测必须用真实种子（防幽灵 swarm）。
- **深翻页**：兼容层 `pagesize ≤50`；Torznab `offset+limit ≤1000`。
- **token 上限**：每人 3 枚有效开放 Token（轮换先 `POST /me/tokens/revoke`）。
- **鉴权头形态**：开放 API 是 `Authorization: Token fxo_...`（不是 Bearer）；
  会话 JWT 才是 Bearer；Torznab 客户端可用 `?apikey=`。
- **限流**：Token 默认 60/min（可调 1-600）、RSS/passkey 下载 30/min、
  凭证下载 20/min、凭证签发 10/min、发种 20/min。
  响应带 `X-RateLimit-Limit/Remaining/Reset`；429 带 `Retry-After: 60`。

## 安全要点（详见 `_doc/第三方工具适配视角-对接需求与改进建议-2026-10-02.md` §7）

- `api_tokens.scopes` 自 0267 起**真正强制**（`upload` 才能发种）；签发时白名单校验。
- passkey 类端点（RSS / userdetails.php）已补限流；凭据类响应统一 `no-store`。
- 表单登录（NP 习惯）带 Origin 闸，防 login CSRF。

## 复测方法

```bash
python scripts/redteam_compat_0267.py   # 48 项断言，本地栈实测
```
