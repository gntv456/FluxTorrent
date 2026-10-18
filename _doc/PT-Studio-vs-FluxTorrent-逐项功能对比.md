# PT Studio vs FluxTorrent — 逐项功能/能力对比（基于真代码，非网页摘要）

> 方法：clone `github.com/imythu/pt-studio`（`C:\Users\52sy\AppData\Local\Temp\pt-studio`）逐文件统计 Rust LOC + grep 验证；FluxTorrent 侧 grep `apps/**/*.rs` 验证。所有结论带代码证据。  
> 结论先行：**你之前「PT Studio 刚需都没实现、都是设计文档」的判断需要大幅修正**——PT Studio 实际落地极深；但它在「反作弊判定 / 考核考试 / 论坛 / 娱乐屋 / 字幕」上确实缺，而这恰恰是 FluxTorrent 已落地或更强的点。下面逐条核对。

---

## 0. 落地规模（先看清「谁真写了代码」）

**PT Studio（clone 实测 LOC）**

- 内核 `crates/kernel` 8878、`crates/server` 10572、`crates/pt-tracker`(独立 AGPL tracker) 2336、`crates/plugin-api`(WIT 契约) 2114。
- Native 插件：identity 3216 / search-meilisearch 3130 / torrent 2415 / accounting 2381 / ledger 2176 / mcp 1590 / llm-claude 891 / ai-assistant 793 / tracker-bridge 378 / recommend 295 / **rss 1408**。
- WASM 插件：invite 2516 / bonus 2083 / type-music 1468 / type-anime 1344 / promotion 1164 / comments 1072 / moderation 1010 / type-generic 926 / notify 910 / hnr 796 / requests 773 / levels 747 / reports 683 / metadata-tmdb 656 / type-tv 654 / messages 607 / type-movie 605 / ai-upload-review 605 / ai-moderation 585 / checkin 240 / x-chaos 56。
- 证据：绝大多数插件是**真实实现**（千行级），不是 stub。

**FluxTorrent（grep 实测存在）**

- 反作弊：`worker/jobs/process_event.rs`、`tracker/src/http_track/{guard,agent_ver,ip_trust}.rs` 等（四轮审计 + BT 三阶段探测 + connectable 四档 + 幽灵做种检测 + 交叉上报）。
- 论坛：`community_http/{forums,forum_perm,topic*,post_*}.rs`；娱乐屋：`games_http/arcade_*.rs` + `games/*`（十几种）；字幕：`content_http/subtitles_*.rs` + `worker/jobs/subtitle_*.rs`；H\&R：`worker/jobs/hr.rs`；邀请：`admin_p3_http/invites.rs`(34)；考核：`task_jobs/exam.rs`+`ops_http/exams.rs`；保种/复活：`ops_http/{preserve,resurrections}.rs`；促销/等级/签到/求种/魔力：均有对应模块。

> 一句话：两边都是「真写了大量代码」，不是 PPT。差异在**能力分布**，不在「实现 vs 画饼」。

---

## 1. 逐项功能对比矩阵

图例：✅实现  ⚠️部分/弱  ❌缺失/未实现  ＞ 表示相对更优

### 1.1 身份 / 注册 / 邀请

| 能力                | PT Studio                                                | FluxTorrent                                     | 优劣                         |
| ----------------- | -------------------------------------------------------- | ----------------------------------------------- | -------------------------- |
| 账号/登录/会话          | ✅ identity 3216（含会话、token、class_id）                      | ✅ `auth_http/*`、`login_origin.rs`               | 平                          |
| 注册策略（开放/关闭/审核）    | ⚠️ identity+invite 实现注册，未见独立「策略框架」                       | ✅ `auth_http/register_mode.rs`（显式注册模式开关）        | **FluxTorrent 略优**（策略化更显式） |
| 邀请制度              | ✅ invite 2516（含邀请码、违规连坐 ban reason `cheating`）           | ✅ `admin_p3_http/invites.rs`(34)+`invite_http*` | 平                          |
| RBAC / 权限域 / 敏感能力 | ✅ 内核 `RBAC`+`scopes`+`sensitive` 二次授权(`runtime.rs:1152`) | ⚠️ `authz.rs` 有基础授权，无扩展级能力模型                    | **PT Studio 优**            |

### 1.2 种子 / 元数据 / 资源类型

