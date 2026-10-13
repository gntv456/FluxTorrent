# FluxTorrent vs 同类站（同类站PT / ReelCore）深度对比分析

> 成文：2026-10-13
> 两侧数据来源：
> - **FluxTorrent 侧 = 源码实证**。本节所有结论均来自本仓代码，附 `file:line`，由 4 路只读核查汇总。
> - **同类站 侧 = 黑盒反推**。未登录只能拿到外壳；登录后可遍历 API；其**整套 OpenAPI 契约（248 路径 / 312 操作 / 469 schema）被打进前端 bundle**，可据此还原服务端设计。凡属"契约已定义但服务端未实现"（其 `x-implementation: defined`）的部分，本文会显式标注，不按既成事实处理。
> **口径警告**：FluxTorrent 的 725 是 actix 路由宏计数（含页面代理、内部端点、管理端 191 条）；同类站 的 312 是 OpenAPI 操作计数（纯 API）。两者**不可直接比大小**，只能比"契约化程度"和"功能覆盖清单"。

---

## 0. 总评：一张评分卡

| 维度 | FluxTorrent | 同类站 | 谁赢 | 说明 |
|---|:---:|:---:|:---:|---|
| 功能广度与深度 | **9** | 5 | 我们 | 108 页面 / 56 一级路由 / 银行+贷款+娱乐屋+勋章+成就+基金池，对方均无 |
| 通用站适配（站型参数化） | **9** | 3 | 我们 | 我们 55 种 section_kinds × 6 种 field_type 参数化；对方硬编码影视字段 |
| 反作弊 / 风控深度 | **9** | 4 | 我们 | BT 三阶段探测 + 交叉上报 + connectable 四档 + 计费守卫，对方只有客户端规则与 announce 限流 |
| 经济系统厚度 | **9** | 6 | 我们 | 我们多套并行（银行/贷款/捐赠/娱乐屋/勋章/成就）；对方更"正统 PT"但薄 |
| 安全响应头与凭据 | **8** | 5 | 我们 | 我们 HSTS 1 年 + CSP 强制；对方 CSP 仅 Report-Only、HSTS 仅 1 天 |
| 运维与可观测 | **7** | 6 | 我们 | 我们 Prometheus（api 7 族 + tracker 23 族）+ 分级探针；对方 ops 面板更产品化但无指标端点外露 |
| API 契约工程 | 3 | **9** | 对方 | 我们只有手写 27 条（≈3.7%）+ 正则对拍门禁；对方 100% 机器可读 + 生命周期标记 |
| 前端工程现代化 | 5 | **7** | 对方 | 我们 `next/dynamic` **0 使用**、组件原语仅 2 个；对方 130 chunk 路由级懒加载 + 契约驱动校验 |
| 内容模型（影视站） | 6 | **8** | 对方 | Media→Series→Members→Versions 强模型，追剧/订阅天然；我们是扁平种子+维度，通用但重 |
| 登录侧防撞库 | 4 | **8** | 对方 | 对方自托管 `sha256-pow-v1` 工作量证明；我们靠失败锁定 + 可插拔第三方验证码 |
| 写操作一致性纪律 | 6 | **8** | 对方 | 对方 74 处幂等键 + **17 处乐观并发**；我们有 152 处 DB 级幂等但**零乐观并发** |
| 成熟度/稳定性 | **7** | 3 | 我们 | 对方域名 14 天、自述内测、94 用户 |

**总判断**：我们赢在**深度与通用性**，对方赢在**契约工程与前端工程纪律**。对方是一个"小而完整、文档密度极高"的新作品；我们是一个"大而深、但工程纪律还没跟上体量"的老项目。**真正值得抄的是对方那套"把契约当产品文档"的方法论，以及几个具体的机制设计；真正不能丢的是我们的反作弊深度与通用站参数化。**

---

## 1. 前端与 UI

### 1.1 技术栈

| | FluxTorrent | 同类站 |
|---|---|---|
| 框架 | Next.js 15.3 / React 19（App Router），SSR 为主，`"use client"` 370/507 tsx（`apps/web/package.json:14-16`） | Vue 3 + Vite，**纯 SPA 无 SSR**（`index.html` 只有 `<div id="app">`） |
| 样式 | Tailwind v4 + 自建令牌链（`app/globals.css:6`；三环拆分注释 `:2-5`） | 无 UI 库，手写 CSS + CSS 变量主题 |
| 构建 | `output: "standalone"`（`next.config.ts:4`），pnpm workspace + turbo（`package.json:4-9`） | Vite，130 个 chunk |
| 主题 | 2 套（`brand` / `brand-night`），`theme-tide.css` 是覆盖层不是第三主题（`app/styles/theme-tide.css:4-24`）；令牌 265 个变量声明（`base.css:11-157` 明、`:158-510` 暗） | 3 套（`minimal` / `cola-cream-red` / `cola-black-red`），`localStorage["reelcore.theme"]`，默认黑红 |
| 防闪烁 | 首绘前内联脚本 + `data-theme`（`app/layout.tsx:120-134`） | 入口 JS 里 `y(v())` 同步设 `dataset.theme`（`index-Cc8_GNb4.js`） |

### 1.2 代码分割与加载 —— **对方明显更彻底**

- **我们：`next/dynamic`、`React.lazy`、`dynamic(` 全仓 0 处命中**（`grep -rn "next/dynamic\|React.lazy\|lazy(" app components`）。唯一显式分割是把娱乐屋 120KB CSS 移出根链（`app/globals.css:29-33`、`app/(main)/games/layout.tsx:8-9`）。
- **对方：130 个 chunk 按路由懒加载**，入口 `index-Cc8_GNb4.js` 仅 **5.7KB**，`bootstrap` gzip 后约 19KB，`__vite__mapDeps` 精确声明每个路由的依赖。
- 代价在另一侧：对方把**整份契约 + ajv 预编译校验器**塞进 `session-*.js`（raw 875KB / gzip 130KB），这个 chunk 是所有业务的公共依赖，一加载就跑不掉。**他们的"懒加载"换来的是"公共依赖过重"**——和我们是两种不同形态的胖。

### 1.3 组件原语与视觉一致性

| 项 | FluxTorrent | 同类站 |
|---|---|---|
| 组件原语层 | `components/ui/` **仅 2 个文件**（`empty-state.tsx`、`skeleton.tsx`） | 有 `StatePanel` / `PageHeading` / `ActionFeedback` / `BusyLabel` / `HintTip` / `UnitField` / `MutationFeedback` 等一批反馈类原语（chunk 名可见） |
| 骨架屏 | 有定义，业务引用仅 1 处（`components/torrent-list-client.tsx`） | `Skeleton`-类原语是 chunk 级公共依赖，推测覆盖面更广（未登录无法验证） |
| Toast | **无独立组件**（`grep "export function Toast"` 无命中） | `AppToast` / `MutationFeedback` 是全局注册（`bootstrap` 里 `AppToast` 作为 route name 出现） |
| 空态 | 业务引用 4 处 | `StatePanel` 原语 |
| 加载/错误边界 | `loading.tsx` **0 个**、`error.tsx` 1 个、`not-found.tsx` 1 个 | SPA，路由级 `UnavailablePage` chunk |
| 硬编码色 | app 12 + components 107 = **119 处**，其中 `components/game/` 独占 76 | 未见同等量级（其 CSS 走主题变量） |

**结论**：两边都不是"有成熟设计系统"的状态，但对方的反馈类原语（加载/空/错误/toast/忙碌标签）是**体系化**的，我们的是**零散且低覆盖**——骨架屏定义了却只有 1 处用、toast 根本没有组件。这是"页面数量已经 108 个、原语层还停在 2 个文件"的典型欠账。

### 1.4 国际化、移动端、可访问性 —— **我们全面领先**

