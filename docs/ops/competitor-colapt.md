# ColaPT / 可乐PT（colapt.com）深度分析

> 分析时间：2026-10-13（北京时间）  
> 方法：纯黑盒。DNS/证书/Whois + 证书透明度 + 前端 bundle 静态分析 + 协议探测 + 已登录状态的只读 API 遍历（未做任何写操作）。  
> 重要：站方公告自述"站点属于自己开发的新架构，处于内测阶段，BUG 可能较多"。以下为推断与实测并存，涉及内部行为的结论均已标注依据。

---

## 0. 一句话结论

**这是一个用自研框架「ReelCore」从零写的影视类 PT 站，不是 NexusPHP 魔改。** 代码量级、契约完整度、文档密度远超同规模新站，是当前与 FluxTorrent 最直接的同类竞品参照；但内容规模极小（94 用户 / 18 条目 / 7 个有完成数种子），选型上做了几个和 NP/Gazelle 都不同的激进决定（媒体中心模型、促销 intent、发布信用免审、前后端契约双重校验）。

| 维度   | 结论                                                                                                   |
| ---- | ---------------------------------------------------------------------------------------------------- |
| 定位   | 影视/剧集为主（国产剧 + 日番 + 欧美电影），中文字幕压制组生态，站点语言全中文                                                           |
| 生命周期 | 域 2026-09-29 注册，**至今 14 天**；站方公告"处于内测阶段"，当前模式为邀请制                                                    |
| 规模   | 94 用户（/leaderboards total=94）、18 个 media、7 个种子进入达成榜、0 个 series、2 个论坛主题、0 个求种                         |
| 架构   | Vue3 SPA + 自研 API（`X-Reelcore-Api-Version: 13.2.0`），**已公布 248 路径 / 312 操作 / 469 schema 的完整 OpenAPI** |
| 部署   | 站点走 Cloudflare；tracker 独立主机在香港（AS55933 Cloudie/Fastmos），非 CF 代理                                      |
| 开放性  | 全站需登录；未登录访问任何路径 302 → `/login?redirect=...`；所有列表类 API 返回 401                                         |

---

## 1. 基础设施与部署拓扑

```
客户端 ──► Cloudflare (NS: lia/trevor.ns.cloudflare.com)
             │  A/AAAA 104.21.47.92 / 172.67.146.96 / 2606:4700:...
             ├─► www.colapt.com        静态资源 + /api/v1/*（同一后端，SPA fallback）
             └─► tracker.colapt.com    ⚠ 裸解析，不过 CF
                    154.89.149.3 + 2404:8c80:82:1059::74
                    AS55933 Cloudie Limited / Fastmos Co Limited
                    香港 · Kowloon · hosting
```

**要点：**

- **tracker 与 Web 站分离部署**，且 tracker 的 DNS 记录**未走 Cloudflare**（直连香港主机）。好处是 announce 不受 CF 回源策略干扰，代价是源 IP 与主机商直接暴露。
- tracker 只暴露两条路由：`/announce`、`/scrape`，其余一律返回 `not found`；`/stats` 返回 403。
- **passkey 放在 query string**：`/announce?passkey=xxx`（`auth_key` 被显式拒绝：`auth_key is not allowed`）。  
  scrape 报错文案里写死了 `https://tracker.colapt.com`，说明 passkey 与 tracker 域名强绑定。
- 证书：crt.sh 只有 4 条记录，`www` / `tracker` 各一；签发方同时出现 Let's Encrypt 与 ZeroSSL——说明 tracker 那一端很可能不是 CF Universal SSL，而是自己在源站签发（与 DNS 裸解析互相印证）。
- 无 `sitemap.xml`、无 `manifest.webmanifest`（有 apple-touch-icon 但没做 PWA）、无 `security.txt`。
- **robots.txt 全是注释**：只有欧盟 CDSM 第 4 条的 content-signals 说明，**一行有效指令都没有**（既没 `User-agent` 也没 `search:`/`ai-train:` 值）。等于默认放行所有抓取——对一个 PT 站来说这是明显的疏漏。

---

## 2. 前端架构