| 能力      | PT Studio                                       | FluxTorrent                                                             | 优劣                             |
| ------- | ----------------------------------------------- | ----------------------------------------------------------------------- | ------------------------------ |
| 种子发布/详情 | ✅ torrent 2415（含 poster/screenshot 上传、swarm 信息） | ✅ `torrent_http/*`、`publish_http/*`                                     | 平                              |
| 资源类型插件化 | ✅ type-anime/movie/tv/music/generic（元数据富化）      | ✅ 内容维度体系（sections/site_dims/content_networks/artists，11 站型）             | **FluxTorrent 优**（通用站适配更成熟、无码） |
| 外部元数据抓取 | ✅ metadata-tmdb 656 + wasmtime 适配器              | ✅ wasmtime 适配器（豆瓣等，`adapter_*.rs`）                                      | 平                              |
| 字幕      | ❌ 无字幕插件（仅发布名含「字幕组」解析）                           | ✅ `content_http/subtitles_*.rs` + `worker/jobs/subtitle_*`(完整任务流/认证/奖励) | **FluxTorrent 优**              |

### 1.3 经济：记账 / 魔力 / 促销 / 等级 / H\&R

| 能力              | PT Studio                                   | FluxTorrent                                     | 优劣                |
| --------------- | ------------------------------------------- | ----------------------------------------------- | ----------------- |
| 流量记账（上传/下载/做种秒） | ✅ accounting 2381（分区 worker、倍率、快照、恰好一次）     | ✅ `worker/jobs/{billing,seeding,settle}*.rs`    | 平                 |
| 魔力/奖励           | ✅ ledger 2176 + bonus 2083（公式规则 `rules.rs`） | ✅ `economy_http/*` + `billing_mults.rs`         | 平                 |
| 促销（freeleech 等） | ✅ promotion 1164                            | ✅ `content_http/promo.rs`+`promos.rs`+`promo_*` | 平                 |
| 用户等级/阶级         | ✅ levels 747                                | ✅ `classes_*.rs`+`class_adj.rs`                 | 平                 |
| H\&R（Hit\&Run）  | ✅ hnr 796（含 cold filter 修复）                 | ✅ `worker/jobs/hr.rs`+`hr_mail.rs`              | 平                 |
| 银行/存贷           | ❌ 未见                                        | ✅ `economy_http/{bank,loans}*` + `bank_jobs/*`  | **FluxTorrent 优** |

### 1.4  tracker / 反作弊（关键差异）

| 能力                                 | PT Studio                                                                  | FluxTorrent                                                                               | 优劣                   |
| ---------------------------------- | -------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------- | -------------------- |
| Tracker（announce/scrape）           | ✅ pt-tracker 2336（独立程序，Redis 通信）+ tracker-bridge 378                       | ✅ `tracker/src/*`（HTTP+UDP，7070/6969）                                                     | 平                    |
| 客户端规则/准入                           | ✅ pt-tracker 有 client rules + admission 缓存                                 | ✅ `agent_ver.rs` 客户端校验                                                                    | 平                    |
| **反作弊判定（幽灵做种/connectable/伪造流量检测）** | ❌ **无检测引擎**（仅 `reason_code:"cheating"` 是 ban 枚举值，非检测；tracker 只有客户端规则+人工封禁） | ✅ 深度：ledger_guard 速率钳/终身上限、connectable 四档、-2 SUSPECT 不误伤、BT 三阶段握手探测、幽灵做种四条件、交叉上报(xreport) | **FluxTorrent 压倒性优** |
| 分享率考核/考试（新人试用/考核）                  | ❌ 无 exam 插件                                                                | ✅ `task_jobs/exam.rs`+`ops_http/exams.rs`+`jixiao*`(绩效)                                   | **FluxTorrent 优**    |

### 1.5 社区：评论 / 论坛 / 站内信 / 举报 / 审核

| 能力           | PT Studio                          | FluxTorrent                                                  | 优劣                |
| ------------ | ---------------------------------- | ------------------------------------------------------------ | ----------------- |
| 种子评论         | ✅ comments 1072                    | ✅ `torrents/comments.rs`                                     | 平                 |
| 论坛（版块/帖子/权限） | ❌ 无 forum 插件（只有 comments+messages） | ✅ `community_http/{forums,forum_perm,topic*,post_*}` 完整论坛+权限 | **FluxTorrent 优** |
| 站内信          | ✅ messages 607                     | ✅ `community_http/*`(feed/follow/tip 等)                      | 平                 |
| 举报           | ✅ reports 683                      | ✅ `http/reports_http.rs`                                     | 平                 |
| 审核/管理端       | ✅ moderation 1010                  | ✅ `staff_http/*`+`admin_http/*`+`admin_p3_http/*`            | 平                 |