| 项 | FluxTorrent | 同类站 |
|---|---|---|
| 语言 | 4 语（`i18n/config.ts:3`），zh-CN 5846 key；**ja 仅 98 条未完成**（`ja.ts` 130 行） | 单语中文，文案硬编码在组件 |
| i18n 门禁 | `scripts/i18n_guard.mjs` + 基线 583 文件 / 462 行存量豁免 | 无需门禁 |
| 移动端 | **M1–M4 完整改造**：安全区（`layout.tsx:9-19`）、管理端抽屉（`admin-shell.tsx:218`）、Bottom Sheet（`bottom-sheet.tsx:7`）、下拉刷新（`pull-to-refresh.tsx:5`）、平板双栏（`tablet-sidebar.tsx:7`）、种子详情固定操作条（`torrent-mobile-actionbar.tsx:6`） | 有 viewport meta 与 apple-touch-icon，但**无 manifest、无 PWA**（`/manifest.webmanifest` 404） |
| 断点体系 | 5 档语义令牌 + 折叠态（`base.css:309-324`），但存量 46 处裸像素断点未收敛（`:297-319`） | 未见断点令牌体系 |
| a11y | `aria-*` 390 处、`role=` 122、`sr-only` 26、焦点陷阱 hook `lib/use-focus-trap.ts`；但 `<main>` 仅 6 处、`tabIndex/onKeyDown` 仅 9 处 | 未登录无法评估 |
| 图片 | **`next/image` 0 使用**，裸 `<img>` 22 处、`loading="lazy"` 仅 8 处；无 `PosterImage` 组件，海报逻辑散在 4 处 | 有 `PosterImage` 独立 chunk（+ 专属 CSS），海报/封面是组件化的 |

**结论**：i18n、移动端、a11y 这三项我们明显做得更多（对方是单语、无 PWA）。但我们的**完成度**有洞：ja 只有 98 条、断点半迁移、图片完全没用 `next/image`、`<main>` 只有 6 个。对方的 `PosterImage` 组件化思路值得抄——我们海报逻辑散在 4 个文件各自实现。

---

## 2. 架构与工程

| 项 | FluxTorrent | 同类站 |
|---|---|---|
| 语言/框架 | Rust actix-web（api）+ 独立 tracker 二进制 + worker + Next.js web | 未知（推断为 Go/Rust/Node 之一）；响应头 `X-Reelcore-Api-Version: 13.2.0`；tracker 是**独立二进制**（只认 `/announce` `/scrape`，其余 `not found`） |
| 域划分 | 24 个 `*_http/` 域包 + 12 个单文件；路由宏 725、`.service(` 725（`.route(` 仅 5） | 312 个 OpenAPI 操作，其中 admin 129、automation 9 |
| 组装 | 21 个 `mount*` 函数链式组装，**无单一全局路由清单**（`community_http/mod.rs:200-214`） | 契约即清单：全部路径集中在一份 OpenAPI 里 |
| 迁移 | **344 个** sqlx 迁移，最新 `0350`；纪律写入 `CONTRIBUTING.md:36-76`（不可变、取最大号、`uniq -d` 撞号检测、SHA384 对账、改迁移必须 rebuild 镜像） | 契约描述里提到 "migration 0119" 等编号，说明同样是编号迁移制 |
| DB 逻辑下沉 | `CREATE FUNCTION` 22 个 / 15 文件（相对 344 迁移属轻量） | 未见，逻辑应在应用层 |
| JSONB | 517 处 / 77 个迁移文件（≈22%） | 配置多为强类型 schema（469 个 schema 全字段声明） |
| worker | **42 个** job 模块（`jobs/mod.rs` 声明数；目录内 46 个 `.rs` 含 `mod.rs`/`run.rs`/`tests.rs`，另有 `ledger_guard/`、`process_event/` 两个子目录与平级的 `task_jobs/`）、现网 `job_status` 50 行 / **7 档周期**（60s→24h，`jobs/run.rs:115-123`）；announce 走 Redis Stream 消费者组 + 陈旧 reclaim 360s；`FLUX_WORKER_JOBS` 角色分片 + 分布式锁 | 契约里有 `GET /admin/ops/workers`、`rank sweep`、`probation sweep`、`points settlement hourly`，说明同样有多档周期作业 |
| 部署 | 6 容器（postgres/redis/api/worker/tracker/web）+ 可选 prometheus/grafana；优雅停机 api 45s / worker 90s | 至少 2 处：站点（Cloudflare 后）+ tracker 独立主机（香港，AS55933，DNS 未过 CF） |
| 冷启动 | `CONTRIBUTING.md:97` api 先跑完全部迁移再 bind 端口 → **冷启动数十秒** | 未知 |

**结论**：架构等级相当，都是"API + worker + 独立 tracker + 富前端"的现代形态。差异在**信息组织方式**：我们把路由散在 21 个 mount 函数里、没有机器可读清单；对方把一切收敛到一份契约。344 个迁移是我们的历史资产也是包袱（冷启动数十秒），对方 14 天还没到这个阶段。

---

## 3. API 与契约 —— **差距最大的一项**

### 3.1 契约化程度

| | FluxTorrent | 同类站 |
|---|---|---|
| 机器可读契约 | **无 utoipa / 无生成产物 / 无 build.rs**；`/api/v1/openapi.json` 是**手写 serde_json**（`openapi_http/spec.rs:1-13`、`spec.rs` 13 条 + `spec_np.rs` 14 条 = **27 条路径**） | 完整 OpenAPI 3.4.0，**248 路径 / 312 操作 / 469 schema**，内嵌于前端 |
| 覆盖率 | 27 / 725 ≈ **3.7%** | 100%（含未实现的） |
| 门禁 | `scripts/check_type_drift.mjs`（298 行）做**源码正则对拍**：错误码数字 ↔ TS 枚举、促销 `KIND_RANK` ↔ TS 联合、**8 条手写 CONTRACTS** 的字段比对（`:173-244`）；跑在 CI（`.github/workflows/build-test.yml:81`），**无 git hook** | 契约本身即门禁：前端运行时用 ajv 校验服务端响应 |
| 生命周期标记 | 无 | `x-implementation`: **145 implemented / 93 defined / 1 partial / 5 site** —— 公开 roadmap |
| 版本策略 | 无 per-operation 版本 | `x-contract-version` 每个操作一个（3.4.0～13.2.0 共 27 个值），加法式语义化 |

**这是结构性差距**：我们的契约门禁只能防"8 条手写契约"的漂移，剩下 99% 的接口靠 TS 类型手写维护；对方是"契约先行、代码跟随、前端运行时校验"。更要命的是**我们连 `defined`（已定未实现）这个概念都没有**——设计意图只存在于对话和记忆里。

### 3.2 请求/响应约定 —— 相当

| | FluxTorrent | 同类站 |
|---|---|---|
| 信封 | `{code,message,data,request_id}`（`dto.rs:6-23`），成功恒 `code:0` | `{error:{code,fields,message,request_id}}`，错误专用信封；成功体无包装 |
| 错误码 | **21 个变体 → 20 个数字码**（`Validation` 与 `FieldErrors` 同映射 1002），分段（1xxx 通用 / 2xxx 认证 / 3xxx 种子 / 4xxx 经济 / 5xxx 业务）（`errors.rs:65-89`）；TS 侧 `packages/domain-types/src/index.ts:9` **25 个常量**（多出的 5 枚是预留码：1001/2008/3001/3002/5001）；HTTP 态在 `:90-121` | 字符串 code（`route_not_found`、`permission_denied`、`api_key_scope_denied`…） |
| request_id | `x-request-id` 头，入站沿用（1–64 位校验防注入）或 UUIDv4，**响应头 + task-local + 日志三处同源**（`request_id.rs:21,46-66`） | `X-Request-Id` 头 + 信封内 `request_id` 同源 |
| 分页 | 统一 `page`/`per_page`，`page_window()` 钳制 per_page 1..=100 / page 1..=100_000 + `checked_mul` 防溢出（`dto.rs:28-36`），**34 处调用**（另有 1 处定义、1 处注释提及） | 统一 `Page`/`PageSize` component，page_size 默认 25 / 上限 100 |
| JSON 解析错 | 也走信封（`main.rs:234-246`，`code:1002`） | 走错误信封 |

两边都做到了"错误可追踪、分页统一"。我们的错误码是**数字枚举**（可用门禁对拍，这是优势），对方是字符串（可读性更好但无法静态校验）。