| 项     | 实测                                                                                                                         |
| ----- | -------------------------------------------------------------------------------------------------------------------------- |
| 框架    | Vue 3（`runtime-core.esm-bundler` / `runtime-dom`），Vite 构建，`crossorigin` modulepreload                                      |
| UI 方案 | 无第三方 UI 库痕迹，手写 CSS + CSS 变量做主题（3 套：`minimal` / `cola-cream-red` / `cola-black-red`，localStorage key `reelcore.theme`，默认黑红） |
| 代码分割  | **130 个 chunk**，按路由懒加载；入口 `index` 仅 5.7KB，`bootstrap` 19KB(gzip)                                                           |
| 总体积   | 全部 JS 合计 2.4MB（未压缩）                                                                                                        |
| 亮点    | 把**整份 OpenAPI 契约 + ajv 预编译校验器**打进了 `session-*.js`（原始 875KB / gzip 130KB），前端在运行时直接校验服务端响应                                   |
| 国际化   | 单语中文，文案写死在组件里（含大量产品决策说明）                                                                                                   |
| 降级    | `<noscript>` 里写了一段很长的说明：解释为什么必须启用 JS（包括人机验证在本地计算这件事），并声明"不加载任何第三方验证码脚本，因此不为无脚本浏览器放宽验证"                                     |

**SPA 回退策略**：未知路径不是 404，而是 302 到 `/login?redirect=<原路径>`（服务端按会话判断，已登录则交给前端路由）。

---

## 3. 后端契约：这是全站最有价值的部分

### 3.1 基本约定

- 统一前缀 `/api/v1`，响应头 `X-Reelcore-Api-Version: 13.2.0`（per-operation 另有 `x-contract-version`，从 3.4.0 到 13.2.0 共 27 个版本标记）。
- **统一错误信封**：`{"error":{"code":"...","fields":{},"message":"...","request_id":"..."}}`，`request_id` 与响应头 `X-Request-Id` 一致。已实测到的 code：`authentication_required` / `permission_denied` / `route_not_found` / `api_key_required` / `api_key_scope_denied` / `invalid_authorization_id` / `invalid_catalog_query`。排错体验很好。
- 会话：`Set-Cookie: reelcore_session=...; Path=/api/v1; Max-Age=86400; HttpOnly; Secure; SameSite=Strict` + 一个 `reelcore_session_marker` 影子 cookie（Path=/）。写操作还需 `X-CSRF-Token`。

### 3.2 三重写闸门（比一般 PT 程序严格）

| 闸门                                | 覆盖操作                             | 实测/契约                                                                  |
| --------------------------------- | -------------------------------- | ---------------------------------------------------------------------- |
| `Origin` 头强校验                     | **148** 个操作（`x-origin-required`） | 包括登录、登出、全部 mutation                                                    |
| `Idempotency-Key` 头（16–128 位可见字符） | **74** 个操作                       | 发邀请、发消息、上传种子、发评论、投稿等都要求幂等键                                             |
| 期望 revision（乐观并发）                 | **17** 个操作                       | 修改 points rules / HR rules / publishing / traffic / profile 等"先读后写"类配置 |

### 3.3 权限模型：37 个 key + 双轨

- `x-permission` 共 37 个不同值（另有 82 个操作继承默认 session）。典型：`catalog.read`(50) / `user.manage`(26) / `torrent.upload`(15) / `accounting.adjust`(10) / `promotion.manage`(9) / `torrent.moderate`(8) / `audit.read`(7) / `staffmail.manage`(6)。
- 权限是**每次请求结算**的：`/me` 给的 `permissions[]` 与 `/me/site-permissions` 给的 `effective[]` 分离，允许多来源叠加，也支持 `explicit_denies`。
- **等级（class level）与权益（benefits）双轨**：6 级阶梯 `mini_can 迷你罐 → classic_can 经典罐 → glass_bottle 玻璃瓶 → sharing_pack 分享装 → family_pack 家庭装 → …`（第 6 级需 300 完成）。等级门槛卡四项：  
  min_uploaded / min_ratio / min_completions / min_days；权益侧另有 daily_image_count、邀请配额、api_keys=20、points_multiplier 等。
- 后台还有 **impact preview**：改等级/用户组前先看受影响的成员集合（`POST /admin/class-levels/{id}/impact-preview`）。这是一个很少见但很实用的设计。

### 3.4 OpenAPI 被当成"产品文档 + 生命周期看板"

- 312 个操作里：**145 implemented / 93 defined / 1 partial / 5 site**（另有 68 未标记）。`defined` = 契约已定、服务端未实现——等于对外公开了 roadmap。
- 每个操作带 `x-contract-version`（语义化加法式版本策略，其文档原话："Additive: new paths, new schemas, one new permission key; no existing path, field or status changed, so the contract goes to 13.2.0"）。
- 描述里写满了「为什么这么做」，例如：
  > 登录名的 pattern 比注册更宽：在新规则之前建立的账号（单字符、或新规则不再接受的半角句点）必须仍然能登录，把注册规则套到登录上会把这些会员锁在自己的账号之外。