### 1.6 通知 / 签到 / 求种 / 搜索

| 能力         | PT Studio                              | FluxTorrent                                              | 优劣                    |
| ---------- | -------------------------------------- | -------------------------------------------------------- | --------------------- |
| 通知（站内+邮件）  | ✅ notify 910                           | ✅ `ops_webhook.rs`+站内通知                                  | 平                     |
| 每日签到       | ✅ checkin 240（连签/奖励/排行榜/幂等键）           | ✅ `economy_http/checkin.rs`+`home_attendance.rs`         | 平                     |
| 求种（bounty） | ✅ requests 773（含保证金/投票/fill）           | ✅ `community_http/bounty.rs`                             | 平                     |
| 搜索（全文/高质量） | ✅ search-meilisearch 3130（Meilisearch） | ⚠️ Postgres 全文/游标（`torrent_http/query.rs`），无 Meilisearch | **PT Studio 优**（搜索质量） |

### 1.7 AI / LLM / MCP（PT Studio 独有强项）

| 能力                   | PT Studio                                              | FluxTorrent                        | 优劣              |
| -------------------- | ------------------------------------------------------ | ---------------------------------- | --------------- |
| LLM 接入               | ✅ llm-claude 891                                       | ❌ 无原生 LLM 插件（仅 wasm 适配器可接外部，非一等公民） | **PT Studio 优** |
| 站内助手                 | ✅ ai-assistant 793                                     | ❌ 无                                | **PT Studio 优** |
| AI 审核/上传审查           | ✅ ai-moderation 585 + ai-upload-review 605             | ❌ 无（有 `embed_rules` 私有规则，但非 AI）    | **PT Studio 优** |
| MCP 工具注册（给 AI 工具调站点） | ✅ mcp 1590（内核原生 MCP 网关，`gateway.rs:69` 仅 mcp 插件可声明根路由） | ❌ 无 MCP                            | **PT Studio 优** |
| 推荐                   | ✅ recommend 295                                        | ❌ 无                                | **PT Studio 优** |

### 1.8 娱乐屋 / 保种 / 任务 / 勋章

| 能力       | PT Studio              | FluxTorrent                                                         | 优劣                |
| -------- | ---------------------- | ------------------------------------------------------------------- | ----------------- |
| 娱乐屋（小游戏） | ❌ 无                    | ✅ `games_http/arcade_*` + `games/*`（十余种，含 casino/farm/fishing/pets） | **FluxTorrent 优** |
| 保种区/复活   | ❌ 无                    | ✅ `ops_http/{preserve,resurrections}.rs`                            | **FluxTorrent 优** |
| 任务中心/勋章  | ⚠️ 无独立任务插件（levels 含部分） | ✅ `ops_http/tasks.rs`+`task_jobs/*`+`medal_*`                       | **FluxTorrent 优** |
| 抽奖/扭蛋    | ❌ 无                    | ✅ `gacha_http/*`+`lottery.rs`                                       | **FluxTorrent 优** |

### 1.9 内容维度 / 站型 / 无码定制（FluxTorrent 强项）

| 能力                               | PT Studio                           | FluxTorrent                                                                            | 优劣                       |
| -------------------------------- | ----------------------------------- | -------------------------------------------------------------------------------------- | ------------------------ |
| 站型预设                             | ⚠️ type-* 资源类型插件，未见「11 站型快照」式预设     | ✅ 11 站型 + 术语改名（`staff_http/sitetype.rs`）                                               | **FluxTorrent 优**        |
| 用户自定义字段                          | ⚠️ 未见                               | ✅ `user-fields`(`staff_http/user_props.rs` 等)                                          | **FluxTorrent 优**        |
| 自定义页面/菜单                         | ⚠️ 前端插槽机制，但无码配置面有限                  | ✅ `pages-menus` + `view_layout.rs`                                                     | **FluxTorrent 优**        |
| 内容包/主题分发                         | ❌ 无（插件即包）                           | ✅ `settings_http/{packs,pack_format,pack_import,pack_catalog,pack_embed}*`             | **FluxTorrent 优**        |
| 模块开关（30 个）                       | ⚠️ cargo feature 选 Native 插件，非运行时开关 | ✅ `modules.rs`(30 模块启停联动)                                                              | **FluxTorrent 优**（运行时无码） |
| 第三方对接（NP 兼容/OpenAPI/Torznab/RSS） | ⚠️ 仅 rss 1408（自有 API，无 NexusPHP 兼容） | ✅ NP 兼容层 `nexusphp.rs`/`np_query.rs` + `fxo_` OpenAPI + Torznab + RSS（`compat_http/*`） | **FluxTorrent 优**（生态互通）  |