### 3.3 认证、CSRF、幂等、并发 —— **对方更标准**

| | FluxTorrent | 同类站 |
|---|---|---|
| 会话 | JWT（HS256/RS256 可选，RS256 缺密钥拒绝启动 `auth.rs:126-135`）+ HttpOnly cookie `flux_token`，TTL 默认 24h（`auth_http/login.rs:194-196`）；Claims 无 jti | Cookie `reelcore_session`（Path=/api/v1, SameSite=Strict, 24h）+ `reelcore_session_marker` 影子 cookie |
| CSRF | **无 CSRF token**；靠 `SameSite=Lax` + 两道 Origin 闸：全局写方法中间件（`http/write_origin.rs:21-42`，覆盖全部 `/api/v1` 非 GET/HEAD/OPTIONS）+ 表单登录专用（`auth_http/login_origin.rs:21-28`） | **有 `X-CSRF-Token`**（登录/GET /me 下发）+ `Origin` 强制 **148 个操作** |
| 幂等 | **152 处**，但 `idempotency_key` 是**业务体字段**、非 HTTP 头；DB 唯一约束兜底 `ON CONFLICT DO NOTHING`（`admin_http/user_ops.rs:66,71,81`） | **74 个操作**强制 `Idempotency-Key` **HTTP 头**（16–128 位可见字符），契约级声明 |
| 乐观并发 | **0**（全仓无 `If-Match` / `ETag` / `expected_revision`）；配置一致性靠 Redis `HINCRBY flux:cfg:ver` + 3s 轮询失效（`cfgver.rs:20-72`）—— 这是**缓存失效**不是并发控制 | **17 个操作**要求期望 revision（points rules / HR rules / publishing / traffic / profile 等"先读后写"配置） |
| 限流 | Redis 固定窗口，**fail-close**（`http/throttle.rs:22-24`，Redis 错即 500）；39 处调用点；登录 5/分·用户名 + 20/分·IP + 连续 5 次锁 15 分钟；429 带 `Retry-After: 60`（`errors.rs:182-187`） | announce limits 可配（min_interval / window / max_in_window / ban_secs / report_interval，按 passkey 临时封禁）；登录 PoW |
| 权限 | **73 个权限键**（`authz/perm.rs:6-99`）；三层并集（等级累进 + 职务持有 + 用户级覆盖），判定下沉 SQL `user_can()`（`authz.rs:34-43`） | **37 个** `x-permission`；等级（class level）与权益（benefits）**双轨**；有 **impact preview**（改等级/组前预览受影响成员） |

**结论**：
- **我们要补的是乐观并发**——17 个"先读后写"的配置接口（积分规则、HR 规则、发布设置、流量配置）没有 revision 保护，两个管理员同时改会互相覆盖。对方用契约强制了这一点。
- **我们要考虑的是幂等键标准化**——我们的 152 处是业务体字段，客户端要"知道该传"才行；对方是 HTTP 头 + 契约声明，客户端可以统一中间件注入。
- **对方要补的是 fail-close 限流**——我们 Redis 挂了直接 500 而不是放行，这是更保守的选择。
- **impact preview 是个好东西**——改等级/用户组前先看影响面，我们没有。

---

## 4. 数据与内容模型

| | FluxTorrent | 同类站 |
|---|---|---|
| 核心模型 | **扁平种子表** `torrents` + `torrent_groups` 外键聚合（`0069_eco_p0.sql:13,24`）；两个按名聚合实体 `artists`（`0327_music_artists.sql:21-30`）、`content_networks`（`0333_content_networks.sql:37-46`） | **媒体中心强模型**：`Media` → `Series` ← `Members` → `Versions`（每个版本挂种子）；group_id / coverage / members 是一等公民 |
| 维度体系 | `section_kinds` 注册表，跨站型累计 **55 种 kind** × 6 种 field_type（select/multiselect/text/number/date/bool，`0195_section_field_types.sql:37`）；站型参数化 `site_type` | 硬编码影视字段（`actors` / `official` / `release_scope` / `resolution` / `hdr` / `audio` / `duration`），无维度注册表 |
| 元数据源 | PT-Gen 代理，**四源 imdb/douban/bangumi/indienova**，host 精确匹配防伪造（`publish_http/ptgen.rs:113-124`）；**无 TMDB 集成**；WASM 适配器优先、失败回退（`:127-133`） | **双源并存**：实测数据里同时出现 `doubaninfo.com` 与 `image.tmdb.org` 的海报；后台有 `/admin/ptgen-settings`（含 AI review 的 provider / endpoint / **prompt_version**） |
| 触发点 | 发种页 `/ptgen`（`ptgen.rs:90`）+ 详情补元 `POST /torrents/{id}/auto-meta`（仅空值写，`auto_meta.rs:14,47,63`） | `/media/identify`、`/media/metadata-preview`（契约可见） |
| 种子列表 | **有全局种子列表/搜索**（`/torrents`，我们是扁平模型，这是天然优势） | **无全局种子列表 API**——只有 `GET /media`（作品维度）、`/torrents/{id}`、`/torrents/pending`、自有种子 |

**结论**：这不是优劣，是**站型选择**。对方是单一影视站，可以把模型做深（追剧订阅、系列聚合天然成立）；我们是通用站，必须扁平 + 维度参数化（55 种 kind 是我们的核心资产）。
但两点值得注意：
1. **我们没有 TMDB 集成**，对方有。影视站场景下 TMDB 是主力源，这是实打实的能力缺口。
2. **对方有 `prompt_version` 字段**——把 AI 审查的 prompt 做成版本化配置项，这说明他们把 LLM 接入当正式能力管。我们没有对应物。
3. **全局种子搜索是我们的强项**——对方的浏览必须先经过作品维度，习惯"资源搜索"的用户会不适应。

---

## 5. 功能面对照表

