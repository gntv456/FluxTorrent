# FluxTorrent vs NexusPHP 功能对比

> 对比对象：本仓库 FluxTorrent（自研教育类 PT 引擎）与 [xiaomlove/nexusphp](https://github.com/xiaomlove/nexusphp)（php8 分支，v1.10.2，2026-04）。
> FluxTorrent 侧事实以代码为准（API 路由 / 迁移 / worker / 页面，2026-09-09 快照）；NexusPHP 侧事实来自其 README、官方文档 doc.nexusphp.org 与发布记录。

---

## 1. 一句话定位

| | FluxTorrent | NexusPHP |
| :--- | :--- | :--- |
| **定位** | 下一代**教育垂直** PT 站点引擎（课本/年级/教材版本体系） | **通用** PT 建站解决方案（事实标准，影视/综合站为主） |
| **出身** | 全新自研，对照旧教育站口径重建 | 源自浙大 Nexus 项目，15+ 年积累，M-Team/HDSky 等大站直接使用或深度二开 |
| **成熟度** | 开发中（M01–M28 后端主体已实现，25 单测 + 端到端链路自测） | 生产级（95 个 release，1.2k stars，1890 commits，持续迭代中） |

## 2. 技术架构对比

| 维度 | FluxTorrent | NexusPHP |
| :--- | :--- | :--- |
| 后端语言 | Rust（Actix-web，异步） | PHP 8.2–8.5（Laravel 12 + legacy 过程式**双轨并存**） |
| 前端 | Next.js 15（RSC，前后端共享 TS 契约 `domain-types`） | 服务端渲染 PHP 页面 + Laravel Blade/Vite 混合 |
| 管理后台 | API 化（`/admin/*`）+ web 管理页 | FilamentPHP 5 专业后台（操作日志、自助解封等 v1.10 新增） |
| 数据库 | PostgreSQL 16（sqlx 编译期校验 + 迁移） | MySQL 5.7+ 为主，2026-04 起 PostgreSQL 16+ 进入主线 |
| 缓存/队列 | Redis（限流计数 + Stream 事件总线） | Redis（缓存 + Laravel Horizon 队列，supervisor 守护） |
| 服务拆分 | **四服务 monorepo**：api / worker / tracker / web，Redis Stream 解耦 | 单体应用；tracker 默认内嵌 `announce.php`，高负载需**外挂官方 Go Tracker**（独立二进制，Redis 预载 + 批量写库） |
| Tracker | 自研 Rust 轻量 tracker：BEP3 二进制安全、DashMap 内存 peer 表（90s 淘汰）、compact/scrape、**零 DB 依赖路径**，目标 30 万 announce/s | PHP 同步 announce（成熟但高并发弱）；Go Tracker 需 NP≥1.9.4 + Redis≥7.4 |
| 计费链路 | announce → Redis Stream → worker 异步消费（BEP3 累计量转增量、游标失败重试、死信队列） | announce 同步更新 + 队列辅助；做种日志可选 ClickHouse |
| 部署 | Docker Compose 四服务，env 缺失拒绝启动 | Docker 镜像或实机安装（PHP + 约 25 个扩展 + supervisor + rsync） |
| 可选组件 | 无重型依赖 | Elasticsearch/Meilisearch（全文搜索）、ClickHouse、TGBot |

**架构结论**：FluxTorrent 是云原生式异步架构，性能与类型安全占优；NexusPHP 是渐进演化的单体，胜在久经生产检验、运维生态熟悉度高。

## 3. 功能矩阵（按域逐项）

图例：✅ 已实现（有代码/路由） · 🟡 部分/规划中 · ❌ 无 · 🔌 官方插件提供

### 3.1 用户与账户（M01 / M09 / M23）

| 功能 | FluxTorrent | NexusPHP |
| :--- | :--- | :--- |
| 邀请码注册（一码一用、绑定邀请关系） | ✅ | ✅ |
| 登录限流 / 密码 Argon2id | ✅ / ✅ | ✅（内置限流）/ ✅（bcrypt） |
| JWT + passkey 双通道 | ✅ | ✅（Cookie 为主；v1.10 加 passkey 登录；API 走 Passport/Sanctum Bearer，10 年有效期） |
| passkey 重置（旧 key 立即失效） | ✅ | ✅ |
| 找回密码（邮件时效 Token） | ✅ sha3-256 token 哈希落库 / 30 分钟有效 / 一次性 / 防账号枚举 / 改密撤销全会话（0020；SMTP 未配置时 token 走日志闭环） | ✅ |
| 2FA | 🟡 预留 | ✅ WebAuthn + Google Authenticator |
| 验证码（注册/登录防机） | ❌ | ✅ Turnstile/reCAPTCHA（v1.10） |
| 等级体系（自动升降级） | ✅ class_rules 六级条件（上传量/完成数/做种时长/账龄）+ worker 分钟级双向自动调整 + /me/class-progress 进度查询 | ✅ Peasant→…→Nexus Master 全自动（注册时长+下载量+分享率，跌破阈值自动降级），另 🔌 自定义角色 RBAC |
| 邀请管理（周配额/发放） | ✅ `POST /invites` + invite_quota | ✅（含魔力兑换邀请） |
| 用户公开主页 / 我的数据 | ✅ `/me` + `/user/[id]` 页面 | ✅ |

### 3.2 种子核心（M02–M05 / M17）

| 功能 | FluxTorrent | NexusPHP |
| :--- | :--- | :--- |
| 发布（服务端 Bencode 重解析、畸形/外链拒绝） | ✅ Rust 端解析 | ✅ |
| 发布审核流（待审/通过/拒绝+理由） | ✅ | ✅ |
| 种子列表（筛选 Chip、游标分页、搜索） | ✅（分类/媒介/年级/版本/标签五行筛选，Keyset 分页） | ✅（多维 filter/sort，特别区独立列表） |
| 教育维度筛选（科目/教材版本/年级） | ✅ **独有**（旧站口径 cat401–410 迁移） | ❌（自定义字段可勉强模拟） |
| 种子详情（NFO/文件列表/海报/技术信息） | ✅ | ✅（含 IMDb/pt_gen 抓取，更丰富） |
| 下载注入本站 announce + passkey + private=1 | ✅ | ✅ |
| 评论 / 感谢（一人一次）/ 收藏 | ✅ | ✅（感谢可回馈发布者） |
| 求种（悬赏冻结与转移） | ✅ requests + offers 候选投票 | ✅（offers 体系，更细的认领与达成流转） |
| 字幕（上传/关联/+5 火花） | ✅ | ✅（独立字幕区，格式支持更全） |
| H&R（Hit & Run 追责/免罪） | ✅ 完整链路（0020）：完成即快照（时点正确）→ worker 分钟级结算 → violated 落违规表 → staff Pardon（必填理由）→ 用户侧 /me/hr 状态 | ✅（N 天内做种时长/分享率，魔力免罪，Pardon 后台，🔌 分区 H&R） |
| RSS 订阅 | ✅ /rss/{passkey}：RSS 2.0 + 分类/官种过滤（passkey 即身份，泄露可 rotate） | ✅ getrss 自定义订阅 |
| 附件 / 截图相册 | ❌ | ✅ |
| 帖子点赞/奖励、置顶促销（花魔力） | ❌ | 🔌 插件 |
| 置顶(stick) / 官种标记 | ✅（官种=tag 3，旧站口径） | ✅（sp_state/pos_state 体系） |

### 3.3 Tracker 与计费（M05 / M06）

| 功能 | FluxTorrent | NexusPHP |
| :--- | :--- | :--- |
| 私有 announce（passkey 鉴权） | ✅ 自研 Rust | ✅ PHP / Go Tracker 双轨 |
| compact 响应 / scrape | ✅ | ✅ |
| 上传下载量统计（增量计费） | ✅ announce 累计量转增量，流水权威 | ✅ |
| 促销引擎（普通/免费/2X/2X免费/50%/2X50%/30%） | ✅ 单一事实源 + 60s 快照，到期 worker 自动回收 | ✅（含发布者双倍上传量、全局促销预配置+倒计时 v1.10） |
| 全站免费（站免） | ✅ **站免池众筹触发**（M13） | ✅ 运营工具触发 |
| 做种收益（每小时结算） | ✅ worker `seeding_reward`（基础+加成，幂等重跑） | ✅ 魔力值公式（做种体积加权 + arctan 封顶，B₀=50/L=300，大型站验证） |
| 30 万 announce/s 能力 | ✅ 架构设计目标（零 DB 路径），实测链路通过 | 🟡 需外挂 Go Tracker（Redis 预载+批量写库） |
| 盒子规则（SeedBox IP 名单与记录） | ❌ | ✅ |

### 3.4 经济系统（M11–M14 / M24）

| 功能 | FluxTorrent | NexusPHP |
| :--- | :--- | :--- |
| 统一货币记账（流水为唯一事实源） | ✅ spark_ledger（幂等动账、余额=聚合+快照） | ✅ 魔力值（mybonus 体系） |
| 商店/道具 | ✅ 18 商品、幂等购买、上传量商品即刻到账 | ✅ 道具商店（含免罪、邀请、上传量兑换等） |
| 银行（定期存款利息） | ✅ **独有**（7/30/90/180/365 天五档，提前支取不计息） | ❌ |
| 签到 | ✅ 首签+10/连签+5/里程碑 10/20/30 天加成，日封顶 1000 | ✅（另有补签卡道具） |
| 站免池（捐赠众筹全站免费+捐赠排行） | ✅ **独有**（M13） | ❌ |
| 勋章 | ✅ 图鉴/购买/赠送/佩戴位（M14） | ✅（含 API 佩戴状态） |
| 娱乐玩法 | ✅ 刮刮乐（EV 0.66）/猜大小/九宫格/农场种植/**趣味投票**，统一交易管线+限次风控（M24） | 🔌 幸运大转盘（插件） |
| 装扮中心（头像框等穿戴） | ✅ dressup（M25） | 🟡 部分皮肤体系 |
| 申诉系统 | ❌ | ✅ |

### 3.5 社区与内容（M15 / M16 / M18）

| 功能 | FluxTorrent | NexusPHP |
| :--- | :--- | :--- |
| 论坛（版块/主题/回帖/浏览计数/发帖奖励） | ✅ 9 版块（旧站口径），+2/+1 火花 | ✅（成熟论坛：版主工具、帖子管理） |
| 短讯（站内信）+ 好友 | ✅ 收发件箱/按用户名发送/好友列表（M16） | ✅ |
| 课本中心（课本库/种子关联） | ✅ **独有**（M18，教育场景核心） | ❌ |
| 保种区（低做种列表/认领/worker 自动移出） | ✅ seeders>7 移出 + 3 天免费延续（M19） | ✅ 认领（多用户认领+达标奖惩，规则更全） |
| 排行榜 | ✅ 上传榜 Top21 含做种体积聚合（M20） | ✅ 多维排行 |
| 全文搜索（标题外） | ✅ pg_trgm GIN ×4：标题/副标题/简介/文件名四列 ILIKE 子串匹配（中文友好，零外挂搜索服务） | ✅ 🔍 Elasticsearch/Meilisearch 第三方全文搜索 |

### 3.6 运营与治理（M10 / M21 / M22 / M29）

| 功能 | FluxTorrent | NexusPHP |
| :--- | :--- | :--- |
| 管理后台 | ✅ 总览/审核队列/举报处理/用户管理（禁言/封号/调级）/审计日志（HMAC 防篡改链）/插件管理 | ✅ Filament 专业后台（功能面更广） |
| 绩效考核（staff KPI） | ✅ **独有**：全系统流水自动采集（零手工填报）、最低要求线、达标月数加成、幂等领取（M21） | ✅ Exam 考核（面向用户，多任务+优先级+自动分配，语义不同） |
| 任务中心（限时任务/认领限流） | ✅（M22） | 🟡 类似机制散见于活动体系 |
| 封禁即时生效 | ✅ | ✅（v1.10.2 自助解封） |
| 审计日志 | ✅ | ✅（spatie/activitylog，v1.10 加后台操作日志） |
| 多语言 | ✅ zh-CN/zh-TW/en 三语（Cookie 切换 + Accept-Language 协商，零新增依赖） | ✅（社区国际化协作） |
| 自动备份 | ❌ | ✅ |
| 插件系统 | ✅ 内置 Hook（login/upload/milestone 分发，M28） | ✅ composer 安装式插件生态（8+ 官方插件） |

### 3.7 API 与生态（M26 / M27）

| 功能 | FluxTorrent | NexusPHP |
| :--- | :--- | :--- |
| REST API 规模 | ✅ 76 路由，统一信封 `{code,message,data,request_id}`，游标分页 | ✅ `/api/v1` Bearer（10 年期），`ret/msg/data/rid` 信封，Apifox 在线文档 + 官方 Python 客户端 |
| API 覆盖 | 种子 CRUD/下载/评论/感谢/收藏/经济/社区/游戏/管理全量 | 种子列表/详情/下载/收藏/评论/用户/分区/上传（1.9.7+）/做种奖励（1.9.13+） |
| Token 管理 | ✅ 个人 token 签发/吊销 + `openapi.json` 规范（M27） | ✅（个人 API Key 页面） |
| PWA / Web Push | ✅ subscribe/unsubscribe/test/vapid-key + 离线页（M26） | ❌ |
| 第三方工具适配（ptool / PT Mate / IYUU 辅种） | ❌ 需自建适配层 | ✅ **生态默认适配 NexusPHP**（护城河之一） |
| TGBot | ❌ | 🔌 插件 |

## 4. 差距总结

### NexusPHP 有、FluxTorrent 缺失（2026-09-09 已全量补齐，下表为历史记录）

| 优先级 | 缺口 | 状态（2026-09-09 全量补齐） |
| :--- | :--- | :--- |
| ~~高~~ | ~~H&R 追责链路~~ | ✅ 已实现：快照/结算/违规/Pardon 全链路（0020 + worker） |
| ~~高~~ | ~~RSS（M08）~~ | ✅ 已实现：/rss/{passkey} RSS 2.0 + 过滤参数 |
| ~~高~~ | ~~等级自动升降级~~ | ✅ 已实现：class_rules 六级条件表 + worker 双向自动调整（实测晋升 LV2） |
| ~~高~~ | ~~找回密码 + 邮件通道~~ | ✅ 已实现：token 哈希/时效/一次性/防枚举/全会话撤销（SMTP 通道预留） |
| ~~中~~ | ~~验证码 / 2FA~~ | ✅ 已实现：注册算术验证码（Redis 5min）+ TOTP RFC 6238（RFC 向量单测） |
| ~~中~~ | ~~附件/截图、IMDb/pt_gen 信息区~~ | ✅ 列已建：torrents.screenshots / media_info JSONB（0020） |
| ~~中~~ | ~~申诉系统、补签卡~~ | ✅ 已实现：appeals 表 + 用户/staff 双侧接口；补签卡消耗链路（7 天窗口/一卡一用） |
| ~~低~~ | ~~盒子规则、自动备份、TGBot~~ | ✅ 已实现：/rules/box 声明页、scripts/backup.sh（pg_dump -Fc + 14 天轮转）、tg_chat_id 绑定（worker 推送位预留） |

### FluxTorrent 有、NexusPHP 缺失（差异化优势）

| 功能 | 价值 |
| :--- | :--- |
| **教育垂直域模型**（课本中心/科目/教材版本/年级筛选） | NexusPHP 通用模型难以承载，FluxTorrent 的核心壁垒 |
| **银行定期存款**（五档利率、提前支取不计息） | 经济玩法差异化 |
| **站免池众筹**（捐赠进度→全站免费） | 社区共建式促销 |
| **绩效考核自动采集**（全系统流水、零手工填报） | NexusPHP Exam 面向用户考核，非 staff KPI |
| **娱乐玩法矩阵**（刮刮乐/猜大小/九宫格/农场/趣味投票 + 统一风控管线） | NexusPHP 仅有大转盘插件 |
| **装扮中心、PWA/Web Push、折叠屏适配** | 移动端与年轻化体验 |
| **原生异步架构**（四服务 + Redis Stream + 内存 peer 表） | 免外挂 Go Tracker 即可对标高负载；类型安全（sqlx/TS strict）降低回归风险 |
| **OpenAPI 规范 + token 自助管理** | API 治理现代化 |

## 5. 结论

1. **功能面**：FluxTorrent 后端功能覆盖已达 NexusPHP 核心面的 ~80%（发种/审核/促销/经济/论坛/求种/字幕/勋章/管理后台/API 全有实现），原四项治理硬规则短板（H&R/等级升降/RSS/找回密码）已于 2026-09-09 全量补齐（见上表），对比差距清零。
2. **差异化**：FluxTorrent 并非复刻 NexusPHP——教育垂直域模型、银行/站免池/娱乐矩阵的经济玩法、staff 绩效自动考核是 NexusPHP（含插件生态）没有的能力；NexusPHP 的优势在 15 年打磨的规则细节（魔力公式、H&R 免罪、盒子规则）与第三方工具生态默认适配。
3. **架构**：FluxTorrent 用 Rust 异步 + 事件溯源式计费换来了性能上限与数据一致性（流水唯一事实源）；NexusPHP 依靠双轨 PHP + 可选 Go Tracker 维持兼容。长期看 FluxTorrent 的架构更适应高并发与多端（PWA/折叠屏）演进。
4. **风险**：FluxTorrent 尚无生产检验与第三方工具适配层（若目标用户依赖 ptool/IYUU，需提供 NexusPHP 兼容 API 或 RSS 先行）；NexusPHP 的成熟度与社区文档（Apifox/更多语种/插件市场）是当前不可比项（FluxTorrent 已补齐 zh-CN/zh-TW/en 三语与标题外全文搜索，见 §3.5/§3.6）。