### 1.10 扩展架构 / 安全（PT Studio 架构强项，已落地）

| 能力                 | PT Studio                                                                                                                               | FluxTorrent                                                               | 优劣                   |
| ------------------ | --------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------- | -------------------- |
| 统一扩展契约（WIT 冻结）     | ✅ `pt:plugin@1` WIT 1.0.0 已冻结（commit "Freeze the plugin API"，`plugin-api` 2114）                                                         | ❌ 三套零散（wasmtime/插件钩子/表达式引擎），无统一契约                                         | **PT Studio 优**      |
| 微内核 + 一切皆插件        | ✅ 内核仅加载/分发，功能全在插件                                                                                                                       | ❌ 单体（Actix+Next），功能在核心                                                    | **PT Studio 优**（架构）  |
| **插件 SQL 隔离**      | ✅ 每插件独立 role + `GRANT … INHERIT FALSE, SET TRUE`(`migrate.rs:166`)、每次调用 `SET LOCAL search_path={schema}`(`migrate.rs:192/248`)、敏感能力二次授权 | ❌ native 插件持完整 `&AppState`、裸 SQL（`plugins.rs:235` `UPDATE torrents`）      | **PT Studio 压倒性优**   |
| 事件 outbox / 钩子管线   | ✅ `event_outbox`(`FOR UPDATE SKIP LOCKED`)+Redis pubsub、cron/delayed(`events.rs`)                                                       | ⚠️ 仅 `plugins.rs` 三个 `dispatch_*`（无 outbox/钩子管线）                          | **PT Studio 优**      |
| HTTP 网关：CSRF/幂等/限流 | ✅ CSRF token(`__Host-pt_csrf`)+幂等键(`idempotency-key`)+GCRA 限流(`limits.rs`/`gateway.rs`)                                                 | ⚠️ 有幂等(`http/idempotency.rs`)+Origin 闸(`login_origin.rs`)，无 CSRF token 框架 | **PT Studio 优**（更正式） |
| WASM 沙箱（第三方代码）     | ✅ 通用插件运行时 + `http_outbound` 能力白名单(`http_client.rs`)+epoch 中断(`wasm/mod.rs`)                                                             | ✅ 仅适配器（fetch/元数据，网络白名单，无 DB）                                              | **PT Studio 优**（更通用） |
| 表达式/规则引擎           | ✅ 插件内可声明公式（bonus `rules.rs`）                                                                                                            | ✅ `rules_engine.rs`（evalexpr 白名单，规则包银行利率）                                 | 平                    |

---

## 2. 对你之前判断的逐条修正

| 你之前的判断                   | 代码核实结果                                                                                                                                                                                | 修正                                                                   |
| ------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------- |
| 「PT Studio 零生产验证 + 单作者」  | 单作者(imythu)+claude、仓库新(2026-10)、有完整 e2e 测试套件，但**未见任何公开生产部署**                                                                                                                          | ✅ 成立（生产验证缺）                                                          |
| 「反作弊判定没实现」               | grep `cheat/ghost/connectable/peer_probe` 仅 ban 枚举值，无检测引擎                                                                                                                             | ✅ 成立（**这是 PT Studio 最大刚需缺口**）                                        |
| 「注册策略没实现」                | invite 2516 + identity 3216 实现注册；但**策略化框架**未见                                                                                                                                         | ⚠️ 修正：注册实现了，富策略框架未见；且 FluxTorrent 有显式 `register_mode.rs` 反而更策略化      |
| 「分享率考核没实现」               | 无 exam/probation 插件                                                                                                                                                                   | ✅ 成立（FluxTorrent 有考核考试+绩效）                                           |
| 「字幕没实现」                  | 无字幕插件                                                                                                                                                                                 | ✅ 成立，且**FluxTorrent 有完整字幕工作流**（之前你暗示 FluxTorrent 有，已证实）              |
| 「论坛没实现」                  | 无 forum 插件                                                                                                                                                                            | ✅ 成立（FluxTorrent 有完整论坛）                                              |
| 「娱乐屋没实现」                 | 无 arcade 插件                                                                                                                                                                           | ✅ 成立（FluxTorrent 有十余种游戏）                                             |
| 「PT Studio 刚需都没实现、是设计文档」 | **过度概括**：identity/invite/torrent/accounting/ledger/bonus/rss/promotion/comments/moderation/reports/messages/notify/checkin/requests/hnr/levels/metadata/search/llm/mcp/ai-* 均为千行级真实实现 | ❌ **修正**：PT Studio 绝大多数 PT 核心功能已落地，并非画饼；缺口集中在「反作弊判定/考核/论坛/娱乐屋/字幕」这几项 |