| 能力 | FluxTorrent | 同类站 | 差异 |
|---|---|---|---|
| **发布流程** | 5 态状态机（0待审/1通过/2拒绝/3下架/4暂缓，`0001_init.sql:103`、`0077_p2_batch.sql:5`）；**无草稿**；三级自动免审（分类 `auto_approve` / 连击≥5 / class≥阈值 / `TORRENT_APPROVAL_AUTO`，`upload.rs:148-198`）；反向闸门：累计被拒≥2 禁发（`upload.rs:168-179`） | `publication-drafts`（**有草稿**）→ `POST /torrents` → `gate_phase`（如 `confirmed`）→ `submit` → `/pending`；**发布信用 5 分制**换免审（`PublishingCredits`，`auto_publish_threshold=5`） | 对方的"信用分换免审"比我们的"连击/权限位"更易理解、更可调；我们有反向闸门（被拒封禁）对方没有 |
| **审核** | 裁决仅允许从 0 出发（`review.rs:239-243`）；过审副作用（自动促销 + 组/厂牌订阅推送）单条与批量共用（`review_side_effects.rs:9-92`） | `admin/torrents` 队列 + `promotion` 拍板；`gate_phase` 说明元数据是异步投影 | 相当 |
| **求种** | **仅 3 条路由**（`content_http/mod.rs:43-45`：`request_create`/`request_list`/`request_fulfill`）；单币种（魔力）；**无上下限**；**无接受/拒绝**（应种即完结，`requests.rs:198-232`）；悬赏即时冻结且同事务（修过吞钱 P0，`requests.rs:27-67`） | `requests` + `/bounty`（min=100 / max=1e9 / min_added=10 / open_limit=10）+ `/fills` + **accept/reject** | **对方更完整**，我们有明显缺口：无加价、无接受/拒绝流转 |
| **论坛** | **29 条路由**（`community_http/mod.rs:123-159`）：分类/版主/举报/回复/点赞/收藏/关注流/悬赏/投票/抽奖/打赏；举报队列 claim/release/resolve（`admin_http/reports.rs:28,85,115`） | 分类/版主组/举报/like/bookmark/follow/`related`/`/sidebar`；无投票抽奖打赏 | 我们更花哨（悬赏投票抽奖打赏），对方更克制 |
| **Wiki** | **无**（`wiki` 仅在 `probes.rs:37` 注释出现，无模块无表） | 完整 Wiki：`/wiki` `/wiki/{slug}` `/wiki/new/article`，markdown 渲染 | **我们的缺口** |
| **站内信/工单** | `contactstaff`（60 秒防刷，`staffbox.rs:21,29-65`）+ **工单四态** 0新→1处理中→2已答复→3关闭，单向推进（`ticket.rs:62-73,105-121`）+ staff 答复（`staff_answer.rs:20,95`） | `staff-mail`（分类 complaint/appeal/suggestion/other、发信人卡片、回复线程）+ **外部来信队列** `internal/staff-mail/external`（邮件服务器 HMAC 摘要入队、纯文本渲染、附件只存元数据、`Message-ID+内容哈希` 幂等） | 对方多了"**邮箱 → 工单**"这一段；我们工单状态机相当但没有邮件入口 |
| **消息流** | 完整邮箱模型（send/markread/delete/move/boxes/inbox/sent/staff，`mod.rs:160-188`）；**未读靠轮询，无 SSE**（全仓无 `text/event-stream`） | `GET /me/messages/stream`（流式未读） | **对方有 SSE，我们靠轮询** |
| **订阅/追更** | 通知偏好 `u_notice_enabled(user,kind)`（`0076_notice_prefs_fn.sql:4-6`）；可订阅 kind 白名单**仅 3 个**（`review_side_effects.rs:45,72-73`） | `subscriptions`（按 media 追剧）+ `/feed` + `/read` + **每个订阅可 preview** | 对方的"追剧订阅"是内容模型带来的能力，我们扁平模型做不了 |
| **推送** | WebPush 完整实现 VAPID JWT + aes128gcm，RFC 8291/8292（`push_http/crypto.rs:1-2`）；topic 仅 3 个（`push_http.rs:44-46`）；拒收内网 endpoint 防 SSRF（`push_http.rs:52-60`） | 未见 | 我们有 |
| **第三方 API** | **四通道**：NP 兼容层 **8 条路由**（`compat_http/nexusphp.rs:18,171,304` + `aliases.rs:72,94,116,154`）+ 开放 API `fxo_` Token（scope `read`/`upload`，1–600 次/分默认 60，TTL 180 天每人 3 枚，签发需密码+TOTP 复核，`tokens.rs:63-145`）+ Torznab（`recon.rs:55,118`）+ RSS | **两套并行**：现代 `/automation/*`（9 ops，`X-API-Key` 头）+ 遗留 NP 风 `/api/torrent/search`、`/api/member/getUserTorrentList`、`/api/msg/search`、`/api/tracker/myPeerStatus`、`/api/torrent/genDlToken`（实测存在，要求 API key + scope） | 我们种类更多（Torznab 对方没有）；对方的**遗留 NP 端点**更贴近第三方工具既有实现（MoviePilot/辅种助手直接认），我们的 NP 层只有 8 条 |
| **RSS** | `/rss/{passkey}`（`rss_http.rs:42`）+ 论坛 RSS；**无 token 管理**（不能命名/轮换/预览，前端 getrss 页仅 20 行做链接生成） | `rss-tokens`：可命名 / 轮换 / 预览 / 删除 + `feeds/releases/{token}.rss` | **对方明显更完整** |
| **下载凭据** | `download_keys`：`sha3-256(fxk_...)` 哈希存储，绑定 user+torrent，30 分钟，**窗口内可复用不做一次性**（`0069_eco_p0.sql:29-36`）；签发 10 次/分（`download_keys.rs:16,48`）；**无 revoke 端点** | `download-authorizations` 对象化（**可 revoke**）+ `torrents/{id}/download-links` | 对方可吊销，我们不能 |
| **passkey** | 32 位小写字母数字（`compat_http/common.rs:23-27`）；双维限流（passkey 桶 + 同 IP 120/分，`common.rs:36-68`）；管理端可 reveal（默认拒绝+审计，`user_passkey.rs:50,70`）/ 代重置（`user_passkey.rs:88,105,124`）；**无用户自助轮换端点** | 契约里有 `POST /me/tracker-credential/rotate`（**用户可自助轮换**） | **对方的更好**：用户自助轮换是隐私基线能力 |
| **等级/权益** | `class_rules` 四维（uploaded / dl_count / seed_hours / account_age_days）累进取最高档，达标即升、不达标按 `demotable` 降（`class_adj.rs:119-137,237-264`）；`class_id<90` 自动升降、90+ 为员工；晋升发奖，幂等键 `class_promo:{uid}:{new_class}`（`:144-181`） | 6 阶同类站罐（迷你罐→经典罐→玻璃瓶→分享装→家庭装→…）；**等级与 benefits 双轨**（benefits 另含日图片数、邀请配额、api_keys=20、points_multiplier）；**impact preview** | 对方的双轨 + preview 更精细；我们的四维门槛+自动降级更完整 |
| **考核 H&R** | 三级 COALESCE（种子 `hr_policy.seed_hours` > 站点 `hr_hours` > 兜底 48；days 同理兜底 14，`jobs/hr.rs:39-56`）；10.4% buffer 豁免（`:74`）；免费窗口内完成不建快照（`:75-85`）；`hr_prewarn_hours` 预警（`0=关闭`，`:123-133`）；违规≥3 锁下载（`:226-236`，降回阈值自动恢复 `:278-279`）；自助免罪（20000 魔力，`admin_p3_http/hr.rs:163`）；批量赦免（`:104-106`） | `admin/hr/rules` + `hr-cases` + **用户申诉** `POST /me/hr-cases/{id}/appeals` + 管理端裁决 `POST /admin/hr-cases/{id}/resolutions`；规则有 revision 与"案例开启时冻结当时要求"的语义（`HRRules` 描述） | 相当；对方"案例开启时冻结当时规则"这个语义我们没有，改规则会追溯影响旧案例 |
| **任务/勋章/成就** | 任务单表 `tasks` + `metric` JSONB **六键**（upload_delta/download_delta/seed_points_delta/seed_seconds_delta/uploads/subtitles，全部满足才算完成，`task_jobs/settle.rs:127-151`）；勋章 `medals` + `user_medals`（`0001_init.sql:406-420`，`0033_medal_fields.sql:12,20` 加成与集齐奖励）；成就四族 `achievement_defs`（seeding/rescue/upload/forum，`0079_v4_gaps.sql:8-30`） | 未见任务/勋章/成就系统 | **我们有、对方没有** |
| **银行/贷款/捐赠/娱乐屋** | 银行定存/活期/贷款（`economy_http/bank.rs:33-67`、`loans_apply.rs:89`、`loan_repay.rs:101,144`、`loans.rs:144,237`）；站免池捐赠（`pool.rs:26-45`、`funding_contribute.rs:110-117`）；娱乐屋 9 玩法 **EV<1**（`games_http/casino.rs:1,22-26`） | 无（商店 `shop/products` 存在但为空） | **我们有、对方没有** |
| **邀请** | `invites` 表仅记录持有关系（`0001_init.sql:44-49`）；**无持续分润**（全仓无 inviter 分成逻辑）；过期回收（`jobs/sweep.rs:52-60`） | **harem 分红**：邀请人按被邀请人小时产出抽成 `min(own/0.2, Σinvitee) × 0.2`（`harem_factor=0.2`） | **对方有、我们没有** |
| **促销** | 站方 + 专属双链路按**事件时点**裁决（`billing_mults.rs:14-64`）；多档位合并（上行 max / 下行 min，`billing.rs:43-48`，修过"取最强一条吞掉 free"）；6 种 kind（free/x2/x2free/half/x2half/p30）+ refundable；券 free→down=0 / neutral→全 0（`billing_mults.rs:69-88`）；促销码按枚举序写账（`promo_audit.rs:23-40`）；0173 起全员可购 | `promotion-intents`（作者发起）→ `admin/torrents/{id}/promotion`（站点拍板）+ `promotion-offers` + `promotion-packages`（`package_key` / `duration_seconds` / `price_points` / **`snapshot_hash`**）+ retry/cancel | 我们的合并口径（max/min + 券叠加）更成熟；对方有 **intent 与生效分离** + **snapshot_hash 防篡改**，我们没有 |
| **管理端规模** | `staff_panel_entries` 44 条（0052 重写）+ 后续 23 条 ≈ 67；前端 `STAFF_TOOL_TABS` 40 个（`admin-tool-switch.tsx:49-56`）；路由 191 条（admin 79 + p2 18 + p3 94） | 129 个 admin 操作 | 我们更大（含备份/作业/版本/审计链校验等运维面） |
| **PWA / 离线** | `pwa-install-bar.tsx`（`layout.tsx:196`）+ sw.js（middleware 豁免名单 `middleware.ts:26-49`） | 无 manifest、无 PWA | 我们有 |