---

## 4. 功能地图（312 个操作，按域归类）

| 域            | 内容                                                                                                                                                                                                                                                                                                    | 备注                                                                                                                                                                                                                       |
| ------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **内容模型**     | `media`(18) / `series`(0) / `torrent`(版本概念)                                                                                                                                                                                                                                                           | **媒体中心模型**：Media → Series ← Members → Versions(各版本种子)。对比 NP/FluxTorrent 扁平种子列表                                                                                                                                           |
| 浏览           | `/browse` 走 `GET /media`（page/page_size 分页，返回海报、豆瓣/TMDB 双来源、演员表、分级评分）。**没有全局种子列表 API**（仅 `/torrents/{id}` / `/torrents/pending` / `/automation/me/torrents` 自有）                                                                                                                                       | 种子必须先挂 media， browsing 只对作品维度                                                                                                                                                                                            |
| 发布           | `publication-drafts`（草稿）→ `POST /torrents` → `gate_phase`（如 `confirmed`）→ `submit` → `/pending` 审核队列                                                                                                                                                                                                  | `gate_phase` + `source_info_hash` + `projection_updated_at` 说明元数据是**异步投影**出来的                                                                                                                                            |
| **发布信用**     | `PublishingCredits`：满分 5，`auto_publish_threshold=5`，`can_auto_publish`                                                                                                                                                                                                                                | 用"信用分"换免审（ ours: task/exam 考核，他们：轻得多的 5 分制）                                                                                                                                                                              |
| 推广/促销        | `torrents/{id}/promotion-intents`（作者发起 intent）→ `admin/torrents/{id}/promotion`（站点拍板）+ `promotion-offers` + `admin/promotion-packages`（`package_key`/`duration_seconds`/`price_points`/`snapshot_hash`）                                                                                               | Intent 与 Realized 分离模型，含 retry/cancel — 很像我们尚未完善的"促销意图 vs 生效"                                                                                                                                                            |
| 货币/积分        | `GET /points/rules` 公布做种结算公式（**NP 原版小时公式移植**）；积分消耗场景：求种悬赏 / 商店兑换 / 付费促销                                                                                                                                                                                                                               | 参数全可配：`t0_weeks=4`、`n0_seeders=7`、`b0_unofficial=60 / l_unofficial=300`、`b0_official=240 / l_official=2400`、`max_torrents_counted=200`、`harem_factor=0.2`；按小时结算、即时入账、**北京时间午夜日切、漏跑的小时不补**（`missed_hours=not_backfilled`） |
| **harem 分红** | 邀请人按被邀请人的小时产出抽成：`min(own/0.2, Σ 直接邀请人小时产出) × 0.2`                                                                                                                                                                                                                                                     | 一般的邀请激励是"一次性发放"，这是可持续的分润模型                                                                                                                                                                                               |
| 请求/求种        | `requests` + `/bounty`（min=100, max=1e9, min_added=10, open_limit=10）+ `/fills` + accept/reject                                                                                                                                                                                                       | 与 NP 一致但 API 更细                                                                                                                                                                                                          |
| 社区           | 论坛（分类、版主组、likes/bookmark/follow、`related`、`/sidebar`）、评论、Wiki（markdown + slug）                                                                                                                                                                                                                        | 论坛 ACL 有独立的 `groups`/`levels` 维度                                                                                                                                                                                         |
| 圈子           | leaderboards（上传榜 94 人）/torrent-leaderboards/completions、favorites、subscriptions（按 media）+ feed                                                                                                                                                                                                        | `subscriptions` 是"追剧"而非追种子                                                                                                                                                                                               |
| 管理端          | **129 个 admin 操作**：会计调整、Rank sweep、Probation sweep、warnings、entitlements、`announce-limits`、`client-rules`、`seedbox` 审核、`image-host`（配额/用量）、`ptgen` 设置（含 AI review：provider/endpoint/**prompt_version**）、`ops` 面板（overview/audit-summary/site-statistics/tracker/workers、`kernel-health`）、站内信 + 外部来信队列 | 完全是"小成本高完整度"—— admin 面比很多老站还全                                                                                                                                                                                            |
| 第三方接口        | **两套并行**：① 现代 `/automation/*`（9 ops，`websiteApiKey` = `X-API-Key` 头）；② 遗留 NP 风 `/api/torrent/search` ` /api/member/getUserTorrentList` `/api/msg/search` `/api/tracker/myPeerStatus` `/api/torrent/genDlToken`（实测存在，但要求 **API key + scope**，实测返 `api_key_required` / `api_key_scope_denied`）          | 与我们 FluxTorrent 的 compat 层 + `fxo_` token 同一思路，**但他们是两套并存**（兼容层 vs 现代 automation API）                                                                                                                                    |
| 订阅/下载        | `rss-tokens`（可命名/轮换/预览 rss 源）+ `feeds/releases/{token}.rss` + `torrents/{id}/download-links` + `download-authorizations`（一次性下载凭证，可 revoke）                                                                                                                                                            | 下载授权对象化（可吊销），比单纯 genDlToken 更细                                                                                                                                                                                           |
| 站内信          | `GET /admin/staff-mail`（分类 complaint/appeal/suggestion/other、发信人卡片、回复线程）+ **外部来信** `POST /internal/staff-mail/external`（邮件服务器用 HMAC 摘要 + 时间戳入队，凭据独立于会话；契约原文：正文一律做纯文本渲染、HTML 部件不交给浏览器、附件只存元数据不入库、按 `Message-ID + 内容哈希` 幂等）                                                                             | 把邮箱当工单入口、在站内做客服队列，比直接看收件箱好维护                                                                                                                                                                                             |

---

## 5. 安全与反作弊

| 机制            | 实现                                                                                                                                                                                                                                  | 评价                                                                                 |
| ------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| **登录 PoW 闸门** | `sha256-pow-v1`：同一 peer/IP 三次失败后，服务端返 `403 captcha_required`，`fields` 下发 `captcha_token` / `captcha_challenge` / `captcha_difficulty` / `captcha_expires_at`；客户端求 `SHA-256("<challenge>:<solution>")` 满足前导零 bit 后回传。**不加载任何第三方验证码** | 成本极低的防撞库，且没有第三方脚本带来的隐私/可用性风险；禁用 JS 时明确不放宽。我们 FluxTorrent 目前只有 Origin 闸，**这道门值得移植** |
| 客户端规则         | `client_rules`：match_kind ∈ `peer_prefix` / `peer_exact` / `ua_contains` / `ua_regex` / `client_name`；可按版本号区间 + 优先级，action = allow/deny                                                                                             | 与我们的 white/black list 一致粒度                                                         |
| Announce 限流   | `announce_limits`：min_interval_secs / window_secs / max_in_window / ban_secs / report_interval_secs（按 passkey 封，可关）                                                                                                                 | 我们没有针对单 passkey 的动态版限流                                                             |
| Seedbox       | 用户申报 + 后台审核（`seedbox-declarations`）                                                                                                                                                                                                 | 同我们 metrics 里有，但他们的 benefits 有 `seedbox_revision` 类型版本号                            |
| 2FA           | TOTP enrollment/confirm/recovery codes + 管理端强制重置；本次账号 `required=false, enabled=false`                                                                                                                                               | 齐全                                                                                 |
| 邮件凭据          | 一次性 token = 64 位小写 hex；重复/畸形/缺失都有专门兜底文案，且 **token 捕获后立刻从地址栏 `history.replaceState` 抹掉**                                                                                                                                             | 细节到位                                                                               |
| 响应头           | CSP（**仅 Report-Only**）/ `X-Frame-Options: DENY` / `nosniff` / `Referrer-Policy: no-referrer` / `Cache-Control: no-store` / HSTS（**仅 max-age=86400，即 1 天**）                                                                          | 头很全，但两条最关键的（CSP 强制、HSTS 时长）都是"半开"状态                                                |
| 幽灵做种取证        | 契约里未见 BT 握手/取 piece 级别探测，只见到客户端规则、announce 限流、管理端 `listPeerObservations`（强度未知）                                                                                                                                                      | 我们 FluxTorrent 的 BT 三阶段探测在这点上明显更强                                                  |

---

## 6. 我该怎么抄：给 FluxTorrent 的可迁移清单

按性价比排序（迁移成本 vs 收益）：

1. **登录/注册的自托管 PoW 闸门**（sha256-pow-v1 形态）：几十行代码，零第三方依赖，能直接砍掉撞库 + 邮件轰炸。他们的"只有在第 N 次失败后才要求"这个延迟激活策略尤其值得照抄。
2. **`Idempotency-Key` + 期望 revision 的双闸门**：我们目前只有部分对象有 revision；把写类操作（发邀请/发消息/上传/评论/投稿）统一加幂等键，能一次性解决"超时重试导致重复记账/重复发信/重复创建"这一整类脏数据。
3. **契约先行 + per-operation 版本 + lifecycle 标记（implemented/defined）**：我们的 `check_type_drift.mjs` 已经是这个方向，可以再往前一步——把 OpenAPI 当 roadmap 看板，`defined` 状态的接口不实现但先承诺。
4. **前端内置 ajv 校验服务端响应**：交互 bug 会在真正写库之前暴露。**但要小心**：他们的做法让 `session` chunk 到 130KB(gzip)，建议我们只在 dev/staging 开启。
5. **发布信用 5 分制换免审**：比 task/exam 考核轻得多，适合用作"哪些人可以自助发布"的轻量开关，可作为现有管理端导航/权限之上的一层。
6. **促销 Intent / 生效状态分离**（`promotion-intents` → `promotion`）：我们现在的促销是"管理端直接挂"，缺中间的 intent/retry/cancel 记录，事后很难解释"这个种子为什么是这个倍率"。
7. **养分双档 + harem 分红**：`b0_official=240` vs `b0_unofficial=60`（4 倍差）用来区分官方/非官方发布；harem 让"邀请人"从一次性奖励变成持续受益。


1. **外部来信队列（staff-mail）**：邮件 → HMAC 摘要入队 → 站内排队 → UI 纯文本渲染 → "附件不同步"。把邮箱当工单入口，比直接看收件箱好维护，且与现有订阅发信白名单不冲突。

### 他们明显做得不如我们的地方（可反向输出）

- **tracker 源暴露**：我们的 tracker 会严格拒绝未注册 info_hash；他们 tracker 对未知 passkey 直接报业务错（这点没问题），但 tracker 域名未走 CF，源 IP 与主机商完全可见（154.89.149.3 / 香港）。
- **缺少跨种子信息**：契约里未见 `info_hash` / pieces 级别的辅种/查重字段（我们有 `open/recent` + 兼容层透出 `info_hash` + `pieces_hash`）。
- **全局种子搜索缺失**：他们的浏览必须先经过 media 作品维度，没有独立于作品的种子列表/搜索——习惯"资源搜索"的用户会不适应。这是我们的强项。
- **第三方通道偏窄**：他们有 `GET /me/messages/stream`（未读流）和 RSS，但没有 Torznab；我们是 Torznab + RSS + NP 兼容层 + 开放 API 四通道。
- **安全头半开**：CSP 只开 Report-Only、HSTS 只有 1 天。另外 `X-Reelcore-Api-Version` 实际值（13.2.0）与 OpenAPI 内声明的 const（3.4.0）不一致——契约已是单源，但版本号约定还不够严。

---

## 7. 数据快照（只读采样，用于以后对照）

```
GET /api/v1/me            id=75 username=gntv roles=[member] groups=[] 2FA 未开
GET /me/rank              level=mini_can(迷你罐) 下一级 classic_can 需 5 完成/7 天/1.0 ratio/50GiB
GET /me/points            balance=0  hourly 结算 credit=immediate  日切=北京时间
GET /promotions           download.num=0/den=1（全站下载不计）, ends 2027-01-01
                          upload 1/1, accounting_applied=false, execution_revision=12
GET /leaderboards         total=94 users; 榜首 ksdxmm 195GB upload, admin 138GB
GET /torrent-leaderboards 7 个种子进榜；榜首「凡人修仙传」S01E194 2160p Bili WEB-DL 7 完成
GET /media                total=18（含 凡人修仙传/兰香如故/一瓯春/假面良人/PSYREN/Spider-Man 等）
GET /series               total=0        GET /requests  total=0
GET /forum/topics         total=2        GET /shop/products  空
GET /me/publishing-credits score=0/5, threshold=5
GET /points/rules         见上文公式参数；spending.shop=[]；paid_promotions 未启用
GET /announcements        1 条（2026-10-12）：新架构内测中，请通过站内信反馈
GET /media 列表(25条)      裸 76KB → gzip 24.8KB
耗时参考                  GET /me 1.0–2.0s（经 CF AMS 边缘，国内请勿直接对照）
```



---

## 8. 遗留动作 / 可继续挖的点

- `/admin/*` 129 个操作的实际数据结构（本次账号是普通 member，全部 `permission_denied`）——需要一个带 `user.manage` / `audit.read` 的账号才能取样。
- `automation/*` 契约写了，但本次为避免写操作没有创建 API key，未验证真实返回。
- `internal/staff-mail/external` 的 HMAC 细节（`/admin/security/settings` 本次一度返 502，未复现）。
- 他们标注为 `defined`（已定契约未实现）的 93 个操作——对照我们自己的 roadmap 差异最大，建议下次逐一比对。

---

## 9. 登录实测复核（2026-10-13 晚，账号 gntv/member/uid=75）

> 用站长提供的账号真机登录复核上文可核对论断。**结论：主要论断全部实证命中；版本水位同日仍在快速推进。**

**精确命中的项（逐条对上原文）：**

- `GET /me/points` → `balance=0.0000`、`credited_through`、`revision=1`；`GET /promotions` → `download num=0/den=1, upload 1/1, accounting_applied=false, execution_revision=12`；`GET /me/publishing-credits` → `score=0, threshold=5` —— 与 §7 快照逐字段一致。
- `GET /me/rank` → `mini_can(迷你罐)`，下一级 `classic_can` 门槛 `min_completions=5 / min_days=7 / min_ratio=1.0` —— 与 §3.3 一致。
- `GET /torrent-leaderboards` → 榜首「凡人修仙传」S01E194 2160p Bilibili WEB-DL、value=7 完成 —— 与 §7 一致。`/series` total=0、`/requests` total=0、`/shop/products` 空、`/announcements` 1 条（10-12 内测公告）—— 全部一致。
- `/me` 权限键形态（`invite.create` / `request.create` / `rss.use` / `torrent.upload`…）与 §3.3 的 `x-permission` 命名系一致；会话下发 `csrf_token`（§3.1 的 `X-CSRF-Token` 机制实证）。
- 安全头：**CSP 仅 Report-Only**（策略全文与 §5 一致）、**HSTS `max-age=86400`**、`X-Frame-Options: DENY`、`nosniff`、`Referrer-Policy: no-referrer` —— 全部命中。`robots.txt` 仍只有 CDSM content-signals 注释、零有效指令。
- tracker：`/announce?passkey=test` → bencode `Invalid or missing passkey`；`/stats` → 403 —— 与 §1 一致。
- 未登录（`credentials: omit`）访问列表 API → 401；统一错误信封 `{error:{code,fields,message,request_id}}` 多次实证。
- `/media?limit=5` → 400 `invalid_catalog_query`；`page/page_size` 正常 —— 分页口径与 §3.1 一致。

**水位更新（同日晚于成文时间）：**

- 响应头 `X-Reelcore-Api-Version` 已从 **13.2.0 → 13.5.0**，页脚自报 **API 13.6.0**；`x-contract-version` 去重出现 **29 个值**（新增 13.3.0 / 13.4.0 / 13.5.0）——一天内连发 3+ 个契约版本，迭代节奏极快。
- 契约 chunk `session-*.js` 实测 **898KB**（成文时 875KB），ajv 校验器仍在内；`"defined"` 字面量 93 处命中（与 §3.4 的 93 个 defined 操作吻合）。
- 榜首 ksdxmm 上传量 **222,249,915,147 字节（≈207GiB）**，高于成文时快照（195GB）——站点当日仍在活跃增长。
- 注意：bundle 里 `IdempotencyKey` `$ref` 字面量计数 90 处、`x-origin-required` 字面量 285 处，与 §3.2 的「74 操作 / 148 操作」不同源——bundle 同时含契约原文与 ajv 预编译器，元数据双份，**裸字面量计数不可比**，原文的操作级口径保留不动。

**新观察（成文时未见）：**

- **SPA 登录态确认失败一例**：会话本身有效（cookie 在、`/api/v1/me` 200 且 613 字节已传回），但 SPA 启动序列自报「暂时无法确认登录状态」，登录页还常驻「页面数据异常，请刷新」alert；与其公告「内测阶段 BUG 可能较多」自洽。在嵌入式浏览器环境复现，不能完全排除环境差异，但错误 UI 是其自身组件。另：`/api/v1/openapi.json`、`/api-docs`、`/docs` 等路径均无独立契约端点（404 或 SPA fallback），契约确只内嵌于前端 bundle。