---

## 3. 按标准的最终判定（更新版）

| 评判标准                             | 谁更优             | 理由                                                                                             |
| -------------------------------- | --------------- | ---------------------------------------------------------------------------------------------- |
| 架构优雅 / 前瞻性                       | **PT Studio**   | 微内核、WIT 冻结契约、per-plugin SQL 隔离、事件 outbox、RBAC/scope/sensitive、MCP、CSRF/幂等/限流网关——全部落地           |
| 插件安全（SQL 隔离/能力最小化）               | **PT Studio**   | `INHERIT FALSE, SET TRUE` + search_path 锁定 + 敏感能力二次授权；FluxTorrent 的 native 插件裸 SQL 是真实 CVE 类风险 |
| AI / LLM / MCP 原生                | **PT Studio**   | 4 个 AI 插件 + MCP 网关，FluxTorrent 缺失                                                              |
| 搜索质量                             | **PT Studio**   | Meilisearch 插件 3130 行                                                                          |
| 反作弊判定深度                          | **FluxTorrent** | PT Studio 无检测引擎；FluxTorrent 有 BT 三阶段探测/connectable/幽灵做种/交叉上报                                   |
| 论坛 / 娱乐屋 / 字幕 / 考核考试 / 保种复活 / 银行 | **FluxTorrent** | PT Studio 这几项缺失                                                                                |
| 通用站适配 + 站长无码定制                   | **FluxTorrent** | 11 站型、用户字段、页面菜单、内容包主题、30 模块开关；PT Studio 偏开发者向                                                  |
| 生态互通（NP 兼容/OpenAPI/Torznab）      | **FluxTorrent** | 可直接对接 NexusPHP 生态；PT Studio 自有 API                                                             |
| 运维简单 / 一个栈打通                     | **FluxTorrent** | 单体 Actix+Next；PT Studio 微内核+每插件 role/Redis 前缀，运维更重（你原硬伤 #3 成立）                                 |
| 已验证可行（生产）                        | **都不算**         | PT Studio 无公开生产；FluxTorrent 单作者未大规模，但经四轮安全/反作弊审计                                               |



---

## 4. 给 FluxTorrent 的结论

1. **PT Studio 是「最好的架构 + 已落地的广功能」，但仍是「未生产验证的单作者项目」**——它比你之前以为的强得多，不是画饼。
2. **FluxTorrent 的真正护城河**不在架构，而在：反作弊判定深度、论坛/娱乐屋/字幕/考核等 PT 社区刚需、通用站无码定制、NP 生态互通。这些 PT Studio 短期补不上。
3. **FluxTorrent 最该抄的**（已验证、低风险）：


- **插件 SQL 隔离**（最高优先级）：把 native 插件从「持 `&AppState` 裸 SQL」改为「per-plugin role + `SET ROLE` + search_path 锁定 + 敏感能力二次授权」——直接消除 `plugins.rs:235` 的 CVE 类风险。
- **统一扩展契约**：把 wasmtime/插件钩子/表达式引擎收口（见另一篇设计稿，本对比不受其干扰）。
- **事件 outbox + 幂等键 + CSRF**：把 `dispatch_*` 升级为 outbox，补 CSRF token。

1. **FluxTorrent 不必追的**：全微内核重写、per-plugin Redis 前缀、MCP/LLM 原生（除非业务需要）、Meilisearch（Postgres 全文够用则不必）。
2. **AI/MCP/搜索**是 PT Studio 的领先项，若 FluxTorrent 要「AI 原生叙事」，可在适配器层补 LLM 接入，而非照搬其内核。