---

## 6. 经济机制

### 6.1 货币与收益

| | FluxTorrent | 同类站 |
|---|---|---|
| 货币 | **单一货币 `spark`**，可见名 `site_settings.currency_name` 默认「魔力」（`0082_currency_name.sql:8`）；前端 `{magic}` 占位替换（`0122_...sql:4,16`）。"积分"**不是第二货币**，只是任务指标单位（1 积分 = 1 小时做种，`task_jobs/mod.rs:17`） | 单一货币 Points（中文界面即「积分」），消费场景为求种悬赏 / 商店 / 付费促销 |
| 账本权威 | `spark_ledger` + `balance_baseline.base_spark + sum(...)` 重算（`jobs/seeding.rs:41-42,124-131`） | 未明 |
| 做种公式 | **公式本体在 DB 函数**（0136），worker/API 共用同一真相源（`jobs/seeding.rs:11-27`）；参数 `base=10` `cap=150` `curve_k=0.12` `vol_base=50GB` `rarity_k=0.6` `rarity_exp=0.35` `scale=0.4` `donor_mult=2`（`0136_seeding_formula.sql:61-81`）；档位：濒危 2.0 / 高龄 1.5 / 老种 1.0 / 大体积 0.75 / 中 0.5 / 日常 0.25；个人时长档 ≥8760h +2.0 …（`0136:85-120`）；`(base + floor((2/π)·cap·atan(Σ·curve_k))) × donor_mult`（`:123-133`） | **NP 原版小时公式移植**：`A = Σ[(1−10^(−Ti/T0))·Si·(1+√2·10^(−(Ni−1)/(N0−1)))·Wi]`，`B = B0·(2/π)·arctan(A/L)`；参数 `t0_weeks=4` `n0_seeders=7` `b0_unofficial=60`/`l_unofficial=300` `b0_official=240`/`l_official=2400` `max_torrents_counted=200` `harem_factor=0.2`；**官方与非官方分两条 bracket 分别封顶** |
| 结算节奏 | 小时 tick（`jobs/run.rs:210-214`），幂等键 `seeding:{user}:{yyyymmddhh}`，一次性 INSERT…SELECT 全站 | hourly cadence、`credit=immediate`、**北京时间午夜日切**、`missed_hours=not_backfilled`（漏跑不补）、有日上限 `daily_cap_points`（乘法后应用） |
| 效率细节 | `WITH p AS MATERIALIZED (SELECT * FROM seeding_params())` 防每行 8 次读配置（`jobs/seeding.rs:46`） | 未明 |

**对比**：两个公式都是"arctan 饱和"形态（对方的 `(2/π)·arctan(A/L)` 和我们的 `(2/π)·cap·atan(Σ·curve_k)` 是同族），说明对方直接移植了 NP。
- **我们的档位维度更丰富**（濒危/高龄/老种/体积/个人时长），是"救种导向"；
- **对方按 official/unofficial 分 bracket**（4 倍差：240 vs 60），是"官方组导向"——鼓励官方发布；
- **对方的日切/日上限/漏跑不补**这三项明确的结算语义，我们没有对应物（我们没有日上限）。

### 6.2 我们独有、对方没有的

银行（定存/活期/贷款）、站免池捐赠、娱乐屋（EV<1 回收）、勋章与成就、任务系统、基金池、魔法池、签到。**这是我们的厚度优势**。

### 6.3 对方有、我们没有的

1. **harem 邀请持续分润**（`harem_factor=0.2`）——邀请人从被邀请人的持续产出中抽成。我们的邀请只发名额、无分润。
2. **促销包 + `snapshot_hash`**——促销配置带内容哈希，可验证"用户买的时候看到的是不是这一版"。我们的促销无快照。
3. **日上限 `daily_cap_points`**——单会员每日做种收益上限（在我们的倍率/捐赠乘数之后应用）。
4. **结算语义显式化**（日切时区、漏跑不补、北京午夜）——我们靠 worker tick 隐式决定。

---

## 7. 反作弊与风控 —— **我们压倒性领先**

| 机制 | FluxTorrent | 同类站 |
|---|---|---|
| **BT 级取证** | **三阶段探测**（`apps/tracker/src/peers/bt_probe.rs`）：68B 握手（校验 `pstrlen=19` + info_hash 匹配，`:93-100`）→ 发 interested 诱 bitfield（声称做种却无 bitfield = 幽灵，`:116-166`）→ **随机 piece** request 比对 SHA-1（`:204-316`）；`Some(false)` = 实锤伪造判 DEAD，`None` = 没验成**不判作弊**（`:193-197`）；探针 peer_id **每次随机**（防开源后被指纹优待，`:35-43`）；`MAX_PIECE_LEN=8MiB` 防巨量分配（`:182`）；预算 `FLUX_TRACKER_PIECE_PROBE` 默认 8、0 关（`tracker/src/main.rs:113-118`） | **未见**。契约里有 `GET /admin/peer-observations`，但没有任何握手/piece 级取证描述 |
| **connectable 分档** | **四档**：`-1` 未测 / `-2` SUSPECT（端口开但不响应明文 BT，最可能是仅加密客户端）/ `0` DEAD / `1` OK（`peers/model.rs:28-31`）；**只有 `0` 阻断收益**，`-2` 明确放行（`jobs/seeding.rs:71-74`），理由是通用站不能假设用户会关加密（`probe_loop.rs:87-91`）；观测端点 `GET /admin/conn/dist`（`staff_http/site.rs:199-220`） | 未见等价物 |
| **幽灵做种判据** | `seeding = left==0 && port>0 && reachable && proven && covered`（`seeding_gate.rs:69-93`）；`proven = has_payload \|\| vouched_ok`、`covered = ratio_ok \|\| vouched_ok`，`ratio_ok = phys_down×1000 >= size×104`（10.4%）；`reachable` 受 `conn_gate_hard` 控制（**默认 off**，`:82-83,133-135`）；`completed` 需 `completed_event && left==0 && proven && covered && had_baseline`（首报自称完成不算，`:84-85`） | 未见等价判据 |
| **计费守卫** | **三道闸**（`jobs/ledger_guard.rs`）：① 锚点锁——首报只立基线、raw 恒 0（`:49-91,212-219`），下降即钉死，仅"两侧同时 ≤16MiB 且 24h 内一次"才认重置（`RESET_ACCEPT=16MiB` `:11`、`RESET_WINDOW_SECS=86400` `:16`）② 速率钳——默认 **128 MiB/s**（`traffic_credit_max_bps` > `speed_alarm_bps` > 缺省 134217728，`:295-312`）③ 终身上限 `torrent_size × 1000`（`:228-233`）；`GuardOutcome` 具名结构（`:263-285`） | **announce limits**：min_interval_secs / window_secs / max_in_window / ban_secs / report_interval_secs，按 passkey 临时封禁。是**频次**维度，不是**流量合法性**维度 |
| **为何必须用 raw** | `phys_down` 取速率钳后、**倍率前**的 `credit_down`（`:255-258`）；freeleech 下 credited 下载恒 0，用它当证据会误杀免费种真做种者 | 未见相关讨论 |
| **交叉上报** | `xreport=<peer_id_hex>:<bytes>` → tracker 校验目标为同 swarm 存活做种 peer 且非自己（`0300_upload_corroboration.sql:6-17`）→ `upload_corroborated` → 计费封顶 `room = (corr_total - acc_uploaded).max(0)`（`process_event.rs:82-106`）；**未接入时维持自报口径**（向后兼容，`:80-81`）；佐证者物理额度取**已入账 credited 下载**（不自证循环，`xreport.rs:105-130`）；单笔荒谬线 10 GiB（`xreport.rs:28-43`）；超报明细 `over_bytes` 供追查（`0305:22-39`） | 未见 |
| **关联作弊** | `collusion.rs` 四类：① `self_deal` 同 IP 双账号对刷 ② `down_under` 佐证和 >> 自报 ③ `bitthief` 完成后零上传挂机 ④ `near_cap` 贴边汇报（≥20 次且贴边占比 ≥80%）（`:3-16,45-60`）；**只落 `cheat_events` 进待办、不自动处罚**（教育站误伤代价更高，`:16`）；`cheat_enforce.rs` 累进处置 L1 站内信 / L2（≥5 hits）管理组信箱+webhook | 未见 |
| **客户端管控** | 黑白名单（NP AgentAllow/AgentDeny 口径，pattern 编译成正则，`admin_http/agent_rules.rs:9,23-31`）；端口黑名单 22 个（25/110/445/3389/5432/6379/8080…，可覆盖可关闭，**刻意不抄 NP 的 6881–6889**，因为那是 qBittorrent/Transmission 默认段，`http_track/limits.rs:20-64`） | `client-rules`：match_kind ∈ `peer_prefix`/`peer_exact`/`ua_contains`/`ua_regex`/`client_name` × allow/deny + **版本区间** + **优先级** + reason。**粒度比我们细**（我们只有 pattern，没有版本区间和优先级） |
| **announce 限流** | interval 默认 1800s（clamp 60–86400，`tracker/src/main.rs:77-84`）；`min_interval = interval/2` 夹 [30,3600]（`guard.rs:208`）；±10% 抖动按 FNV-1a(info_hash,user) **稳定取值**防惊群（`limits.rs:73-110`）；三级桶 user 1800/分、IP 3600/分、全局默认关（`main.rs:78-83`）；`stopped` 事件独立 4× 宽阈（`guard.rs:158-174`）；周期合并窗 `SET ann:evt:... NX EX window`（`guard.rs:216-245`） | announce limits 可配封禁（ban_secs）。**我们无限流封禁机制** |
| **IP/设备关联** | 24h 内单账号登录来源 ≥8 个不同 /24 段 → `login_ip_spread`（只记录不处置，`jobs/audit.rs:269-292`）；同 IP 双账号一传一下 30 分钟窗口聚合（`announce_main.rs:67`） | 未见 |
| **seedbox** | 声明式规则页（同 IP 并发 ≥50 种子视为盒子，建议登记，**未登记不影响功能**，`twofa_http/admin_misc.rs:81`） | 申报 + 后台审核（`seedbox-declarations` / `admin/seedbox-declarations/{id}/review`）；benefits 带 `seedbox_revision` 版本号 |
| **登录侧防撞库** | 连续 5 次失败锁 15 分钟（Redis `acctlock:{username}`，fail-close，`login.rs:46-60,102-107`）+ 用户名 5/分 + IP 20/分（`login.rs:69-75`）；验证码四驱动（none 自研算术题 / turnstile / recaptcha / hcaptcha，`gaps_http/captcha_drivers.rs:25-31`）；**无 PoW**（全仓无 `sha256-pow`/`proof_of_work`） | **自托管 `sha256-pow-v1` 工作量证明**：同一 peer/IP 三次失败后返 `403 captcha_required` + `fields{captcha_token, captcha_challenge, captcha_difficulty, captcha_expires_at}`；`SHA-256("<challenge>:<solution>")` 前导零 bit；**不加载任何第三方验证码脚本**，禁用 JS 时明确不放宽 |
| **2FA** | 自建 TOTP（RFC 6238 含测试向量，`twofa_http.rs:33,48,255`）+ **WebAuthn passkey 第二登录通道**（challenge 存 Redis 5 分钟，`0227_...sql:4-8`）+ 敏感操作 step_up 复核（`twofa_http/step_up.rs`） | TOTP enrollment/confirm/recovery codes + 管理端强制重置；**无 WebAuthn** |

**结论**：反作弊是我们最强的护城河，深度和严谨度都远超对方（尤其是"哈希不符才判 DEAD、拿不到数据不判作弊"这个免责语义，以及 freeleech 下必须用 raw 的判断）。
但对方有 **两处比我们强、值得直接抄**：
1. **`client-rules` 的版本区间与优先级**——我们只有 pattern，无法表达"qBittorrent 4.x 允许、5.0 以下拒绝"这类规则。
2. **announce 封禁（ban_secs）**——我们能限流但不能临时封禁一个疯狂 announce 的 passkey。
3. **登录 PoW**——我们靠失败锁定 + 可选第三方验证码；对方零依赖、无隐私外泄。

---

## 8. 运维与可观测

| 项 | FluxTorrent | 同类站 |
|---|---|---|
| 探针 | **分级正确**：`/health` = liveness 只表进程活；`/ready` 探 `SELECT 1` + Redis PING，任一失败 503（`http/probes.rs:6-7,63-86`）；compose 里 api healthcheck 打 `/api/v1/ready`（注释 ZT81：原 `/health` 静态 up 导致 DB 挂了仍导流） | 未见探针端点（`/api/v1/health` 404） |
| 指标 | Prometheus 文本：api **7 族**（`# TYPE` 8 行，延迟直方图占 bucket+sum 两行；requests / 5xx / 连接池 / 池空闲 / announce DLQ 深度 / stream 长度 / 延迟直方图，`v4_http/metrics.rs:113-136`）+ tracker **23 族 = 19 counter + 4 gauge**（announce_total/ip_banned/agent_blocked/fake_left/evt_merged/xreport_rejected/ratio_gate_block/peers_active/swarms_active/guard_cache…，`http_track/metrics.rs:74-120`）；`ANN_METRICS_TOKEN` 门禁，**未配 token 直接 404 隐藏**（读 env `:18-21`、404 分支 `:57-59`），比较走定长时间防侧信道（`:34`） | ops 面板（`/admin/ops/overview`、`site-statistics`、`workers`、`tracker`、`kernel-health`），但**未发现对外 metrics 端点** |
| 日志 | `tracing_subscriber::fmt::layer()` **人读格式非 JSON** + `runtime_log` 层把 WARN+ 落库供后台"运行日志"页读（`main.rs:122-131`）；有 request_id 贯穿 | 有 `X-Request-Id`；日志形态未知 |
| trace / 错误上报 | **无 OTel、无 Sentry**（`apps/api/Cargo.toml` 只有 tracing） | 未知 |
| 审计 | append-only 触发器禁 UPDATE/DELETE（防篡改链，`0266_...sql:104,107,128-130`）+ `/admin/audit/chain-verify` 校验（`admin_http/audit.rs:48,93`）；**无保留策略**（全仓无 `DELETE FROM audit_log`） | `GET /admin/audit-events` + `ops/audit-summary`；契约里对每个写操作声明了审计语义 |
| 管理端运维面 | `/admin/overview`（待审/举报/成员/种/封禁 + 14 天趋势 + 活种率 + WAU，`admin_http/overview.rs:10-46`）；`/admin/backups`+`run`、`/admin/jobs`+`run`、`/admin/version`（`admin_p3_http/ops.rs:18,54,188,221,287`） | `/admin/ops/*` + `admin/kernel-health` + **impact preview**（改等级/组前看影响面） |
| 优雅停机 | api `stop_grace_period: 45s`（> actix drain 30s）、worker **90s** | 未知 |

**结论**：我们的**探针语义与指标覆盖面**更好（23 个 tracker 指标族、分级探针、metrics token 门禁）；对方的 ops 面板更"产品化"（kernel health、site statistics、impact preview）。
**我们的三个明显欠账**：① 日志非结构化 JSON，机器不可解析；② 无 trace / 无错误上报；③ 审计表无保留策略，会无限增长。

---

## 9. 安全

| 项 | FluxTorrent | 同类站 |
|---|---|---|
| CSP | **强制生效**（含 `script-src 'self' 'unsafe-inline'` 妥协，为 Next 内联引导脚本，`next.config.ts:25-29,45-70`） | **仅 `Content-Security-Policy-Report-Only`**（写了完整策略但不阻断，report-uri `/security/csp-report`） |
| HSTS | `max-age=31536000; includeSubDomains`（**1 年**，`next.config.ts:33-43`） | `max-age=86400`（**1 天**） |
| 其他头 | `nosniff`、`X-Frame-Options: DENY`、`Referrer-Policy`、`Permissions-Policy: camera=() microphone=() geolocation=()` | `nosniff`、`X-Frame-Options: DENY`、`Referrer-Policy: no-referrer`、`Cache-Control: no-store` |
| 邮件凭据 | 无对应物 | 一次性 token = **64 位小写 hex**；缺失/重复/畸形各有专门兜底文案；**捕获后立即 `history.replaceState` 从地址栏抹掉** |
| Token 存储 | `fxo_` Token + `download_keys` 均哈希存储（`0069_eco_p0.sql:29-36`） | 未见说明 |
| 源暴露 | tracker 与 api 同域部署（tracker 走 `/announce`） | **tracker 独立域名且 DNS 未过 CF，源 IP 与主机商完全可见**（154.89.149.3 / 香港 AS55933） |
| robots.txt | 有 | **只有欧盟 CDSM 注释，零条有效指令**（等于默认放行全部抓取） |

**结论**：安全响应头我们更强（HSTS 1 年 vs 1 天、CSP 强制 vs 报告）。但对方的**邮件凭据处理细节**（64 位 hex、捕获即抹地址栏、三类兜底文案）比我们精细。

---

## 10. 我们该抄什么（按 ROI 排序）

| # | 抄什么 | 为什么 | 我们的现状 | 成本 |
|---|---|---|---|---|
| 1 | **自托管 PoW 登录闸门**（`sha256-pow-v1`：第 3 次失败后下发挑战，零第三方脚本） | 撞库防护 + 无隐私外泄 + 无第三方可用性依赖。我们有失败锁定和四驱动验证码，但没有"零依赖"这一档 | 全仓无 PoW；验证码默认自研算术题或接三家第三方 | 低（几十行 + 前端 worker） |
| 2 | **乐观并发（expected revision）** | 17 个"先读后写"的配置接口（积分规则/HR 规则/发布设置/流量配置/个人资料）现在**会互相覆盖** | 全仓 0 处 `If-Match`/`ETag`/`expected_revision`；只有 Redis `cfgver` 做缓存失效 | 中（需给配置表加 revision 列 + 中间件） |
| 3 | **契约先行 + `implemented`/`defined` 生命周期 + per-operation 版本** | 我们的设计意图现在只存在于对话和记忆里，容易丢；`check_type_drift.mjs` 只能防 8 条手写契约 | 无 utoipa、无生成产物、覆盖率 3.7% | 高（但可分域推进） |
| 4 | **promotion intent / 生效分离 + `snapshot_hash`** | 促销现在是"管理端直接挂"，事后无法解释"为什么是这个倍率"，也无法证明用户当时看到的是哪一版 | `promotions` 表只有 scope/kind/时间/source，**无 intent/审批/状态列** | 中 |
| 5 | **harem 邀请持续分润** | 邀请人从被邀请人持续产出中抽成，把"拉新"变成长期激励 | `invites` 只记录持有关系，**无分润** | 中（需要在计费链路加一路） |
| 6 | **`Idempotency-Key` HTTP 头标准化** | 我们 152 处是业务体字段，客户端必须"知道该传"；改头后可统一中间件注入 | 业务体字段 + DB 唯一约束 | 低-中 |
| 7 | **client-rules 的版本区间与优先级** | 无法表达"qBittorrent 5.0 以下允许、以上拒绝" | 只有 pattern 编译成正则（`agent_rules.rs:9,23-31`） | 低 |
| 8 | **announce 临时封禁（ban_secs）** | 我们只能限流，不能临时封禁疯狂 announce 的 passkey | 三级桶限流，无封禁 | 低 |
| 9 | **RSS token 管理**（命名/轮换/预览）+ **passkey 用户自助轮换** | 隐私基线能力；用户泄露后可自助止损 | RSS 只有 passkey 拼 URL；passkey 只能管理员代重置 | 低 |
| 10 | **下载授权可吊销** | 我们 30 分钟凭证窗口内可复用且不可吊销 | `download_keys` 无 revoke 端点 | 低 |
| 11 | **发布信用 5 分制免审** | 比"连击≥5 / class≥阈值"更易理解、更可调 | 三级自动免审，无信用分 | 中 |
| 12 | **未读消息 SSE 流** | 我们现在靠轮询 | 全仓无 `text/event-stream` | 低 |
| 13 | **impact preview**（改等级/组前看影响面） | 我们现在改了才知道影响谁 | 无 | 中 |
| 14 | **TMDB 元数据源** | 影视站主力源，我们只有 PT-Gen 四源（imdb/douban/bangumi/indienova） | `ptgen.rs:113-124`，无 TMDB | 中 |
| 15 | **Wiki** | 规则/帮助文档目前散在页面里 | 无 wiki 模块/表 | 中 |
| 16 | **PosterImage 组件化** | 海报逻辑现在散在 4 个文件各自实现 | 无 `PosterImage` 组件，裸 `<img>` 22 处、`lazy` 仅 8 处 | 低 |
| 17 | **结构化 JSON 日志 + 审计保留策略** | 机器不可解析；审计表会无限增长 | `fmt::layer()` 人读格式；全仓无 `DELETE FROM audit_log` | 低-中 |

---

## 11. 我们的护城河（明确别丢）

1. **反作弊深度**：BT 三阶段取证（随机 piece + 随机探针身份 + 8MiB 上限）、connectable 四档（`-2` 放行加密客户端的通用站口径）、计费守卫三道闸（锚点锁 / 128MiB/s 速率钳 / size×1000 终身上限）、交叉上报封顶 + 未接入时的向后兼容、`collusion` 四类作业只落待办不自动处罚。**这是对方完全不具备的能力。**
2. **通用站参数化**：55 种 `section_kinds` × 6 种 field_type + `site_type` 站型包。对方是硬编码影视字段，换站型要改代码。
3. **全局种子列表与搜索**：扁平模型带来的能力，对方必须先经过作品维度。
4. **经济系统厚度**：银行/贷款/捐赠/娱乐屋/勋章/成就/任务/基金池——对方一项都没有。
5. **安全头基线**：HSTS 1 年 + CSP 强制 + Permissions-Policy。
6. **分级探针 + tracker 23 个指标族**：`/ready` 探 DB+Redis 的语义是对的，对方没有探针端点。
7. **移动端 M1–M4 + 4 语言 i18n**：对方单语、无 PWA。
8. **第三方四通道**（NP 兼容 + `fxo_` + Torznab + RSS）：对方无 Torznab。
9. **2FA 双通道**（TOTP + WebAuthn + step_up 复核）：对方只有 TOTP。
10. **限流 fail-close**：Redis 挂了返回 500 而不是放行，比"挂了就不管"保守。

---

## 12. 结论

**对方值得尊重的地方，不是功能多，而是"把设计意图写成了机器可读的东西"。** 他们 14 天做出 312 个带生命周期标记的契约操作、37 个权限键、74 处幂等声明、17 处乐观并发，说明他们**先设计后编码**，而且把设计留在了代码能读到的地方。我们的 725 条路由里，设计意图大多只存在于对话记录和工作记忆里——`check_type_drift.mjs` 只能守住 8 条手写契约。

**我们真正的问题不是能力不够，而是体量已经超过工程纪律的承载**：108 个页面配 2 个组件原语、725 条路由配 3.7% 的契约覆盖、152 处幂等配 0 处乐观并发、344 个迁移配数十秒冷启动。这些都是"长出来的债务"，不是设计错误。

**建议的推进顺序**：先做 1/2/6/7/8/9/10/12/16（低成本高收益，一两周内可完成），再做 4/5/11/13/14（机制层），最后把 3（契约工程）作为长期工程——因为它是唯一能防止"第 N 个功能又变成口头约定"的东西。

---

## 13. 复核配方（2026-10-13 逐项重跑，别再凭记忆）

本文所有 FluxTorrent 侧数字已在成文后逐项重跑：13.1 的配方逐条执行，命中的每个数字（725 / 24 / 344 / 73 / 21→20 / 8·23 / 152 / 34 / 8 / 42 / 108 / 56 / 12+107 / 370 / 517·77 / 0）都与正文一致；另有 10 处反作弊与安全引用逐条读到源码行确认。成文时写偏的 7 处见 13.2；配方本身也纠出一处假绿（裸 `etag` 匹配到 `pricetag`/`activeTag`，得 688 条假命中）。以后引用请按配方重算。

### 13.1 可复算命令

```bash
# 路由宏 725：必须含全限定写法，否则只得 724
grep -rE '#\[(actix_web::)?(get|post|put|patch|delete|route)\(' apps/api/src --include=*.rs | wc -l
# 域包 24 / 迁移 344（最新 0350）
ls -d apps/api/src/*_http | wc -l; ls apps/api/migrations/*.sql | wc -l
# 权限键 73（以现网 DB 为准，不在代码里）
docker exec flux-postgres psql -U flux -d fluxtorrent -Atc "select count(*) from permissions"
# 错误码 21 变体 / 20 数字码；TS 侧常量在 packages/domain-types
awk '/fn code/,/^    }/' apps/api/src/errors.rs | grep -cE '=>'
awk '/fn code/,/^    }/' apps/api/src/errors.rs | grep -oE '=> [0-9]+' | sort -u | wc -l
# 指标族：api 7 族=8 行 # TYPE（直方图占 bucket+sum 两行）；tracker 23 族=19 counter+4 gauge
grep -c '# TYPE' apps/api/src/v4_http/metrics.rs apps/tracker/src/http_track/metrics.rs
# 幂等 152 / page_window 34 处调用 / CONTRACTS 8 条
grep -rn 'idempotency_key' apps/api/src --include=*.rs | wc -l
grep -rn 'page_window(' apps/api/src --include=*.rs | grep -vc 'fn page_window'
# worker：42 个 mod 声明（目录内 46 个 .rs 含 mod/run/tests，job_status 现网 50 行）
grep -cE '^\s*(pub )?mod [a-z_]+;' apps/worker/src/jobs/mod.rs
# 前端：页面 108 / 一级路由 56（= app/(main) 下目录数）/ 原语 2 / loading.tsx 0
find apps/web/app -name page.tsx | wc -l; ls -d apps/web/app/\(main\)/*/ | wc -l
# 硬编码色 119 = app 12 + components 107（必须按 6 位十六进制数出现次数）
grep -rEoh '#[0-9a-fA-F]{6}' apps/web/app --include=*.tsx --include=*.ts | wc -l
grep -rEoh '#[0-9a-fA-F]{6}' apps/web/components --include=*.tsx --include=*.ts | wc -l
# "use client" 370/507（必须限定 .tsx，否则会数进 .ts 得到 384）
grep -rl --include=*.tsx '"use client"' apps/web/app apps/web/components | wc -l
# JSONB 517 处 / 77 文件（必须大小写不敏感；别用 grep -rc 配 awk -F: ，$1 是文件名不是计数）
grep -rio jsonb apps/api/migrations/*.sql | wc -l; grep -rli jsonb apps/api/migrations/*.sql | wc -l
# 乐观并发 0：必须限定扩展名并锚定大小写——裸写 etag 会打到 pricetag/activeTag（实测 688 条假命中）
grep -rnE 'If-Match|if_match|"[Ee]tag|expected_revision|row_version|lock_version|optimistic' \
  apps/api/src apps/api/migrations apps/web/app apps/web/components packages/domain-types/src \
  --include=*.rs --include=*.sql --include=*.ts --include=*.tsx | wc -l
```

**唯一带 revision 的东西不是并发控制**：`rules_revisions`（站规编辑历史快照，`0079_v4_gaps.sql:40-49`，写入点 `staff_http/rules_cats.rs:75`、读取点 `v4_http.rs:215`）只留痕、不校验期望版本。要补乐观并发时，仓内没有现成的 revision 管道可接。

### 13.2 本轮改正的七处

| 位置 | 原写 | 复核后 | 错因 |
|---|---|---|---|
| §3.1 / §10 / §12 | 手写契约 26 条、覆盖率 3.6% | **27 条**（`spec.rs` 13 + `spec_np.rs` 14）、**3.7%** | 漏了 `/plugins/ptppUserInfo` 一条 |
| §3.2 | 错误码 **20 个**变体 | **21 个变体 → 20 个数字码**（`Validation`/`FieldErrors` 同 1002），TS 侧 25 常量含 5 枚预留 | 把"码"当成"变体" |
| §0 / §8 | tracker **19** 个指标 | **23 族 = 19 counter + 4 gauge** | 只数了 counter |
| §2 | worker **47** 个 job 模块 | **42 个**（`jobs/mod.rs` 声明），目录 46 个 `.rs` | 把子目录与非 job 文件算进去 |
| §3.2 / §3.3 | `page_window` **36** 处调用 / 幂等 **151** 处 | **34** 处调用（另有 1 定义 + 1 注释）/ **152** 处 | 命中数与基数没分开 |
| §8 | metrics token 门禁引用 `metrics.rs:87-89` | 读 env `:18-21` + 未配置即 404 的 `:57-59` + 定长比较 `:34` | `:87-89` 那段是 `# TYPE` 文本 |
| §5 功能面 | 论坛「**约 20 条**路由」（`:122-159`）；求种引用 `:47-50` | 论坛 **29 条**（`community_http/mod.rs:123-159`）；求种是 `:43-45`，`:47-50` 那段是候选 `offer_*` | 行号没回到挂载点，论坛按记忆估了个约数 |

### 13.3 顺带关掉一条旧账

`scripts/check_type_drift.mjs` 现跑 **TYPE_DRIFT_OK**：`PoolGuard=4102` 已进 `packages/domain-types` 的 `ErrorCode`（此前记为未同步的契约漂移）。当前仅剩 5 条 WARN，全是 TS 侧预留码（1001/2008/3001/3002/5001）在 `errors.rs` 无对应分支——是"已定未实现"，不是漂移。

### 13.4 同类站 侧登录实测复核（2026-10-13 晚，站长账号 gntv）

用账号真机登录 同类站.com 复核对方侧论断（详见 `competitor-同类站.md` §9）。要点：

- **逐字段命中**：`/me/points`（balance=0、revision=1）、`/promotions`（execution_revision=12、download 0/1）、`/me/publishing-credits`（0/5）、`/me/rank`（mini_can→classic_can 门槛 5 完成/7 天/1.0）、torrent 榜首「凡人修仙传」S01E194、`/series`=0、`/requests`=0、安全头（CSP Report-Only / HSTS 86400 / XFO DENY / no-referrer）、tracker bencode 报错、未登录 401、错误信封形态——全部与本文引用一致。
- **水位更新**：`X-Reelcore-Api-Version` 13.2.0 → **13.5.0**（页脚自报 13.6.0）；`x-contract-version` 去重 **29 个值**；契约 chunk 875KB → **898KB**；榜首上传量已 ≈207GiB（高于成文快照）。§0/§3 引用的 13.2.0 是成文时快照，不回改——对方一天连发 3+ 版本，按日快照口径理解即可。
- **一处口径提醒**：bundle 裸字面量计数（`IdempotencyKey` 90 处、`x-origin-required` 285 处）与本文 §3.3 的「74/148 操作」不同源——bundle 里契约与 ajv 预编译器各带一份元数据，**以操作级口径（74/148）为准**。
- **新观察**：其 SPA 在会话有效（cookie 在、`/api/v1/me` 200）时仍自报「暂时无法确认登录状态」并常驻「页面数据异常」alert——与其自述「内测 BUG 较多」一致；另外对方**没有独立公开的 openapi.json 端点**（`/api/v1/openapi.json` 等 404/SPA fallback），契约只内嵌前端 bundle，我们的 `/api/v1/openapi.json` 至少是个独立端点（§3.1 说"对方 100% 机器可读"仍成立，但"可获取性"上双方都不是标准 OpenAPI 端点形态）。
